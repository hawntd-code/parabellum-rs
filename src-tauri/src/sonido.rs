//! El sonido de mensaje nuevo (como el de Discord): dos notas cortas que suben, generadas aquí mismo
//! (sin archivo de audio) y tocadas por el sistema, así suena aunque Parabellum esté junto al reloj.

use std::f64::consts::TAU;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const RATE: u32 = 44_100;
/// Si llegan varios mensajes seguidos, suena una vez.
const QUIET: Duration = Duration::from_millis(1200);

/// El sonido como archivo WAV (PCM de 16 bits, mono).
fn chime() -> Vec<u8> {
    // (empieza en s, frecuencia en Hz, cuánto tarda en apagarse): do y sol agudos.
    const NOTES: [(f64, f64, f64); 2] = [(0.0, 1046.5, 0.045), (0.075, 1568.0, 0.09)];
    const LENGTH: f64 = 0.42;
    const FADE: f64 = 0.02;
    let total = (LENGTH * RATE as f64) as usize;
    let mut samples = vec![0f64; total];
    for (start, frequency, decay) in NOTES {
        let first = (start * RATE as f64) as usize;
        for (i, sample) in samples.iter_mut().enumerate().skip(first) {
            let t = (i - first) as f64 / RATE as f64;
            // Entra en 4 ms (sin chasquido) y se apaga como una campanita.
            let envelope = (t / 0.004).min(1.0) * (-t / decay).exp();
            let phase = TAU * frequency * t;
            *sample += envelope * (phase.sin() + 0.18 * (2.0 * phase).sin());
        }
    }
    let fade_from = total - (FADE * RATE as f64) as usize;
    for (i, sample) in samples.iter_mut().enumerate().skip(fade_from) {
        *sample *= (total - i) as f64 / (total - fade_from) as f64;
    }
    let peak = samples.iter().fold(0f64, |m, s| m.max(s.abs())).max(1e-9);
    let pcm: Vec<u8> = samples.iter().flat_map(|s| ((s / peak * 0.35 * i16::MAX as f64) as i16).to_le_bytes()).collect();

    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(&pcm);
    wav
}

fn wav() -> &'static [u8] {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    WAV.get_or_init(chime)
}

/// Toca el sonido de mensaje nuevo sin esperar a que termine. `wait`: esperar (para probarlo).
pub fn message(wait: bool) -> bool {
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    {
        let mut last = LAST.lock().unwrap();
        if last.is_some_and(|at| at.elapsed() < QUIET) {
            return true;
        }
        *last = Some(Instant::now());
    }
    crate::plataforma::play_wav(wav(), wait)
}

/// El sonido como archivo (para escucharlo fuera de Parabellum).
pub fn save(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(path, wav())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_sonido_es_un_wav_valido() {
        let wav = chime();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        let size = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
        assert_eq!(wav.len(), 44 + size);
        // Empieza y termina en silencio (sin chasquidos).
        let sample = |i: usize| i16::from_le_bytes([wav[44 + 2 * i], wav[45 + 2 * i]]);
        assert_eq!(sample(0), 0);
        assert!(sample(size / 2 - 1).abs() < 50);
    }
}
