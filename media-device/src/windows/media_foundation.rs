use std::{
    mem::MaybeUninit,
    num::NonZeroU32,
    ptr::null_mut,
    slice::from_raw_parts,
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc, Condvar, Mutex, Weak,
    },
    time::Duration,
};

use media_base::{
    error::Error,
    frame::Frame,
    none_param_error, not_found_error,
    time::NSEC_PER_MSEC,
    variant::Variant,
    video::{ColorRange, CompressionFormat, Origin, PixelFormat, VideoFormat, VideoFrameDescriptor},
    Result,
};
use windows::{
    core::{implement, AsImpl, Interface, GUID, PWSTR},
    Win32::{
        Media::MediaFoundation::{
            IMF2DBuffer, IMF2DBuffer2, IMFActivate, IMFAttributes, IMFMediaBuffer, IMFMediaEvent, IMFMediaSource, IMFMediaType, IMFSample,
            IMFSourceReader, IMFSourceReaderCallback, IMFSourceReaderCallback_Impl, MF2DBuffer_LockFlags_Read, MFCreateAttributes,
            MFCreateSourceReaderFromMediaSource, MFEnumDeviceSources, MFGetStrideForBitmapInfoHeader, MFMediaType_Video, MFNominalRange,
            MFNominalRange_Normal, MFNominalRange_Wide, MFShutdown, MFStartup, MFVideoFormat_ARGB32, MFVideoFormat_I420, MFVideoFormat_MJPG,
            MFVideoFormat_NV12, MFVideoFormat_RGB24, MFVideoFormat_UYVY, MFVideoFormat_YUY2, MFVideoFormat_YV12, MFSTARTUP_LITE,
            MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE,
            MF_MT_SUBTYPE, MF_MT_VIDEO_NOMINAL_RANGE, MF_READWRITE_DISABLE_CONVERTERS, MF_SOURCE_READER_ASYNC_CALLBACK,
            MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_VERSION,
        },
        System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE},
    },
};

use crate::{camera::CameraFormat, Device, DeviceEvent, DeviceEventHandler, DeviceInformation, DeviceManager, OutputDevice, OutputHandler};

pub struct MediaFoundationDeviceManager {
    devices: Option<Vec<MediaFoundationDevice>>,
    handler: Option<DeviceEventHandler>,
}

impl DeviceManager for MediaFoundationDeviceManager {
    type DeviceType = MediaFoundationDevice;

    fn init() -> Result<Self>
    where
        Self: Sized,
    {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok().map_err(|err| Error::InitializationFailed(err.message()))?;
            MFStartup(MF_VERSION, MFSTARTUP_LITE).map_err(|err| Error::InitializationFailed(err.message()))?;
        }
        Ok(Self {
            devices: None,
            handler: None,
        })
    }

    fn uninit(&mut self) {
        unsafe {
            MFShutdown().ok();
            CoUninitialize();
        }
    }

    fn list(&self) -> Vec<&Self::DeviceType> {
        self.devices.as_ref().map(|devices| devices.iter().collect()).unwrap_or_default()
    }

    fn index(&self, index: usize) -> Option<&Self::DeviceType> {
        self.devices.as_ref().and_then(|devices| devices.get(index))
    }

    fn index_mut(&mut self, index: usize) -> Option<&mut Self::DeviceType> {
        self.devices.as_mut().and_then(|devices| devices.get_mut(index))
    }

    fn lookup(&self, id: &str) -> Option<&Self::DeviceType> {
        self.devices.as_ref().and_then(|devices| devices.iter().find(|device| device.info.id == id))
    }

    fn lookup_mut(&mut self, id: &str) -> Option<&mut Self::DeviceType> {
        self.devices.as_mut().and_then(|devices| devices.iter_mut().find(|device| device.info.id == id))
    }

    fn refresh(&mut self) -> Result<()> {
        let device_sources = Self::get_device_sources()?;
        let mut devices = Vec::with_capacity(device_sources.len());

        for (index, activate) in device_sources.iter().enumerate() {
            let dev_info = DeviceInformation::from_source_activate(activate)?;
            devices.push(MediaFoundationDevice::new(dev_info, index)?);
        }

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

impl Default for MediaFoundationDeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaFoundationDeviceManager {
    pub fn new() -> Self {
        Self {
            devices: None,
            handler: None,
        }
    }

    fn get_device_sources() -> Result<Vec<IMFActivate>> {
        let attributes: IMFAttributes = unsafe {
            let mut attributes: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes, 1).map_err(|err| Error::CreationFailed(err.message()))?;
            attributes.ok_or_else(|| none_param_error!(attributes))?
        };

        unsafe {
            attributes
                .SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID)
                .map_err(|err| Error::SetFailed(err.message()))?;
        }

        let mut source_activate_ptr: MaybeUninit<*mut Option<IMFActivate>> = MaybeUninit::uninit();
        let mut source_activate_count: u32 = 0;
        unsafe {
            MFEnumDeviceSources(&attributes, source_activate_ptr.as_mut_ptr(), &mut source_activate_count)
                .map_err(|err| Error::Failed(err.message()))?;
        }

        let mut device_sources = vec![];

        if source_activate_count > 0 {
            unsafe { from_raw_parts(source_activate_ptr.assume_init(), source_activate_count as usize) }.iter().for_each(|ptr| {
                if let Some(activate) = ptr {
                    device_sources.push(activate.clone());
                }
            });
        };

        Ok(device_sources)
    }
}

