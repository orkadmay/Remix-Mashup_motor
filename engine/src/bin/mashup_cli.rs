use mashup_engine::{detect_bpm, load_audio, mix, time_stretch, write_wav};
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Folosire: mashup_cli <piesa_a.wav> <piesa_b.wav> [iesire.wav]");
        std::process::exit(1);
    }
    let path_a = Path::new(&args[1]);
    let path_b = Path::new(&args[2]);
    let out_path = args.get(3).cloned().unwrap_or_else(|| "/home/claude/mashup-engine/testdata/mashup_out.wav".to_string());

    println!("Incarc {}...", path_a.display());
    let a = load_audio(path_a).expect("nu am putut incarca piesa A");
    println!("Incarc {}...", path_b.display());
    let b = load_audio(path_b).expect("nu am putut incarca piesa B");

    let bpm_a = detect_bpm(&a, 70.0, 180.0);
    let bpm_b = detect_bpm(&b, 70.0, 180.0);
    println!("BPM detectat A: {:.1}", bpm_a);
    println!("BPM detectat B: {:.1}", bpm_b);

    // aliniem B la tempo-ul lui A: speed>1 scurteaza (grabeste), speed<1 lungeste (incetineste)
    let speed = if bpm_a > 0.0 && bpm_b > 0.0 { bpm_a / bpm_b } else { 1.0 };
    println!("Intind piesa B cu factor de viteza {:.4} ca sa se alinieze la BPM-ul lui A...", speed);
    let b_aligned = time_stretch(&b, speed);

    let bpm_b_after = detect_bpm(&b_aligned, 70.0, 180.0);
    println!("BPM B dupa aliniere: {:.1} (tinta: {:.1})", bpm_b_after, bpm_a);

    let out = mix(&a, 0.6, &b_aligned, 0.6);
    write_wav(Path::new(&out_path), &out).expect("nu am putut scrie fisierul de iesire");
    println!("Scris: {out_path}");
}
