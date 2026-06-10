use std::{
    ffi::c_void,
    ptr, slice,
    slice::{Iter, IterMut},
    sync::Arc,
    thread,
    thread::JoinHandle,
};

#[cfg(feature = "capture")]
use media_core::{audio::AudioFrameDescriptor, frame_pool::FramePool};
use media_core::{
    audio::{SampleFormat, SAMPLE_RATE_48K, STANDARD_SAMPLE_RATES},
    error::Error,
    failed_error,
    frame::Frame,
    none_param_error,
    variant::Variant,
    Result,
};
use media_device_types::device::{Device, DeviceEvent, DeviceEventHandler, DeviceInformation, DeviceManager};
#[cfg(feature = "capture")]
use media_device_types::{capture::CaptureHanlder, device::OutputHandler};
#[cfg(feature = "render")]
use media_device_types::{device::InputHandler, render::RenderHandler};
#[cfg(feature = "capture")]
use windows::Win32::Media::Audio::{eCapture, IAudioCaptureClient};
#[cfg(feature = "render")]
use windows::Win32::Media::Audio::{eRender, IAudioRenderClient};
use windows::{
    core::{BSTR, HSTRING, PCWSTR},
    Win32::{
        Devices::FunctionDiscovery::PKEY_Device_FriendlyName,
        Foundation::{CloseHandle, HANDLE, RPC_E_CHANGED_MODE, S_OK, WAIT_FAILED, WAIT_OBJECT_0},
        Media::{
            Audio::{
                EDataFlow, IAudioClient, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, DEVICE_STATE_ACTIVE,
                WAVEFORMATEX,
            },
            Multimedia::WAVE_FORMAT_IEEE_FLOAT,
        },
        System::{
            Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ},
            Threading::{CreateEventW, SetEvent, WaitForMultipleObjectsEx, INFINITE},
        },
    },
};

/// Fixed client sample format: 32-bit float, interleaved. WASAPI shared-mode
/// auto-conversion bridges this to the device native format.
const CLIENT_SAMPLE_FORMAT: SampleFormat = SampleFormat::F32;
/// Sample rate used when a device does not advertise a nominal rate.
const DEFAULT_SAMPLE_RATE: u32 = SAMPLE_RATE_48K;

/// Capture (input) maps to the capture data flow, render (output) to render.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    #[cfg(feature = "capture")]
    Input,
    #[cfg(feature = "render")]
    Output,
}

impl Direction {
    #[inline]
    fn data_flow(self) -> EDataFlow {
        match self {
            #[cfg(feature = "capture")]
            Direction::Input => eCapture,
            #[cfg(feature = "render")]
            Direction::Output => eRender,
        }
    }
}

/// COM lifetime guard for a thread (apartment is initialized for the duration
/// of the guard). Tolerates an already-initialized apartment.
struct ComGuard {
    owned: bool,
}

impl ComGuard {
    fn new() -> Result<Self> {
        // SAFETY: standard COM initialization; matched by `CoUninitialize` in Drop.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            Ok(Self {
                owned: true,
            })
        } else if hr == RPC_E_CHANGED_MODE {
            // Apartment already initialized in another mode; reuse it.
            Ok(Self {
                owned: false,
            })
        } else {
            Err(Error::InitializationFailed(format!("CoInitializeEx (HRESULT {:#010x})", hr.0).into()))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.owned {
            // SAFETY: balances the `CoInitializeEx` performed in `new`.
            unsafe { CoUninitialize() };
        }
    }
}

/// RAII wrapper for COM memory returned by WASAPI APIs.
struct CoTaskMemPtr<T>(*mut T);

impl<T> CoTaskMemPtr<T> {
    #[inline]
    fn new(ptr: *mut T) -> Self {
        Self(ptr)
    }

    #[inline]
    unsafe fn as_ref(&self) -> Option<&T> {
        // SAFETY: caller guarantees the COM allocation is valid for reads while this
        // wrapper lives.
        unsafe { self.0.as_ref() }
    }
}

impl<T> Drop for CoTaskMemPtr<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: pointer came from a COM API documented to allocate with
            // CoTaskMemAlloc.
            unsafe { CoTaskMemFree(Some(self.0 as *const c_void)) };
        }
    }
}