impl DeviceInformation {
    fn from_source_activate(activate: &IMFActivate) -> Result<Self> {
        let mut symbolic_link_ptr = PWSTR(null_mut());
        let mut symbolic_link_len = 0;
        unsafe {
            activate
                .GetAllocatedString(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, &mut symbolic_link_ptr, &mut symbolic_link_len)
                .map_err(|err| Error::Failed(err.message()))?;
        }
        if symbolic_link_ptr.is_null() {
            return Err(none_param_error!(symbolic_link_ptr));
        }
        let id = unsafe {
            let symbolic_link = symbolic_link_ptr.to_string().map_err(|err| Error::Failed(err.to_string()));
            CoTaskMemFree(Some(symbolic_link_ptr.as_ptr() as _));
            symbolic_link?
        };

        let mut friendly_name_ptr = PWSTR(null_mut());
        let mut friendly_name_len = 0;
        unsafe {
            activate
                .GetAllocatedString(&MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, &mut friendly_name_ptr, &mut friendly_name_len)
                .map_err(|err| Error::Failed(err.message()))?;
        }
        if friendly_name_ptr.is_null() {
            return Err(none_param_error!(friendly_name_ptr));
        }
        let name = unsafe {
            let name = friendly_name_ptr.to_string().map_err(|err| Error::Failed(err.to_string()));
            CoTaskMemFree(Some(friendly_name_ptr.as_ptr() as _));
            name?
        };

        Ok(Self {
            id,
            name,
        })
    }
}

struct BufferLockGuard<'a> {
    buffer: &'a IMFMediaBuffer,
    buffer_2d: Option<IMF2DBuffer>,
    data: &'a [u8],
    stride: i32,
}

