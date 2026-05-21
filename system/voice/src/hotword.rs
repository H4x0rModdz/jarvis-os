//! Hotword detection — sliding-window Whisper.
//!
//! When enabled, holds its own cpal stream, keeps a small ring buffer
//! of audio, and every `TICK_INTERVAL` runs whisper-cli on the latest
//! `WINDOW_SECONDS` of audio. If the transcript contains one of the
//! wake-word substrings, the actor pushes the recognised text out over
//! the event channel — the main service binds the receiver to the
//! `HotwordDetected` DBus signal.
//!
//! Design rationale: see ADR 0015. Quick recap: we reuse the Whisper
//! model already in the ISO instead of shipping a dedicated wake-word
//! engine, accepting higher CPU for less complexity and one mic owner.
//!
//! The actor runs on its own thread because `cpal::Stream` is `!Send`,
//! same pattern as `capture::CaptureActor`.

use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

const TARGET_SAMPLE_RATE: u32 = 16_000;
/// Past audio we hang on to. V2 cut the buffer to 3 s — just enough
/// for the window plus margin — since the window is now 1.5 s.
const BUFFER_SECONDS: usize = 3;
/// How often we transcribe + check for a wake word. V2 dropped from
/// 2 s to 1.5 s so the worst-case latency between "user said it" and
/// "we noticed" is the tick interval, not the tick + window.
const TICK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1500);
/// Slice fed to whisper-cli each tick. V1 was 3 s — too much audio
/// for a wake-word; the operator only ever says two syllables.
/// 1.5 s keeps the surrounding context for Whisper while halving
/// the per-tick whisper-cli work.
const WINDOW_MS: u64 = 1500;
/// RMS below this means "no speech" — skip transcribe.
const SILENCE_RMS_THRESHOLD: f64 = 350.0;
/// Zero-crossing rate ceiling. Speech sits in roughly [0.02, 0.25];
/// noise / fricatives / electrical hum often go well above 0.4.
/// Combined with the RMS gate above this rejects most non-speech
/// windows without spending whisper-cli CPU on them.
const SPEECH_ZCR_CEILING: f64 = 0.40;

/// Lowercased substrings. Loose by design — Whisper's Portuguese head
/// mishears "lilith" as "lilit"/"lilis" intermittently and the cost of
/// a missed wake-word is much higher than a rare false-fire that
/// resolves to "no command found" downstream.
const WAKE_PHRASES: &[&str] = &[
    "oi lilith",
    "ei lilith",
    "olá lilith",
    "ola lilith",
    "hey lilith",
    "ok lilith",
];

enum HotwordCmd {
    Enable(oneshot::Sender<Result<()>>),
    Disable(oneshot::Sender<()>),
    IsEnabled(oneshot::Sender<bool>),
}

#[derive(Clone)]
pub struct HotwordHandle {
    tx: mpsc::Sender<HotwordCmd>,
}

impl HotwordHandle {
    pub async fn enable(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(HotwordCmd::Enable(tx))
            .await
            .map_err(|_| anyhow!("hotword actor stopped"))?;
        rx.await
            .map_err(|_| anyhow!("hotword actor dropped reply"))?
    }

    pub async fn disable(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(HotwordCmd::Disable(tx)).await.is_ok() {
            let _ = rx.await;
        }
    }

    pub async fn is_enabled(&self) -> bool {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(HotwordCmd::IsEnabled(tx)).await.is_err() {
            return false;
        }
        rx.await.unwrap_or(false)
    }
}

