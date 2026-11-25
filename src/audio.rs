use crate::settings::SoundPreset;
use rand::Rng;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::io::Cursor;

pub struct AudioEngine {
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    wav_data: [Vec<u8>; 5],
}

impl AudioEngine {
    pub fn new() -> Self {
        let wav_data = [
            generate_wav(&SoundParams {
                duration: 0.045,
                base_freq: 190.0,
                sweep_freq: 760.0,
                pitch_decay: 140.0,
                amplitude_decay: 95.0,
                transient_gain: 0.4,
                transient_decay: 600.0,
                amplitude: 26000.0,
                noise_gain: 0.0,
                noise_decay: 0.0,
            }),
            generate_wav(&SoundParams {
                duration: 0.022,
                base_freq: 1500.0,
                sweep_freq: 1500.0,
                pitch_decay: 320.0,
                amplitude_decay: 260.0,
                transient_gain: 0.2,
                transient_decay: 1400.0,
                amplitude: 24000.0,
                noise_gain: 0.0,
                noise_decay: 0.0,
            }),
            generate_wav(&SoundParams {
                duration: 0.22,
                base_freq: 720.0,
                sweep_freq: 480.0,
                pitch_decay: 28.0,
                amplitude_decay: 20.0,
                transient_gain: 0.0,
                transient_decay: 100.0,
                amplitude: 16000.0,
                noise_gain: 0.0,
                noise_decay: 0.0,
            }),
            generate_wav(&SoundParams {
                duration: 0.075,
                base_freq: 90.0,
                sweep_freq: 420.0,
                pitch_decay: 110.0,
                amplitude_decay: 50.0,
                transient_gain: 0.1,
                transient_decay: 800.0,
                amplitude: 27000.0,
                noise_gain: 1.0,
                noise_decay: 190.0,
            }),
            generate_jingle_bell(),
        ];

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

    pub fn play_poke(&self, combo_heat: f32, preset: SoundPreset, volume: u8) {
        if volume == 0 {
            return;
        }
        let Some(ref handle) = self.handle else {
            return;
        };
        let cursor = Cursor::new(self.wav_data[preset.as_u8() as usize].clone());

        if let Ok(source) = Decoder::new_wav(cursor) {
            let mut rng = rand::thread_rng();
            let jitter: f32 = rng.gen_range(-0.04..0.04);
            let pitch = (1.0 + (combo_heat * 0.04).min(0.45) + jitter).max(0.8);
            let level = (0.7 + (combo_heat * 0.08).min(0.8)).min(1.5) * (volume as f32 / 100.0);

            if let Ok(sink) = Sink::try_new(handle) {
                sink.append(source.speed(pitch).amplify(level));
                sink.detach();
            }
        }
    }
}

struct SoundParams {
    duration: f32,
    base_freq: f32,
    sweep_freq: f32,
    pitch_decay: f32,
    amplitude_decay: f32,
    transient_gain: f32,
    transient_decay: f32,
    amplitude: f32,
    noise_gain: f32,
    noise_decay: f32,
}

fn generate_wav(params: &SoundParams) -> Vec<u8> {
    let sample_rate = 44100u32;
    let num_samples = (sample_rate as f32 * params.duration) as usize;
    let mut pcm = Vec::with_capacity(num_samples);
    let mut noise_state = 0x9E37_79B9u32;

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let freq = params.base_freq + params.sweep_freq * (-t * params.pitch_decay).exp();
        let env = (-t * params.amplitude_decay).exp();
        let click_transient = (-t * params.transient_decay).exp() * params.transient_gain;
        let noise = if params.noise_gain == 0.0 {
            0.0
        } else {
            noise_state = noise_state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            let white = ((noise_state >> 8) as f32 / 8_388_608.0) - 1.0;
            white * params.noise_gain * (-t * params.noise_decay).exp()
        };
        let signal =
            ((2.0 * std::f32::consts::PI * freq * t).sin() + click_transient) * env + noise;
        let sample = (signal * params.amplitude).clamp(-32767.0, 32767.0) as i16;
        pcm.push(sample);
    }

    encode_wav(&pcm)
}

fn generate_jingle_bell_pcm() -> Vec<i16> {
    let sample_rate = 44100u32;
    let num_samples = (sample_rate as f32 * 0.32) as usize;
    let partials = [
        (1.0f32, 1.0f32, 16.0f32),
        (1.51, 0.55, 20.0),
        (2.0, 0.40, 26.0),
        (2.67, 0.25, 34.0),
        (3.45, 0.15, 44.0),
    ];
    let strikes = [(0.0f32, 1.0f32), (0.075, 0.6f32)];
    let mut pcm = Vec::with_capacity(num_samples);
    let mut noise_state = 0x9E37_79B9u32;

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        noise_state = noise_state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        let white = ((noise_state >> 8) as f32 / 8_388_608.0) - 1.0;
        let mut signal = 0.0f32;

        for &(at, level) in &strikes {
            if t < at {
                continue;
            }
            let age = t - at;
            for &(ratio, amplitude, decay) in &partials {
                signal += (2.0 * std::f32::consts::PI * 1180.0 * ratio * age).sin()
                    * amplitude
                    * (-age * decay).exp();
            }
            signal += white * 0.22 * (-age * 180.0).exp();
            signal *= level;
        }
        let sample = (signal * 0.2 * 26000.0).clamp(-32767.0, 32767.0) as i16;
        pcm.push(sample);
    }
    pcm
}

fn generate_jingle_bell() -> Vec<u8> {
    encode_wav(&generate_jingle_bell_pcm())
}

fn encode_wav(pcm: &[i16]) -> Vec<u8> {
    let sample_rate = 44100u32;
    let data_len = (pcm.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
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

#[cfg(test)]
mod tests {
    use super::{generate_jingle_bell_pcm, AudioEngine};
    use crate::settings::SoundPreset;

    #[test]
    fn jingle_bell_has_two_strikes() {
        let pcm = generate_jingle_bell_pcm();
        assert!(pcm.len() > 10000);
        let first = pcm[..1323].iter().map(|s| s.abs()).max().unwrap_or(0);
        let second = pcm[3307..4630].iter().map(|s| s.abs()).max().unwrap_or(0);
        assert!(first > 3000, "first strike too quiet: {first}");
        assert!(second > 2000, "second strike too quiet: {second}");
    }

    #[test]
    fn play_poke_accepts_every_preset() {
        let engine = AudioEngine::new();
        for preset in [
            SoundPreset::Wood,
            SoundPreset::Click,
            SoundPreset::Bell,
            SoundPreset::Pop,
            SoundPreset::JingleBell,
        ] {
            engine.play_poke(4.0, preset, 50);
        }
    }
}
