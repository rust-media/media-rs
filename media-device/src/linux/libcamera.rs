//! Linux support using libcamera - https://libcamera.org/
//!
//! libcamera cameras are different to USB web cameras in that they expose a min/max/default frame rate
//! range, instead of a fixed set of frame rates.  This is usually because the SoC to which a MIPI
//! camera is connected to controls the rate at which it captures images, and the min/max are based
//! on the image sensor's capabilities.
//!
//! See `variable-frame-durations` in the `formats` response.
//!
//! Safety:
//! This implementation spawns a thread that holds the libcamera manager, which spawns threads for
//! each camera.  The implementations of `Device` and `DeviceManager` communicate with their worker
//! threads via command+response channels.  The device manager from `libcamera` cannot be shared
//! between threads.  The libcamera camera pointers for each cameras are shareable to other threads,
//! but each camera is only accessed by one thread.
//!
//! Libcamera API: https://libcamera.org/api-html/classlibcamera_1_1CameraManager.html
//!
//! Original Author: Dominic Clifton <me@dominiclifton.name>
use std::{sync::Arc, thread};
use std::fmt::{Debug, Display, Formatter};
use std::num::NonZeroU32;
use std::sync::{mpsc, Mutex, MutexGuard};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use libcamera::{
    camera::ActiveCamera,
    camera_manager::CameraManager,
    framebuffer_allocator::{FrameBufferAllocator},
    request::ReuseFlag,
    stream::StreamRole,
};
use libcamera::camera::{Camera, CameraConfiguration, CameraConfigurationStatus};
use libcamera::control_value::ControlValue;
use libcamera::controls::ControlId;
use libcamera::framebuffer::AsFrameBuffer;
use libcamera::framebuffer_allocator::FrameBuffer;
use libcamera::framebuffer_map::MemoryMappedFrameBuffer;
use libcamera::properties::Model;
use libcamera::request::RequestStatus;
use log::{debug, error, info, trace, warn};
use media_core::{
    error::Error,
    frame::Frame,
    variant::Variant,
    video::{ColorRange, CompressionFormat, Origin, PixelFormat, VideoFormat, VideoFrameDescriptor},
    Result,
    data::{DataFormat, DataFrameDescriptor},
    time::NSEC_PER_MSEC,
};
use crate::{Device, DeviceEvent, DeviceEventHandler, DeviceManager, OutputDevice};

enum CameraManagerCmd {
    Initialize,
    Deinitialize,
    Refresh,
}

enum CameraManagerCmdResponse<> {
    Ok,
    Refreshed(RefreshResult),
    Error(Error),
}

struct RefreshResult {
    device_count: usize,
}

struct CameraManagerRequest {
    command: CameraManagerCmd,
    response_tx: mpsc::Sender<CameraManagerCmdResponse>,
}