/// Spawn the hotword actor and return its handle + the event channel
/// the main service consumes to fire DBus signals.
pub fn spawn() -> (HotwordHandle, mpsc::Receiver<String>) {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<HotwordCmd>(8);
    let (event_tx, event_rx) = mpsc::channel::<String>(8);

    std::thread::Builder::new()
        .name("jarvis-voice-hotword".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("hotword thread runtime");
            rt.block_on(async move {
                let mut live: Option<LiveHotword> = None;
                let mut tick = tokio::time::interval(TICK_INTERVAL);
                // Skip the immediate first tick — interval fires at t=0.
                tick.tick().await;

                loop {
                    tokio::select! {
                        cmd = cmd_rx.recv() => {
                            let Some(cmd) = cmd else { break };
                            match cmd {
                                HotwordCmd::Enable(reply) => {
                                    if live.is_some() {
                                        let _ = reply.send(Ok(()));
                                    } else {
                                        let r = LiveHotword::start();
                                        match r {
                                            Ok(c) => { live = Some(c); let _ = reply.send(Ok(())); }
                                            Err(e) => { let _ = reply.send(Err(e)); }
                                        }
                                    }
                                }
                                HotwordCmd::Disable(reply) => {
                                    live = None;
                                    let _ = reply.send(());
                                }
                                HotwordCmd::IsEnabled(reply) => {
                                    let _ = reply.send(live.is_some());
                                }
                            }
                        }
                        _ = tick.tick(), if live.is_some() => {
                            let snapshot = live.as_ref().and_then(|h| h.snapshot_window());
                            let Some(window) = snapshot else { continue };
                            // Two-feature VAD: cheap energy gate first
                            // (rejects silence), zero-crossing-rate
                            // ceiling second (rejects most non-speech
                            // noise: keyboard typing, AC hum, the cooler
                            // ramping up). Both run in microseconds and
                            // save the ~150–500 ms whisper-cli would
                            // otherwise spend on a noise window.
                            if !is_speech(&window) { continue; }
                            match transcribe_window(window).await {
                                Ok(text) => {
                                    let lower = text.to_lowercase();
                                    if WAKE_PHRASES.iter().any(|p| lower.contains(p)) {
                                        tracing::info!(transcript = %text, "Wake-word detected");
                                        if event_tx.send(text).await.is_err() {
                                            // Receiver dropped — service is gone.
                                            break;
                                        }
                                    }
                                }
                                Err(e) => tracing::warn!(error = %e, "hotword transcribe failed"),
                            }
                        }
                    }
                }
            });
        })
        .expect("spawn hotword thread");

    (HotwordHandle { tx: cmd_tx }, event_rx)
}

struct LiveHotword {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<VecDeque<i16>>>,
    source_sample_rate: u32,
    source_channels: u16,
}

impl LiveHotword {
    fn start() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device"))?;
        let supported = device
            .default_input_config()
            .context("default_input_config")?;
        let source_sample_rate = supported.sample_rate().0;
        let source_channels = supported.channels();
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();

        let cap = TARGET_SAMPLE_RATE as usize * BUFFER_SECONDS;
        let buffer = Arc::new(Mutex::new(VecDeque::with_capacity(cap)));

        let err_buf = buffer.clone();
        let err_fn = move |err| {
            tracing::warn!(error = ?err, "hotword stream error");
            err_buf.lock().unwrap().clear();
        };

        // Each branch needs its own clone of `buffer` because each
        // closure captures by move; one closure is built per format.
        let stream = match format {
            cpal::SampleFormat::I16 => {
                let b = buffer.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _| append_ring(&b, data, cap),
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::F32 => {
                let b = buffer.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _| {
                        let converted: Vec<i16> = data
                            .iter()
                            .map(|s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                            .collect();
                        append_ring(&b, &converted, cap);
                    },
                    err_fn,
                    None,
                )?
            }
            cpal::SampleFormat::U16 => {
                let b = buffer.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        let converted: Vec<i16> =
                            data.iter().map(|s| (*s as i32 - 32768) as i16).collect();
                        append_ring(&b, &converted, cap);
                    },
                    err_fn,
                    None,
                )?
            }
            other => return Err(anyhow!("unsupported sample format: {other:?}")),
        };

        stream.play().context("stream.play")?;

        tracing::info!(
            rate = source_sample_rate,
            channels = source_channels,
            "Hotword capture started"
        );

        Ok(Self {
            _stream: stream,
            buffer,
            source_sample_rate,
            source_channels,
        })
    }

    /// Pull the last `WINDOW_MS` of audio out of the ring buffer,
    /// downmixed to mono and resampled to 16 kHz so whisper-cli is
    /// happy without us thinking about it.
    fn snapshot_window(&self) -> Option<Vec<i16>> {
        let raw: Vec<i16> = {
            let b = self.buffer.lock().unwrap();
            b.iter().cloned().collect()
        };
        if raw.is_empty() {
            return None;
        }
        let mono = downmix(&raw, self.source_channels);
        let resampled = if self.source_sample_rate == TARGET_SAMPLE_RATE {
            mono
        } else {
            resample_linear(&mono, self.source_sample_rate, TARGET_SAMPLE_RATE)
        };
        let needed = (TARGET_SAMPLE_RATE as u64 * WINDOW_MS / 1000) as usize;
        let tail = if resampled.len() > needed {
            resampled[resampled.len() - needed..].to_vec()
        } else {
            resampled
        };
        Some(tail)
    }
}

fn append_ring(buf: &Arc<Mutex<VecDeque<i16>>>, data: &[i16], cap: usize) {
    let mut b = buf.lock().unwrap();
    for s in data {
        if b.len() == cap {
            b.pop_front();
        }
        b.push_back(*s);
    }
}

fn downmix(samples: &[i16], channels: u16) -> Vec<i16> {
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

fn rms(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|s| (*s as f64).powi(2)).sum();
    (sum_sq / samples.len() as f64).sqrt()
}

