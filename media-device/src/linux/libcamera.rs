//! Linux support using libcamera - https://libcamera.org/
//!
//! libcamera cameras are different to USB web cameras in that they expose a min/max/default frame rate
//! range, instead of a fixed set of frame rates.  This is usually because the SoC to which a MIPI
//! camera is connected to controls the rate at which it captures images, and the min/max are based
//! on the image sensor's capabilities.
//!
//! See `variable-frame-durations` in the `formats` response.
//!
//! Original Author: Dominic Clifton <me@dominiclifton.name>
use std::{io, sync::Arc, thread, time::{SystemTime, UNIX_EPOCH}};
use std::fmt::Debug;
use std::num::NonZeroU32;
use std::slice::{Iter, IterMut};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;
use libcamera::{
    camera::ActiveCamera,
    camera_manager::CameraManager,
    framebuffer_allocator::{FrameBufferAllocator},
    request::ReuseFlag,
    stream::StreamRole,
};
use libcamera::camera::{Camera, CameraConfiguration};
use libcamera::control_value::ControlValue;
use libcamera::controls::ControlId;
use libcamera::framebuffer::AsFrameBuffer;
use libcamera::framebuffer_allocator::FrameBuffer;
use libcamera::framebuffer_map::MemoryMappedFrameBuffer;
use libcamera::properties::Model;
use log::{debug, error, info, warn};
use media_core::{
    error::Error,
    frame::Frame,
    variant::Variant,
    video::{ColorRange, CompressionFormat, Origin, PixelFormat, VideoFormat, VideoFrameDescriptor},
    Result,
};
use media_core::data::{DataFormat, DataFrameDescriptor};
use crate::{Device, DeviceEvent, DeviceManager, OutputDevice};
use crate::device::DeviceEventHandler;

/// Linux backend device manager
pub struct LibcameraDeviceManager {
    mgr: CameraManager,
    devices: Vec<LibcameraDevice>,
    handler: Option<DeviceEventHandler>,
}

impl DeviceManager for LibcameraDeviceManager {
    type DeviceType = LibcameraDevice;
    type Iter<'a> = Iter<'a, LibcameraDevice> where Self: 'a;

    type IterMut<'a> = IterMut<'a, LibcameraDevice> where Self: 'a;

    fn init() -> Result<Self>
    where
        Self: Sized
    {
        let mgr = CameraManager::new()
            .map_err(|e| Error::InitializationFailed(format!("{:?}", e)))?;

        let devices = Vec::new();

        Ok(Self {
            mgr,
            devices,
            handler: None,
        })
    }

    fn deinit(&mut self) {
        self.devices.clear();
    }

    fn index(&self, index: usize) -> Option<&Self::DeviceType> {
        self.devices.get(index)
    }

    fn index_mut(&mut self, index: usize) -> Option<&mut Self::DeviceType> {
        self.devices.get_mut(index)
    }

    fn lookup(&self, id: &str) -> Option<&Self::DeviceType> {
        self.devices.iter().find(|d| d.id() == id)
    }

    fn lookup_mut(&mut self, id: &str) -> Option<&mut Self::DeviceType> {
        self.devices.iter_mut().find(|d| d.id() == id)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.devices.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.devices.iter_mut()
    }

    fn refresh(&mut self) -> Result<()> {

        self.devices.clear();

        let cameras = self.mgr.cameras();
        for i in 0..cameras.len() {
            if let Some(cam) = cameras.get(i) {
                let cam: Camera<'static> = unsafe { std::mem::transmute(cam) };

                let dev = LibcameraDevice::new(cam);
                self.devices.push(dev);
            }
        }

        Ok(())
    }

    fn set_change_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(&DeviceEvent) + Send + Sync + 'static
    {
        self.handler = Some(Box::new(handler));
        Ok(())
    }
}

// TODO rename to LibcameraCameraDevice
pub struct LibcameraDevice {
    id: String,
    name: String,
    running: bool,
    worker_handle: Option<LinuxCameraWorkerHandle>,
    cmd_tx: mpsc::Sender<CameraCmd>,
    cmd_response_rx: mpsc::Receiver<CameraCmdResponse>,
}


