//! Print a file to a PDF (for checking `retro_core::print`).
//!
//!     cargo run -p atari --example print_pdf -- FILE OUT.pdf [SIZE] [40]
use retro_core::print::{Paper, PrintSettings, pdf};
use retro_core::{Machine, format};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let atari = atari::Atari::default();
    let bytes = std::fs::read(&args[1]).unwrap();
    let ext = std::path::Path::new(&args[1])
        .extension()
        .and_then(|e| e.to_str());
    let f = format::detect(atari.formats(), &bytes, ext);
    let lines = f.decode(&bytes).unwrap().lines;
    let s = PrintSettings {
        paper: Paper::Letter,
        columns: (args.get(4).map(String::as_str) == Some("40")).then_some(40),
        size: args.get(3).and_then(|a| a.parse().ok()).unwrap_or(10.0),
        header: true,
    };
    let name = std::path::Path::new(&args[1])
        .file_name()
        .unwrap()
        .to_string_lossy();
    std::fs::write(&args[2], pdf(&atari, &lines, &name, "Edit Atari", &s)).unwrap();
}
