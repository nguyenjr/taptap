use rand::Rng;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::io::Cursor;
use std::sync::Arc;

pub struct AudioEngine {
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    wav_data: Arc<Vec<u8>>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let wav_data = Arc::new(generate_wood_knock_wav());
        match OutputStream::try_default() {
            Ok((stream, handle)) => Self {
                _stream: Some(stream),
                handle: Some(handle),
                wav_data,
            },
            Err(_) => Self {
                _stream: None,
                handle: None,
                wav_data,
            },
        }
    }

    pub fn play_poke(&self, combo_heat: f32) {
        let Some(ref handle) = self.handle else { return };
        let cursor = Cursor::new(self.wav_data.as_ref().clone());

        if let Ok(source) = Decoder::new_wav(cursor) {
            let mut rng = rand::thread_rng();
            let jitter: f32 = rng.gen_range(-0.04..0.04);
            let pitch = (1.0 + (combo_heat * 0.04).min(0.45) + jitter).max(0.8);
            let volume = (0.7 + (combo_heat * 0.08).min(0.8)).min(1.5);

            if let Ok(sink) = Sink::try_new(handle) {
                sink.append(source.speed(pitch).amplify(volume));
                sink.detach();
            }
        }
    }
}

fn generate_wood_knock_wav() -> Vec<u8> {
    let sample_rate = 44100u32;
    let num_samples = (sample_rate as f32 * 0.045) as usize; // 45ms percussive tap
    let mut pcm = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        // Rapid pitch drop from 950Hz to 190Hz for a hollow wood knock feel
        let freq = 190.0 + 760.0 * (-t * 140.0).exp();
        let env = (-t * 95.0).exp();
        let click_transient = (-t * 600.0).exp() * 0.4;
        let signal = ((2.0 * std::f32::consts::PI * freq * t).sin() + click_transient) * env;
        let sample = (signal * 26000.0).clamp(-32767.0, 32767.0) as i16;
        pcm.push(sample);
    }

    let data_len = (pcm.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // Mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in pcm {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}