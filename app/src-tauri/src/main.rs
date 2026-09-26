// src-tauri/src/main.rs — comenzile Tauri care leaga interfata (JS) de nucleul audio testat
// (crate-ul `mashup-engine`, adus ca dependinta locala).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mashup_engine::{
    apply_custom_description, apply_style_chain, auto_segments, crossfade as engine_crossfade,
    detect_bpm, downsample_peaks, echo, extract_segment, load_audio, mix, normalize_peak, segment_rms,
    splice_segments, stutter, style_list, time_stretch, write_wav, AudioBuffer,
};
use serde::{Deserialize, Serialize};
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
struct AudioResult {
    out_path: String,
    bpm_final: f32,
    duration_secs: f32,
    waveform: Vec<f32>,
}

fn write_result(out: &AudioBuffer, prefix: &str, bpm_final: f32) -> Result<AudioResult, String> {
    let out = normalize_peak(out, 0.97);
    let out_dir = dirs_output_dir()?;
    let file_name = format!("{prefix}-{}.wav", chrono_like_timestamp());
    let out_path: PathBuf = out_dir.join(file_name);
    write_wav(&out_path, &out)?;
    Ok(AudioResult {
        out_path: out_path.to_string_lossy().to_string(),
        bpm_final,
        duration_secs: out.duration_secs(),
        waveform: downsample_peaks(&out, 1200),
    })
}

/// Aliniaza piesa B la BPM-ul lui A (sau la un BPM tinta dat manual) si le amesteca (piesele intregi).
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
) -> Result<AudioResult, String> {
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

    write_result(&out, "mashup", target)
}

/// O bucata selectata dintr-o piesa, pentru mashup-ul "pe segmente" (taie-si-lipeste).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SegmentSpec {
    path: String,
    start_secs: f32,
    end_secs: f32,
    bpm_override: Option<f32>,
}

/// Construieste un mashup adevarat: taie bucatile indicate (fiecare din propria ei piesa),
/// aliniaza tempo-ul fiecarei bucati la un BPM tinta comun, si le lipeste in ordinea data,
/// cu un scurt crossfade la fiecare imbinare ca sa nu se auda pocnituri.
#[tauri::command]
fn build_segment_mashup(
    segments: Vec<SegmentSpec>,
    target_bpm: Option<f32>,
    seam_crossfade_secs: Option<f32>,
) -> Result<AudioResult, String> {
    if segments.is_empty() {
        return Err("adauga cel putin o bucata inainte de a construi mashup-ul".to_string());
    }

    // incarcam fiecare piesa o singura data, chiar daca e folosita in mai multe bucati
    let mut loaded: Vec<(String, AudioBuffer)> = Vec::new();
    let mut sample_rate: Option<u32> = None;
    let mut aligned_segments: Vec<AudioBuffer> = Vec::new();
    let target = target_bpm.unwrap_or(0.0);
    let mut resolved_target = target;

    for (idx, seg) in segments.iter().enumerate() {
        let buf = if let Some((_, b)) = loaded.iter().find(|(p, _)| p == &seg.path) {
            b.clone()
        } else {
            let b = load_cached(&seg.path)?;
            loaded.push((seg.path.clone(), b.clone()));
            b
        };
        if let Some(sr) = sample_rate {
            if sr != buf.sample_rate {
                return Err(format!(
                    "piesele au rate de esantionare diferite ({} vs {}); nu pot fi lipite direct",
                    sr, buf.sample_rate
                ));
            }
        } else {
            sample_rate = Some(buf.sample_rate);
        }

        let bpm = seg
            .bpm_override
            .unwrap_or_else(|| detect_bpm(&buf, 60.0, 200.0));
        if bpm <= 0.0 {
            return Err(format!("nu am putut detecta BPM pentru bucata {}; introdu-l manual", idx + 1));
        }
        if idx == 0 && target_bpm.is_none() {
            resolved_target = bpm; // implicit: BPM-ul primei bucati devine tinta
        }

        let segment_audio = extract_segment(&buf, seg.start_secs, seg.end_secs);
        let speed = resolved_target / bpm;
        aligned_segments.push(time_stretch(&segment_audio, speed));
    }

    let out = splice_segments(&aligned_segments, seam_crossfade_secs.unwrap_or(0.15));
    write_result(&out, "mashup-segmente", resolved_target)
}