fn camera_manager_main(command_rx: mpsc::Receiver<CameraManagerRequest>) {

    let mut camera_manager = None;
    let mut shutdown = false;
    while !shutdown {
        match command_rx.recv() {
            Ok(request) => {
                if camera_manager.is_none() {
                    match &request.command {
                        CameraManagerCmd::Initialize => {
                            let result = CameraManager::new();
                            match result {
                                Ok(new_camera_manager) => {
                                    camera_manager = Some(new_camera_manager);
                                    request.response_tx.send(CameraManagerCmdResponse::Ok).ok();
                                }
                                Err(e) => {
                                    let error = Error::InitializationFailed(format!("{:?}", e));
                                    request.response_tx.send(CameraManagerCmdResponse::Error(error)).ok();
                                }
                            }

                            continue
                        }
                        _ => {}
                    }
                }

                // all other commands require a camera manager to be initialized

                let Some(mgr) = camera_manager.as_mut() else {
                    let error = Error::InitializationFailed("CameraManager not initialized".into());
                    request.response_tx.send(CameraManagerCmdResponse::Error(error)).ok();
                    continue;
                };

                match &request.command {
                    CameraManagerCmd::Initialize => {
                        info!("Already initialized");
                        request.response_tx.send(CameraManagerCmdResponse::Ok).ok();
                    }
                    CameraManagerCmd::Deinitialize => {
                        info!("Camera manager deinitialize request received");

                        let remaining_instances = MANAGER_INSTANCE_COUNT.fetch_sub(1, Ordering::SeqCst);
                        if remaining_instances == 1 {  // 1 because we've already decremented
                            shutdown = true;
                        }

                        request.response_tx.send(CameraManagerCmdResponse::Ok).ok();
                    }
                    CameraManagerCmd::Refresh => {
                        info!("Camera manager refreshing devices");

                        let mut devices = CAMERA_DEVICES.lock().unwrap();

                        let cameras = mgr.cameras();

                        let mut ids: Vec<String> = vec![];

                        for i in 0..cameras.len() {
                            if let Some(cam) = cameras.get(i) {
                                let id = cam.id().to_string();

                                let exists = devices.iter().any(|dev| dev.id() == id);

                                if !exists {
                                    info!("Adding device: {}", id);

                                    // Safe, because get returns a shared pointer according to the libcamera docs.
                                    let cam: Camera<'static> = unsafe { std::mem::transmute(cam) };

                                    let dev = LibcameraDevice::new(cam);
                                    devices.push(dev);
                                }

                                ids.push(id);
                            }
                        }

                        // now remove any devices that are no longer present
                        devices.retain(|device| {
                            let keep = ids.contains(&device.id().to_string());
                            if !keep {
                                info!("Removing device: {}", device.id());
                            }
                            keep
                        });

                        let device_count = devices.len();

                        request.response_tx.send(CameraManagerCmdResponse::Refreshed(RefreshResult { device_count })).ok();
                    }
                }
            }
            Err(_) => {
                error!("Camera manager command channel closed, improper shutdown");
                // channel closed, shutdown
                shutdown = true;
            }
        }
    }

    // shut down the cameras
    {
        // using a scope to limit the scope of the lock on the devices

        let mut devices = CAMERA_DEVICES.lock().unwrap();
        devices.drain(..).for_each(|device| {
            drop(device);
        });

        assert!(devices.is_empty());
    }

    {
        // 'camera_manager' runs cleanup though it's 'Drop' impl
        let _ = camera_manager.take();
    }

    info!("Camera manager worker thread shutdown.");
}


struct LibcameraDeviceManagerWorker {
    handle: JoinHandle<()>,
    command_tx: mpsc::Sender<CameraManagerRequest>,
}

impl LibcameraDeviceManagerWorker {
    fn call(&self, command: CameraManagerCmd) -> std::result::Result<CameraManagerCmdResponse, CommandError> {
        let (response_tx, response_rx) = mpsc::channel::<CameraManagerCmdResponse>();

        self.command_tx.send(CameraManagerRequest {
            command,
            response_tx,
        })
            .map_err(|_e|CommandError::SendFailed)?;

        response_rx.recv()
            .map_err(|_e|CommandError::ReceiveFailed)
    }
}

static CAMERA_MANAGER: Mutex<Option<LibcameraDeviceManagerWorker>> = Mutex::new(None);
static CAMERA_DEVICES: Mutex<Vec<LibcameraDevice>> = Mutex::new(Vec::new());
static MANAGER_INSTANCE_COUNT: AtomicUsize = AtomicUsize::new(0);


/// Linux backend device manager.
///
/// Multiple instances of this can be created, when the last one is dropped the worker thread
/// will be shut down.
pub struct LibcameraDeviceManager {
    handler: Option<DeviceEventHandler>,
}

impl LibcameraDeviceManager {
    fn call(&self, command: CameraManagerCmd) -> std::result::Result<CameraManagerCmdResponse, CommandError> {

        let mut camera_manager = CAMERA_MANAGER.lock().unwrap();
        let Some(worker) = camera_manager.as_mut() else {
            // this can occur during shutdown with multiple threads.
            return Err(CommandError::NotReady);
        };

        worker.call(command)
    }
}

impl Drop for LibcameraDeviceManager {
    fn drop(&mut self) {
        let remaining_instances = MANAGER_INSTANCE_COUNT.load(Ordering::SeqCst);
        if remaining_instances == 0 {
            let mut camera_manager = CAMERA_MANAGER.lock().unwrap();
            if let Some(handle) = camera_manager.take() {
                info!("Joining worker thread (last instance dropped)");
                handle.handle.join().ok();
                info!("Camera manager shutdown.");
            }
        }
    }
}

