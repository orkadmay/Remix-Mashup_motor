# mashup-engine — nucleu DSP

Nucleul in Rust din spatele motorului de remix/mashup. Cod pur, fara interfata
grafica — folosit de aplicatia Tauri din `../app`.

## Istoric de calitate (ce am gasit si reparat, nu doar ce am scris)

- **Time-stretch (aliniere de tempo, pastrand tonul)** — prima versiune era scrisa
  de mine de la zero si avea o problema reala, confirmata prin test (un ton pur
  iesea cu o mica deviatie constanta de frecventa - semn ca algoritmul de cautare
  a fazei avea o eroare de fond). Inlocuita cu biblioteca `wsola` (Rust, testata
  separat de comunitate), in loc sa continui s-o depanez singur.
- **Sidechain pump (pompare ritmica, folosita de majoritatea stilurilor de remix)** —
  prima versiune avea un bug real: volumul sarea brusc, instantaneu, la fiecare
  bataie, ceea ce se aude ca un tacanit/clic peste tot in piesa — nu era stilizare,
  era un defect de semnal. Testat si confirmat: inainte de reparatie, saltul intre
  2 esantioane consecutive ajungea la ~0.6 (foarte audibil); dupa reparatie
  (o forma neteda de cosinus, fara "resetare" bruscă), saltul maxim e ~0.0001 —
  practic inaudibil ca discontinuitate.
- **Normalizare de siguranta la export** — unele combinatii de efecte (mai ales
  ecou cu feedback mare) puteau impinge volumul peste limita, ceea ce ar fi produs
  clipping/distorsiune la scriere. Acum orice rezultat e verificat si, daca e
  nevoie, scazut proportional inainte de scriere.

## Ce am verificat efectiv, rulan cod (nu doar citind)

- Decodare audio (mp3/wav/flac/ogg) — functioneaza.
- Detectare BPM — testata pe click-track-uri la 95/120/128 BPM, eroare sub 0.5 BPM.
- Extragere de bucati + lipire (splice) cu crossfade la imbinari.
- Efecte: ecou, stutter, filtre trece-jos/trece-sus, distorsiune, sidechain pump,
  17 stiluri predefinite (combinatii ale efectelor de mai sus) — toate testate ca
  nu produc NaN/valori infinite si ca raman in limite rezonabile de volum.
- Auto-segmentare + scor de energie (RMS) pe bucati — verificat ca distinge corect
  portiunile "tari" de cele "line" ale unei piese (testat cu semnal sintetic cu
  energie alternanta cunoscuta).

## Ce NU am putut testa direct, in acest mediu

Inlocuirea time-stretch-ului cu `wsola` nu a putut fi compilata *in sandbox-ul meu*
(are un Rust instalat prin `apt`, prea vechi pentru cerintele acestei biblioteci -
vezi si nota din `../README.md`). Am verificat API-ul exact citind sursa publicata
a bibliotecii (docs.rs), dar validarea finala (sunet curat, fara artefacte) se
intampla abia la prima rulare reala, pe calculatorul tau, prin build-ul din GitHub
Actions (care are un Rust modern).

## Fisiere

- `src/lib.rs` — nucleul: `load_audio`, `detect_bpm`, `time_stretch`, `mix`,
  `crossfade`, `extract_segment`, `splice_segments`, `echo`, `stutter`, `lowpass`,
  `highpass`, `distortion`, `sidechain_pump`, `apply_style` (+ `style_list`,
  `apply_style_chain`), `apply_custom_description`, `auto_segments`,
  `segment_rms`, `normalize_peak`, `write_wav`
- `src/bin/gen_test.rs` — genereaza semnale de test (click-track-uri + ton pur)
- `src/bin/mashup_cli.rs` — flux complet in linie de comanda (fara interfata)

## Cum rulezi (pe o masina cu Rust instalat prin rustup.rs)

```
cargo build --release
./target/release/gen_test testdata
./target/release/mashup_cli piesa_a.mp3 piesa_b.mp3 iesire.wav
```
