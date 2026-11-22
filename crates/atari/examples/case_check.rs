//! Uppercase an imported listing and print it (for checking case.rs).
//!
//!     cargo run -p atari --example case_check -- FILE.txt
use retro_core::Machine;
fn main() {
    let path = std::env::args().nth(1).expect("FILE");
    let atari = atari::Atari::default();
    let d = atari.import_text(&std::fs::read(path).unwrap());
    let lines = atari
        .case_offer(&d.lines)
        .map(|o| o.lines)
        .unwrap_or(d.lines);
    for l in lines {
        println!("{}", String::from_utf8_lossy(&l));
    }
}
