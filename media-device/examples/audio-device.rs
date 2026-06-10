use std::sync::{Arc, Condvar, Mutex};

use env_logger;
use log::{error, info, warn};
use media_core::{
    audio::{SAMPLE_RATE_48K, SampleFormat, circular_buffer::AudioCircularBuffer},
    variant::Variant,
};
use media_device::{
    audio_device::{AudioDeviceManager, DefaultMicrophoneManager, DefaultSpeakerManager},
    capture::CaptureHanlder,
    render::RenderHandler,
    Device,
};

const SAMPLE_RATE: u32 = SAMPLE_RATE_48K;
const CHANNELS: u8 = 1;
const MAX_BUFFERED_SAMPLES: u32 = SAMPLE_RATE;

fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::init();

    if let Err(e) = run_loopback() {
        error!("audio loopback exited with error: {:?}", e);
    }
}

fn run_loopback() -> media_core::Result<()> {
    let ring = Arc::new(Mutex::new(AudioCircularBuffer::new(SampleFormat::F32, CHANNELS, MAX_BUFFERED_SAMPLES)));

    let mut mic_mgr = AudioDeviceManager::<DefaultMicrophoneManager>::new()?;
    let mut speaker_mgr = AudioDeviceManager::<DefaultSpeakerManager>::new()?;

    for device in mic_mgr.iter() {
        info!("[microphone] discovered: name=\"{}\", id={}", device.name(), device.id());
        match device.formats() {
            Ok(formats) => info!("[microphone] supported formats for \"{}\": {:#?}", device.name(), formats),
            Err(e) => warn!("[microphone] failed to query formats for \"{}\": {:?}", device.name(), e),
        }
    }
    for device in speaker_mgr.iter() {
        info!("[speaker] discovered: name=\"{}\", id={}", device.name(), device.id());
        match device.formats() {
            Ok(formats) => info!("[speaker] supported formats for \"{}\": {:#?}", device.name(), formats),
            Err(e) => warn!("[speaker] failed to query formats for \"{}\": {:?}", device.name(), e),
        }
    }

    let mic = match mic_mgr.index_mut(0) {
        Some(device) => device,
        None => {
            error!("[microphone] no input device available, aborting loopback");
            return Ok(());
        }
    };

    let producer = ring.clone();
    mic.set_output_handler(move |frame| {
        let samples = frame.audio_descriptor().map(|desc| desc.samples.get()).unwrap_or(0);
        let mut buffer = producer.lock().unwrap();
        if buffer.len() + samples > MAX_BUFFERED_SAMPLES {
            warn!("[loopback] ring buffer overflow ({} + {} > {}); dropping backlog", buffer.len(), samples, MAX_BUFFERED_SAMPLES);
            buffer.clear();
        }
        if let Err(e) = buffer.write(&frame) {
            warn!("[loopback] failed to enqueue captured frame: {:?}", e);
        }
        Ok(())
    })?;

    let mut mic_options = Variant::new_dict();
    mic_options["sample-rate"] = SAMPLE_RATE.into();
    mic_options["channels"] = (CHANNELS as u32).into();
    mic.configure(&mic_options)?;
    mic.start()?;
    info!("[microphone] capturing from {} at {} Hz, {} channel(s)", mic.name(), SAMPLE_RATE, CHANNELS);

    let speaker = match speaker_mgr.index_mut(0) {
        Some(device) => device,
        None => {
            error!("[speaker] no output device available, aborting loopback");
            return Ok(());
        }
    };

    let consumer = ring.clone();
    speaker.set_input_handler(move |frame| {
        let mut buffer = consumer.lock().unwrap();
        if let Err(e) = buffer.read(frame) {
            warn!("[loopback] failed to dequeue frame for playback: {:?}", e);
        }
        Ok(())
    })?;

    let mut speaker_options = Variant::new_dict();
    speaker_options["sample-rate"] = SAMPLE_RATE.into();
    speaker_options["channels"] = (CHANNELS as u32).into();
    speaker.configure(&speaker_options)?;
    speaker.start()?;
    info!("[speaker] rendering loopback to {} at {} Hz, {} channel(s)", speaker.name(), SAMPLE_RATE, CHANNELS);
    info!("loopback running; press Ctrl+C to exit");

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

    info!("shutdown requested, stopping devices");
    if let Err(e) = speaker.stop() {
        error!("[speaker] failed to stop {}: {:?}", speaker.name(), e);
    }
    if let Err(e) = mic.stop() {
        error!("[microphone] failed to stop {}: {:?}", mic.name(), e);
    }

    Ok(())
}
