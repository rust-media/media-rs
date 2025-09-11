use env_logger;
use log::{error, info, warn};
use media_core::{frame::SharedFrame, variant::Variant};
use media_device::{camera::CameraManager, Device, OutputDevice};

fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();
    // Create a default instance of camera manager
    let mut cam_mgr = match CameraManager::new_default() {
        Ok(cam_mgr) => cam_mgr,
        Err(e) => {
            error!("{:?}", e.to_string());
            return;
        }
    };

    // List all camera devices
    let devices = cam_mgr.list();
    for device in devices {
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
        let _shared_frame = SharedFrame::new(frame.into_owned());

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

    std::thread::sleep(std::time::Duration::from_secs(5));

    // Stop the camera
    if let Err(e) = device.stop() {
        error!("{:?}", e.to_string());
    }
}
