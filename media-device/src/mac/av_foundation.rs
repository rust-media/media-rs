use std::sync::Arc;

#[cfg(target_os = "macos")]
use av_foundation::capture_device::AVCaptureDeviceTypeExternalUnknown;
use av_foundation::{
    capture_device::{
        AVCaptureDevice, AVCaptureDeviceDiscoverySession, AVCaptureDeviceFormat, AVCaptureDevicePositionUnspecified,
        AVCaptureDeviceTypeBuiltInWideAngleCamera, AVCaptureDeviceTypeExternal,
    },
    capture_input::AVCaptureDeviceInput,
    capture_output_base::AVCaptureOutput,
    capture_session::{AVCaptureConnection, AVCaptureSession},
    capture_video_data_output::{AVCaptureVideoDataOutput, AVCaptureVideoDataOutputSampleBufferDelegate},
    media_format::AVMediaTypeVideo,
};
use cfg_if::cfg_if;
use core_foundation::{base::TCFType, string::CFString};
use core_media::{
    format_description::{CMVideoCodecType, CMVideoFormatDescription},
    sample_buffer::{CMSampleBuffer, CMSampleBufferRef},
    time::CMTime,
};
use core_video::pixel_buffer::{
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange, kCVPixelFormatType_420YpCbCr8Planar, kCVPixelFormatType_422YpCbCr8,
    kCVPixelFormatType_422YpCbCr8_yuvs, CVPixelBuffer, CVPixelBufferKeys,
};
use dispatch2::Queue;
use media_core::{
    error::Error,
    frame::Frame,
    none_param_error, not_found_error,
    time::MSEC_PER_SEC,
    variant::Variant,
    video::{ColorRange, PixelFormat, VideoFormat},
    Result,
};
use objc2::{
    declare_class, extern_methods, msg_send_id, mutability,
    rc::{Allocated, Id, Retained},
    runtime::ProtocolObject,
    ClassType, DeclaredClass,
};
use objc2_foundation::{NSArray, NSMutableArray, NSMutableDictionary, NSNumber, NSObject, NSObjectProtocol, NSString};
use os_ver::if_greater_than;

use crate::{camera::CameraFormat, Device, DeviceEvent, DeviceEventHandler, DeviceInformation, DeviceManager, OutputDevice, OutputHandler};

pub struct AVFoundationCaptureDeviceManager {
    devices: Option<Vec<AVFoundationCaptureDevice>>,
    handler: Option<DeviceEventHandler>,
}

impl DeviceManager for AVFoundationCaptureDeviceManager {
    type DeviceType = AVFoundationCaptureDevice;

    fn init() -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self {
            devices: None,
            handler: None,
        })
    }

    fn uninit(&mut self) {}

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
        let devices = Self::list_devices()?;

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

impl AVFoundationCaptureDeviceManager {
    cfg_if! {
        if #[cfg(target_os = "macos")] {
            fn get_av_devices() -> Id<NSArray<AVCaptureDevice>> {
                unsafe {
                    if_greater_than! {(10, 15) => {
                        let mut device_types = NSMutableArray::new();

                        device_types.addObject(AVCaptureDeviceTypeBuiltInWideAngleCamera);
                        if_greater_than! {(14) => {
                            device_types.addObject(AVCaptureDeviceTypeExternal);
                        } else {
                            device_types.addObject(AVCaptureDeviceTypeExternalUnknown);
                        }};
                        device_types.addObject(AVCaptureDeviceTypeExternalUnknown);

                        AVCaptureDeviceDiscoverySession::discovery_session_with_device_types(
                            &device_types,
                            AVMediaTypeVideo,
                            AVCaptureDevicePositionUnspecified,
                        ).devices()
                    } else {
                        AVCaptureDevice::devices_with_media_type(AVMediaTypeVideo)
                    }}
                }
            }
        } else if #[cfg(target_os = "ios")] {
            fn get_av_devices() -> Id<NSArray<AVCaptureDevice>> {
                unsafe {
                    if_greater_than! {(10) => {
                        let mut device_types = NSMutableArray::new();

                        device_types.addObject(AVCaptureDeviceTypeBuiltInWideAngleCamera);

                        AVCaptureDeviceDiscoverySession::discovery_session_with_device_types(
                            &device_types,
                            AVMediaTypeVideo,
                            AVCaptureDevicePositionUnspecified,
                        ).devices()
                    } else {
                        AVCaptureDevice::devices_with_media_type(AVMediaTypeVideo)
                    }}
                }
            }
        } else {
            fn get_av_devices() -> Id<NSArray<AVCaptureDevice>> {
                AVCaptureDevice::devices_with_media_type(AVMediaTypeVideo)
            }
        }
    }

    fn list_devices() -> Result<Vec<AVFoundationCaptureDevice>> {
        let av_capture_devices = Self::get_av_devices();

        let mut devices = Vec::with_capacity(av_capture_devices.count() as _);

        for device in av_capture_devices.iter() {
            let dev_info = DeviceInformation::from_av_capture_device(device);
            devices.push(AVFoundationCaptureDevice::new(dev_info)?);
        }

        Ok(devices)
    }
}

