# media-rs

[![Version](https://img.shields.io/crates/v/media)](https://crates.io/crates/media)
[![Documentation](https://docs.rs/media/badge.svg)](https://docs.rs/media)
[![License](https://img.shields.io/badge/License-Apache%202-blue.svg)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE-MIT)

A pure Rust media framework for handling multimedia tasks such as encoding, decoding, capture, playback, processing.

## Features

### Core
- [x] **Media Types** - Define common media types
- [x] **Media Frame** - Represent audio and video frame data, including hardware abstraction
- [x] **Video Pixel Format Conversion** - Convert between RGB, YUV, and other pixel formats
- [x] **Video Scaling** - Scale video resolution
- [x] **Audio Sample Format Conversion** - Convert between different audio sample formats
- [ ] **Audio Resampling** - Resample audio sample rates

### Device
- **Camera**
  - [x] **AVFoundation** (macOS/iOS)
  - [x] **Media Foundation** (Windows)
  - [x] **Libcamera** (Linux) - Libcamera supports Raspberry Pi MIPI cameras as well as V4L2 cameras.
- **Speaker/Microphone**
  - [ ] **Core Audio** (macOS/iOS)
  - [ ] **WASAPI** (Windows)
  - [ ] **ALSA** (Linux)
  - [ ] **PulseAudio** (Linux)

### Codec
- **Video Encoders**
  - [ ] **H.264/AVC**
  - [ ] **VP8/VP9**
  - [ ] **AV1**
- **Video Decoders**
  - [ ] **H.264/AVC**
  - [ ] **VP8/VP9**
  - [ ] **AV1**
- **Audio Encoders**
  - [ ] **AAC**
  - [ ] **Opus**
- **Audio Decoders**
  - [ ] **AAC**
  - [ ] **Opus**

### Filter
- **Video Filters** - Process video and apply effects
- **Audio Filters** - Process audio and apply effects

## History

This crate is newer than the `video-capture` and `x-media` crates and should be considered as a replacement.