impl LibcameraDevice {
    pub fn new(
        camera: Camera<'static>
    ) -> Self {
        let id = camera.id().to_string();
        let name = camera.properties().get::<Model>().unwrap_or(Model("N/A".to_string())).to_string();

        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<CameraCmd>();
        let (cmd_response_tx, cmd_response_rx) = std::sync::mpsc::channel::<CameraCmdResponse>();

        let config = camera.generate_configuration(&[StreamRole::VideoRecording]).unwrap();
        let alloc = FrameBufferAllocator::new(&camera);

        let worker = LinuxCameraWorker {
            pending_camera: camera,
            camera: None,
            config,
            alloc,
            output_handler: None,
            cmd_rx,
            cmd_response_tx,
            config_applied: false,
        };

        let worker_join_handle = thread::spawn(move || { LinuxCameraWorker::run(worker)});

        Self {
            id,
            name,
            running: false,
            worker_handle: Some(LinuxCameraWorkerHandle { join: worker_join_handle }),
            cmd_tx,
            cmd_response_rx,
        }
    }
}

impl Device for LibcameraDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn start(&mut self) -> Result<()> {
        self.cmd_tx.send(CameraCmd::Start)
            .map_err(|e| Error::StartFailed(format!("Failed to send start command: {:?}", e)))?;
        match self.cmd_response_rx.recv()
            .map_err(|e| Error::StartFailed(format!("No response to command: {:?}", e)))?
        {
            CameraCmdResponse::Ok => {
                self.running = true;
                Ok(())
            },
            CameraCmdResponse::DeviceError(e) => Err(e),
            _ => unreachable!(),
        }
    }

    fn stop(&mut self) -> Result<()> {
        self.cmd_tx.send(CameraCmd::Stop)
            .map_err(|e| Error::CloseFailed(format!("Failed to send close command: {:?}", e)))?;
        match self.cmd_response_rx.recv()
            .map_err(|e| Error::StartFailed(format!("No response to command: {:?}", e)))?
        {
            CameraCmdResponse::Ok => {
                self.running = false;
                Ok(())
            },
            CameraCmdResponse::DeviceError(e) => Err(e),
            _ => unreachable!(),
        }
    }

    fn configure(&mut self, options: &Variant) -> Result<()> {
        self.cmd_tx.send(CameraCmd::Configure(options.clone()))
            .map_err(|e| Error::SetFailed(format!("Failed to send configure command: {:?}", e)))?;
        match self.cmd_response_rx.recv()
            .map_err(|e| Error::SetFailed(format!("No response to command: {:?}", e)))?
        {
            CameraCmdResponse::Ok => Ok(()),
            CameraCmdResponse::DeviceError(e) => Err(e),
            _ => unreachable!(),
        }
    }

    fn control(&mut self, action: &Variant) -> Result<()> {
        Err(Error::Unsupported("Control not supported".into()))
    }

    fn running(&self) -> bool {
        self.running
    }

    fn formats(&self) -> Result<Variant> {
        self.cmd_tx.send(CameraCmd::GetFormats)
            .map_err(|e| Error::GetFailed(format!("Failed to send configure command: {:?}", e)))?;
        match self.cmd_response_rx.recv()
            .map_err(|e| Error::GetFailed(format!("No response to command: {:?}", e)))?
        {
            CameraCmdResponse::DeviceError(e) => Err(e),
            CameraCmdResponse::Formats(v) => Ok(v),
            _ => unreachable!(),
        }
    }
}

impl Drop for LibcameraDevice {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(CameraCmd::Shutdown);

        let handle = self.worker_handle.take().unwrap();

        let _ = handle.join.join();
    }
}

impl<'a> OutputDevice for LibcameraDevice {
    fn set_output_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(Frame) -> Result<()> + Send + Sync + 'static,
    {
        self.cmd_tx.send(CameraCmd::SetOutputHandler(Arc::new(handler)))
            .map_err(|e| Error::SetFailed(format!("Failed to send set output handler command: {:?}", e)))?;
        match self.cmd_response_rx.recv()
            .map_err(|e| Error::SetFailed(format!("No response to command: {:?}", e)))?
        {
            CameraCmdResponse::Ok => Ok(()),
            CameraCmdResponse::DeviceError(e) => Err(e),
            _ => unreachable!(),
        }
    }
}

enum CameraCmd
{
    Start,
    Stop,
    Shutdown,
    SetOutputHandler(OutputHandlerArc),
    Configure(Variant),
    GetFormats,
}

enum CameraCmdResponse {
    Ok,
    DeviceError(Error),
    Formats(Variant),
}