impl DeviceInformation {
    fn from_av_capture_device(device: &AVCaptureDevice) -> Self {
        Self {
            name: device.localized_name().to_string(),
            id: device.unique_id().to_string(),
        }
    }
}

pub struct OutputDelegateIvars {
    info: Option<DeviceInformation>,
    handler: Option<OutputHandler>,
}

impl OutputDelegateIvars {
    fn new() -> Self {
        Self {
            info: None,
            handler: None,
        }
    }

    fn set_infomation(&mut self, info: DeviceInformation) {
        self.info = Some(info);
    }

    fn set_handler(&mut self, handler: OutputHandler) {
        self.handler = Some(handler);
    }
}

declare_class!(
    struct OutputDelegate;

    unsafe impl ClassType for OutputDelegate {
        type Super = NSObject;
        type Mutability = mutability::Mutable;
        const NAME: &'static str = "OutputSampleBufferDelegate";
    }

    impl DeclaredClass for OutputDelegate {
        type Ivars = OutputDelegateIvars;
    }

    unsafe impl NSObjectProtocol for OutputDelegate {}

    unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for OutputDelegate {
        #[method(captureOutput:didOutputSampleBuffer:fromConnection:)]
        unsafe fn capture_output_did_output_sample_buffer(
            &self,
            _capture_output: &AVCaptureOutput,
            sample_buffer: CMSampleBufferRef,
            _connection: &AVCaptureConnection,
        ) {
            let sample_buffer = CMSampleBuffer::wrap_under_get_rule(sample_buffer);
            let video_frame = sample_buffer
                .get_image_buffer()
                .and_then(|image_buffer| image_buffer.downcast::<CVPixelBuffer>())
                .and_then(|pixel_buffer| Frame::video_creator().create_from_pixel_buffer(&pixel_buffer).ok());

            if let Some(mut video_frame) = video_frame {
                if let Some(handler) = self.ivars().handler.as_ref() {
                    if let Some(info) = self.ivars().info.as_ref() {
                        video_frame.source = Some(info.id.clone());
                    }
                    video_frame.pts = Some((sample_buffer.get_presentation_time_stamp().get_seconds() * MSEC_PER_SEC as f64) as i64);
                    handler(video_frame).ok();
                }
            }
        }
    }

    unsafe impl OutputDelegate {
        #[method_id(init)]
        fn init(this: Allocated<Self>) -> Option<Id<Self>> {
            let this = this.set_ivars(OutputDelegateIvars::new());
            unsafe { msg_send_id![super(this), init] }
        }
    }
);

extern_methods!(
    unsafe impl OutputDelegate {
        #[method_id(new)]
        pub fn new() -> Id<Self>;
    }
);

fn from_cm_codec_type(codec_type: CMVideoCodecType) -> Option<(VideoFormat, ColorRange)> {
    #[allow(non_upper_case_globals)]
    match codec_type {
        kCVPixelFormatType_420YpCbCr8Planar => Some((VideoFormat::Pixel(PixelFormat::I420), ColorRange::Video)),
        kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange => Some((VideoFormat::Pixel(PixelFormat::NV12), ColorRange::Video)),
        kCVPixelFormatType_422YpCbCr8_yuvs => Some((VideoFormat::Pixel(PixelFormat::YUYV), ColorRange::Video)),
        kCVPixelFormatType_422YpCbCr8 => Some((VideoFormat::Pixel(PixelFormat::UYVY), ColorRange::Video)),
        _ => None,
    }
}

fn into_cv_pixel_format(format: VideoFormat) -> u32 {
    match format {
        VideoFormat::Pixel(PixelFormat::I420) => kCVPixelFormatType_420YpCbCr8Planar,
        VideoFormat::Pixel(PixelFormat::NV12) => kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
        VideoFormat::Pixel(PixelFormat::YUYV) => kCVPixelFormatType_422YpCbCr8_yuvs,
        VideoFormat::Pixel(PixelFormat::UYVY) => kCVPixelFormatType_422YpCbCr8,
        _ => kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange, // default to NV12
    }
}