/// Rezultatul unei analize de piesa, cu segmentele auto-detectate si scorul lor de energie,
/// pt. a arata utilizatorului ce a ales programul (transparenta, nu doar o cutie neagra).
#[derive(Serialize)]
struct AutoSegmentInfo {
    start_secs: f32,
    end_secs: f32,
    energy: f32,
    chosen: bool,
}

#[derive(Serialize)]
struct AutoMashupResult {
    out_path: String,
    bpm_final: f32,
    duration_secs: f32,
    waveform: Vec<f32>,
    picked_a: Vec<AutoSegmentInfo>,
    picked_b: Vec<AutoSegmentInfo>,
}

/// Mashup complet automat: alege singur cele mai energice fraze din fiecare piesa (fara nicio
/// selectie manuala), le aliniaza la un BPM comun si le lipeste alternat (A, B, A, B, ...).
#[tauri::command]
fn auto_mashup(
    path_a: String,
    path_b: String,
    bpm_a_override: Option<f32>,
    bpm_b_override: Option<f32>,
    target_bpm: Option<f32>,
    phrase_beats: Option<f32>,
    segments_per_track: Option<usize>,
    seam_crossfade_secs: Option<f32>,
) -> Result<AutoMashupResult, String> {
    let a = load_cached(&path_a)?;
    let b = load_cached(&path_b)?;
    if a.sample_rate != b.sample_rate {
        return Err(format!(
            "cele doua piese au rate de esantionare diferite ({} vs {})",
            a.sample_rate, b.sample_rate
        ));
    }

    let bpm_a = bpm_a_override.unwrap_or_else(|| detect_bpm(&a, 60.0, 200.0));
    let bpm_b = bpm_b_override.unwrap_or_else(|| detect_bpm(&b, 60.0, 200.0));
    if bpm_a <= 0.0 || bpm_b <= 0.0 {
        return Err("nu am putut detecta BPM pe una dintre piese; introdu-l manual".to_string());
    }
    let target = target_bpm.unwrap_or(bpm_a);
    let phrase = phrase_beats.unwrap_or(16.0);
    let n_pick = segments_per_track.unwrap_or(3).max(1);

    let pick_top = |buf: &AudioBuffer, bpm: f32| -> Vec<AutoSegmentInfo> {
        let ranges = auto_segments(buf, bpm, phrase);
        let mut scored: Vec<AutoSegmentInfo> = ranges
            .iter()
            .map(|&(s, e)| AutoSegmentInfo { start_secs: s, end_secs: e, energy: segment_rms(buf, (s, e)), chosen: false })
            .collect();
        // alegem top n_pick dupa energie, dar pastram ordinea cronologica in rezultat
        let mut idx_by_energy: Vec<usize> = (0..scored.len()).collect();
        idx_by_energy.sort_by(|&i, &j| scored[j].energy.partial_cmp(&scored[i].energy).unwrap());
        for &i in idx_by_energy.iter().take(n_pick.min(scored.len())) {
            scored[i].chosen = true;
        }
        scored
    };

    let picked_a = pick_top(&a, bpm_a);
    let picked_b = pick_top(&b, bpm_b);

    let mut aligned_segments: Vec<AudioBuffer> = Vec::new();
    let chosen_a: Vec<&AutoSegmentInfo> = picked_a.iter().filter(|s| s.chosen).collect();
    let chosen_b: Vec<&AutoSegmentInfo> = picked_b.iter().filter(|s| s.chosen).collect();
    if chosen_a.is_empty() || chosen_b.is_empty() {
        return Err("piesele sunt prea scurte pentru lungimea de fraza aleasa".to_string());
    }

    let max_len = chosen_a.len().max(chosen_b.len());
    for i in 0..max_len {
        if let Some(seg) = chosen_a.get(i % chosen_a.len()) {
            let extracted = extract_segment(&a, seg.start_secs, seg.end_secs);
            aligned_segments.push(time_stretch(&extracted, target / bpm_a));
        }
        if let Some(seg) = chosen_b.get(i % chosen_b.len()) {
            let extracted = extract_segment(&b, seg.start_secs, seg.end_secs);
            aligned_segments.push(time_stretch(&extracted, target / bpm_b));
        }
    }

    let out = splice_segments(&aligned_segments, seam_crossfade_secs.unwrap_or(0.2));
    let result = write_result(&out, "auto-mashup", target)?;

    Ok(AutoMashupResult {
        out_path: result.out_path,
        bpm_final: result.bpm_final,
        duration_secs: result.duration_secs,
        waveform: result.waveform,
        picked_a,
        picked_b,
    })
}

