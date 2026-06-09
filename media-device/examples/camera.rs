use std::sync::{Arc, Condvar, Mutex, RwLock};

use env_logger;
use log::{error, info, warn};
use media_core::{
    frame::{Frame, SharedFrame},
    variant::Variant,
};
use media_device::{
    camera::{CameraManager, DefaultCameraManager},
    capture::CaptureHanlder,
    Device,
};

fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();
    // Create a camera manager from the platform default backend
    let mut cam_mgr = match CameraManager::<DefaultCameraManager>::new() {
        Ok(cam_mgr) => cam_mgr,
        Err(e) => {
            error!("{:?}", e.to_string());
            return;
        }
    };

    // List all camera devices
    for device in cam_mgr.iter() {
        info!("name: {}", device.name());
        info!("id: {}", device.id());
    }

    // Get the first camera
    let device = match cam_mgr.index_mut(0) {
        Some(device) => device,
        None => {
            warn!("no camera found");
            return;
        }
    };

    // Set the output handler for the camera
    if let Err(e) = device.set_output_handler(|frame| {
        info!("frame source: {:?}", frame.source);
        info!("frame desc: {:?}", frame.descriptor());
        info!("frame timestamp: {:?}", frame.pts);

        if let Ok(mapped_guard) = frame.map() {
            if let Some(planes) = mapped_guard.planes() {
                for plane in planes {
                    let plane_stride = plane.stride();
                    let plane_height = plane.height();
                    let _plane_data = plane.data();

                    info!("plane stride: {:?}", plane_stride);
                    info!("plane height: {:?}", plane_height);
                }
            }
        }

        // Create a video frame that can be sent across threads
        let _shared_frame = SharedFrame::<RwLock<Frame<'static>>>::new(frame.into_owned());

        Ok(())
    }) {
        error!("{:?}", e.to_string());
    };

    // Configure the camera
    let mut option = Variant::new_dict();
    option["width"] = 1280.into();
    option["height"] = 720.into();
    option["frame-rate"] = 30.0.into();
    if let Err(e) = device.configure(&option) {
        error!("{:?}", e.to_string());
    }

    // Start the camera
    if let Err(e) = device.start() {
        error!("{:?}", e.to_string());
    }

    // Get supported formats
    let formats = device.formats();
    if let Ok(formats) = formats {
        if let Some(iter) = formats.array_iter() {
            for format in iter {
                info!("format: {:?}", format["format"]);
                info!("color-range: {:?}", format["color-range"]);
                info!("width: {:?}", format["width"]);
                info!("height: {:?}", format["height"]);
                info!("frame-rates: {:?}", format["frame-rates"]);
            }
        }
    }

    let running = Arc::new((Mutex::new(true), Condvar::new()));
    let stop_flag = running.clone();
    ctrlc::set_handler(move || {
        let (lock, cvar) = &*stop_flag;
        *lock.lock().unwrap() = false;
        cvar.notify_one();
    })
    .expect("failed to install Ctrl+C handler");

    let (lock, cvar) = &*running;
    let mut guard = lock.lock().unwrap();
    while *guard {
        guard = cvar.wait(guard).unwrap();
    }

    // Stop the camera
    if let Err(e) = device.stop() {
        error!("{:?}", e.to_string());
    }
}