fn from_av_capture_device_format(format: &AVCaptureDeviceFormat) -> Option<CameraFormat> {
    if let Some(desc) = format.format_description().downcast_into::<CMVideoFormatDescription>() {
        let dimensions = desc.get_dimensions();
        match from_cm_codec_type(desc.get_codec_type()) {
            Some((video_format, color_range)) => {
                let frame_rate_ranges = format.video_supported_frame_rate_ranges();
                let frame_rates = frame_rate_ranges.iter().map(|range| range.max_frame_rate() as f32).collect();

                Some(CameraFormat {
                    format: video_format,
                    color_range,
                    width: dimensions.width as u32,
                    height: dimensions.height as u32,
                    frame_rates,
                })
            }
            None => None,
        }
    } else {
        None
    }
}

fn get_formats(device: &AVCaptureDevice) -> Vec<CameraFormat> {
    device.formats().iter().filter_map(from_av_capture_device_format).collect()
}

const SIMILAR_FORMAT_DIFF: f32 = 1.0;
const DIFFERENT_FORMAT_DIFF: f32 = 2.0;

fn select_supported_format(
    device: &AVCaptureDevice,
    width: Option<u32>,
    height: Option<u32>,
    video_format: Option<VideoFormat>,
    frame_rate: Option<f32>,
) -> Option<CameraFormat> {
    let formats = device.formats();
    let mut min_diff = f32::MAX;
    let mut matched_format = None;
    let mut matched_frame_rate = None;

    for format in formats.iter() {
        if let Some(camera_format) = from_av_capture_device_format(format) {
            let resolution_diff = match (width, height) {
                (Some(width), Some(height)) => {
                    (camera_format.width as f32 - width as f32).abs() + (camera_format.height as f32 - height as f32).abs()
                }
                _ => 0.0,
            };

            let frame_rate_diff = match &frame_rate {
                Some(frame_rate) => {
                    let mut min_frame_rate_diff = f32::MAX;
                    for supported_frame_rate in camera_format.frame_rates.iter() {
                        let diff = (supported_frame_rate - frame_rate).abs();
                        if (supported_frame_rate >= frame_rate) && diff < min_frame_rate_diff {
                            min_frame_rate_diff = diff;
                            matched_frame_rate = Some(*supported_frame_rate);
                        }
                    }
                    min_frame_rate_diff
                }
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
                matched_format = Some(format);
            }
        }
    }

    match matched_format {
        Some(format) => {
            if device.lock_for_configuration().unwrap_or_default().is_true() {
                device.set_active_format(format);
                if let Some(frame_rate) = matched_frame_rate {
                    let frame_duration: CMTime = CMTime::make(1, frame_rate as i32);
                    device.set_active_video_min_frame_duration(frame_duration);
                    device.set_active_video_max_frame_duration(frame_duration);
                }
                device.unlock_for_configuration();
            }

            from_av_capture_device_format(format)
        }
        None => None,
    }
}

fn set_output_settings(output: &AVCaptureVideoDataOutput, width: u32, height: u32, video_format: VideoFormat) {
    let mut settings = NSMutableDictionary::<NSString, NSObject>::new();
    let pixel_format = into_cv_pixel_format(video_format);

    settings.insert_id(
        cf_string_to_ns_string(&CVPixelBufferKeys::PixelFormatType.into()),
        Retained::into_super(Retained::into_super(NSNumber::numberWithUnsignedInt(pixel_format))),
    );
    settings.insert_id(
        cf_string_to_ns_string(&CVPixelBufferKeys::Width.into()),
        Retained::into_super(Retained::into_super(NSNumber::numberWithUnsignedInt(width))),
    );
    settings.insert_id(
        cf_string_to_ns_string(&CVPixelBufferKeys::Height.into()),
        Retained::into_super(Retained::into_super(NSNumber::numberWithUnsignedInt(height))),
    );

    output.set_video_settings(&settings);
}

fn cf_string_to_ns_string(str: &CFString) -> &NSString {
    let ptr: *const NSString = str.as_concrete_TypeRef().cast();
    unsafe { ptr.as_ref().unwrap() }
}

pub struct AVFoundationCaptureDevice {
    info: DeviceInformation,
    running: bool,
    formats: Option<Vec<CameraFormat>>,
    current_format: Option<CameraFormat>,
    handler: Option<OutputHandler>,
    session: Option<Id<AVCaptureSession>>,
    device: Option<Id<AVCaptureDevice>>,
    input: Option<Id<AVCaptureDeviceInput>>,
    output: Option<Id<AVCaptureVideoDataOutput>>,
    delegate: Option<Id<OutputDelegate>>,
}

impl Device for AVFoundationCaptureDevice {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn id(&self) -> &str {
        &self.info.id
    }