type OutputHanderFn = dyn Fn(Frame) -> Result<()> + Send + Sync;
type OutputHandlerArc = Arc<OutputHanderFn>;

struct LinuxCameraWorker {
    pending_camera: Camera<'static>,
    camera: Option<ActiveCamera<'static>>,
    alloc: FrameBufferAllocator,
    output_handler: Option<OutputHandlerArc>,
    config: CameraConfiguration,
    cmd_rx: mpsc::Receiver<CameraCmd>,
    cmd_response_tx: mpsc::Sender<CameraCmdResponse>,

    config_applied: bool,
}

struct LinuxCameraWorkerHandle {
    join: JoinHandle<()>,
}


// Safety: the `ActiveCamera` is only used by the worker thread
unsafe impl Send for LinuxCameraWorker {}

impl LinuxCameraWorker {
    fn run(mut instance: LinuxCameraWorker) {

        let mut req_rx = None;
        let mut running = false;
        let mut shutdown = false;

        let mut desired_frame_interval = None;

        while !shutdown {
            // process all outstanding commands
            while let Ok(cmd) = instance.cmd_rx.try_recv() {
                if shutdown {
                    break;
                }

                match cmd {
                    CameraCmd::GetFormats => {
                        let config = instance.pending_camera.generate_configuration(&[StreamRole::ViewFinder]).unwrap();
                        let view_finder_config = config.get(0).unwrap();
                        let camera_formats = view_finder_config.formats();

                        let controls: &libcamera::control::ControlInfoMap = instance.pending_camera.controls();

                        let result = controls.find(ControlId::FrameDurationLimits.into()).map(|frame_duration_limits|{
                            debug!("Frame Duration. Min: {:?}, Max: {:?}, Default: {:?}",
                                frame_duration_limits.min(),
                                frame_duration_limits.max(),
                                frame_duration_limits.def(),
                            );

                            // there really must be a better way of doing this...
                            let (min, max, default) = match (frame_duration_limits.min(), frame_duration_limits.max(), frame_duration_limits.def()) {
                                (ControlValue::Int64(min), ControlValue::Int64(max), ControlValue::Int64(default)) => (min[0], max[0], default[0]),
                                _ => {
                                    return Err(Error::GetFailed("Unexpected types for frame duration limits".into()))
                                }
                            };
                            let (fps_min, fps_max, fps_default) = (
                                1_000_000_f64 / max as f64, // fps min = max interval
                                1_000_000_f64 / min as f64, // fps max = min interval
                                1_000_000_f64 / default as f64,
                            );

                            // now, since there is no ACTUAL fps (like you get with a USB camera) but a range instead, we have to invent some...
                            let mut frame_rates = vec![fps_min, fps_max, fps_default];
                            frame_rates.dedup();

                            let mut variable_frame_durations = Variant::new_dict();
                            variable_frame_durations["min"] = min.into();
                            variable_frame_durations["max"] = max.into();
                            variable_frame_durations["default"] = default.into();

                            Ok((frame_rates, variable_frame_durations))
                        }).unwrap_or({
                            // libcamera does not expose the frame duration limits control for V4L2 USB cameras. See https://gitlab.freedesktop.org/camera/libcamera/-/issues/296
                            // FIXME update this when libcamera exposes a better API for enumerating frame rate limits on a per format and resolution basis.
                            warn!("camera does not expose 'frame duration limits' control; using hard-coded values which can result in an invalid configuration");
                            let frame_rates = vec![7.5, 10.0, 15.0, 20.0, 25.0, 30.0, 60.0, 100.0, 120.0];

                            let mut variable_frame_durations = Variant::new_dict();
                            variable_frame_durations["min"] = (*frame_rates.first().unwrap()).into();
                            variable_frame_durations["max"] = (*frame_rates.first().unwrap()).into();
                            variable_frame_durations["default"] = (*frame_rates.last().unwrap()).into();

                            Ok((frame_rates, variable_frame_durations))
                        });

                        let (frame_rates, variable_frame_durations) = match result {
                            Ok((frame_rates, variable_frame_durations)) => (frame_rates, variable_frame_durations),
                            Err(e) => {
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(e));
                                continue
                            }
                        };

                        let mut formats = Variant::new_array();
                        for pixel_format in camera_formats.pixel_formats().into_iter() {

                            let video_format = match pixel_format.fourcc() {
                                FOURCC_YUYV => VideoFormat::Pixel(PixelFormat::YUYV),
                                FOURCC_YU12 => VideoFormat::Pixel(PixelFormat::YV12),
                                FOURCC_NV12 => VideoFormat::Pixel(PixelFormat::NV12),
                                FOURCC_MJPG => VideoFormat::Compression(CompressionFormat::MJPEG),
                                _ => {
                                    // TODO support more formats (Contribution/PR's welcomed)
                                    continue
                                }
                            };

                            let mut sizes = camera_formats.sizes(pixel_format).into_iter()
                                .collect::<Vec<_>>();
                            sizes.sort_by(|a,b|a.width.cmp(&b.width).then(a.height.cmp(&b.height)));

                            for size in sizes {
                                let mut format = Variant::new_dict();
                                format["format"] = (Into::<u32>::into(video_format)).into();
                                format["width"] = size.width.into();
                                format["height"] = size.height.into();

                                format["frame-rates"] = frame_rates.iter().map(|frame_rate| Variant::from(frame_rate.clone())).collect();

                                format["variable-frame-durations"] = variable_frame_durations.clone().into();

                                formats.array_add(format);
                            }
                        }

                        let _ = instance.cmd_response_tx.send(CameraCmdResponse::Formats(formats));
                    }
                    CameraCmd::Start => {
                        if running {
                            let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::StartFailed("Already running".into())));
                            continue
                        }

                        let active_camera = match instance.pending_camera.acquire() {
                            Ok(camera) => camera,
                            Err(e) => {
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::StartFailed(format!("Acqurie failed. error: {:?}", e).into())));
                                continue
                            }
                        };