/// Zero-crossing rate: fraction of adjacent sample pairs that change
/// sign. Cheap proxy for "spectral content above the energy floor" —
/// speech sits in roughly [0.02, 0.25]; broadband noise climbs past
/// 0.4. We combine it with the RMS gate so silence and most non-
/// speech noise are both filtered before whisper-cli is invoked.
fn zcr(samples: &[i16]) -> f64 {
    if samples.len() < 2 {
        return 0.0;
    }
    let crossings = samples
        .windows(2)
        .filter(|w| (w[0] >= 0) != (w[1] >= 0))
        .count();
    crossings as f64 / (samples.len() - 1) as f64
}

/// Two-feature VAD. Rejects:
///   - windows quieter than `SILENCE_RMS_THRESHOLD` (no signal)
///   - windows with ZCR above `SPEECH_ZCR_CEILING` (noisy fricatives,
///     fans, keyboard tapping, AC hum)
///
/// Anything that survives both gates gets fed to whisper-cli.
fn is_speech(samples: &[i16]) -> bool {
    if rms(samples) < SILENCE_RMS_THRESHOLD {
        return false;
    }
    if zcr(samples) > SPEECH_ZCR_CEILING {
        return false;
    }
    true
}

async fn transcribe_window(samples: Vec<i16>) -> Result<String> {
    let path = temp_wav_path();
    let p = path.clone();
    tokio::task::spawn_blocking(move || write_wav(&p, &samples)).await??;
    let result = crate::stt::transcribe(&path).await;
    let _ = tokio::fs::remove_file(&path).await;
    result
}

fn temp_wav_path() -> PathBuf {
    let pid = std::process::id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("jarvis-hotword-{pid}-{ts}.wav"))
}

fn write_wav(path: &std::path::Path, samples: &[i16]) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for s in samples {
        writer.write_sample(*s)?;
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_zero_for_silence() {
        assert_eq!(rms(&[0, 0, 0, 0]), 0.0);
    }

    #[test]
    fn rms_nonzero_for_signal() {
        let r = rms(&[1000, -1000, 1000, -1000]);
        assert!((r - 1000.0).abs() < 0.01);
    }

    #[test]
    fn wake_phrase_lower_substring_matches() {
        let text = "Oi Lilith, pode abrir o navegador?";
        let lower = text.to_lowercase();
        assert!(WAKE_PHRASES.iter().any(|p| lower.contains(p)));
    }

    #[test]
    fn wake_phrase_not_present_in_random_speech() {
        let text = "Olá Maria, tudo bem com você hoje?";
        let lower = text.to_lowercase();
        assert!(!WAKE_PHRASES.iter().any(|p| lower.contains(p)));
    }

    #[test]
    fn append_ring_drops_oldest_at_capacity() {
        let buf = Arc::new(Mutex::new(VecDeque::with_capacity(4)));
        append_ring(&buf, &[1, 2, 3, 4], 4);
        append_ring(&buf, &[5, 6], 4);
        let snapshot: Vec<i16> = buf.lock().unwrap().iter().cloned().collect();
        assert_eq!(snapshot, vec![3, 4, 5, 6]);
    }

    // ── V2 VAD ──────────────────────────────────────────────────────

    #[test]
    fn zcr_constant_signal_is_zero() {
        assert_eq!(zcr(&[1000, 1000, 1000, 1000]), 0.0);
    }

    #[test]
    fn zcr_alternating_is_one() {
        let r = zcr(&[1, -1, 1, -1, 1]);
        assert!((r - 1.0).abs() < 1e-9, "got {r}");
    }

    #[test]
    fn is_speech_rejects_silence() {
        assert!(!is_speech(&vec![0i16; 24000]));
    }

    #[test]
    fn is_speech_rejects_alternating_noise() {
        // Loud + perfectly alternating = ZCR ≈ 1.0; classic fricative
        // or square-wave noise pattern that the energy gate alone
        // would let through.
        let n = 24000;
        let noise: Vec<i16> = (0..n)
            .map(|i| if i % 2 == 0 { 5000 } else { -5000 })
            .collect();
        assert!(rms(&noise) > SILENCE_RMS_THRESHOLD, "test setup");
        assert!(!is_speech(&noise));
    }

    #[test]
    fn is_speech_accepts_voiced_pattern() {
        // 200 Hz sine at 16 kHz → ZCR ≈ 0.025, in speech range.
        let sr = 16000.0;
        let f = 200.0;
        let samples: Vec<i16> = (0..16000)
            .map(|i| {
                let t = i as f32 / sr;
                (8000.0 * (2.0 * std::f32::consts::PI * f * t).sin()) as i16
            })
            .collect();
        assert!(rms(&samples) > SILENCE_RMS_THRESHOLD, "test setup");
        assert!(is_speech(&samples));
    }
}
