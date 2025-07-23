use media::{
    device::{camera::CameraManager, Device, OutputDevice},
    media_frame::SharedMediaFrame,
    variant::Variant,
};

fn main() {
    // Create a default instance of camera manager
    let mut cam_mgr = match CameraManager::new_default() {
        Ok(cam_mgr) => cam_mgr,
        Err(e) => {
            println!("{:?}", e.to_string());
            return;
        }
    };

    // List all camera devices
    let devices = cam_mgr.list();
    for device in devices {
        println!("name: {}", device.name());
        println!("id: {}", device.id());
    }

    // Get the first camera
    let device = match cam_mgr.index_mut(0) {
        Some(device) => device,
        None => {
            println!("no camera found");
            return;
        }
    };

    // Set the output handler for the camera
    if let Err(e) = device.set_output_handler(|frame| {
        println!("frame source: {:?}", frame.source);
        println!("frame desc: {:?}", frame.descriptor());
        println!("frame timestamp: {:?}", frame.pts);

        if let Ok(mapped_guard) = frame.map() {
            if let Some(planes) = mapped_guard.planes() {
                for plane in planes {
                    let plane_stride = plane.stride();
                    let plane_height = plane.height();
                    let _plane_data = plane.data();

                    println!("plane stride: {:?}", plane_stride);
                    println!("plane height: {:?}", plane_height);
                }
            }
        }

        // Create a video frame that can be sent across threads
        let _shared_frame = SharedMediaFrame::new(frame.into_owned());

        Ok(())
    }) {
        println!("{:?}", e.to_string());
    };

    // Configure the camera
    let mut option = Variant::new_dict();
    option["width"] = 1280.into();
    option["height"] = 720.into();
    option["frame-rate"] = 30.0.into();
    if let Err(e) = device.configure(option) {
        println!("{:?}", e.to_string());
    }

    // Start the camera
    if let Err(e) = device.start() {
        println!("{:?}", e.to_string());
    }

    // Get supported formats
    let formats = device.formats();
    if let Ok(formats) = formats {
        if let Some(iter) = formats.array_iter() {
            for format in iter {
                println!("format: {:?}", format["format"]);
                println!("color-range: {:?}", format["color-range"]);
                println!("width: {:?}", format["width"]);
                println!("height: {:?}", format["height"]);
                println!("frame-rates: {:?}", format["frame-rates"]);
            }
        }
    }

    std::thread::sleep(std::time::Duration::from_secs(5));

    // Stop the camera
    if let Err(e) = device.stop() {
        println!("{:?}", e.to_string());
    }
}
