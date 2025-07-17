# media-rs

[![Version](https://img.shields.io/crates/v/media)](https://crates.io/crates/media)
[![Documentation](https://docs.rs/media/badge.svg)](https://docs.rs/media)
[![License](https://img.shields.io/badge/License-Apache%202-blue.svg)](LICENSE-APACHE)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE-MIT)

A pure Rust media framework for handling multimedia tasks such as encoding, decoding, capture, playback, processing.

## Features

### Base
- [x] **Media Types** - Definitions for media types
- [x] **Media Frame** - Abstraction for raw multimedia data
- [x] **Video Pixel Format Conversion** - Convert between RGB, YUV, and other pixel formats
- [ ] **Video Scaling** - Change video resolution
- [ ] **Audio Sample Format Conversion** - Convert between different audio sample formats
- [ ] **Audio Resampling** - Change audio sample rates

### Device
- **Camera**
  - [x] **AVFoundation** (macOS/iOS)
  - [x] **Media Foundation** (Windows)
  - [ ] **V4L** (Linux)
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
- **Video Filters** - Video processing filters
- **Audio Filters** - Audio processing filters
