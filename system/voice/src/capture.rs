//! Microphone capture via cpal, wrapped in an actor so the rest of the
//! daemon doesn't have to worry about `cpal::Stream` being `!Send + !Sync`.
//!
//! `CaptureActor::spawn` returns a `CaptureHandle` that exposes async
//! `start` / `stop` over an internal mpsc channel. The actor itself runs
//! on a dedicated `std::thread` so the cpal stream stays on the thread
//! that created it (required on some platforms; harmless on Linux/ALSA).
//!
//! V2 ships with a single configuration target — 16 kHz mono i16 —
//! because that's what whisper.cpp wants. If the user's input device
//! doesn't natively support that shape we open at the device's native
//! config and resample/downmix on stop.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

/// Abstraction over microphone capture so tests can swap in a
/// scripted fake. The production impl is `CaptureHandle`; tests use
/// `MockCapture` in the test module.
///
/// All three methods are async because the production path talks to
/// a tokio actor over an mpsc channel. The trait stays minimal —
/// start, stop, cancel — to match the actor's protocol.
#[async_trait]
pub trait AudioCapture: Send + Sync {
    /// Begin capturing from the default microphone. Returns
    /// immediately once the stream is set up; samples accumulate
    /// until `stop` is called. Returns an error if a stream is
    /// already live, or if cpal can't open the device.
    async fn start(&self) -> Result<()>;

    /// Stop the in-flight recording and return the captured samples
    /// (16 kHz mono i16). Returns an error if `start` wasn't called
    /// first.
    async fn stop(&self) -> Result<Vec<i16>>;

    /// Abort whatever is in flight. Fire-and-forget; no-op when
    /// nothing is capturing.
    async fn cancel(&self);
}

#[async_trait]
impl AudioCapture for CaptureHandle {
    async fn start(&self) -> Result<()> {
        CaptureHandle::start(self).await
    }
    async fn stop(&self) -> Result<Vec<i16>> {
        CaptureHandle::stop(self).await
    }
    async fn cancel(&self) {
        CaptureHandle::cancel(self).await
    }
}

const TARGET_SAMPLE_RATE: u32 = 16_000;
const TARGET_CHANNELS: u16 = 1;

/// Commands sent to the capture actor thread.
enum CaptureCmd {
    Start(oneshot::Sender<Result<()>>),
    Stop(oneshot::Sender<Result<Vec<i16>>>),
    Cancel,
}

/// Handle held by the DBus service. `Send + Sync` so it can live in
/// `VoiceService` without infecting it.
#[derive(Clone)]
pub struct CaptureHandle {
    tx: mpsc::Sender<CaptureCmd>,
}

impl CaptureHandle {
    pub async fn start(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(CaptureCmd::Start(tx))
            .await
            .map_err(|_| anyhow!("capture actor stopped"))?;
        rx.await
            .map_err(|_| anyhow!("capture actor dropped reply"))?
    }

    pub async fn stop(&self) -> Result<Vec<i16>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(CaptureCmd::Stop(tx))
            .await
            .map_err(|_| anyhow!("capture actor stopped"))?;
        rx.await
            .map_err(|_| anyhow!("capture actor dropped reply"))?
    }

    pub async fn cancel(&self) {
        // Fire-and-forget — if the channel is closed we're already idle.
        let _ = self.tx.send(CaptureCmd::Cancel).await;
    }
}

/// Spawn the capture actor on its own thread. Returns the handle the
/// service uses to talk to it.
pub fn spawn() -> CaptureHandle {
    let (tx, mut rx) = mpsc::channel::<CaptureCmd>(8);

    std::thread::Builder::new()
        .name("jarvis-voice-capture".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("capture thread runtime");
            rt.block_on(async move {
                let mut live: Option<LiveCapture> = None;
                while let Some(cmd) = rx.recv().await {
                    match cmd {
                        CaptureCmd::Start(reply) => {
                            let r = match LiveCapture::start() {
                                Ok(c) => {
                                    live = Some(c);
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            };
                            let _ = reply.send(r);
                        }
                        CaptureCmd::Stop(reply) => {
                            let r = match live.take() {
                                Some(c) => c.finish(),
                                None => Err(anyhow!("not capturing")),
                            };
                            let _ = reply.send(r);
                        }
                        CaptureCmd::Cancel => {
                            live = None; // drops the stream
                        }
                    }
                }
            });
        })
        .expect("spawn capture thread");

    CaptureHandle { tx }
}

/// The actual cpal stream + the shared buffer it pushes into. Lives on
/// the actor thread; never crosses thread boundaries.
struct LiveCapture {
    stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<i16>>>,
    source_sample_rate: u32,
    source_channels: u16,
}

impl LiveCapture {
    fn start() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device"))?;
        let device_name = device.name().unwrap_or_else(|_| "<unnamed>".into());