/// Read a WASAPI `PWSTR`/`PCWSTR` device id and free the COM allocation.
fn read_device_id(device: &IMMDevice) -> Option<String> {
    // SAFETY: `GetId` returns a COM-allocated NUL-terminated wide string; we copy
    // it before `CoTaskMemPtr` releases the allocation.
    unsafe {
        let raw = device.GetId().ok()?;
        if raw.is_null() {
            return None;
        }
        let _raw_guard = CoTaskMemPtr::new(raw.0);
        raw.to_string().ok()
    }
}

/// Read the device friendly name; falls back to `None` on any failure.
fn read_friendly_name(device: &IMMDevice) -> Option<String> {
    // SAFETY: open the property store read-only and read the friendly-name
    // property. `BSTR::try_from` decodes the `PROPVARIANT` (which clears itself
    // on drop) without any manual COM-memory management.
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }.ok()?;
    let prop = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }.ok()?;
    let name = BSTR::try_from(&prop).ok()?.to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Check whether WASAPI shared mode accepts the exact client format at
/// `sample_rate`.
fn is_sample_rate_supported(client: &IAudioClient, channels: u8, sample_rate: u32) -> bool {
    let format = make_waveformat(sample_rate, channels as u16);
    let mut closest: *mut WAVEFORMATEX = ptr::null_mut();
    let hr = unsafe { client.IsFormatSupported(AUDCLNT_SHAREMODE_SHARED, &format, Some(&mut closest)) };
    let _closest = CoTaskMemPtr::new(closest);
    hr == S_OK
}

/// Query sample rates accepted by WASAPI for the client format. The device mix
/// rate is always kept because it is reported by WASAPI as the shared-mode mix
/// format for this endpoint.
fn supported_sample_rates(client: &IAudioClient, channels: u8, mix_rate: u32) -> Vec<u32> {
    let mut rates: Vec<u32> = STANDARD_SAMPLE_RATES.iter().copied().filter(|&rate| is_sample_rate_supported(client, channels, rate)).collect();

    if !rates.contains(&mix_rate) {
        rates.push(mix_rate);
    }
    rates.sort_unstable();
    rates.dedup();
    rates
}

/// Activate an `IAudioClient` for a device and read its mix format plus the
/// sample rates accepted for the fixed client format.
fn query_device_format(device: &IMMDevice) -> Result<(u8, u32, Vec<u32>)> {
    // SAFETY: `Activate` returns a fresh `IAudioClient`; `GetMixFormat` returns a
    // COM-allocated `WAVEFORMATEX` released by `CoTaskMemPtr` after copying fields.
    let client: IAudioClient = unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|err| failed_error!(format!("Activate ({})", err.message())))?;
    let format = CoTaskMemPtr::new(unsafe { client.GetMixFormat() }.map_err(|err| failed_error!(format!("GetMixFormat ({})", err.message())))?);
    let Some(format_ref) = (unsafe { format.as_ref() }) else {
        return Err(failed_error!("GetMixFormat returned null"));
    };
    let (channels, sample_rate) = (format_ref.nChannels, format_ref.nSamplesPerSec);

    let channels = channels.clamp(1, u8::MAX as u16) as u8;

    let sample_rate = if sample_rate == 0 {
        DEFAULT_SAMPLE_RATE
    } else {
        sample_rate
    };
    let sample_rates = supported_sample_rates(&client, channels, sample_rate);
    Ok((channels, sample_rate, sample_rates))
}

/// Create the process-wide device enumerator.
fn create_enumerator() -> Result<IMMDeviceEnumerator> {
    // SAFETY: standard `CoCreateInstance` for the WASAPI device enumerator.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
        .map_err(|err| failed_error!(format!("CoCreateInstance(MMDeviceEnumerator) ({})", err.message())))
}