    fn start(&mut self) -> Result<()> {
        let (running, formats) = {
            let session = AVCaptureSession::new();
            let id = NSString::from_str(self.info.id.as_str());
            let device = AVCaptureDevice::device_with_unique_id(&id).ok_or(not_found_error!(id))?;
            let output = AVCaptureVideoDataOutput::new();
            let input = AVCaptureDeviceInput::from_device(&device).map_err(|err| Error::Invalid(err.to_string()))?;
            let mut delegate = OutputDelegate::new();
            let queue = Queue::new("com.x-device.video-capture", dispatch2::QueueAttribute::Serial);
            let handler = self.handler.as_ref().ok_or_else(|| none_param_error!(handler))?;
            let ivars = delegate.ivars_mut();

            ivars.set_infomation(self.info.clone());
            ivars.set_handler(handler.clone());

            output.set_sample_buffer_delegate(ProtocolObject::from_ref(&*delegate), &queue);
            output.set_always_discards_late_video_frames(true);

            if session.can_add_input(&input) && session.can_add_output(&output) {
                session.add_input(&input);
                session.add_output(&output);
            } else {
                return Err(Error::Invalid("cannot add input or output".to_string()));
            }

            session.begin_configuration();
            #[cfg(target_os = "ios")]
            session.set_uses_application_audio_session(false);

            let formats = get_formats(&device);
            let current_format = self.current_format.clone();
            if let Some(current_format) = current_format {
                self.current_format = select_supported_format(
                    &device,
                    Some(current_format.width),
                    Some(current_format.height),
                    Some(current_format.format),
                    current_format.frame_rates.first().cloned(),
                );
            }

            if let Some(camera_format) = self.current_format.as_ref() {
                set_output_settings(&output, camera_format.width, camera_format.height, camera_format.format);
            }

            session.commit_configuration();
            session.start_running();

            self.session = Some(session);
            self.device = Some(device);
            self.input = Some(input);
            self.output = Some(output);
            self.delegate = Some(delegate);

            (true, formats)
        };

        self.running = running;
        self.formats = Some(formats);

        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if !self.running {
            return Err(Error::NotRunning(self.info.name.clone()));
        }

        self.running = false;

        let session = self.session.as_ref().ok_or_else(|| none_param_error!(session))?;

        if let Some(output) = self.output.as_ref() {
            session.remove_output(output);
        }

        session.stop_running();

        if let Some(input) = self.input.as_ref() {
            session.remove_input(input);
        }

        self.delegate = None;
        self.output = None;
        self.input = None;
        self.device = None;
        self.session = None;

        Ok(())
    }

    fn configure(&mut self, options: &Variant) -> Result<()> {
        let width = options["width"].get_uint32();
        let height = options["height"].get_uint32();
        let video_format = options["format"].get_uint32();
        let frame_rate = options["frame-rate"].get_float();

        let video_format = match video_format {
            Some(video_format) => VideoFormat::try_from(video_format).ok(),
            None => None,
        };

        let camera_format = if self.running {
            let session = self.session.as_ref().ok_or_else(|| none_param_error!(session))?;
            let device = self.device.as_ref().ok_or_else(|| none_param_error!(device))?;

            session.begin_configuration();
            let camera_format = select_supported_format(device, width, height, video_format, frame_rate);
            if let Some(camera_format) = &camera_format {
                set_output_settings(self.output.as_ref().unwrap(), camera_format.width, camera_format.height, camera_format.format);
            }
            session.commit_configuration();

            camera_format
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

    fn control(&mut self, _action: &Variant) -> Result<()> {
        Err(Error::NotImplemented)
    }

    fn running(&self) -> bool {
        self.running
    }

    fn formats(&self) -> Result<Variant> {
        if !self.running {
            return Err(Error::NotRunning(self.info.name.clone()));
        }

        let video_formats = self.formats.as_ref().ok_or(Error::NotFound(String::from("video formats")))?;
        let mut formats = Variant::new_array();
        for video_format in video_formats {
            let mut format = Variant::new_dict();
            format["format"] = video_format.format.to_string().into();
            format["color-range"] = (video_format.color_range as u32).into();
            format["width"] = video_format.width.into();
            format["height"] = video_format.height.into();
            format["frame-rates"] = video_format.frame_rates.iter().map(|frame_rate| Variant::from(*frame_rate)).collect();
            formats.array_add(format);
        }

        Ok(formats)
    }
}

impl OutputDevice for AVFoundationCaptureDevice {
    fn set_output_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(Frame) -> Result<()> + Send + Sync + 'static,
    {
        self.handler = Some(Arc::new(handler));
        Ok(())
    }
}

impl AVFoundationCaptureDevice {
    fn new(dev_info: DeviceInformation) -> Result<Self> {
        Ok(Self {
            info: dev_info,
            running: false,
            formats: None,
            current_format: None,
            handler: None,
            session: None,
            device: None,
            input: None,
            output: None,
            delegate: None,
        })
    }
}