        // Try the ideal config first — most modern inputs accept 16k mono.
        let want = cpal::SupportedStreamConfig::new(
            TARGET_CHANNELS,
            cpal::SampleRate(TARGET_SAMPLE_RATE),
            cpal::SupportedBufferSize::Unknown,
            cpal::SampleFormat::I16,
        );

        let supported = device
            .default_input_config()
            .context("default_input_config")?;
        let chosen = if supported_is_ok_for(&supported, &want) {
            want
        } else {
            supported
        };

        let source_sample_rate = chosen.sample_rate().0;
        let source_channels = chosen.channels();
        let format = chosen.sample_format();
        let config: cpal::StreamConfig = chosen.into();

        tracing::info!(
            device = %device_name,
            rate = source_sample_rate,
            channels = source_channels,
            ?format,
            "Opening capture stream"
        );

        let buffer: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::with_capacity(
            (TARGET_SAMPLE_RATE as usize) * 8, // ~8 s of audio without re-alloc
        )));

        let err_buffer = buffer.clone();
        let err_fn = move |err| {
            tracing::warn!(error = ?err, "cpal stream error");
            err_buffer.lock().unwrap().clear();
        };

        let stream = match format {
            cpal::SampleFormat::I16 => {
                let buf = buffer.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _| buf.lock().unwrap().extend_from_slice(data),
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::F32 => {
                let buf = buffer.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        let mut b = buf.lock().unwrap();
                        b.extend(
                            data.iter()
                                .map(|s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16),
                        );
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let buf = buffer.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        let mut b = buf.lock().unwrap();
                        b.extend(data.iter().map(|s| (*s as i32 - 32768) as i16));
                    },
                    err_fn,
                    None,
                )?
            }
            other => return Err(anyhow!("unsupported sample format: {other:?}")),
        };

        stream.play().context("stream.play")?;

        Ok(Self {
            stream,
            buffer,
            source_sample_rate,
            source_channels,
        })
    }

    fn finish(self) -> Result<Vec<i16>> {
        drop(self.stream);

        let captured = {
            let lock = self.buffer.lock().unwrap();
            lock.clone()
        };

        let mono = downmix_to_mono(&captured, self.source_channels);
        let resampled = if self.source_sample_rate == TARGET_SAMPLE_RATE {
            mono
        } else {
            resample_linear(&mono, self.source_sample_rate, TARGET_SAMPLE_RATE)
        };

        tracing::info!(
            captured = captured.len(),
            mono = resampled.len(),
            "Capture finished"
        );
        Ok(resampled)
    }
}

fn supported_is_ok_for(
    supported: &cpal::SupportedStreamConfig,
    want: &cpal::SupportedStreamConfig,
) -> bool {
    supported.channels() == want.channels()
        && supported.sample_rate() == want.sample_rate()
        && supported.sample_format() == want.sample_format()
}

/// Mean-downmix interleaved multi-channel PCM into mono.
fn downmix_to_mono(samples: &[i16], channels: u16) -> Vec<i16> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let ch = channels as usize;
    samples
        .chunks_exact(ch)
        .map(|frame| {
            let sum: i32 = frame.iter().map(|s| *s as i32).sum();
            (sum / ch as i32) as i16
        })
        .collect()
}

/// Trivial linear-interpolation resampler. Whisper is forgiving on input
/// quality; a proper SRC isn't worth the dependency for V2.
fn resample_linear(samples: &[i16], from_rate: u32, to_rate: u32) -> Vec<i16> {
    if samples.is_empty() || from_rate == to_rate {
        return samples.to_vec();
    }
    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = ((samples.len() as f64) * ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_pos = i as f64 / ratio;
        let src_idx = src_pos as usize;
        if src_idx + 1 >= samples.len() {
            out.push(samples[samples.len() - 1]);
        } else {
            let frac = src_pos - src_idx as f64;
            let a = samples[src_idx] as f64;
            let b = samples[src_idx + 1] as f64;
            out.push((a + (b - a) * frac) as i16);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_two_channels_averages() {
        let mixed = downmix_to_mono(&[10, 20, 30, 40], 2);
        assert_eq!(mixed, vec![15, 35]);
    }

    #[test]
    fn downmix_mono_passthrough() {
        let in_buf = vec![1, 2, 3];
        assert_eq!(downmix_to_mono(&in_buf, 1), in_buf);
    }

    #[test]
    fn resample_same_rate_is_passthrough() {
        let in_buf = vec![1, 2, 3, 4, 5];
        assert_eq!(resample_linear(&in_buf, 16_000, 16_000), in_buf);
    }

    #[test]
    fn resample_halves_length_when_target_is_half() {
        let out = resample_linear(&[100, 100, 200, 200], 32_000, 16_000);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn resample_doubles_length_when_target_doubles() {
        let out = resample_linear(&[100, 200], 16_000, 32_000);
        assert_eq!(out.len(), 4);
    }
}
