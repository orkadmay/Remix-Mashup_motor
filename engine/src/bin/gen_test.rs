use mashup_engine::{write_wav, AudioBuffer};
use std::env;
use std::path::Path;

fn gen_click_track(bpm: f32, duration_secs: f32, sample_rate: u32) -> AudioBuffer {
    let n = (duration_secs * sample_rate as f32) as usize;
    let mut samples = vec![0.0f32; n];
    let interval = 60.0 / bpm;
    let click_len = (0.008 * sample_rate as f32) as usize; // 8ms click
    let mut t = 0.0f32;
    while t < duration_secs {
        let start = (t * sample_rate as f32) as usize;
        for k in 0..click_len {
            if start + k < n {
                // click = ton scurt de 2kHz cu plic exponential descrescator
                let phase = 2.0 * std::f32::consts::PI * 2000.0 * (k as f32 / sample_rate as f32);
                let env = (-(k as f32) / (click_len as f32 * 0.3)).exp();
                samples[start + k] += phase.sin() * env * 0.9;
            }
        }
        t += interval;
    }
    AudioBuffer { samples, sample_rate }
}

fn gen_sine(freq: f32, duration_secs: f32, sample_rate: u32) -> AudioBuffer {
    let n = (duration_secs * sample_rate as f32) as usize;
    let samples: Vec<f32> = (0..n)
        .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32).sin() * 0.6)
        .collect();
    AudioBuffer { samples, sample_rate }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let out_dir = args.get(1).map(|s| s.as_str()).unwrap_or("/home/claude/mashup-engine/testdata");
    std::fs::create_dir_all(out_dir).unwrap();

    let sr = 44100;

    let click_120 = gen_click_track(120.0, 12.0, sr);
    write_wav(Path::new(&format!("{out_dir}/click_120bpm.wav")), &click_120).unwrap();

    let click_128 = gen_click_track(128.0, 12.0, sr);
    write_wav(Path::new(&format!("{out_dir}/click_128bpm.wav")), &click_128).unwrap();

    let click_95 = gen_click_track(95.0, 12.0, sr);
    write_wav(Path::new(&format!("{out_dir}/click_95bpm.wav")), &click_95).unwrap();

    let sine_440 = gen_sine(440.0, 5.0, sr);
    write_wav(Path::new(&format!("{out_dir}/sine_440.wav")), &sine_440).unwrap();

    println!("fisiere de test generate in {out_dir}");
}