#[derive(Debug)]
enum CommandError {
    NotReady,
    SendFailed,
    ReceiveFailed,
}

impl Display for CommandError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::NotReady => {f.write_str("Not ready")}
            CommandError::SendFailed => {f.write_str("Send failed")}
            CommandError::ReceiveFailed => {f.write_str("Receive failed")}
        }
    }
}

impl DeviceManager for LibcameraDeviceManager {
    type DeviceType = LibcameraDevice;
    type Iter<'a> = CameraDeviceIter<'a> where Self: 'a;

    type IterMut<'a> = CameraDeviceIterMut<'a> where Self: 'a;

    fn init() -> Result<Self>
    where
        Self: Sized
    {
        {
            let mut maybe_mgr = CAMERA_MANAGER.lock().unwrap();

            if maybe_mgr.is_none() {
                let (command_tx, command_rx) = mpsc::channel::<CameraManagerRequest>();

                let handle = thread::spawn(|| camera_manager_main(command_rx));
                let handle = LibcameraDeviceManagerWorker {
                    handle,
                    command_tx,
                };

                *maybe_mgr = Some(handle);
            }

            MANAGER_INSTANCE_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        let instance = Self {
            handler: None,
        };

        instance.call(CameraManagerCmd::Initialize)
            .map_err(|e|Error::InitializationFailed(format!("Failed to initialize camera manager: {:?}", e)))?;

        Ok(instance)
    }

    fn deinit(&mut self) {
        self.call(CameraManagerCmd::Deinitialize).ok();
    }

    fn index(&self, index: usize) -> Option<&Self::DeviceType> {
        let guard = CAMERA_DEVICES.lock().unwrap();
        let device = guard.get(index)?;
        // Safe because the data in static Mutex lives for the entire program
        Some(unsafe { std::mem::transmute(device) })
    }

    fn index_mut(&mut self, index: usize) -> Option<&mut Self::DeviceType> {
        let mut guard = CAMERA_DEVICES.lock().unwrap();
        let device = guard.get_mut(index)?;
        // Safe because the data in static Mutex lives for the entire program
        Some(unsafe { std::mem::transmute(device) })
    }

    fn lookup(&self, id: &str) -> Option<&Self::DeviceType> {
        let guard = CAMERA_DEVICES.lock().unwrap();
        let device = guard.iter().find(|d| d.id() == id)?;
        // Safe because the data in static Mutex lives for the entire program
        Some(unsafe { std::mem::transmute(device) })
    }

    fn lookup_mut(&mut self, id: &str) -> Option<&mut Self::DeviceType> {
        let mut guard = CAMERA_DEVICES.lock().unwrap();
        let device = guard.iter_mut().find(|d| d.id() == id)?;
        // Safe because the data in static Mutex lives for the entire program
        Some(unsafe { std::mem::transmute(device) })
    }

    fn iter(&self) -> Self::Iter<'_> {
        CameraDeviceIter::new()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        CameraDeviceIterMut::new()
    }

    fn refresh(&mut self) -> Result<()> {
        let result = self.call(CameraManagerCmd::Refresh)
            .map_err(|e|Error::Failed(e.to_string()))?;

        match result {
            CameraManagerCmdResponse::Refreshed(r) => {
                if let Some(handler) = &self.handler {
                    handler(&DeviceEvent::Refreshed(r.device_count));
                }
                Ok(())
            },
            CameraManagerCmdResponse::Error(e) => Err(Error::Failed(e.to_string())),
            _ => unreachable!(),
        }
    }