impl<'a> BufferLockGuard<'a> {
    fn new(buffer: &'a IMFMediaBuffer, height: NonZeroU32) -> windows_core::Result<Self> {
        let buffer_2d = buffer.cast::<IMF2DBuffer>();
        let mut buffer_ptr = null_mut::<u8>();
        let mut buffer_stride = 0;
        let mut buffer_length = 0;

        let mut result = match &buffer_2d {
            Ok(buffer_2d_ref) => {
                let mut scanline_ptr = null_mut::<u8>();
                let mut result = match buffer.cast::<IMF2DBuffer2>() {
                    Ok(buffer_2d_2) => unsafe {
                        buffer_2d_2.Lock2DSize(MF2DBuffer_LockFlags_Read, &mut scanline_ptr, &mut buffer_stride, &mut buffer_ptr, &mut buffer_length)
                    },
                    Err(err) => Err(err),
                };

                if result.is_err() {
                    unsafe {
                        result = buffer_2d_ref.Lock2D(&mut scanline_ptr, &mut buffer_stride);
                        if result.is_ok() {
                            if buffer_stride < 0 {
                                buffer_ptr = scanline_ptr.offset((height.get() - 1) as isize * buffer_stride as isize);
                            } else {
                                buffer_ptr = scanline_ptr;
                            }

                            match buffer.GetCurrentLength() {
                                Ok(length) => buffer_length = length,
                                Err(err) => {
                                    result = Err(err);
                                    buffer_2d_ref.Unlock2D().ok();
                                }
                            }
                        }
                    };
                }

                result
            }
            Err(err) => Err(err.clone()),
        };

        let buffer_2d = if result.is_err() {
            result = unsafe { buffer.Lock(&mut buffer_ptr, None, Some(&mut buffer_length)) };
            None
        } else {
            buffer_2d.ok()
        };

        result.map(|_| {
            let data = unsafe { from_raw_parts(buffer_ptr, buffer_length as usize) };
            Self {
                buffer,
                buffer_2d,
                data,
                stride: buffer_stride,
            }
        })
    }
}

impl Drop for BufferLockGuard<'_> {
    fn drop(&mut self) {
        if let Some(buffer_2d) = &self.buffer_2d {
            unsafe {
                buffer_2d.Unlock2D().ok();
            }
        } else {
            unsafe {
                self.buffer.Unlock().ok();
            }
        }
    }
}

#[implement(IMFSourceReaderCallback)]
struct SourceReaderCallback {
    info: DeviceInformation,
    current_format: Option<CameraFormat>,
    stride_cache: Option<i32>,
    handler: OutputHandler,
    source_reader: Weak<Mutex<IMFSourceReader>>,
    running: AtomicBool,
    signal: Option<Arc<(Mutex<bool>, Condvar)>>,
}

impl SourceReaderCallback {
    pub fn new(info: DeviceInformation, handler: OutputHandler) -> Self {
        Self {
            info,
            current_format: None,
            stride_cache: None,
            handler,
            source_reader: Weak::new(),
            running: AtomicBool::new(false),
            signal: None,
        }
    }

    pub fn get_default_stride(&self) -> i32 {
        self.stride_cache.unwrap_or(0)
    }

    pub fn set_default_stride(&mut self, stride: i32) {
        self.stride_cache = Some(stride);
    }

    pub fn set_source_reader(&mut self, source_reader: &Arc<Mutex<IMFSourceReader>>) {
        self.source_reader = Arc::downgrade(source_reader);
    }

    pub fn set_current_format(&mut self, current_format: CameraFormat) {
        self.current_format = Some(current_format);
    }

    pub fn set_running(&self, running: bool) {
        self.running.store(running, SeqCst);
    }

    pub fn set_signal(&mut self, signal: Option<Arc<(Mutex<bool>, Condvar)>>) {
        self.signal = signal;
    }
}