                        let active_camera: ActiveCamera<'static> = unsafe { std::mem::transmute(active_camera) };

                        instance.camera = Some(active_camera);

                        let source = instance.pending_camera.id().to_string();

                        if !instance.config_applied {
                            if let Err(e) = Self::validate_and_configure(&mut instance) {
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::StartFailed(format!("Configuration failed. error: {:?}", e).into())));
                                continue
                            }
                        }

                        let camera = instance.camera.as_mut().unwrap();
                        let handler = instance.output_handler.clone();
                        let stream_cfg = instance.config
                            .get_mut(0).unwrap();

                        let stream = stream_cfg.stream().unwrap();

                        let size = stream_cfg.get_size();
                        let stride: u32 = stream_cfg.get_stride();
                        // libcamera only returns positve strides
                        let origin = Origin::TopDown;

                        let format: libcamera::pixel_format::PixelFormat = stream_cfg.get_pixel_format();

                        // TODO support more formats (Contribution/PR's welcomed)
                        let video_format = match format.fourcc() {
                            FOURCC_NV12 => VideoFormat::Pixel(PixelFormat::NV12),
                            FOURCC_YU12 => VideoFormat::Pixel(PixelFormat::YV12),
                            FOURCC_YUYV => VideoFormat::Pixel(PixelFormat::YUYV),
                            FOURCC_MJPG => VideoFormat::Compression(CompressionFormat::MJPEG),
                            _ => {
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::StartFailed(format!("Unsupported format. {:?}", format).into())));
                                continue;
                            },
                        };

                        let (vfd, dfd) = match &video_format {
                            VideoFormat::Pixel(pixel_format) => {
                                let mut desc = VideoFrameDescriptor::new(
                                    *pixel_format,
                                    unsafe { NonZeroU32::new_unchecked(size.width) },
                                    unsafe { NonZeroU32::new_unchecked(size.height) },
                                );
                                desc.origin = origin;
                                (Some(desc), None)
                            },
                            VideoFormat::Compression(_compression_format) => {
                                (None, Some(DataFrameDescriptor::new(DataFormat::Variant)))
                            }
                        };

                        let (req_tx, new_req_rx) = mpsc::channel::<libcamera::request::Request>();
                        req_rx = Some(new_req_rx);

                        if let Some(handler) = handler {
                            // Set callback for completed requests
                            camera.on_request_completed({
                                let vfd = vfd.clone();
                                let dfd = dfd.clone();
                                let source = source.clone();

                                move |req| {
                                    if let Some(framebuffer) = req.buffer::<MemoryMappedFrameBuffer<FrameBuffer>>(&stream) {
                                        if let Some(plane) = framebuffer.data().get(0) {
                                            let bytes_used = framebuffer.planes().get(0).unwrap().len() as usize;
                                            let frame_data = plane[..bytes_used].to_vec();

                                            let timestamp = SystemTime::now()
                                                .duration_since(UNIX_EPOCH)
                                                .unwrap()
                                                .as_micros() as u64;

                                            let frame = match (&vfd, &dfd) {
                                                (Some(vfd), None) => {
                                                    if stride != 0 {
                                                        Frame::video_creator().create_from_aligned_buffer_with_descriptor(vfd.clone(), NonZeroU32::new(stride).unwrap(), frame_data)
                                                    } else {
                                                        Frame::video_creator().create_from_buffer_with_descriptor(vfd.clone(), frame_data)
                                                    }
                                                }
                                                (None, Some(dfd)) => {
                                                    let mut frame = Frame::data_creator().create_with_descriptor(dfd.clone());
                                                    if let Ok(frame) = frame.as_mut() {
                                                        let mut variant = Variant::new_dict();

                                                        variant.dict_set("buffer", Variant::Buffer(frame_data));
                                                        variant.dict_set("format", Variant::UInt32(video_format.into()));

                                                        *frame.data_mut().unwrap() = variant;
                                                    }

                                                    frame
                                                }
                                                _ => unreachable!()
                                            };

                                            if let Ok(mut frame) = frame {
                                                frame.source = Some(source.clone());

                                                // TODO duration support
                                                //video_frame.timestamp = timestamp;

                                                let _ = handler(frame);
                                            }

                                        }
                                    }

                                    // Reuse and requeue
                                    req_tx.send(req).unwrap();
                                }
                            });
                        }

                        let buffers = instance.alloc
                            .alloc(&stream)
                            .unwrap()
                            .into_iter()
                            .map(|b| MemoryMappedFrameBuffer::new(b).unwrap())
                            .collect::<Vec<_>>();

                        let reqs = buffers
                            .into_iter()
                            .enumerate()
                            .map(|(i, buf)| {
                                let mut req = camera.create_request(Some(i as u64)).unwrap();
                                req.add_buffer(&stream, buf).unwrap();
                                req
                            })
                            .collect::<Vec<_>>();

                        if let Err(e) = camera.start(None) {
                            let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::StartFailed(format!("{e:?}"))));
                            continue;
                        };

                        // Enqueue all requests to the camera
                        for req in reqs {
                            camera.queue_request(req).unwrap();
                        }

                        running = true;

                        let _ = instance.cmd_response_tx.send(CameraCmdResponse::Ok);
                    }
                    CameraCmd::Stop => {
                        let Some(mut camera) = instance.camera.take() else {
                            let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::NotRunning("Not running".to_string())));
                            continue;
                        };

                        if let Err(e) = camera.stop() {
                            let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::StopFailed(format!("{e:?}"))));
                        }

                        // active camera dropped at end of scope (`ActiveCamera::Drop` impl releases it)

                        running = false;
                        let _ = instance.cmd_response_tx.send(CameraCmdResponse::Ok);
                    }
                    CameraCmd::Shutdown => {
                        shutdown = true;
                        break
                    }
                    CameraCmd::SetOutputHandler(handler) => {
                        instance.output_handler = Some(handler);
                        let _ = instance.cmd_response_tx.send(CameraCmdResponse::Ok);
                    }
                    CameraCmd::Configure(options) => {
                        let mut stream_config = instance.config
                            .get_mut(0).unwrap();

                        // TODO match the options against a valid format for this device, since the supplied values may be wrong or result in an invalid combination.
                        let desired_size = if let (Some(width), Some(height)) = (options["width"].get_uint32(), options["height"].get_uint32()) {
                            Some(libcamera::geometry::Size { width, height })
                        } else {
                            None
                        };

                        if let Some(desired_size) = desired_size {
                            println!("desired size: {:?}", desired_size);
                            stream_config.set_size(desired_size);
                        }

                        let video_format = options["format"].get_uint32();

                        let video_format = match video_format {
                            Some(video_format) => VideoFormat::try_from(video_format).ok(),
                            None => None,
                        };

                        println!("video format: {:?}", video_format);

                        match video_format {
                            Some(VideoFormat::Pixel(PixelFormat::NV12)) => stream_config.set_pixel_format(PIXEL_FORMAT_NV12),
                            Some(VideoFormat::Pixel(PixelFormat::YUYV)) => stream_config.set_pixel_format(PIXEL_FORMAT_YUYV),
                            // YV12 == YU12 ?
                            Some(VideoFormat::Pixel(PixelFormat::YV12)) => stream_config.set_pixel_format(PIXEL_FORMAT_YU12),
                            Some(VideoFormat::Compression(CompressionFormat::MJPEG)) => stream_config.set_pixel_format(PIXEL_FORMAT_MJPG),
                            Some(_) => {
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::SetFailed(format!("Unsupported format. '{:?}'", video_format))));
                            },
                            None => {
                                // XXX temporarily use YUYV as default format
                                stream_config.set_pixel_format(PIXEL_FORMAT_YUYV)
                            }
                        };

                        let frame_rate = options["frame-rate"].get_float();
                        if let Some(frame_rate) = frame_rate {
                            desired_frame_interval = Some(Duration::from_secs_f32(1.0 / frame_rate));
                        } else {
                            // TODO get the default frame rate for the format and use that, for now default to 30FPS
                            desired_frame_interval = Some(Duration::from_secs_f32(1.0 / 30.0));
                        }

                        // drop the reference to avoid borrow checker issues
                        drop(stream_config);

                        // we can't actually apply the configuration until the camera is acquired

                        if let Some(desired_size) = desired_size {
                            let stream_config = instance.config
                                .get_mut(0).unwrap();
                            let actual_size = stream_config.get_size();
                            assert_eq!((desired_size.width, desired_size.height), (actual_size.width, actual_size.height));
                        }
                        if instance.cmd_response_tx.send(CameraCmdResponse::Ok).is_err() {
                            break
                        }
                    }
                }
            }
            if let Some(req_rx) = req_rx.as_mut() {
                if let Ok(mut req) = req_rx.recv_timeout(Duration::from_millis(250)) {
                    if let Some(ref camera) = instance.camera {
                        req.reuse(ReuseFlag::REUSE_BUFFERS);

                        if let Some(desired_frame_interval) = desired_frame_interval {
                            // TODO queue the request based on the desired frame rate / frame duration.
                            // TODO pace the requests at a constant increment from when the camera was started
                            //      currently this approach will cause capture timestamps to jitter
                            // TODO ensure the method of sleeping has a high enough resolution for the desired duration
                            thread::sleep(desired_frame_interval);
                        } else {
                            // queue immediately
                        }

                        if let Err(e) = camera.queue_request(req) {
                            error!("queue_request failed: {:?}", e);
                            break;
                        }
                    } else {
                        drop(req);
                    }
                }
            }
        }
    }

    fn validate_and_configure(instance: &mut LinuxCameraWorker) -> io::Result<()> {
        instance.config.validate();
        let result = instance.camera.as_mut().unwrap().configure(&mut instance.config);

        info!("camera: {}, config: {:?}", instance.pending_camera.id(), instance.config);

        instance.config_applied = true;
        result
    }
}


