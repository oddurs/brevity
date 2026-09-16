//! A short chime, with no audio dependency: synthesize a WAV once, cache it,
//! hand it to whatever player the system has.

use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::{cache_dir, Ui};

pub enum Chime {
    Done,
    Error,
}

pub fn play(ui: &Ui, which: Chime) {
    if !ui.sound {
        return;
    }
    let explicit = match which {
        Chime::Done => ui.sound_file.clone(),
        Chime::Error => ui.sound_error_file.clone(),
    };
    let path = match explicit {
        Some(p) => PathBuf::from(p),
        None => match synthesize(ui, &which) {
            Ok(p) => p,
            Err(_) => return bell(),
        },
    };
    if !play_file(ui, &path) {
        bell();
    }
}

fn play_file(ui: &Ui, path: &Path) -> bool {
    let p = path.to_string_lossy().to_string();

    if let Some(tmpl) = &ui.sound_cmd {
        let cmd = if tmpl.contains("{}") {
            tmpl.replace("{}", &shell_quote(&p))
        } else {
            format!("{tmpl} {}", shell_quote(&p))
        };
        return run("sh", &["-c", &cmd]);
    }

    let players: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("afplay", &[])]
    } else {
        &[
            ("pw-play", &[]),
            ("paplay", &[]),
            ("aplay", &["-q"]),
            ("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet"]),
            ("play", &["-q"]),
            ("mpv", &["--really-quiet", "--no-video"]),
        ]
    };
    for (bin, args) in players {
        let mut argv: Vec<&str> = args.to_vec();
        argv.push(&p);
        if run(bin, &argv) {
            return true;
        }
    }
    false
}

fn run(bin: &str, args: &[&str]) -> bool {
    Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

fn bell() {
    use std::io::Write;
    let mut err = std::io::stderr();
    let _ = err.write_all(b"\x07");
    let _ = err.flush();
}

fn synthesize(ui: &Ui, which: &Chime) -> Result<PathBuf, String> {
    let vol = (ui.sound_volume * 100.0).round() as u32;
    let name = match which {
        Chime::Done => format!("done-{vol}.wav"),
        Chime::Error => format!("error-{vol}.wav"),
    };
    let dir = cache_dir();
    let path = dir.join(&name);
    if path.exists() {
        return Ok(path);
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let wav = render_wav(notes(which), ui.sound_volume);
    let tmp = dir.join(format!(".{name}.{}", std::process::id()));
    std::fs::write(&tmp, &wav).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Done: a rising two-note blip. Error: the same interval, falling.
fn notes(which: &Chime) -> &'static [(f32, f32)] {
    match which {
        Chime::Done => &[(880.0, 0.075), (1318.5, 0.11)],
        Chime::Error => &[(440.0, 0.09), (329.6, 0.14)],
    }
}

const SAMPLE_RATE: u32 = 44_100;

/// A 16-bit mono PCM WAV, built in memory. No audio crate, no shipped asset.
fn render_wav(notes: &[(f32, f32)], volume: f32) -> Vec<u8> {
    let mut samples: Vec<i16> = Vec::new();
    for (freq, secs) in notes {
        let n = (SAMPLE_RATE as f32 * secs) as usize;
        for i in 0..n {
            let t = i as f32 / SAMPLE_RATE as f32;
            let progress = i as f32 / n as f32;
            // fast attack, exponential decay - reads as a "blip", not a beep
            let attack = (progress / 0.02).min(1.0);
            let env = attack * (-5.0 * progress).exp();
            let s = (2.0 * PI * freq * t).sin() * env * volume * 0.9;
            samples.push((s * i16::MAX as f32) as i16);
        }
    }

    let data_len = (samples.len() * 2) as u32;
    let mut wav: Vec<u8> = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    wav.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le32(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
    }

    #[test]
    fn wav_header_is_well_formed() {
        let wav = render_wav(notes(&Chime::Done), 0.5);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[36..40], b"data");

        let data_len = le32(&wav, 40) as usize;
        assert_eq!(wav.len(), 44 + data_len, "header must describe the payload it ships");
        assert_eq!(le32(&wav, 4) as usize, wav.len() - 8, "RIFF size counts everything after it");
        assert_eq!(le32(&wav, 24), SAMPLE_RATE);
    }

    #[test]
    fn the_chime_is_short_enough_to_be_a_chime() {
        let secs =
            (render_wav(notes(&Chime::Done), 0.5).len() - 44) as f32 / 2.0 / SAMPLE_RATE as f32;
        assert!(secs > 0.05 && secs < 0.5, "{secs}s is not a blip");
    }

    #[test]
    fn volume_scales_the_signal_and_zero_is_silent() {
        let peak = |v: f32| {
            render_wav(notes(&Chime::Done), v)[44..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| i16::from_le_bytes(*c).unsigned_abs())
                .max()
                .unwrap()
        };
        assert!(peak(1.0) > peak(0.5), "louder must mean louder");
        assert_eq!(peak(0.0), 0, "volume 0 must be actual silence");
    }

    #[test]
    fn samples_never_clip() {
        for v in [0.25, 0.5, 1.0] {
            for which in [Chime::Done, Chime::Error] {
                let wav = render_wav(notes(&which), v);
                let clipped = wav[44..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .filter(|c| i16::from_le_bytes(**c).unsigned_abs() >= i16::MAX as u16)
                    .count();
                assert_eq!(clipped, 0, "volume {v} clips");
            }
        }
    }
}
