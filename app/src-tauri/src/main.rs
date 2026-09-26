// src-tauri/src/main.rs — comenzile Tauri care leaga interfata (JS) de nucleul audio testat
// (crate-ul `mashup-engine`, adus ca dependinta locala).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mashup_engine::{crossfade as engine_crossfade, detect_bpm, downsample_peaks, load_audio, mix, time_stretch, write_wav, AudioBuffer};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
struct TrackInfo {
    bpm: f32,
    duration_secs: f32,
    sample_rate: u32,
    waveform: Vec<f32>,
}

fn load_cached(path: &str) -> Result<AudioBuffer, String> {
    load_audio(std::path::Path::new(path))
}

/// Incarca o piesa si intoarce BPM detectat + o forma de unda redusa, pt. afisare in UI.
#[tauri::command]
fn analyze_track(path: String) -> Result<TrackInfo, String> {
    let buf = load_cached(&path)?;
    let bpm = detect_bpm(&buf, 60.0, 200.0);
    let waveform = downsample_peaks(&buf, 1200);
    Ok(TrackInfo {
        bpm,
        duration_secs: buf.duration_secs(),
        sample_rate: buf.sample_rate,
        waveform,
    })
}

#[derive(Serialize)]
struct MashupResult {
    out_path: String,
    bpm_final: f32,
    duration_secs: f32,
    waveform: Vec<f32>,
}

/// Aliniaza piesa B la BPM-ul lui A (sau la un BPM tinta dat manual) si le amesteca.
/// `mode`: "overlay" (suprapunere completa) sau "crossfade" (trecere lina intre ele).
#[tauri::command]
fn mashup_tracks(
    path_a: String,
    path_b: String,
    bpm_a_override: Option<f32>,
    bpm_b_override: Option<f32>,
    target_bpm: Option<f32>,
    mode: String,
    gain_a: f32,
    gain_b: f32,
    fade_secs: Option<f32>,
) -> Result<MashupResult, String> {
    let a = load_cached(&path_a)?;
    let b = load_cached(&path_b)?;

    if a.sample_rate != b.sample_rate {
        return Err(format!(
            "cele doua piese au rate de esantionare diferite ({} vs {}); trebuie egalizate inainte de amestecare",
            a.sample_rate, b.sample_rate
        ));
    }

    let bpm_a = bpm_a_override.unwrap_or_else(|| detect_bpm(&a, 60.0, 200.0));
    let bpm_b = bpm_b_override.unwrap_or_else(|| detect_bpm(&b, 60.0, 200.0));
    if bpm_a <= 0.0 || bpm_b <= 0.0 {
        return Err("nu am putut detecta BPM pe una dintre piese; introdu-l manual".to_string());
    }
    let target = target_bpm.unwrap_or(bpm_a);

    let speed_a = target / bpm_a;
    let speed_b = target / bpm_b;
    let a_aligned = time_stretch(&a, speed_a);
    let b_aligned = time_stretch(&b, speed_b);

    let out = match mode.as_str() {
        "crossfade" => engine_crossfade(&a_aligned, &b_aligned, fade_secs.unwrap_or(4.0)),
        _ => mix(&a_aligned, gain_a, &b_aligned, gain_b),
    };

    let out_dir = dirs_output_dir()?;
    let file_name = format!("mashup-{}.wav", chrono_like_timestamp());
    let out_path: PathBuf = out_dir.join(file_name);
    write_wav(&out_path, &out)?;

    Ok(MashupResult {
        out_path: out_path.to_string_lossy().to_string(),
        bpm_final: target,
        duration_secs: out.duration_secs(),
        waveform: downsample_peaks(&out, 1200),
    })
}

fn dirs_output_dir() -> Result<PathBuf, String> {
    let base = dirs_home_dir()?.join("MashupOutput");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    Ok(base)
}

fn dirs_home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("USERPROFILE").map(PathBuf::from))
        .map_err(|_| "nu gasesc directorul home al utilizatorului".to_string())
}

fn chrono_like_timestamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![analyze_track, mashup_tracks])
        .run(tauri::generate_context!())
        .expect("eroare la pornirea aplicatiei Tauri");
}