impl IMFSourceReaderCallback_Impl for SourceReaderCallback_Impl {
    fn OnReadSample(
        &self,
        hrstatus: windows_core::HRESULT,
        _dwstreamindex: u32,
        _dwstreamflags: u32,
        lltimestamp: i64,
        psample: windows_core::Ref<'_, IMFSample>,
    ) -> windows_core::Result<()> {
        if hrstatus.is_err() || !self.running.load(SeqCst) {
            return Ok(());
        }

        let current_format = match &self.current_format {
            Some(format) => format,
            None => return Ok(()),
        };

        let width = match NonZeroU32::new(current_format.width) {
            Some(width) => width,
            None => return Ok(()),
        };
        let height = match NonZeroU32::new(current_format.height) {
            Some(height) => height,
            None => return Ok(()),
        };
        let pixel_format = match current_format.format {
            VideoFormat::Pixel(format) => format,
            _ => return Ok(()),
        };

        let buffer = psample.as_ref().and_then(|sample| unsafe { sample.ConvertToContiguousBuffer().ok() });

        if let Some(buffer) = buffer {
            if let Ok(locked_buffer) = BufferLockGuard::new(&buffer, height) {
                let stride = if locked_buffer.stride == 0 {
                    self.get_default_stride()
                } else {
                    locked_buffer.stride
                };

                let (stride, origin) = if stride >= 0 {
                    (stride as u32, Origin::TopDown)
                } else {
                    (-stride as u32, Origin::BottomUp)
                };

                let mut desc = VideoFrameDescriptor::new(pixel_format, width, height);
                desc.color_range = current_format.color_range;
                desc.origin = origin;

                let video_frame = if stride != 0 {
                    Frame::video_creator().create_from_aligned_buffer_with_descriptor(desc, NonZeroU32::new(stride).unwrap(), locked_buffer.data)
                } else {
                    Frame::video_creator().create_from_buffer_with_descriptor(desc, locked_buffer.data)
                };

                if let Ok(mut video_frame) = video_frame {
                    video_frame.source = Some(self.info.id.clone());
                    video_frame.pts = Some(lltimestamp * 100 / NSEC_PER_MSEC as i64); // lltimestamp is in 100ns units
                    let handler = self.handler.as_ref();
                    handler(video_frame).ok();
                }
            }
        };

        let source_reader = self.source_reader.upgrade();
        if let Some(source_reader) = source_reader {
            unsafe {
                source_reader.lock().unwrap().ReadSample(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0, None, None, None, None).ok();
            }
        };

        Ok(())
    }

    fn OnFlush(&self, _dwstreamindex: u32) -> windows_core::Result<()> {
        if let Some(signal) = &self.signal {
            let (lock, condvar) = &**signal;
            let mut flushed = lock.lock().unwrap();
            *flushed = true;
            condvar.notify_one();
        }
        Ok(())
    }

    fn OnEvent(&self, _dwstreamindex: u32, _pevent: windows_core::Ref<'_, IMFMediaEvent>) -> windows_core::Result<()> {
        Ok(())
    }
}

fn from_mf_video_format(subtype: GUID) -> Option<VideoFormat> {
    #[allow(non_snake_case, non_upper_case_globals)]
    match subtype {
        MFVideoFormat_I420 => Some(VideoFormat::Pixel(PixelFormat::I420)),
        MFVideoFormat_YUY2 => Some(VideoFormat::Pixel(PixelFormat::YUYV)),
        MFVideoFormat_UYVY => Some(VideoFormat::Pixel(PixelFormat::UYVY)),
        MFVideoFormat_ARGB32 => Some(VideoFormat::Pixel(PixelFormat::ARGB32)),
        MFVideoFormat_RGB24 => Some(VideoFormat::Pixel(PixelFormat::RGB24)),
        MFVideoFormat_MJPG => Some(VideoFormat::Compression(CompressionFormat::MJPEG)),
        MFVideoFormat_NV12 => Some(VideoFormat::Pixel(PixelFormat::NV12)),
        MFVideoFormat_YV12 => Some(VideoFormat::Pixel(PixelFormat::YV12)),
        _ => None,
    }
}

fn get_radio(media_type: &IMFMediaType, key: &GUID) -> Result<f32> {
    let (numerator, denominator) = match unsafe { media_type.GetUINT64(key) } {
        Ok(value) => {
            let numerator = (value >> 32) as u32;
            let denominator = value as u32;
            (numerator, denominator)
        }
        Err(err) => return Err(Error::GetFailed(err.message())),
    };

    Ok(numerator as f32 / denominator as f32)
}