/// Efecte optionale de remix, aplicate pe o singura piesa.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RemixEffects {
    styles: Option<Vec<String>>,
    custom_description: Option<String>,
    echo_delay_secs: Option<f32>,
    echo_feedback: Option<f32>,
    echo_mix: Option<f32>,
    stutter_chunk_secs: Option<f32>,
    stutter_repeats: Option<usize>,
}

/// Lista de stiluri disponibile (id + nume prietenos), pt. afisare in interfata.
#[tauri::command]
fn list_styles() -> Vec<(String, String)> {
    style_list().into_iter().map(|(id, name)| (id.to_string(), name.to_string())).collect()
}

/// Remix pe o singura piesa: aplica (in ordinea asta) stilurile alese, o descriere text
/// (recunoastere de cuvinte cheie), apoi ecou si/sau stutter, daca sunt cerute.
#[tauri::command]
fn remix_track(path: String, bpm_override: Option<f32>, effects: RemixEffects) -> Result<AudioResult, String> {
    let buf = load_cached(&path)?;
    let bpm = bpm_override.unwrap_or_else(|| detect_bpm(&buf, 60.0, 200.0));

    let mut out = buf;
    if let Some(styles) = effects.styles.as_ref() {
        if !styles.is_empty() {
            out = apply_style_chain(&out, styles, bpm);
        }
    }
    if let Some(desc) = effects.custom_description.as_deref() {
        if !desc.trim().is_empty() {
            out = apply_custom_description(&out, desc, bpm);
        }
    }
    if let Some(delay) = effects.echo_delay_secs {
        out = echo(&out, delay, effects.echo_feedback.unwrap_or(0.35), effects.echo_mix.unwrap_or(0.3));
    }
    if let Some(chunk) = effects.stutter_chunk_secs {
        out = stutter(&out, chunk, effects.stutter_repeats.unwrap_or(2));
    }

    write_result(&out, "remix", bpm)
}

/// Copiaza fisierul deja scris (dintr-un rezultat anterior) la locul ales explicit de utilizator
/// (de obicei printr-un dialog nativ de "Salveaza ca...").
#[tauri::command]
fn export_to(src_path: String, dest_path: String) -> Result<(), String> {
    std::fs::copy(&src_path, &dest_path)
        .map(|_| ())
        .map_err(|e| format!("nu am putut salva fisierul: {e}"))
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
        .invoke_handler(tauri::generate_handler![
            analyze_track,
            mashup_tracks,
            build_segment_mashup,
            auto_mashup,
            remix_track,
            list_styles,
            export_to
        ])
        .run(tauri::generate_context!())
        .expect("eroare la pornirea aplicatiei Tauri");
}