    fn set_change_handler<F>(&mut self, handler: F) -> Result<()>
    where
        F: Fn(&DeviceEvent) + Send + Sync + 'static
    {
        self.handler = Some(Box::new(handler));
        Ok(())
    }
}

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
        let worker = LinuxCameraWorker {
            pending_camera: camera,
            camera: None,
            config,
            alloc: None,
            output_handler: None,
            cmd_rx,
            cmd_response_tx,
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
        info!("Shutting down camera worker thread. id: {}", self.id);
        let _ = self.cmd_tx.send(CameraCmd::Shutdown);

        let handle = self.worker_handle.take().unwrap();

        let _ = handle.join.join();
        info!("Camera shut down. id: {}", self.id);
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
    alloc: Option<FrameBufferAllocator>,
    output_handler: Option<OutputHandlerArc>,
    config: CameraConfiguration,
    cmd_rx: mpsc::Receiver<CameraCmd>,
    cmd_response_tx: mpsc::Sender<CameraCmdResponse>,
}

struct LinuxCameraWorkerHandle {
    join: JoinHandle<()>,
}

/// Safety: the `Camera` instance is obtained via libcamera's `CameraManager::get` or `CameraManager::cameras`
/// methods, which return shared pointers, they are marked as-thread safe in the docs.
///
/// Reference:
/// https://libcamera.org/api-html/classlibcamera_1_1CameraManager.html#a3b20427687e9920b256625838bea8f9a
/// https://libcamera.org/api-html/classlibcamera_1_1CameraManager.html#a004d822ffc9ad72137711ce20aebb7cc
unsafe impl Send for LinuxCameraWorker {}
unsafe impl Sync for LinuxCameraWorker {}