const PIXEL_FORMAT_NV12: libcamera::pixel_format::PixelFormat = libcamera::pixel_format::PixelFormat::new(FOURCC_NV12, 0);
const PIXEL_FORMAT_YUYV: libcamera::pixel_format::PixelFormat = libcamera::pixel_format::PixelFormat::new(FOURCC_YUYV, 0);
const PIXEL_FORMAT_YU12: libcamera::pixel_format::PixelFormat = libcamera::pixel_format::PixelFormat::new(FOURCC_YU12, 0);
const PIXEL_FORMAT_MJPG: libcamera::pixel_format::PixelFormat = libcamera::pixel_format::PixelFormat::new(FOURCC_MJPG, 0);
const FOURCC_NV12: u32 = u32::from_le_bytes([b'N', b'V', b'1', b'2']);
const FOURCC_YUYV: u32 = u32::from_le_bytes([b'Y', b'U', b'Y', b'V']);
const FOURCC_YU12: u32 = u32::from_le_bytes([b'Y', b'U', b'1', b'2']);
const FOURCC_MJPG: u32 = u32::from_le_bytes([b'M', b'J', b'P', b'G']);


#[cfg(test)]
mod tests {
    use crate::backend::libcamera::FOURCC_YU12;

    #[test]
    pub fn fourcc() {
        assert_eq!(FOURCC_YU12, 0x32315559);
    }
}