fn from_mf_media_type(media_type: &IMFMediaType) -> Option<CameraFormat> {
    if let Ok(major_type) = unsafe { media_type.GetGUID(&MF_MT_MAJOR_TYPE) } {
        if major_type != MFMediaType_Video {
            return None;
        }
    } else {
        return None;
    }

    let subtype = match unsafe { media_type.GetGUID(&MF_MT_SUBTYPE) } {
        Ok(subtype) => subtype,
        Err(_) => return None,
    };

    let format = from_mf_video_format(subtype)?;

    let color_range = match unsafe { media_type.GetUINT32(&MF_MT_VIDEO_NOMINAL_RANGE) } {
        #[allow(non_upper_case_globals)]
        Ok(range) => match MFNominalRange(range as i32) {
            MFNominalRange_Normal => ColorRange::Full,
            MFNominalRange_Wide => ColorRange::Video,
            _ => ColorRange::Unspecified,
        },
        Err(_) => ColorRange::Unspecified,
    };

    let (width, height) = match unsafe { media_type.GetUINT64(&MF_MT_FRAME_SIZE) } {
        Ok(value) => {
            let width = (value >> 32) as u32;
            let height = value as u32;
            (width, height)
        }
        Err(_) => return None,
    };

    let frame_rate = match get_radio(media_type, &MF_MT_FRAME_RATE) {
        Ok(frame_rate) => frame_rate,
        Err(_) => return None,
    };

    Some(CameraFormat {
        format,
        color_range,
        width,
        height,
        frame_rates: vec![frame_rate],
    })
}

fn get_formats(source_reader: &IMFSourceReader) -> Result<Vec<CameraFormat>> {
    let mut video_formats = vec![];
    let mut index = 0;

    while let Ok(media_type) = unsafe { source_reader.GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, index) } {
        if let Some(camera_format) = from_mf_media_type(&media_type) {
            video_formats.push(camera_format);
        }
        index += 1;
    }

    Ok(video_formats)
}

fn get_current_format(source_reader: &IMFSourceReader) -> Result<CameraFormat> {
    let media_type = unsafe { source_reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32) };
    let media_type = media_type.map_err(|err| Error::GetFailed(err.message()))?;
    let camera_format = from_mf_media_type(&media_type).ok_or(Error::Unsupported(stringify!(media_type).to_string()))?;
    Ok(camera_format)
}

fn get_default_stride_with_width(source_reader: &IMFSourceReader, width: u32) -> Option<i32> {
    let media_type = unsafe { source_reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32) }.ok()?;
    let stride = unsafe { media_type.GetUINT32(&MF_MT_DEFAULT_STRIDE) }.ok().map(|s| s as i32);

    if stride.is_some() {
        return stride;
    }

    let sub_type = unsafe { media_type.GetGUID(&MF_MT_SUBTYPE) }.ok()?;
    let stride = unsafe { MFGetStrideForBitmapInfoHeader(sub_type.data1, width).ok() };

    if let Some(stride) = stride {
        unsafe { media_type.SetUINT32(&MF_MT_DEFAULT_STRIDE, stride as u32) }.ok();
    }

    stride
}

fn start_internal(source_reader: &IMFSourceReader, callback: &mut SourceReaderCallback, camera_format: &CameraFormat) {
    callback.set_current_format(camera_format.clone());
    if let Some(stride) = get_default_stride_with_width(source_reader, camera_format.width) {
        callback.set_default_stride(stride);
    }

    unsafe {
        source_reader.SetStreamSelection(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, true).ok();
        source_reader.ReadSample(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0, None, None, None, None).ok();
    }

    callback.set_running(true);
}

const TIMEOUT_SECONDS: u64 = 1;

fn stop_internal(source_reader: &IMFSourceReader, callback: &mut SourceReaderCallback) -> Result<()> {
    callback.set_running(false);
    let signal = Arc::new((Mutex::new(false), Condvar::new()));
    callback.set_signal(Some(signal.clone()));

    let wait = unsafe {
        match source_reader.Flush(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32) {
            Ok(()) => true,
            Err(err) => return Err(Error::StopFailed(err.message())),
        }
    };

    if wait {
        let (lock, condvar) = &*signal;
        let flushed = lock.lock().map_err(|err| Error::StopFailed(err.to_string()))?;
        if !*flushed {
            condvar.wait_timeout(flushed, Duration::from_secs(TIMEOUT_SECONDS)).ok();
        }
    }

    Ok(())
}

const SIMILAR_FORMAT_DIFF: f32 = 1.0;
const DIFFERENT_FORMAT_DIFF: f32 = 2.0;