impl LinuxCameraWorker {
    fn run(mut instance: LinuxCameraWorker) {

        let mut req_rx = None;
        let mut running = false;
        let mut shutdown = false;

        let mut desired_frame_interval = None;

        let mut next_frame_at = Instant::now();

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
                            warn!("camera does not expose 'frame duration limits' control; using hard-coded values which can result in an invalid configuration. id: {}", instance.pending_camera.id());
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
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::StartFailed(format!("Acquire failed. error: {:?}", e).into())));
                                continue
                            }
                        };

                        let active_camera: ActiveCamera<'static> = unsafe { std::mem::transmute(active_camera) };

                        instance.camera = Some(active_camera);

                        let source = instance.pending_camera.id().to_string();

                        info!("camera: {}, config: {:?}", instance.pending_camera.id(), instance.config);

                        let configuration_status = instance.config.validate();
                        match configuration_status {
                            CameraConfigurationStatus::Valid | CameraConfigurationStatus::Adjusted => {}
                            CameraConfigurationStatus::Invalid => {
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::StartFailed("Configuration invalid.".into())));
                                continue
                            }
                        }
                        if let Err(e) = instance.camera.as_mut().unwrap().configure(&mut instance.config) {
                            let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::StartFailed(format!("Configuration failed. error: {:?}", e).into())));
                            continue
                        }

                        let camera = instance.camera.as_mut().unwrap();
                        let handler = instance.output_handler.clone();

                        for i in 0..instance.config.len() {
                            let cfg = instance.config.get(i).unwrap();
                            debug!("Stream {}: {:?}, size={:?}, stride={}, frame_size={}",
                                     i, cfg.get_pixel_format(), cfg.get_size(), cfg.get_stride(), cfg.get_frame_size());
                        }

                        let stream_cfg = instance.config
                            .get_mut(0).unwrap();

                        let stream = stream_cfg.stream().unwrap();

                        let size = stream_cfg.get_size();
                        let stride: u32 = stream_cfg.get_stride();
                        // libcamera only returns positive strides
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
                                        let mut frame_data = Vec::new();

                                        for (plane_index, plane) in framebuffer.data().iter().enumerate() {
                                            let planes = framebuffer.planes();
                                            let plane_info = planes.get(plane_index).unwrap();
                                            let bytes_used = plane_info.len();
                                            trace!("bytes used for plane {}: {}", plane_index, bytes_used);
                                            frame_data.extend_from_slice(&plane[..bytes_used]);
                                        }

                                        if !frame_data.is_empty() {
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

                                                frame.duration = desired_frame_interval.map(|it: Duration|it.as_millis() as i64);

                                                // Get timestamp from metadata ControlList
                                                let metadata = req.metadata();
                                                if let Ok(sensor_timestamp) = metadata.get::<libcamera::controls::SensorTimestamp>() {
                                                    let sensor_timestamp: i64 = (*sensor_timestamp).into();
                                                    // frame.pts (presentation time stamp) is in milliseconds, sensor timestamp is nanoseconds
                                                    frame.pts = Some(sensor_timestamp / (NSEC_PER_MSEC as i64));
                                                }

                                                let _ = handler(frame);
                                            }

                                        }
                                    }

                                    match req.status() {
                                        status @ RequestStatus::Cancelled |
                                        status @ RequestStatus::Pending => {
                                            warn!("request completed. status: {:?}", status);
                                        },
                                        RequestStatus::Complete => {
                                            // Reuse and requeue
                                            req_tx.send(req).unwrap();
                                        }
                                    }
                                }
                            });
                        }

                        let allocator = FrameBufferAllocator::new(&instance.pending_camera);
                        instance.alloc = Some(allocator);

                        let buffers = instance
                            .alloc
                            .as_mut()
                            .unwrap()
                            .alloc(&stream)
                            .map(|buffers|{
                                buffers.into_iter()
                                    .map(|b| MemoryMappedFrameBuffer::new(b).unwrap())
                                    .collect::<Vec<_>>()
                            });

                        let buffers = match buffers {
                            Ok(buffers) => buffers,
                            Err(e) => {
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::StartFailed(format!("Buffer allocation failure. error: {e:?}"))));
                                continue;
                            }
                        };

                        let reqs = buffers
                            .into_iter()
                            .enumerate()
                            .map(|(i, buf)| {
                                let mut req = camera.create_request(Some(i as u64)).unwrap();
                                req.add_buffer(&stream, buf).unwrap();
                                req
                            })
                            .collect::<Vec<_>>();

                        next_frame_at = Instant::now();

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

                        // explict drops, so that log messages are correctly ordered.
                        drop(camera);

                        if let Some(allocator) = instance.alloc.take() {
                            info!("Destroying allocator (releasing buffers)");
                            drop(allocator)
                        }

                        running = false;
                        info!("Camera stopped. id: {}", instance.pending_camera.id().to_string());

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

                        let current_size = stream_config.get_size();

                        // both width and height are required for the libcamera API.
                        let desired_size = if let (Some(width), Some(height)) = (options["width"].get_uint32(), options["height"].get_uint32()) {
                            Some(libcamera::geometry::Size { width, height })
                        } else {
                            None
                        };

                        if let Some(desired_size) = desired_size {
                            info!("Configuring camera. current size: {:?}, desired_size: {:?}", current_size, desired_size);
                            stream_config.set_size(desired_size);
                        }

                        let desired_video_format = options["format"].get_uint32()
                            .map(|value|{
                                VideoFormat::try_from(value)
                                    .map(|video_format| match video_format {
                                        VideoFormat::Pixel(PixelFormat::NV12) => Ok(PIXEL_FORMAT_NV12),
                                        VideoFormat::Pixel(PixelFormat::YUYV) => Ok(PIXEL_FORMAT_YUYV),
                                        // YV12 == YU12 ?
                                        VideoFormat::Pixel(PixelFormat::YV12) => Ok(PIXEL_FORMAT_YU12),
                                        VideoFormat::Compression(CompressionFormat::MJPEG) => Ok(PIXEL_FORMAT_MJPG),
                                        // known, but un-supported.
                                        _ => Err(Error::SetFailed(format!("Unsupported format. '{:?}'", video_format).into()))
                                    })
                                    .map_err(|e|Error::SetFailed(format!("Unknown format. error: {:?}", e).into()))
                                    .flatten()
                            });

                        match desired_video_format {
                            Some(Err(e)) => {
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(e));
                                continue
                            }
                            Some(Ok(desired_video_format)) => {
                                stream_config.set_pixel_format(desired_video_format)
                            }
                            None => {}
                        }

                        // Note the mix of pixel format (libcamera) vs video format (media-rs) here
                        let pixel_format = stream_config.get_pixel_format();
                        info!("video format. desired video format: {:?}, actual pixel format: {:?}", desired_video_format, pixel_format);


                        let frame_rate = options["frame-rate"].get_float();
                        if let Some(frame_rate) = frame_rate {
                            desired_frame_interval = Some(Duration::from_secs_f32(1.0 / frame_rate));
                        } else {
                            // Libcamera does not currently support obtaining usable frame rates, so we have to default to something reasonable, which
                            // may not be supported by the camera.
                            desired_frame_interval = Some(Duration::from_secs_f32(1.0 / 30.0));
                        }

                        // drop the reference to avoid borrow checker issues
                        drop(stream_config);

                        // we can't /apply/ the configuration until the camera is acquired, but we can validate it.

                        let configuation_status = instance.config.validate();
                        match configuation_status {
                            CameraConfigurationStatus::Valid => {}
                            CameraConfigurationStatus::Adjusted => {
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::SetFailed(format!("Configuration adjusted. config: {:?}", instance.config).into())));
                                continue
                            }
                            CameraConfigurationStatus::Invalid => {
                                let _ = instance.cmd_response_tx.send(CameraCmdResponse::DeviceError(Error::SetFailed(format!("Configuration invalid. config: {:?}", instance.config).into())));
                                continue
                            }
                        }
                        let _ = instance.cmd_response_tx.send(CameraCmdResponse::Ok);
                    }
                }
            }
            if let Some(req_rx) = req_rx.as_mut() {
                if let Ok(mut req) = req_rx.recv_timeout(Duration::from_millis(250)) {
                    if let Some(camera) = instance.camera.as_mut() {
                        req.reuse(ReuseFlag::REUSE_BUFFERS);

                        if let Some(desired_frame_interval) = desired_frame_interval {
                            // queue the request based on the desired frame rate / frame duration.

                            next_frame_at += desired_frame_interval;
                            let now = Instant::now();

                            // ensure next_frame_at is in the future
                            while now > next_frame_at {
                                // catch up if behind, skipping missed capture points.
                                next_frame_at = now + desired_frame_interval;
                            }
                            // pace the requests at a constant increment from when the camera was started
                            let delay = next_frame_at - now;

                            // FUTURE ensure the method of sleeping has a high enough resolution for the desired duration
                            // FUTURE determine the average processing time and decrement the sleeping time by the processing time.
                            trace!("now: {:?}, next_frame_at: {:?}, sleep: {:?}us", now, next_frame_at, delay.as_micros());
                            thread::sleep(delay);
                        } else {
                            // queue immediately
                        }

                        if let Err(e) = camera.queue_request(req) {
                            error!("queue_request failed: {:?}", e);
                            break;
                        }
                    }
                }
            }
        }

        if let Some(mut camera) = instance.camera.take() {
            error!("Improper camera shutdown sequence. Camera was still running.");
            camera.stop().unwrap();
        }
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

