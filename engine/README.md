# mashup-engine — nucleu DSP, faza 1 (mashup: aliniere BPM + suprapunere)

Acesta e primul strat, testat efectiv (nu doar scris), din motorul de remix/mashup.
E cod Rust pur, fara interfata grafica — un pas inainte de a construi shell-ul Tauri
peste el, ca sa nu construim UI peste un motor care nu functioneaza corect.

## Ce am verificat, cu adevarat, ruland codul (nu doar citindu-l)

Am generat semnale de test sintetice (click-track-uri la BPM cunoscut: 95, 120, 128)
si am rulat efectiv pipeline-ul complet:

- **Decodare audio** (mp3/wav/flac/ogg, via `symphonia`) — functioneaza.
- **Detectare BPM** — testata pe click-track-uri la 95/120/128 BPM: eroare sub 0.5 BPM
  in toate cazurile. Pe muzica reala, cu ritm mai putin regulat decat un click-track,
  precizia va fi mai mica — de asta interfata va trebui sa permita corectie manuala.
- **Aliniere de tempo (durata)** — verificat: dupa "intindere", piesa B cade exact pe
  BPM-ul tintei (ex: 94.8 -> 120.2 dupa aliniere, verificat si invers).
- **Amestecare (mix) si crossfade liniar** — logica simpla, aritmetica directa, fara
  motive sa nu functioneze; nu am gasit probleme la testare.

## Ce NU e inca gata de productie

**Calitatea tonala a time-stretch-ului (WSOLA)** — durata iese corect, dar am testat
si pe un ton pur (sinus 440 Hz) si tonul rezultat are o mica deviatie de frecventa,
constanta indiferent de cat de mult intinzi semnalul. Asta inseamna ca algoritmul de
cautare a fazei (partea care ar trebui sa evite pocnituri/artefacte la imbinarea
bucatilor) are inca o eroare de fond. Pe muzica reala efectul o sa fie mai putin
evident decat pe un ton pur, dar tot o sa se auda ca un usor "warble"/tremur, nu
calitate de studio.

Time-stretch de calitate e o problema DSP genuin grea (nu e o gluma industria are
biblioteci intregi dedicate doar la asta). Recomandarea mea, ca sa "facem bine" cu
adevarat: inlocuim `time_stretch()` din acest fisier cu o biblioteca Rust deja
testata, in loc sa continui s-o perfectionez de la zero. Candidati verificati ca
exista pe crates.io (nu i-am putut compila *in acest mediu*, pentru ca are un Rust
prea vechi instalat din apt — dar pe orice masina cu Rust instalat normal, prin
`rustup`, vor merge fara probema, e nevoie de rustup oricum pentru Tauri):
- `wsola` — WSOLA in Rust pur, fara dependinte C
- `timestretch` — dedicata muzicii electronice (EDM), exact profilul nostru
- `signalsmith-stretch` — cea mai buna calitate (algoritm profesionist folosit si in
  software audio comercial), dar leaga un C++ existent, nu e Rust pur

## Fisiere

- `src/lib.rs` — nucleul: `load_audio`, `detect_bpm`, `time_stretch` (de inlocuit),
  `mix`, `crossfade`, `write_wav`
- `src/bin/gen_test.rs` — genereaza semnale de test (click-track-uri + ton pur),
  utile pentru verificarea oricarei schimbari viitoare la algoritmi
- `src/bin/mashup_cli.rs` — flux complet in linie de comanda: incarca 2 piese,
  detecteaza BPM, aliniaza, amesteca, scrie fisierul rezultat

## Cum rulezi (pe masina ta, cu Rust instalat prin rustup.rs)

```
cargo build --release
./target/release/gen_test testdata          # genereaza semnale de test
./target/release/mashup_cli piesa_a.mp3 piesa_b.mp3 iesire.wav
```

## Pasul urmator

1. Confirmi ca ai (sau instalezi) Rust prin `rustup` pe masina ta — e nevoie oricum
   pentru Tauri, deci facem asta o singura data.
2. Inlocuim `time_stretch` cu o biblioteca testata (`wsola` ca prim candidat, pur
   Rust, fara complicatii de compilare C++).
3. Scheletul aplicatiei Tauri (interfata: incarcare fisiere, forma de unda, grila
   de beat, timeline de aranjare) — peste acest nucleu, odata calitatea audio
   confirmata.