fn match_supported_format(
    source_reader: &IMFSourceReader,
    width: Option<u32>,
    height: Option<u32>,
    video_format: Option<VideoFormat>,
    frame_rate: Option<f32>,
) -> Option<IMFMediaType> {
    let mut matched_media_type: Option<IMFMediaType> = None;
    let mut min_diff = f32::MAX;
    let mut index = 0;

    while let Ok(media_type) = unsafe { source_reader.GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, index) } {
        if let Some(camera_format) = from_mf_media_type(&media_type) {
            let resolution_diff = match (width, height) {
                (Some(width), Some(height)) => {
                    (camera_format.width as f32 - width as f32).abs() + (camera_format.height as f32 - height as f32).abs()
                }
                _ => 0.0,
            };

            let frame_rate_diff = match frame_rate {
                Some(frame_rate) => (camera_format.frame_rates[0] - frame_rate).abs(),
                None => 0.0,
            };

            let format_diff = match video_format {
                Some(video_format) => {
                    if camera_format.format == video_format {
                        0.0
                    } else if camera_format.format.is_yuv() && video_format.is_yuv() {
                        SIMILAR_FORMAT_DIFF
                    } else {
                        DIFFERENT_FORMAT_DIFF
                    }
                }
                None => 0.0,
            };

            let diff = resolution_diff + frame_rate_diff + format_diff;

            if diff < min_diff {
                min_diff = diff;
                matched_media_type = Some(media_type.clone());
            }
        }
        index += 1;
    }

    matched_media_type
}

pub struct MediaFoundationDevice {
    info: DeviceInformation,
    index: usize,
    running: bool,
    formats: Option<Vec<CameraFormat>>,
    current_format: Option<CameraFormat>,
    handler: Option<OutputHandler>,
    source_reader: Option<(Arc<Mutex<IMFSourceReader>>, IMFSourceReaderCallback)>,
}

impl Device for MediaFoundationDevice {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn id(&self) -> &str {
        &self.info.id
    }

    fn start(&mut self) -> Result<()> {
        let (running, current_format, formats) = {
            let camera_format = self.current_format.clone();
            let (source_reader, callback) = self.get_source_reader()?;
            let source_reader = source_reader.lock().map_err(|err| Error::StartFailed(err.to_string()))?;

            if let Some(camera_format) = camera_format {
                let media_type = match_supported_format(
                    &source_reader,
                    Some(camera_format.width),
                    Some(camera_format.height),
                    Some(camera_format.format),
                    Some(camera_format.frame_rates[0]),
                );
                if let Some(media_type) = media_type {
                    unsafe { source_reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &media_type).ok() };
                }
            }

            let current_format = get_current_format(&source_reader)?;
            let formats = get_formats(&source_reader)?;

            start_internal(&source_reader, callback, &current_format);

            (true, current_format, formats)
        };

        self.running = running;
        self.current_format = Some(current_format);
        self.formats = Some(formats);

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.running = false;

        {
            let (source_reader, callback) = self.get_source_reader()?;
            let source_reader = source_reader.lock().map_err(|err| Error::StopFailed(err.to_string()))?;

            stop_internal(&source_reader, callback)?;
        }

        self.source_reader = None;