pub struct CameraDeviceIter<'a> {
    devices: MutexGuard<'static, Vec<LibcameraDevice>>,
    index: usize,
    _phantom: std::marker::PhantomData<&'a LibcameraDevice>,
}

impl<'a> CameraDeviceIter<'a> {
    pub fn new() -> Self {
        Self {
            devices: CAMERA_DEVICES.lock().unwrap(),
            index: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a> Iterator for CameraDeviceIter<'a> {
    type Item = &'a LibcameraDevice;

    fn next(&mut self) -> Option<Self::Item> {
        let device = self.devices.get(self.index)?;
        self.index += 1;
        // Extend the lifetime of the reference to the device to the lifetime of the iterator
        Some(unsafe { std::mem::transmute(device) })
    }
}

pub struct CameraDeviceIterMut<'a> {
    devices: MutexGuard<'static, Vec<LibcameraDevice>>,
    index: usize,
    _phantom: std::marker::PhantomData<&'a mut LibcameraDevice>,
}

impl<'a> CameraDeviceIterMut<'a> {
    pub fn new() -> Self {
        Self {
            devices: CAMERA_DEVICES.lock().unwrap(),
            index: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a> Iterator for CameraDeviceIterMut<'a> {
    type Item = &'a mut LibcameraDevice;

    fn next(&mut self) -> Option<Self::Item> {
        let device = self.devices.get_mut(self.index)?;
        self.index += 1;
        // Safe because the data in static Mutex lives for the entire program
        Some(unsafe { std::mem::transmute(device) })
    }
}
