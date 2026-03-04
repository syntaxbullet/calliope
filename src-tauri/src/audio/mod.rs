use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, StreamConfig};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

pub const TARGET_SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioLevelEvent {
    pub rms: f32,
}

/// Shared audio buffer filled during recording.
pub type AudioBuffer = Arc<Mutex<Vec<f32>>>;

/// Enumerate all available input (microphone) devices.
pub fn list_input_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();

    host.input_devices()
        .map(|devices| {
            devices
                .filter_map(|d| {
                    let name = d.name().ok()?;
                    Some(AudioDevice {
                        id: name.clone(),
                        is_default: name == default_name,
                        name,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Get the default or user-selected input device.
pub fn get_input_device(device_id: Option<&str>) -> Option<Device> {
    let host = cpal::default_host();
    if let Some(id) = device_id {
        host.input_devices().ok()?.find(|d| {
            d.name().map(|n| n == id).unwrap_or(false)
        })
    } else {
        host.default_input_device()
    }
}

/// Build a 16 kHz mono f32 stream config for the given device.
pub fn build_stream_config(device: &Device) -> Option<StreamConfig> {
    let supported = device.supported_input_configs().ok()?;
    for range in supported {
        if range.sample_format() == SampleFormat::F32
            && range.min_sample_rate().0 <= TARGET_SAMPLE_RATE
            && range.max_sample_rate().0 >= TARGET_SAMPLE_RATE
        {
            return Some(StreamConfig {
                channels: 1,
                sample_rate: SampleRate(TARGET_SAMPLE_RATE),
                buffer_size: cpal::BufferSize::Default,
            });
        }
    }
    // Fallback: use default config (will need downmix to mono in callback)
    device
        .default_input_config()
        .ok()
        .map(|c| c.into())
}

/// Start recording into the shared buffer; emits `audio-level` events.
/// Returns the active stream (caller must keep it alive for the recording duration).
pub fn start_recording(
    app: AppHandle,
    device: &Device,
    config: &StreamConfig,
    buffer: AudioBuffer,
) -> Option<cpal::Stream> {
    let buffer_clone = buffer.clone();
    let app_clone = app.clone();
    let level_state = app.state::<crate::state::CurrentAudioLevel>().0.clone();

    let channels = config.channels as usize;
    let source_rate = config.sample_rate.0;

    log::debug!(
        "audio: recording at {}Hz, {}ch",
        source_rate, channels
    );

    let stream = device
        .build_input_stream(
            config,
            move |data: &[f32], _info| {
                // Downmix to mono if needed
                let mono: Vec<f32> = if channels == 1 {
                    data.to_vec()
                } else {
                    data.chunks(channels)
                        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                        .collect()
                };

                // Compute RMS for VU meter (lock-free atomic for overlay polling)
                let sum_sq: f32 = mono.iter().map(|s| s * s).sum();
                let rms = (sum_sq / mono.len() as f32).sqrt().clamp(0.0, 1.0);
                level_state.store(rms.to_bits(), std::sync::atomic::Ordering::Relaxed);
                let _ = app_clone.emit("audio-level", AudioLevelEvent { rms });

                // Append samples to buffer
                let mut buf = buffer_clone.lock().unwrap();
                buf.extend_from_slice(&mono);
            },
            |err| log::error!("audio stream error: {err}"),
            None,
        )
        .ok()?;

    stream.play().ok()?;
    Some(stream)
}

/// Explicitly pause a stream before dropping it.
/// On macOS, this gives CoreAudio a chance to cleanly stop its audio unit
/// before disposal, preventing resource leaks on rapid record cycles.
pub fn stop_stream(stream: &cpal::Stream) {
    use cpal::traits::StreamTrait;
    if let Err(e) = stream.pause() {
        log::warn!("audio: failed to pause stream before drop: {e}");
    }
}

/// Resample audio from `from_rate` to `to_rate`.
///
/// Uses rubato's windowed-sinc resampler with proper anti-aliasing.
pub fn resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }

    use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let ratio = to_rate as f64 / from_rate as f64;
    let mut resampler = SincFixedIn::<f32>::new(
        ratio,
        2.0,    // max relative ratio deviation
        params,
        input.len(),
        1,      // mono
    ).expect("failed to create resampler");

    let input_buf = vec![input.to_vec()];
    let output_buf = resampler.process(&input_buf, None).expect("resampling failed");
    output_buf.into_iter().next().unwrap()
}

/// Trim leading and trailing silence from a 16 kHz sample buffer.
///
/// `threshold` is the RMS level (0.0–1.0) below which a window is considered
/// silent. `window_ms` sets the analysis window size in milliseconds.
/// A small padding of silence is kept at each end to avoid clipping speech onset.
pub fn trim_silence(samples: &[f32], threshold: f32, window_ms: u32) -> &[f32] {
    if samples.is_empty() {
        return samples;
    }

    let window_size = (TARGET_SAMPLE_RATE * window_ms / 1000) as usize;
    if window_size == 0 || samples.len() < window_size {
        return samples;
    }

    let is_voiced = |window: &[f32]| -> bool {
        let sum_sq: f32 = window.iter().map(|s| s * s).sum();
        let rms = (sum_sq / window.len() as f32).sqrt();
        rms > threshold
    };

    // Find first voiced window
    let mut start = 0;
    for i in (0..samples.len()).step_by(window_size) {
        let end = (i + window_size).min(samples.len());
        if is_voiced(&samples[i..end]) {
            start = i;
            break;
        }
        start = samples.len(); // will be overridden or mean all-silent
    }

    if start >= samples.len() {
        return &samples[0..0]; // entirely silent
    }

    // Find last voiced window
    let mut end = samples.len();
    for i in (0..samples.len()).rev().step_by(window_size) {
        let w_start = i.saturating_sub(window_size);
        if is_voiced(&samples[w_start..=i.min(samples.len() - 1)]) {
            end = (i + 1).min(samples.len());
            break;
        }
    }

    // Add a small padding (~50ms) to avoid cutting off speech edges
    let pad = (TARGET_SAMPLE_RATE as usize * 50) / 1000;
    let start = start.saturating_sub(pad);
    let end = (end + pad).min(samples.len());

    &samples[start..end]
}

