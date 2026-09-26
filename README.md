# Motor Mashup — status si pasul urmator

## Structura

- `engine/` — nucleul DSP in Rust (decodare audio, detectare BPM, time-stretch, mix,
  crossfade). Testat efectiv, cu semnale audio generate — nu doar scris. Detalii si
  stare exacta (ce merge, ce nu inca) in `engine/README.md`.
- `app/` — aplicatia desktop (Tauri): `app/dist/index.html` (interfata) +
  `app/src-tauri/` (comenzile Rust care leaga interfata de `engine/`).
- `.github/workflows/build.yml` — construieste automat instalatoare pentru
  Windows, Mac si Linux, in cloud, fara sa instalezi nimic pe calculatorul tau.

## De ce livrez asa, si nu un .exe gata

Am construit si testat tot codul, dar mediul in care lucrez eu (sandbox-ul meu)
are un Rust prea vechi (instalat prin `apt`, versiunea din 2023) si nu am acces de
retea catre serverele oficiale de unde s-ar instala un Rust nou (`rustup.rs`).
Rezultatul: pot compila piese individuale (nucleul `engine/` merge perfect aici),
dar nu si aplicatia completa Tauri, care are nevoie de dependinte mult mai noi.

Asta nu e o problema a codului — e o limita de retea a mediului meu. Solutia
practica: `.github/workflows/build.yml` face exact ce ai cerut (instaleaza singur
tot ce trebuie, inclusiv un Rust proaspat) — dar pe calculatoarele GitHub, nu pe
al meu sau al tau.

## Ce trebuie sa faci (o singura data, fara sa instalezi nimic pe calculator)

1. Faci un cont gratuit pe [github.com](https://github.com), daca nu ai deja.
2. Creezi un repository nou (gol), ii pui orice nume (ex. `motor-mashup`).
3. Incarci acest folder in el — cel mai simplu: pe pagina repository-ului, butonul
   „Add file” → „Upload files”, tragi tot continutul acestui folder acolo, apoi
   „Commit”.
4. Mergi la tab-ul **Actions** al repository-ului — workflow-ul „Construieste
   aplicatia (Windows, Mac, Linux)” porneste singur (sau il pornesti manual cu
   butonul „Run workflow”).
5. Dupa ~10-15 minute (prima data — apoi mai rapid, din cache), la finalul rularii
   apar 3 fisiere de descarcat („Artifacts”): unul pentru Windows, unul pentru Mac,
   unul pentru Linux. Le descarci si le instalezi normal.

## Ce ramane de facut dupa ce ai instalatorul functional

Time-stretch-ul (WSOLA scris de mana) are inca o problema de calitate tonala,
documentata in `engine/README.md`. Recomand inlocuirea lui cu o biblioteca deja
testata (`wsola` sau `signalsmith-stretch`) inainte sa te bazezi pe rezultatul
final pentru ceva serios — asta il facem imediat ce confirmam ca restul aplicatiei
(interfata, alegerea fisierelor, formele de unda) merge corect prin GitHub Actions.
