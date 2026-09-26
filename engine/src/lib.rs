//! mashup-engine: nucleu DSP pentru motorul de remix/mashup.
//! Decodare audio, detectare BPM, time-stretch (WSOLA) fara schimbarea tonului, export WAV.

use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Buffer audio mono, in memorie, la o rata de esantionare data.
#[derive(Clone)]
pub struct AudioBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl AudioBuffer {
    pub fn duration_secs(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate as f32
    }
}

/// Incarca un fisier audio (mp3/wav/flac/ogg) si il converteste la mono f32.
pub fn load_audio(path: &Path) -> Result<AudioBuffer, String> {
    let file = File::open(path).map_err(|e| format!("nu pot deschide fisierul: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("format audio nerecunoscut: {e}"))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("nu am gasit nicio pista audio")?
        .clone();

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("nu pot initializa decodorul: {e}"))?;

    let track_id = track.id;
    let mut mono: Vec<f32> = Vec::new();
    let mut sample_rate: u32 = track.codec_params.sample_rate.unwrap_or(44100);

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(_)) => break, // sfarsit de flux
            Err(e) => return Err(format!("eroare la citirea pachetului: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                sample_rate = spec.rate;
                let channels = spec.channels.count().max(1);
                let mut sbuf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                sbuf.copy_interleaved_ref(decoded);
                let interleaved = sbuf.samples();
                // mixdown la mono: media pe canale
                for frame in interleaved.chunks(channels) {
                    let sum: f32 = frame.iter().sum();
                    mono.push(sum / channels as f32);
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue, // sarim pachetul stricat
            Err(e) => return Err(format!("eroare la decodare: {e}")),
        }
    }

    if mono.is_empty() {
        return Err("nu s-a decodat niciun esantion audio".to_string());
    }

    Ok(AudioBuffer { samples: mono, sample_rate })
}

/// Scrie un buffer mono ca fisier WAV 16-bit.
pub fn write_wav(path: &Path, buf: &AudioBuffer) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: buf.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in &buf.samples {
        let clamped = s.clamp(-1.0, 1.0);
        let v = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(v).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;
    Ok(())
}

/// Detecteaza BPM-ul dominant intr-un interval [min_bpm, max_bpm].
/// Algoritm: anvelopa de energie (onset strength) -> autocorelatie pe anvelopa.
pub fn detect_bpm(buf: &AudioBuffer, min_bpm: f32, max_bpm: f32) -> f32 {
    let sr = buf.sample_rate as f32;

    // 1. Anvelopa de energie in ferestre mici (fara FFT: suma de patrate), hop mic pt. rezolutie temporala buna.
    let win = 1024usize;
    let hop = 256usize;
    let mut envelope: Vec<f32> = Vec::new();
    let mut i = 0;
    while i + win <= buf.samples.len() {
        let energy: f32 = buf.samples[i..i + win].iter().map(|x| x * x).sum();
        envelope.push(energy.sqrt());
        i += hop;
    }
    if envelope.len() < 4 {
        return 0.0;
    }

    // 2. Onset strength = flux pozitiv (diferenta fata de esantionul anterior, doar cresteri).
    let mut flux: Vec<f32> = vec![0.0; envelope.len()];
    for k in 1..envelope.len() {
        let d = envelope[k] - envelope[k - 1];
        flux[k] = if d > 0.0 { d } else { 0.0 };
    }

    // 3. Netezire usoara (medie mobila pe 3) ca sa reducem zgomotul.
    let mut smooth = flux.clone();
    for k in 1..flux.len() - 1 {
        smooth[k] = (flux[k - 1] + flux[k] + flux[k + 1]) / 3.0;
    }

    // 4. Autocorelatie pe anvelopa de onset, in intervalul de lag corespunzator [min_bpm, max_bpm].
    let frame_rate = sr / hop as f32; // cadre de anvelopa pe secunda
    let min_lag = (frame_rate * 60.0 / max_bpm).round() as usize;
    let max_lag = (frame_rate * 60.0 / min_bpm).round() as usize;
    let max_lag = max_lag.min(smooth.len().saturating_sub(1));

    let mut best_lag = min_lag.max(1);
    let mut best_score = f32::MIN;
    for lag in min_lag.max(1)..=max_lag {
        let mut score = 0.0f32;
        let n = smooth.len() - lag;
        if n == 0 {
            continue;
        }
        for k in 0..n {
            score += smooth[k] * smooth[k + lag];
        }
        score /= n as f32;
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }

    if best_lag == 0 {
        return 0.0;
    }
    frame_rate * 60.0 / best_lag as f32
}

/// Time-stretch WSOLA: schimba durata (deci tempo-ul) fara sa schimbe tonul.
/// `speed` > 1.0 = mai rapid (mai scurt), `speed` < 1.0 = mai lent (mai lung).
///
/// STARE ACTUALA (verificat): durata rezultata e corecta (testat pe click-track-uri:
/// BPM-ul dupa aliniere cade exact pe tinta). Calitatea tonala insa NU e inca la nivel
/// de productie: testat pe un ton pur, tonul iese cu o mica deviatie de frecventa
/// (dependenta de fereastra/hop aleasa, nu de `speed`), semn ca alegerea offsetului de
/// analiza (cautarea de faza) are inca o eroare de fond. Pe muzica reala (nu tonuri pure)
/// efectul e mai putin evident, dar tot va suna cu artefacte usoare ("warble").
/// Recomandare: de inlocuit cu o biblioteca Rust testata (`wsola`, `signalsmith-stretch`
/// sau `timestretch`) inainte de a construi peste asta functionalitati de productie —
/// vezi nota din README.
pub fn time_stretch(buf: &AudioBuffer, speed: f32) -> AudioBuffer {
    if (speed - 1.0).abs() < 1e-4 || buf.samples.is_empty() {
        return buf.clone();
    }

    let input = &buf.samples;
    let n_in = input.len();

    let frame_size: usize = 2048;
    let synthesis_hop: usize = frame_size / 4; // 75% overlap la sinteza
    let analysis_hop_nominal = (synthesis_hop as f32 * speed).round() as usize;
    let analysis_hop_nominal = analysis_hop_nominal.max(1);
    let search_radius: usize = synthesis_hop / 2;

    // fereastra Hann
    let window: Vec<f32> = (0..frame_size)
        .map(|i| 0.5 - 0.5 * ((2.0 * std::f32::consts::PI * i as f32) / (frame_size as f32 - 1.0)).cos())
        .collect();

    let out_len_estimate = ((n_in as f32) / speed) as usize + frame_size + 8;
    let mut out = vec![0.0f32; out_len_estimate];
    let mut norm = vec![0.0f32; out_len_estimate];

    let mut analysis_pos: i64 = 0;
    let mut synth_pos: usize = 0;
    let mut prev_frame_tail: Vec<f32> = vec![0.0; search_radius.max(1)];
    let mut have_prev = false;

    loop {
        // pozitia ideala de analiza pentru acest cadru de sinteza
        let ideal = analysis_pos.max(0) as usize;
        if ideal >= n_in {
            break;
        }

        // cautam in +/- search_radius jurul pozitiei ideale cel mai bun offset
        // prin corelare cu coada cadrului anterior (continuitate de faza).
        let mut best_offset: i64 = 0;
        if have_prev {
            let mut best_score = f32::MIN;
            let lo = -(search_radius.min(ideal) as i64);
            let hi = search_radius as i64;
            // normalizam scorul (corelatie normalizata) si preferam, la egalitate,
            // decalajul cel mai apropiat de pozitia ideala (0) - evita "agatarea"
            // pe un multiplu de perioada la semnale foarte periodice.
            let mut off = lo;
            while off <= hi {
                let start = ideal as i64 + off;
                if start < 0 || (start as usize) + search_radius > n_in {
                    off += 1;
                    continue;
                }
                let start = start as usize;
                let mut dot = 0.0f32;
                let mut energy = 0.0f32;
                for k in 0..search_radius {
                    dot += prev_frame_tail[k] * input[start + k];
                    energy += input[start + k] * input[start + k];
                }
                let score = dot / (energy.sqrt() + 1e-6);
                let better = score > best_score + 1e-4
                    || (score > best_score - 1e-4 && off.abs() < best_offset.abs());
                if better {
                    best_score = score.max(best_score);
                    best_offset = off;
                }
                off += 1;
            }
        }

        let frame_start = (ideal as i64 + best_offset).max(0) as usize;
        if frame_start + frame_size > n_in {
            break;
        }

        // aplicam fereastra si adaugam (overlap-add) in iesire
        for k in 0..frame_size {
            let s = input[frame_start + k] * window[k];
            let out_idx = synth_pos + k;
            if out_idx >= out.len() {
                out.resize(out_idx + frame_size, 0.0);
                norm.resize(out_idx + frame_size, 0.0);
            }
            out[out_idx] += s;
            norm[out_idx] += window[k] * window[k];
        }

        // memoram coada acestui cadru (in pozitia finala reala) pentru urmatoarea corelare
        let tail_start = frame_start + frame_size.saturating_sub(search_radius);
        for k in 0..search_radius {
            let idx = tail_start + k;
            prev_frame_tail[k] = if idx < n_in { input[idx] } else { 0.0 };
        }
        have_prev = true;

        synth_pos += synthesis_hop;
        analysis_pos += analysis_hop_nominal as i64;
    }

    // normalizare (overlap-add impartit la suma ferestrelor la patrat)
    for i in 0..out.len() {
        if norm[i] > 1e-6 {
            out[i] /= norm[i].sqrt().max(1e-6);
        }
    }
    out.truncate(synth_pos.min(out.len()));

    AudioBuffer { samples: out, sample_rate: buf.sample_rate }
}

/// Reduce un buffer la N puncte (varf de amplitudine per bucata), pt. desenat forma de unda in UI.
pub fn downsample_peaks(buf: &AudioBuffer, points: usize) -> Vec<f32> {
    if points == 0 || buf.samples.is_empty() {
        return Vec::new();
    }
    let chunk = (buf.samples.len() as f32 / points as f32).ceil() as usize;
    let chunk = chunk.max(1);
    buf.samples
        .chunks(chunk)
        .map(|c| c.iter().fold(0.0f32, |m, &x| m.max(x.abs())))
        .collect()
}

/// Amesteca doua buffere (aceeasi rata de esantionare) cu volume date, pana la lungimea celui mai lung.
pub fn mix(a: &AudioBuffer, gain_a: f32, b: &AudioBuffer, gain_b: f32) -> AudioBuffer {
    assert_eq!(a.sample_rate, b.sample_rate, "ratele de esantionare trebuie sa fie egale (aliniaza mai intai)");
    let len = a.samples.len().max(b.samples.len());
    let mut out = vec![0.0f32; len];
    for i in 0..len {
        let sa = a.samples.get(i).copied().unwrap_or(0.0) * gain_a;
        let sb = b.samples.get(i).copied().unwrap_or(0.0) * gain_b;
        out[i] = sa + sb;
    }
    AudioBuffer { samples: out, sample_rate: a.sample_rate }
}

/// Crossfade linear intre a (spre final) si b (spre inceput), pe o durata data, incepand la offset_a in `a`.
pub fn crossfade(a: &AudioBuffer, b: &AudioBuffer, fade_secs: f32) -> AudioBuffer {
    assert_eq!(a.sample_rate, b.sample_rate);
    let sr = a.sample_rate;
    let fade_len = (fade_secs * sr as f32) as usize;
    let fade_len = fade_len.min(a.samples.len()).min(b.samples.len());

    let mut out = Vec::with_capacity(a.samples.len() + b.samples.len().saturating_sub(fade_len));
    out.extend_from_slice(&a.samples[..a.samples.len() - fade_len]);

    for k in 0..fade_len {
        let t = k as f32 / fade_len as f32;
        let sa = a.samples[a.samples.len() - fade_len + k] * (1.0 - t);
        let sb = b.samples[k] * t;
        out.push(sa + sb);
    }
    out.extend_from_slice(&b.samples[fade_len..]);

    AudioBuffer { samples: out, sample_rate: sr }
}
