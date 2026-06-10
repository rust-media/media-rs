#[cfg(feature = "capture")]
use std::mem::ManuallyDrop;
#[cfg(feature = "render")]
use std::slice;
use std::{
    ffi::c_void,
    mem, ptr,
    ptr::NonNull,
    slice::{Iter, IterMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[cfg(feature = "render")]
use audio_toolbox::kAudioUnitProperty_SetRenderCallback;
use audio_toolbox::{
    audio_component::{Component, ComponentInstance},
    kAudioOutputUnitProperty_CurrentDevice, kAudioOutputUnitProperty_EnableIO, kAudioUnitManufacturer_Apple, kAudioUnitProperty_StreamFormat,
    kAudioUnitScope_Global, kAudioUnitScope_Input, kAudioUnitScope_Output, kAudioUnitSubType_HALOutput, kAudioUnitType_Output,
    AURenderCallbackStruct, AudioComponentDescription, AudioUnitElement, AudioUnitRenderActionFlags,
};
#[cfg(feature = "capture")]
use audio_toolbox::{audio_unit::Unit, kAudioOutputUnitProperty_SetInputCallback, AudioUnit};
#[cfg(feature = "capture")]
use core_audio::kAudioObjectPropertyScopeInput;
#[cfg(feature = "render")]
use core_audio::kAudioObjectPropertyScopeOutput;
use core_audio::{
    audio_hardware::{global_property_address, property_address, AudioObject},
    host_time::host_time_to_nanos,
    kAudioDevicePropertyAvailableNominalSampleRates, kAudioDevicePropertyDeviceUID, kAudioDevicePropertyNominalSampleRate,
    kAudioDevicePropertyStreamConfiguration, kAudioHardwarePropertyDevices, kAudioObjectPropertyElementMain, kAudioObjectPropertyName, AudioObjectID,
    AudioObjectPropertyScope,
};
use core_audio_types::{
    kAudioFormatFlagIsFloat, kAudioFormatFlagIsPacked, kAudioFormatLinearPCM, AudioBuffer, AudioBufferList as CoreAudioBufferList,
    AudioStreamBasicDescription, AudioTimeStamp, AudioValueRange,
};
use core_foundation::{
    base::{OSStatus, TCFType},
    string::{CFString, CFStringRef},
};
#[cfg(feature = "render")]
use media_core::FrameDescriptor;
#[cfg(feature = "capture")]
use media_core::{audio::AudioFrame, frame::MappedGuard};
use media_core::{
    audio::{AudioFrameDescriptor, SampleFormat, SAMPLE_RATE_48K, STANDARD_SAMPLE_RATES},
    error::Error,
    failed_error,
    frame::Frame,
    frame_pool::FramePool,
    invalid_error, none_param_error, not_found_error,
    time::NSEC_PER_MSEC,
    unsupported_error,
    variant::Variant,
    Result,
};
use media_device_types::device::{Device, DeviceEvent, DeviceEventHandler, DeviceInformation, DeviceManager};
#[cfg(feature = "capture")]
use media_device_types::{capture::CaptureHanlder, device::OutputHandler};
#[cfg(feature = "render")]
use media_device_types::{device::InputHandler, render::RenderHandler};

/// AUHAL output bus (element 0).
const OUTPUT_ELEMENT: AudioUnitElement = 0;
/// AUHAL input bus (element 1).
const INPUT_ELEMENT: AudioUnitElement = 1;
/// Sample rate used when a device does not advertise a nominal sample rate.
const DEFAULT_SAMPLE_RATE: u32 = SAMPLE_RATE_48K;
/// Fixed client sample format: 32-bit float, interleaved.
const CLIENT_SAMPLE_FORMAT: SampleFormat = SampleFormat::F32;
/// Defensive upper bound for CoreAudio render callback buffer lists.
#[cfg(feature = "render")]
const MAX_AUDIO_BUFFER_COUNT: usize = 32;

/// Capture (input) uses the input scope, render (output) uses the output scope.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    #[cfg(feature = "capture")]
    Input,
    #[cfg(feature = "render")]
    Output,
}

impl Direction {
    #[inline]
    fn scope(self) -> AudioObjectPropertyScope {
        match self {
            #[cfg(feature = "capture")]
            Direction::Input => kAudioObjectPropertyScopeInput,
            #[cfg(feature = "render")]
            Direction::Output => kAudioObjectPropertyScopeOutput,
        }
    }
}

/// Read a `CFString` device property as an owned `String`.
fn cfstring_property(obj: AudioObject, selector: u32) -> Option<String> {
    let address = global_property_address(selector);
    let raw: CFStringRef = obj.get_property(&address).ok()?;
    if raw.is_null() {
        return None;
    }
    Some(unsafe { CFString::wrap_under_create_rule(raw) }.to_string())
}

/// Count the number of channels the device exposes on the given scope.
fn channel_count(obj: AudioObject, scope: AudioObjectPropertyScope) -> u32 {
    let address = property_address(kAudioDevicePropertyStreamConfiguration, scope, kAudioObjectPropertyElementMain);
    let bytes = match obj.get_property_bytes(&address) {
        Ok(bytes) => bytes,
        Err(_) => return 0,
    };

    let number_buffers_offset = mem::offset_of!(CoreAudioBufferList, mNumberBuffers);
    let buffers_offset = mem::offset_of!(CoreAudioBufferList, mBuffers);
    let channels_offset = mem::offset_of!(AudioBuffer, mNumberChannels);
    let buffer_stride = mem::size_of::<AudioBuffer>();
    let u32_size = mem::size_of::<u32>();

    if bytes.len() < number_buffers_offset + u32_size {
        return 0;
    }

    let num_buffers = unsafe { ptr::read_unaligned(bytes[number_buffers_offset..].as_ptr().cast::<u32>()) } as usize;
    let mut total = 0u32;
    for index in 0..num_buffers {
        let Some(buffer_offset) = index.checked_mul(buffer_stride).and_then(|offset| buffers_offset.checked_add(offset)) else {
            break;
        };
        let Some(offset) = buffer_offset.checked_add(channels_offset) else {
            break;
        };
        let Some(end) = offset.checked_add(u32_size) else {
            break;
        };
        if end > bytes.len() {
            break;
        }
        total = total.saturating_add(unsafe { ptr::read_unaligned(bytes[offset..].as_ptr().cast::<u32>()) });
    }

    total
}

/// Query the device nominal sample rate, falling back to
/// [`DEFAULT_SAMPLE_RATE`].
fn nominal_sample_rate(obj: AudioObject) -> u32 {
    let address = global_property_address(kAudioDevicePropertyNominalSampleRate);
    obj.get_property::<f64>(&address).ok().filter(|rate| *rate > 0.0).map(|rate| rate as u32).unwrap_or(DEFAULT_SAMPLE_RATE)
}

/// Query the discrete set of sample rates supported by the device.
fn available_sample_rates(obj: AudioObject, scope: AudioObjectPropertyScope) -> Vec<u32> {
    let address = property_address(kAudioDevicePropertyAvailableNominalSampleRates, scope, kAudioObjectPropertyElementMain);
    let ranges: Vec<AudioValueRange> = match obj.get_property_array(&address) {
        Ok(ranges) => ranges,
        Err(_) => return Vec::new(),
    };

    let mut rates = Vec::with_capacity(ranges.len());
    for range in ranges {
        if range.mMinimum <= 0.0 || range.mMaximum <= 0.0 {
            continue;
        }
        if (range.mMaximum - range.mMinimum).abs() < f64::EPSILON {
            rates.push(range.mMinimum as u32);
        } else {
            // Continuous range: pick well-known anchors inside it.
            let lo = range.mMinimum;
            let hi = range.mMaximum;
            for &anchor in STANDARD_SAMPLE_RATES {
                let value = anchor as f64;
                if value >= lo && value <= hi {
                    rates.push(anchor);
                }
            }
        }
    }
    rates.sort_unstable();
    rates.dedup();
    rates
}

/// Enumerate all hardware devices that expose channels on the given direction.
fn enumerate(direction: Direction) -> Result<Vec<AudioDeviceDescriptor>> {
    let system = AudioObject::system();
    let address = global_property_address(kAudioHardwarePropertyDevices);
    let ids: Vec<AudioObjectID> =
        system.get_property_array(&address).map_err(|status| failed_error!(format!("get device list (OSStatus {status})")))?;

    let mut devices = Vec::with_capacity(ids.len());
    for id in ids {
        let obj = AudioObject::new(id);
        let channels = channel_count(obj, direction.scope());
        if channels == 0 {
            continue;
        }

        let uid = cfstring_property(obj, kAudioDevicePropertyDeviceUID).unwrap_or_else(|| id.to_string());
        let name = cfstring_property(obj, kAudioObjectPropertyName).unwrap_or_else(|| uid.clone());

        let sample_rate = nominal_sample_rate(obj);
        let mut sample_rates = available_sample_rates(obj, direction.scope());
        if !sample_rates.contains(&sample_rate) {
            sample_rates.push(sample_rate);
            sample_rates.sort_unstable();
        }
        let sample_rate_index = sample_rates.iter().position(|&r| r == sample_rate).unwrap_or(0);

        let channels = channels.min(u8::MAX as u32) as u8;
        devices.push(AudioDeviceDescriptor {
            info: DeviceInformation {
                id: uid,
                name,
            },
            device_id: id,
            channels,
            max_channels: channels,
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
impl_device_manager!(CoreAudioInputDeviceManager, CoreAudioInputDevice, Direction::Input);
#[cfg(feature = "render")]
impl_device_manager!(CoreAudioOutputDeviceManager, CoreAudioOutputDevice, Direction::Output);

#[inline]
fn check(result: std::result::Result<(), OSStatus>, ctx: &'static str) -> Result<()> {
    result.map_err(|status| failed_error!(format!("{ctx} (OSStatus {status})")))
}

/// Build an interleaved `AudioStreamBasicDescription` matching
/// [`CLIENT_SAMPLE_FORMAT`].
fn make_asbd(sample_rate: f64, channels: u32) -> AudioStreamBasicDescription {
    let bytes_per_frame = CLIENT_SAMPLE_FORMAT.bytes() as u32 * channels;
    let mut format_flags = kAudioFormatFlagIsPacked;
    if CLIENT_SAMPLE_FORMAT.is_float() {
        format_flags |= kAudioFormatFlagIsFloat;
    }
    AudioStreamBasicDescription {
        mSampleRate: sample_rate,
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: format_flags,
        mBytesPerPacket: bytes_per_frame,
        mFramesPerPacket: 1,
        mBytesPerFrame: bytes_per_frame,
        mChannelsPerFrame: channels,
        mBitsPerChannel: CLIENT_SAMPLE_FORMAT.bits() as u32,
        mReserved: 0,
    }
}

/// A device descriptor shared by both capture and render devices.
#[derive(Clone)]
struct AudioDeviceDescriptor {
    info: DeviceInformation,
    device_id: AudioObjectID,
    channels: u8,
    max_channels: u8,
    /// Supported sample rates on this direction's scope.
    sample_rates: Vec<u32>,
    /// Index into [`Self::sample_rates`] for the currently active rate.
    sample_rate_index: usize,
}

impl AudioDeviceDescriptor {
    #[inline]
    fn current_sample_rate(&self) -> u32 {
        self.sample_rates.get(self.sample_rate_index).copied().unwrap_or(DEFAULT_SAMPLE_RATE)
    }

    /// Set the active sample rate. If `rate` is in the supported list,
    /// updates the index; otherwise returns `false`.
    fn set_current_sample_rate(&mut self, rate: u32) -> bool {
        if let Some(pos) = self.sample_rates.iter().position(|&r| r == rate) {
            self.sample_rate_index = pos;
            true
        } else {
            false
        }
    }
}

fn create_output_unit() -> Result<ComponentInstance> {
    let description = AudioComponentDescription {
        componentType: kAudioUnitType_Output,
        componentSubType: kAudioUnitSubType_HALOutput,
        componentManufacturer: kAudioUnitManufacturer_Apple,
        componentFlags: 0,
        componentFlagsMask: 0,
    };

    let component = Component::find_next(None, &description).ok_or_else(|| not_found_error!("HAL output audio unit"))?;
    component.instantiate().map_err(|status| failed_error!(format!("instantiate audio unit (OSStatus {status})")))
}

#[cfg(feature = "capture")]
fn clear_capture_callback(instance: &ComponentInstance) {
    let callback = AURenderCallbackStruct {
        inputProc: None,
        inputProcRefCon: ptr::null_mut(),
    };
    let _ = instance.set_property(kAudioOutputUnitProperty_SetInputCallback, kAudioUnitScope_Global, OUTPUT_ELEMENT, &callback);
}

#[cfg(feature = "render")]
fn clear_render_callback(instance: &ComponentInstance) {
    let callback = AURenderCallbackStruct {
        inputProc: None,
        inputProcRefCon: ptr::null_mut(),
    };
    let _ = instance.set_property(kAudioUnitProperty_SetRenderCallback, kAudioUnitScope_Input, OUTPUT_ELEMENT, &callback);
}

/// Build the format-list `Variant` advertised through `Device::formats()`.
///
/// Note on `format`: AUHAL performs format conversion between the device's
/// native format and the client ASBD we install in `start()`. Since the
/// client side is hard-wired to [`CLIENT_SAMPLE_FORMAT`], every reported
/// entry uses that format regardless of the hardware native format. The
/// list still varies in `channels` and `sample-rates` per device.
fn format_variant(desc: &AudioDeviceDescriptor) -> Result<Variant> {
    let mut format = Variant::new_dict();
    format["format"] = CLIENT_SAMPLE_FORMAT.to_string().into();
    format["channels"] = (desc.max_channels as u32).into();
    format["sample-rates"] = desc.sample_rates.iter().map(|rate| Variant::from(*rate)).collect();

    let mut formats = Variant::new_array();
    formats.array_add(format);
    Ok(formats)
}

fn configure_descriptor(desc: &mut AudioDeviceDescriptor, options: &Variant, running: bool) -> Result<()> {
    if running {
        return Err(Error::SetFailed("cannot configure a running CoreAudio device".into()));
    }

    if let Some(sample_rate) = options["sample-rate"].get_uint32() {
        if sample_rate == 0 {
            return Err(invalid_error!("sample-rate", sample_rate));
        }
        if !desc.set_current_sample_rate(sample_rate) {
            return Err(unsupported_error!("sample-rate", sample_rate));
        }
    }

    if let Some(channels) = options["channels"].get_uint32() {
        if channels == 0 {
            return Err(invalid_error!("channels", channels));
        }
        if channels > desc.max_channels as u32 {
            return Err(unsupported_error!("channels", channels));
        }
        desc.channels = channels as u8;
    }

    Ok(())
}

trait CallbackControl {
    fn deactivate(&self);
}

struct CallbackContext<T> {
    ptr: Option<NonNull<T>>,
}

impl<T> CallbackContext<T> {
    fn new(value: T) -> Self {
        let ptr = Arc::into_raw(Arc::new(value)).cast_mut();
        Self {
            ptr: NonNull::new(ptr),
        }
    }

    fn as_ref_con(&self) -> *mut c_void {
        self.ptr.map_or(ptr::null_mut(), |ptr| ptr.as_ptr().cast::<c_void>())
    }

    fn retain(in_ref_con: NonNull<c_void>) -> Arc<T> {
        let raw = in_ref_con.as_ptr().cast::<T>();
        unsafe {
            let arc = Arc::from_raw(raw);
            let retained = Arc::clone(&arc);
            let _ = Arc::into_raw(arc);
            retained
        }
    }

    fn as_ref(&self) -> Option<&T> {
        self.ptr.map(|ptr| unsafe { ptr.as_ref() })
    }

    fn deactivate(&self)
    where
        T: CallbackControl,
    {
        if let Some(ctx) = self.as_ref() {
            ctx.deactivate();
        }
    }

    fn clear(&mut self) {
        if let Some(ptr) = self.ptr.take() {
            unsafe { drop(Arc::from_raw(ptr.as_ptr())) };
        }
    }
}

impl<T> Default for CallbackContext<T> {
    fn default() -> Self {
        Self {
            ptr: None,
        }
    }
}

impl<T> Drop for CallbackContext<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

enum AudioBufferListStorage<'a> {
    #[cfg(feature = "capture")]
    Owned(CoreAudioBufferList),
    #[cfg(feature = "render")]
    Borrowed(&'a mut CoreAudioBufferList),
    #[cfg(not(feature = "render"))]
    #[allow(dead_code)]
    Marker(std::marker::PhantomData<&'a mut CoreAudioBufferList>),
}

struct AudioBufferList<'a> {
    storage: AudioBufferListStorage<'a>,
    _marker: std::marker::PhantomData<&'a mut CoreAudioBufferList>,
}

impl<'a> AudioBufferList<'a> {
    #[cfg(feature = "capture")]
    fn from_contiguous_parts(channels: u32, data: *mut u8, len: usize) -> Self {
        Self {
            storage: AudioBufferListStorage::Owned(CoreAudioBufferList {
                mNumberBuffers: 1,
                mBuffers: [AudioBuffer {
                    mNumberChannels: channels,
                    mDataByteSize: len.min(u32::MAX as usize) as u32,
                    mData: data as *mut c_void,
                }],
            }),
            _marker: std::marker::PhantomData,
        }
    }

    #[cfg(feature = "capture")]
    fn from_audio_frame<'frame>(frame: &'frame mut AudioFrame<'_>) -> Option<(MappedGuard<'frame>, Self)> {
        let channels = frame.descriptor().channels().get() as u32;
        let mut guard = frame.map_mut().ok()?;
        let (data, len) = {
            let mut planes = guard.planes_mut()?;
            let data = planes.plane_data_mut(0)?;
            (data.as_mut_ptr(), data.len())
        };

        Some((guard, Self::from_contiguous_parts(channels, data, len)))
    }

    #[cfg(feature = "render")]
    unsafe fn from_raw_mut(raw: *mut CoreAudioBufferList) -> Option<Self> {
        Some(Self {
            storage: AudioBufferListStorage::Borrowed(unsafe { raw.as_mut()? }),
            _marker: std::marker::PhantomData,
        })
    }

    fn as_raw_mut(&mut self) -> &mut CoreAudioBufferList {
        match &mut self.storage {
            #[cfg(feature = "capture")]
            AudioBufferListStorage::Owned(raw) => raw,
            #[cfg(feature = "render")]
            AudioBufferListStorage::Borrowed(raw) => raw,
            #[cfg(not(feature = "render"))]
            AudioBufferListStorage::Marker(_) => unreachable!(),
        }
    }

    #[cfg(feature = "render")]
    fn buffers_mut(&mut self) -> Option<&mut [AudioBuffer]> {
        let raw = self.as_raw_mut();
        let count = raw.mNumberBuffers as usize;
        if count == 0 || count > MAX_AUDIO_BUFFER_COUNT {
            return None;
        }
        Some(unsafe { slice::from_raw_parts_mut(raw.mBuffers.as_mut_ptr(), count) })
    }

    #[cfg(feature = "render")]
    fn contiguous_buffer_mut(&mut self) -> Option<&mut [u8]> {
        if self.as_raw_mut().mNumberBuffers != 1 {
            return None;
        }

        let buffer = self.buffers_mut()?.first_mut()?;
        Self::buffer_data_mut(buffer)
    }

    #[cfg(feature = "render")]
    fn fill_silence(&mut self) {
        let Some(buffers) = self.buffers_mut() else {
            return;
        };
        for buffer in buffers {
            if let Some(dst) = Self::buffer_data_mut(buffer) {
                dst.fill(0);
            }
        }
    }

    #[cfg(feature = "render")]
    fn fill_buffer(&mut self, src: &[u8]) {
        let Some(buffers) = self.buffers_mut() else {
            return;
        };
        let mut offset = 0usize;
        for buffer in buffers {
            let Some(dst) = Self::buffer_data_mut(buffer) else {
                continue;
            };

            let available = src.len().saturating_sub(offset);
            let copy = dst.len().min(available);
            if copy > 0 {
                dst[..copy].copy_from_slice(&src[offset..offset + copy]);
            }
            if copy < dst.len() {
                dst[copy..].fill(0);
            }
            offset += copy;
        }
    }

    #[cfg(feature = "render")]
    fn fill_frame(&mut self, frame: &Frame) -> bool {
        let Ok(guard) = frame.map() else {
            self.fill_silence();
            return false;
        };
        let Some(planes) = guard.planes() else {
            self.fill_silence();
            return false;
        };
        let Some(data) = planes.plane_data(0) else {
            self.fill_silence();
            return false;
        };

        self.fill_buffer(data);
        true
    }

    #[cfg(feature = "render")]
    fn buffer_data_mut(buffer: &mut AudioBuffer) -> Option<&mut [u8]> {
        if buffer.mData.is_null() || buffer.mDataByteSize == 0 {
            return None;
        }

        Some(unsafe { slice::from_raw_parts_mut(buffer.mData as *mut u8, buffer.mDataByteSize as usize) })
    }
}

#[cfg(feature = "capture")]
struct CaptureContext {
    active: AtomicBool,
    unit: AudioUnit,
    handler: OutputHandler,
    source: String,
    channels: u8,
    sample_rate: u32,
    pool: Arc<FramePool<Frame<'static, AudioFrameDescriptor>>>,
}

#[cfg(feature = "capture")]
impl CallbackControl for CaptureContext {
    fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
    }
}

#[cfg(feature = "capture")]
unsafe extern "C-unwind" fn capture_callback(
    in_ref_con: NonNull<c_void>,
    mut io_action_flags: NonNull<AudioUnitRenderActionFlags>,
    in_time_stamp: NonNull<AudioTimeStamp>,
    in_bus_number: u32,
    in_number_frames: u32,
    _io_data: *mut CoreAudioBufferList,
) -> i32 {
    let ctx = CallbackContext::<CaptureContext>::retain(in_ref_con);
    if !ctx.active.load(Ordering::Acquire) {
        return 0;
    }
    let desc = match AudioFrameDescriptor::try_new(CLIENT_SAMPLE_FORMAT, ctx.channels, in_number_frames, ctx.sample_rate) {
        Ok(desc) => desc,
        Err(_) => return 0,
    };
    let mut shared_frame = match ctx.pool.get_frame_with_descriptor(desc) {
        Ok(frame) => frame,
        Err(_) => return 0,
    };
    let Some(unit) = Unit::from_raw(ctx.unit) else {
        return -1;
    };
    let unit = ManuallyDrop::new(unit);
    let render_result = {
        let Some(frame) = shared_frame.write() else {
            return 0;
        };
        let Some((_, mut buffer_list)) = AudioBufferList::from_audio_frame(frame) else {
            return 0;
        };
        let result = unit.render(Some(io_action_flags.as_mut()), in_time_stamp.as_ref(), in_bus_number, in_number_frames, buffer_list.as_raw_mut());
        result
    };

    if let Err(status) = render_result {
        return status;
    }

    if let Some(frame) = shared_frame.write() {
        frame.source = Some(ctx.source.clone());
        let host_ns = host_time_to_nanos((*in_time_stamp.as_ptr()).mHostTime);
        frame.pts = Some((host_ns / NSEC_PER_MSEC as u64) as i64);
        let _ = (ctx.handler)(frame.clone().into());
    }

    0
}

#[cfg(feature = "capture")]
pub struct CoreAudioInputDevice {
    desc: AudioDeviceDescriptor,
    running: bool,
    handler: Option<OutputHandler>,
    instance: Option<ComponentInstance>,
    context: CallbackContext<CaptureContext>,
}

#[cfg(feature = "capture")]
impl CoreAudioInputDevice {
    fn new(desc: AudioDeviceDescriptor) -> Self {
        Self {
            desc,
            running: false,
            handler: None,
            instance: None,
            context: CallbackContext::default(),
        }
    }
}

#[cfg(feature = "capture")]
impl Device for CoreAudioInputDevice {
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
        let instance = create_output_unit()?;
        let channels = self.desc.channels;
        let sample_rate = self.desc.current_sample_rate();

        let enable: u32 = 1;
        check(instance.set_property(kAudioOutputUnitProperty_EnableIO, kAudioUnitScope_Input, INPUT_ELEMENT, &enable), "AudioUnitSetProperty")?;
        let disable: u32 = 0;
        check(instance.set_property(kAudioOutputUnitProperty_EnableIO, kAudioUnitScope_Output, OUTPUT_ELEMENT, &disable), "AudioUnitSetProperty")?;
        check(
            instance.set_property(kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, OUTPUT_ELEMENT, &self.desc.device_id),
            "AudioUnitSetProperty",
        )?;

        let asbd = make_asbd(sample_rate as f64, channels as u32);
        check(instance.set_property(kAudioUnitProperty_StreamFormat, kAudioUnitScope_Output, INPUT_ELEMENT, &asbd), "AudioUnitSetProperty")?;

        let context = CallbackContext::new(CaptureContext {
            active: AtomicBool::new(true),
            unit: instance.as_raw(),
            handler,
            source: self.desc.info.id.clone(),
            channels,
            sample_rate,
            pool: FramePool::<Frame<'static, AudioFrameDescriptor>>::new(),
        });

        let callback = AURenderCallbackStruct {
            inputProc: Some(capture_callback),
            inputProcRefCon: context.as_ref_con(),
        };
        check(
            instance.set_property(kAudioOutputUnitProperty_SetInputCallback, kAudioUnitScope_Global, OUTPUT_ELEMENT, &callback),
            "AudioUnitSetProperty",
        )?;

        if let Err(err) = check(instance.initialize(), "AudioUnitInitialize") {
            context.deactivate();
            clear_capture_callback(&instance);
            return Err(err);
        }
        if let Err(err) = check(instance.start_output_unit(), "AudioOutputUnitStart") {
            context.deactivate();
            clear_capture_callback(&instance);
            let _ = instance.uninitialize();
            return Err(err);
        }

        self.context = context;
        self.instance = Some(instance);
        self.running = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if !self.running {
            return Err(media_core::error::Error::NotRunning(self.desc.info.name.clone().into()));
        }

        self.context.deactivate();
        if let Some(instance) = self.instance.take() {
            clear_capture_callback(&instance);
            let _ = instance.stop_output_unit();
            let _ = instance.uninitialize();
        }

        self.context.clear();
        self.running = false;
        Ok(())
    }

    fn configure(&mut self, options: &Variant) -> Result<()> {
        configure_descriptor(&mut self.desc, options, self.running)
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
impl CaptureHanlder for CoreAudioInputDevice {
    fn set_output_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(Frame) -> Result<()> + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(handler));
        Ok(())
    }
}

#[cfg(feature = "capture")]
impl Drop for CoreAudioInputDevice {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(feature = "render")]
struct RenderContext {
    active: AtomicBool,
    handler: InputHandler,
    channels: u8,
    sample_rate: u32,
    pool: Arc<FramePool<Frame<'static>>>,
}

#[cfg(feature = "render")]
impl CallbackControl for RenderContext {
    fn deactivate(&self) {
        self.active.store(false, Ordering::Release);
    }
}

#[cfg(feature = "render")]
unsafe extern "C-unwind" fn render_callback(
    in_ref_con: NonNull<c_void>,
    _io_action_flags: NonNull<AudioUnitRenderActionFlags>,
    in_time_stamp: NonNull<AudioTimeStamp>,
    _in_bus_number: u32,
    in_number_frames: u32,
    io_data: *mut CoreAudioBufferList,
) -> i32 {
    let ctx = CallbackContext::<RenderContext>::retain(in_ref_con);
    if !ctx.active.load(Ordering::Acquire) {
        return 0;
    }
    let Some(mut buffer_list) = AudioBufferList::from_raw_mut(io_data) else {
        return 0;
    };
    let host_ns = host_time_to_nanos((*in_time_stamp.as_ptr()).mHostTime);
    let pts = Some((host_ns / NSEC_PER_MSEC as u64) as i64);

    // Zero-copy — wrap the AUHAL buffer directly as a Frame.
    if let Some(buf) = buffer_list.contiguous_buffer_mut() {
        // Convert the Result into Option<bool> so the Frame borrow is released.
        let handler_ok =
            match Frame::audio_creator().create_from_mut_buffer(CLIENT_SAMPLE_FORMAT, ctx.channels, in_number_frames, ctx.sample_rate, buf) {
                Ok(mut frame) => {
                    frame.pts = pts;
                    Some((ctx.handler)(&mut frame).is_ok())
                }
                Err(_) => None,
            };
        match handler_ok {
            Some(true) => return 0,
            Some(false) => {
                buffer_list.fill_silence();
                return 0;
            }
            None => {} // fall through to slow path
        }
    }

    // Reuse a pooled Frame, let handler fill it, then copy to AUHAL buffer.
    let desc = match AudioFrameDescriptor::try_new(CLIENT_SAMPLE_FORMAT, ctx.channels, in_number_frames, ctx.sample_rate) {
        Ok(desc) => FrameDescriptor::Audio(desc),
        Err(_) => {
            buffer_list.fill_silence();
            return 0;
        }
    };
    let mut shared_frame = match ctx.pool.get_frame_with_descriptor(desc) {
        Ok(frame) => frame,
        Err(_) => {
            buffer_list.fill_silence();
            return 0;
        }
    };
    let Some(frame) = shared_frame.write() else {
        buffer_list.fill_silence();
        return 0;
    };

    frame.pts = pts;

    if (ctx.handler)(frame).is_err() {
        buffer_list.fill_silence();
        return 0;
    }

    buffer_list.fill_frame(frame);

    0
}

#[cfg(feature = "render")]
pub struct CoreAudioOutputDevice {
    desc: AudioDeviceDescriptor,
    running: bool,
    handler: Option<InputHandler>,
    instance: Option<ComponentInstance>,
    context: CallbackContext<RenderContext>,
}

#[cfg(feature = "render")]
impl CoreAudioOutputDevice {
    fn new(desc: AudioDeviceDescriptor) -> Self {
        Self {
            desc,
            running: false,
            handler: None,
            instance: None,
            context: CallbackContext::default(),
        }
    }
}

#[cfg(feature = "render")]
impl Device for CoreAudioOutputDevice {
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
        let instance = create_output_unit()?;
        let channels = self.desc.channels;
        let sample_rate = self.desc.current_sample_rate();

        let enable: u32 = 1;
        check(instance.set_property(kAudioOutputUnitProperty_EnableIO, kAudioUnitScope_Output, OUTPUT_ELEMENT, &enable), "AudioUnitSetProperty")?;
        let disable: u32 = 0;
        check(instance.set_property(kAudioOutputUnitProperty_EnableIO, kAudioUnitScope_Input, INPUT_ELEMENT, &disable), "AudioUnitSetProperty")?;
        check(
            instance.set_property(kAudioOutputUnitProperty_CurrentDevice, kAudioUnitScope_Global, OUTPUT_ELEMENT, &self.desc.device_id),
            "AudioUnitSetProperty",
        )?;

        let asbd = make_asbd(sample_rate as f64, channels as u32);
        check(instance.set_property(kAudioUnitProperty_StreamFormat, kAudioUnitScope_Input, OUTPUT_ELEMENT, &asbd), "AudioUnitSetProperty")?;

        let context = CallbackContext::new(RenderContext {
            active: AtomicBool::new(true),
            handler,
            channels,
            sample_rate,
            pool: FramePool::<Frame<'static>>::new(),
        });

        let callback = AURenderCallbackStruct {
            inputProc: Some(render_callback),
            inputProcRefCon: context.as_ref_con(),
        };
        check(instance.set_property(kAudioUnitProperty_SetRenderCallback, kAudioUnitScope_Input, OUTPUT_ELEMENT, &callback), "AudioUnitSetProperty")?;

        if let Err(err) = check(instance.initialize(), "AudioUnitInitialize") {
            context.deactivate();
            clear_render_callback(&instance);
            return Err(err);
        }
        if let Err(err) = check(instance.start_output_unit(), "AudioOutputUnitStart") {
            context.deactivate();
            clear_render_callback(&instance);
            let _ = instance.uninitialize();
            return Err(err);
        }

        self.context = context;
        self.instance = Some(instance);
        self.running = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if !self.running {
            return Err(media_core::error::Error::NotRunning(self.desc.info.name.clone().into()));
        }

        self.context.deactivate();
        if let Some(instance) = self.instance.take() {
            clear_render_callback(&instance);
            let _ = instance.stop_output_unit();
            let _ = instance.uninitialize();
        }

        self.context.clear();
        self.running = false;
        Ok(())
    }

    fn configure(&mut self, options: &Variant) -> Result<()> {
        configure_descriptor(&mut self.desc, options, self.running)
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
impl RenderHandler for CoreAudioOutputDevice {
    fn set_input_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(&mut Frame) -> Result<()> + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(handler));
        Ok(())
    }
}

#[cfg(feature = "render")]
impl Drop for CoreAudioOutputDevice {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