        Ok(())
    }

    fn configure(&mut self, options: Variant) -> Result<()> {
        let width = options["width"].get_uint32();
        let height = options["height"].get_uint32();
        let video_format = options["format"].get_uint32();
        let frame_rate = options["frame-rate"].get_float();

        let video_format = match video_format {
            Some(video_format) => VideoFormat::try_from(video_format).ok(),
            None => None,
        };

        let camera_format = if self.running {
            let (source_reader, callback) = self.get_source_reader()?;
            let source_reader = source_reader.lock().map_err(|err| Error::SetFailed(err.to_string()))?;
            let media_type = match_supported_format(&source_reader, width, height, video_format, frame_rate);

            if let Some(media_type) = media_type {
                match from_mf_media_type(&media_type) {
                    Some(camera_format) => {
                        stop_internal(&source_reader, callback).ok();
                        unsafe { source_reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &media_type).ok() };
                        callback.set_current_format(camera_format.clone());
                        start_internal(&source_reader, callback, &camera_format);
                        Some(camera_format)
                    }
                    None => return Err(Error::Unsupported("format".to_string())),
                }
            } else {
                return Err(Error::Unsupported("format".to_string()));
            }
        } else {
            Some(CameraFormat {
                format: video_format.unwrap_or(VideoFormat::Pixel(PixelFormat::NV12)),
                color_range: ColorRange::default(),
                width: width.unwrap_or_default(),
                height: height.unwrap_or_default(),
                frame_rates: vec![frame_rate.unwrap_or_default()],
            })
        };

        self.current_format = camera_format;

        Ok(())
    }

    fn control(&mut self, _action: Variant) -> Result<()> {
        Err(Error::NotImplemented)
    }

    fn running(&self) -> bool {
        self.running
    }

    fn formats(&self) -> Result<Variant> {
        if !self.running {
            return Err(Error::NotRunning(self.info.name.clone()));
        }

        let video_formats = self.formats.as_ref().ok_or(Error::NotFound("video formats".to_string()))?;
        let mut formats = Variant::new_array();
        for video_format in video_formats {
            let mut format = Variant::new_dict();
            format["format"] = (Into::<u32>::into(video_format.format)).into();
            format["width"] = video_format.width.into();
            format["height"] = video_format.height.into();
            format["frame-rates"] = video_format.frame_rates.iter().map(|frame_rate| Variant::from(*frame_rate)).collect();
            formats.array_add(format);
        }

        Ok(formats)
    }
}

impl OutputDevice for MediaFoundationDevice {
    fn set_output_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(Frame) -> Result<()> + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(handler));
        Ok(())
    }
}

impl MediaFoundationDevice {
    fn new(dev_info: DeviceInformation, index: usize) -> Result<Self> {
        Ok(Self {
            info: dev_info,
            index,
            running: false,
            current_format: None,
            formats: None,
            handler: None,
            source_reader: None,
        })
    }

    fn get_source_reader(&mut self) -> Result<(&Mutex<IMFSourceReader>, &mut SourceReaderCallback)> {
        if self.source_reader.is_none() {
            let device_sources = MediaFoundationDeviceManager::get_device_sources()?;
            let activate = device_sources.get(self.index).ok_or(not_found_error!(self.index))?;
            let media_source = unsafe { activate.ActivateObject::<IMFMediaSource>().map_err(|err| Error::OpenFailed(err.message()))? };

            let attributes: IMFAttributes = unsafe {
                let mut attributes: Option<IMFAttributes> = None;
                MFCreateAttributes(&mut attributes, 1).map_err(|err| Error::CreationFailed(err.message()))?;
                attributes.ok_or_else(|| none_param_error!(attributes))?
            };

            unsafe {
                attributes.SetUINT32(&MF_READWRITE_DISABLE_CONVERTERS, true as u32).map_err(|err| Error::SetFailed(err.message()))?;
            }

            let handler = self.handler.as_ref().ok_or(none_param_error!(output_handler))?;
            let callback: IMFSourceReaderCallback = SourceReaderCallback::new(self.info.clone(), handler.clone()).into();

            unsafe {
                attributes.SetUnknown(&MF_SOURCE_READER_ASYNC_CALLBACK, &callback).map_err(|err| Error::SetFailed(err.message()))?;
            }

            let source_reader =
                unsafe { MFCreateSourceReaderFromMediaSource(&media_source, &attributes).map_err(|err| Error::CreationFailed(err.message()))? };

            #[allow(clippy::arc_with_non_send_sync)]
            let source_reader = Arc::new(Mutex::new(source_reader));

            unsafe {
                callback.as_impl_ptr().as_mut().set_source_reader(&source_reader);
            }

            self.source_reader = Some((source_reader.clone(), callback));
        }
        let (source_reader, reader_callback) = self.source_reader.as_ref().unwrap();
        let reader_callback = unsafe { reader_callback.as_impl_ptr().as_mut() };
        Ok((source_reader.as_ref(), reader_callback))
    }
}