/// Enumerate all active endpoints for the given direction.
fn enumerate(direction: Direction) -> Result<Vec<AudioDeviceDescriptor>> {
    let enumerator = create_enumerator()?;
    let collection = unsafe { enumerator.EnumAudioEndpoints(direction.data_flow(), DEVICE_STATE_ACTIVE) }
        .map_err(|err| failed_error!(format!("EnumAudioEndpoints ({})", err.message())))?;
    let count = unsafe { collection.GetCount() }.map_err(|err| failed_error!(format!("GetCount ({})", err.message())))?;

    let mut devices = Vec::with_capacity(count as usize);
    for index in 0..count {
        let Ok(device) = (unsafe { collection.Item(index) }) else {
            continue;
        };
        let Some(id) = read_device_id(&device) else {
            continue;
        };
        let name = read_friendly_name(&device).unwrap_or_else(|| id.clone());
        let Ok((channels, mix_rate, sample_rates)) = query_device_format(&device) else {
            continue;
        };
        let sample_rate_index = sample_rates.iter().position(|&r| r == mix_rate).unwrap_or(0);

        devices.push(AudioDeviceDescriptor {
            info: DeviceInformation {
                id,
                name,
            },
            channels,
            sample_rates,
            sample_rate_index,
        });
    }

    Ok(devices)
}

macro_rules! impl_device_manager {
    ($manager:ident, $device:ident, $direction:expr) => {
        pub struct $manager {
            devices: Option<Vec<$device>>,
            handler: Option<DeviceEventHandler>,
            // Held purely to keep COM initialized for the manager's lifetime
            // (released on drop); never read directly.
            #[allow(dead_code)]
            com: ComGuard,
        }

        impl DeviceManager for $manager {
            type DeviceType = $device;
            type Iter<'a>
                = Iter<'a, $device>
            where
                Self: 'a;
            type IterMut<'a>
                = IterMut<'a, $device>
            where
                Self: 'a;

            fn init() -> Result<Self>
            where
                Self: Sized,
            {
                Ok(Self {
                    devices: None,
                    handler: None,
                    com: ComGuard::new()?,
                })
            }

            fn deinit(&mut self) {}

            fn index(&self, index: usize) -> Option<&Self::DeviceType> {
                self.devices.as_ref().and_then(|devices| devices.get(index))
            }

            fn index_mut(&mut self, index: usize) -> Option<&mut Self::DeviceType> {
                self.devices.as_mut().and_then(|devices| devices.get_mut(index))
            }

            fn lookup(&self, id: &str) -> Option<&Self::DeviceType> {
                self.devices.as_ref().and_then(|devices| devices.iter().find(|device| device.desc.info.id == id))
            }

            fn lookup_mut(&mut self, id: &str) -> Option<&mut Self::DeviceType> {
                self.devices.as_mut().and_then(|devices| devices.iter_mut().find(|device| device.desc.info.id == id))
            }

            fn iter(&self) -> Self::Iter<'_> {
                self.devices.as_deref().unwrap_or(&[]).iter()
            }

            fn iter_mut(&mut self) -> Self::IterMut<'_> {
                self.devices.as_deref_mut().unwrap_or(&mut []).iter_mut()
            }

            fn refresh(&mut self) -> Result<()> {
                let devices: Vec<$device> = enumerate($direction)?.into_iter().map($device::new).collect();
                let count = devices.len();
                self.devices = Some(devices);
                if let Some(handler) = &self.handler {
                    handler(&DeviceEvent::Refreshed(count));
                }
                Ok(())
            }

            fn set_change_handler<F>(&mut self, handler: F) -> Result<()>
            where
                F: Fn(&DeviceEvent) + Send + Sync + 'static,
            {
                self.handler = Some(Box::new(handler));
                Ok(())
            }
        }
    };
}

#[cfg(feature = "capture")]
impl_device_manager!(WasapiInputDeviceManager, WasapiInputDevice, Direction::Input);
#[cfg(feature = "render")]
impl_device_manager!(WasapiOutputDeviceManager, WasapiOutputDevice, Direction::Output);

/// Build an interleaved 32-bit float `WAVEFORMATEX` matching
/// [`CLIENT_SAMPLE_FORMAT`].
fn make_waveformat(sample_rate: u32, channels: u16) -> WAVEFORMATEX {
    let bits = (CLIENT_SAMPLE_FORMAT.bits() as u16).max(8);
    let block_align = channels * (bits / 8);
    WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_IEEE_FLOAT as u16,
        nChannels: channels,
        nSamplesPerSec: sample_rate,
        nAvgBytesPerSec: sample_rate * block_align as u32,
        nBlockAlign: block_align,
        wBitsPerSample: bits,
        cbSize: 0,
    }
}

#[derive(Clone, Copy)]
struct EventSignal(HANDLE);

impl EventSignal {
    #[inline]
    fn raw(self) -> HANDLE {
        self.0
    }
}

unsafe impl Send for EventSignal {}
unsafe impl Sync for EventSignal {}

struct EventHandle(HANDLE);

impl EventHandle {
    fn create() -> Result<Self> {
        // SAFETY: creates an unnamed auto-reset event, initially non-signaled.
        let event =
            unsafe { CreateEventW(None, false, false, PCWSTR::null()) }.map_err(|err| failed_error!(format!("CreateEventW ({})", err.message())))?;
        Ok(Self(event))
    }

    #[inline]
    fn raw(&self) -> HANDLE {
        self.0
    }

    #[inline]
    fn signal(&self) -> EventSignal {
        EventSignal(self.0)
    }

    fn set(&self) -> Result<()> {
        // SAFETY: `self.0` is a live event HANDLE owned by this wrapper.
        unsafe { SetEvent(self.0) }.map_err(|err| failed_error!(format!("SetEvent ({})", err.message())))
    }
}

impl Drop for EventHandle {
    fn drop(&mut self) {
        // SAFETY: `EventHandle` uniquely owns this HANDLE and closes it once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

unsafe impl Send for EventHandle {}
unsafe impl Sync for EventHandle {}

/// A device descriptor shared by both capture and render devices.
#[derive(Clone)]
struct AudioDeviceDescriptor {
    info: DeviceInformation,
    channels: u8,
    /// Sample rates accepted for this direction.
    sample_rates: Vec<u32>,

    /// Index into [`Self::sample_rates`] for the currently active rate.
    sample_rate_index: usize,
}

impl AudioDeviceDescriptor {
    #[inline]
    fn current_sample_rate(&self) -> u32 {
        self.sample_rates.get(self.sample_rate_index).copied().unwrap_or(DEFAULT_SAMPLE_RATE)
    }

    fn set_current_sample_rate(&mut self, rate: u32) -> bool {
        if let Some(pos) = self.sample_rates.iter().position(|&r| r == rate) {
            self.sample_rate_index = pos;
            true
        } else {
            false
        }
    }
}

/// Build the format-list `Variant` advertised through `Device::formats()`.
fn format_variant(desc: &AudioDeviceDescriptor) -> Result<Variant> {
    let mut format = Variant::new_dict();
    format["format"] = CLIENT_SAMPLE_FORMAT.to_string().into();
    format["channels"] = (desc.channels as u32).into();
    format["sample-rates"] = desc.sample_rates.iter().map(|rate| Variant::from(*rate)).collect();

    let mut formats = Variant::new_array();
    formats.array_add(format);
    Ok(formats)
}

enum WaitSignal {
    Audio,
    Stop,
}

/// Initialized audio client plus derived parameters, used by the worker loops.
struct ClientSession {
    client: IAudioClient,
    event: EventHandle,
    #[cfg(feature = "render")]
    buffer_frames: u32,
    block_align: usize,
}

impl ClientSession {
    /// Look up a device by id, activate and initialize a shared-mode,
    /// event-driven client using the fixed client format with
    /// sample-rate/channel auto-conversion.
    fn open(device_id: &str, channels: u8, sample_rate: u32) -> Result<Self> {
        let enumerator = create_enumerator()?;
        let device =
            unsafe { enumerator.GetDevice(&HSTRING::from(device_id)) }.map_err(|err| failed_error!(format!("GetDevice ({})", err.message())))?;
        let client: IAudioClient =
            unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|err| failed_error!(format!("Activate ({})", err.message())))?;

        let format = make_waveformat(sample_rate, channels as u16);
        let stream_flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        unsafe { client.Initialize(AUDCLNT_SHAREMODE_SHARED, stream_flags, 0, 0, &format, None) }
            .map_err(|err| failed_error!(format!("IAudioClient::Initialize ({})", err.message())))?;

        #[cfg(feature = "render")]
        let buffer_frames = unsafe { client.GetBufferSize() }.map_err(|err| failed_error!(format!("GetBufferSize ({})", err.message())))?;
        let block_align = channels as usize * CLIENT_SAMPLE_FORMAT.bytes() as usize;

        let event = EventHandle::create()?;
        if let Err(err) = unsafe { client.SetEventHandle(event.raw()) } {
            return Err(failed_error!(format!("SetEventHandle ({})", err.message())));
        }

        Ok(Self {
            client,
            event,
            #[cfg(feature = "render")]
            buffer_frames,
            block_align,
        })
    }

    fn wait_for_signal(&self, stop_event: EventSignal) -> Result<WaitSignal> {
        let handles = [self.event.raw(), stop_event.raw()];
        // SAFETY: both handles are live event objects for the duration of the wait.
        let result = unsafe { WaitForMultipleObjectsEx(&handles, false, INFINITE, false) };
        if result == WAIT_FAILED {
            return Err(failed_error!("WaitForMultipleObjectsEx failed"));
        }

        match result.0 - WAIT_OBJECT_0.0 {
            0 => Ok(WaitSignal::Audio),
            1 => Ok(WaitSignal::Stop),
            index => Err(failed_error!(format!("WaitForMultipleObjectsEx returned unexpected index {index}"))),
        }
    }

    fn start(&self) -> Result<()> {
        unsafe { self.client.Start() }.map_err(|err| failed_error!(format!("IAudioClient::Start ({})", err.message())))
    }

    fn stop(&self) {
        let _ = unsafe { self.client.Stop() };
    }

    #[cfg(feature = "capture")]
    fn capture_client(&self) -> Result<IAudioCaptureClient> {
        unsafe { self.client.GetService() }.map_err(|err| failed_error!(format!("GetService(IAudioCaptureClient) ({})", err.message())))
    }

    #[cfg(feature = "render")]
    fn render_client(&self) -> Result<IAudioRenderClient> {
        unsafe { self.client.GetService() }.map_err(|err| failed_error!(format!("GetService(IAudioRenderClient) ({})", err.message())))
    }

    #[cfg(feature = "render")]
    fn available_render_frames(&self) -> u32 {
        let padding = unsafe { self.client.GetCurrentPadding() }.unwrap_or(0);
        self.buffer_frames.saturating_sub(padding)
    }
}

/// Handle to a running worker thread; signals and joins it on stop/drop.
struct Worker {
    stop_event: EventHandle,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    fn shutdown(&mut self) {
        let _ = self.stop_event.set();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(feature = "capture")]
fn deliver_capture_packet(
    handler: &OutputHandler,
    pool: &Arc<FramePool<Frame<'static, AudioFrameDescriptor>>>,
    data: *const u8,
    frames: u32,
    block_align: usize,
    silent: bool,
    channels: u8,
    sample_rate: u32,
    source: &str,
) {
    if silent {
        // Recycle a pooled zeroed frame instead of allocating per silent packet.
        let Ok(desc) = AudioFrameDescriptor::try_new(CLIENT_SAMPLE_FORMAT, channels, frames, sample_rate) else {
            return;
        };
        let Ok(mut shared) = pool.get_frame_with_descriptor(desc) else {
            return;
        };
        if let Some(frame) = shared.write() {
            if let Ok(mut guard) = frame.map_mut() {
                if let Some(mut planes) = guard.planes_mut() {
                    if let Some(plane) = planes.plane_data_mut(0) {
                        plane.fill(0);
                    }
                }
            }
            frame.source = Some(source.to_string());
            let _ = handler(frame.clone().into());
        }
        return;
    }

    // Zero-copy: borrow the WASAPI endpoint buffer directly into the frame.
    let buffer = unsafe { slice::from_raw_parts(data, frames as usize * block_align) };
    if let Ok(mut frame) = Frame::audio_creator().create_from_buffer(CLIENT_SAMPLE_FORMAT, channels, frames, sample_rate, buffer) {
        frame.source = Some(source.to_string());
        let _ = handler(frame);
    }
}

#[cfg(feature = "capture")]
fn run_capture(device_id: String, channels: u8, sample_rate: u32, handler: OutputHandler, stop_event: EventSignal) -> Result<()> {
    let _com = ComGuard::new()?;
    let session = ClientSession::open(&device_id, channels, sample_rate)?;
    let capture_client = session.capture_client()?;
    let pool = FramePool::<Frame<'static, AudioFrameDescriptor>>::new();

    session.start()?;

    while let WaitSignal::Audio = session.wait_for_signal(stop_event)? {
        // Drain all queued packets.
        loop {
            let packet = unsafe { capture_client.GetNextPacketSize() }.unwrap_or(0);
            if packet == 0 {
                break;
            }

            let mut data: *mut u8 = ptr::null_mut();
            let mut frames: u32 = 0;
            let mut flags = 0u32;
            if unsafe { capture_client.GetBuffer(&mut data, &mut frames, &mut flags, None, None) }.is_err() {
                break;
            }

            if frames > 0 {
                let silent = (flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;
                if silent || !data.is_null() {
                    deliver_capture_packet(&handler, &pool, data, frames, session.block_align, silent, channels, sample_rate, &device_id);
                }
            }

            let _ = unsafe { capture_client.ReleaseBuffer(frames) };
        }
    }

    session.stop();
    Ok(())
}

#[cfg(feature = "capture")]
pub struct WasapiInputDevice {
    desc: AudioDeviceDescriptor,
    running: bool,
    handler: Option<OutputHandler>,
    worker: Option<Worker>,
}

#[cfg(feature = "capture")]
impl WasapiInputDevice {
    fn new(desc: AudioDeviceDescriptor) -> Self {
        Self {
            desc,
            running: false,
            handler: None,
            worker: None,
        }
    }
}

#[cfg(feature = "capture")]
impl Device for WasapiInputDevice {
    fn name(&self) -> &str {
        &self.desc.info.name
    }

    fn id(&self) -> &str {
        &self.desc.info.id
    }

    fn start(&mut self) -> Result<()> {
        if self.running {
            return Ok(());
        }

        let handler = self.handler.clone().ok_or_else(|| none_param_error!(handler))?;
        let stop_event = EventHandle::create()?;
        let worker_stop = stop_event.signal();
        let device_id = self.desc.info.id.clone();
        let channels = self.desc.channels;
        let sample_rate = self.desc.current_sample_rate();

        let handle = thread::Builder::new()
            .name("wasapi-capture".into())
            .spawn(move || {
                let _ = run_capture(device_id, channels, sample_rate, handler, worker_stop);
            })
            .map_err(|err| failed_error!(format!("spawn capture thread ({err})")))?;

        self.worker = Some(Worker {
            stop_event,
            handle: Some(handle),
        });

        self.running = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if !self.running {
            return Err(Error::NotRunning(self.desc.info.name.clone().into()));
        }

        if let Some(mut worker) = self.worker.take() {
            worker.shutdown();
        }
        self.running = false;
        Ok(())
    }

    fn configure(&mut self, options: &Variant) -> Result<()> {
        if self.running {
            return Err(Error::NotImplemented);
        }

        if let Some(sample_rate) = options["sample-rate"].get_uint32() {
            if sample_rate > 0 {
                self.desc.set_current_sample_rate(sample_rate);
            }
        }
        if let Some(channels) = options["channels"].get_uint32() {
            if channels > 0 {
                self.desc.channels = channels.min(u8::MAX as u32) as u8;
            }
        }

        Ok(())
    }

    fn control(&mut self, _action: &Variant) -> Result<()> {
        Err(Error::NotImplemented)
    }

    fn running(&self) -> bool {
        self.running
    }

    fn formats(&self) -> Result<Variant> {
        format_variant(&self.desc)
    }
}

#[cfg(feature = "capture")]
impl CaptureHanlder for WasapiInputDevice {
    fn set_output_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(Frame) -> Result<()> + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(handler));
        Ok(())
    }
}

#[cfg(feature = "capture")]
impl Drop for WasapiInputDevice {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(feature = "render")]
fn fill_render_buffer(handler: &InputHandler, data: *mut u8, frames: u32, block_align: usize, channels: u8, sample_rate: u32) -> bool {
    // Zero-copy: wrap the WASAPI render buffer mutably so the handler fills it
    // in place; no per-callback allocation.
    let buffer = unsafe { slice::from_raw_parts_mut(data, frames as usize * block_align) };
    match Frame::audio_creator().create_from_mut_buffer(CLIENT_SAMPLE_FORMAT, channels, frames, sample_rate, buffer) {
        Ok(mut frame) => handler(&mut frame).is_ok(),
        Err(_) => false,
    }
}

#[cfg(feature = "render")]
fn run_render(device_id: String, channels: u8, sample_rate: u32, handler: InputHandler, stop_event: EventSignal) -> Result<()> {
    let _com = ComGuard::new()?;
    let session = ClientSession::open(&device_id, channels, sample_rate)?;
    let render_client = session.render_client()?;

    // Prime the endpoint buffer with silence to avoid an initial glitch.
    if let Ok(data) = unsafe { render_client.GetBuffer(session.buffer_frames) } {
        if !data.is_null() {
            let len = session.buffer_frames as usize * session.block_align;
            unsafe { slice::from_raw_parts_mut(data, len).fill(0) };
        }
        let _ = unsafe { render_client.ReleaseBuffer(session.buffer_frames, 0) };
    }

    session.start()?;

    while let WaitSignal::Audio = session.wait_for_signal(stop_event)? {
        let available = session.available_render_frames();
        if available == 0 {
            continue;
        }

        let data = match unsafe { render_client.GetBuffer(available) } {
            Ok(data) if !data.is_null() => data,
            _ => continue,
        };

        let filled = fill_render_buffer(&handler, data, available, session.block_align, channels, sample_rate);
        // On handler failure emit silence for this period.
        let flags = if filled {
            0
        } else {
            AUDCLNT_BUFFERFLAGS_SILENT.0 as u32
        };

        let _ = unsafe { render_client.ReleaseBuffer(available, flags) };
    }

    session.stop();
    Ok(())
}

#[cfg(feature = "render")]
pub struct WasapiOutputDevice {
    desc: AudioDeviceDescriptor,
    running: bool,
    handler: Option<InputHandler>,
    worker: Option<Worker>,
}

#[cfg(feature = "render")]
impl WasapiOutputDevice {
    fn new(desc: AudioDeviceDescriptor) -> Self {
        Self {
            desc,
            running: false,
            handler: None,
            worker: None,
        }
    }
}

#[cfg(feature = "render")]
impl Device for WasapiOutputDevice {
    fn name(&self) -> &str {
        &self.desc.info.name
    }

    fn id(&self) -> &str {
        &self.desc.info.id
    }

    fn start(&mut self) -> Result<()> {
        if self.running {
            return Ok(());
        }

        let handler = self.handler.clone().ok_or_else(|| none_param_error!(handler))?;
        let stop_event = EventHandle::create()?;
        let worker_stop = stop_event.signal();
        let device_id = self.desc.info.id.clone();
        let channels = self.desc.channels;
        let sample_rate = self.desc.current_sample_rate();

        let handle = thread::Builder::new()
            .name("wasapi-render".into())
            .spawn(move || {
                let _ = run_render(device_id, channels, sample_rate, handler, worker_stop);
            })
            .map_err(|err| failed_error!(format!("spawn render thread ({err})")))?;

        self.worker = Some(Worker {
            stop_event,
            handle: Some(handle),
        });

        self.running = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if !self.running {
            return Err(Error::NotRunning(self.desc.info.name.clone().into()));
        }

        if let Some(mut worker) = self.worker.take() {
            worker.shutdown();
        }
        self.running = false;
        Ok(())
    }

    fn configure(&mut self, options: &Variant) -> Result<()> {
        if self.running {
            return Err(Error::NotImplemented);
        }

        if let Some(sample_rate) = options["sample-rate"].get_uint32() {
            if sample_rate > 0 {
                self.desc.set_current_sample_rate(sample_rate);
            }
        }
        if let Some(channels) = options["channels"].get_uint32() {
            if channels > 0 {
                self.desc.channels = channels.min(u8::MAX as u32) as u8;
            }
        }

        Ok(())
    }

    fn control(&mut self, _action: &Variant) -> Result<()> {
        Err(Error::NotImplemented)
    }

    fn running(&self) -> bool {
        self.running
    }

    fn formats(&self) -> Result<Variant> {
        format_variant(&self.desc)
    }
}

#[cfg(feature = "render")]
impl RenderHandler for WasapiOutputDevice {
    fn set_input_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(&mut Frame) -> Result<()> + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(handler));
        Ok(())
    }
}

#[cfg(feature = "render")]
impl Drop for WasapiOutputDevice {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
