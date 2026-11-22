//! List tokenized BASIC files as text, for checking the detokenizer.
//!
//!     cargo run -p atari --example list_bas -- FILE.BAS...
//!
//! With one file, prints its listing (PC line endings, ATASCII bytes
//! above $7F shown as `{$xx}`). With several, prints one summary line each.

use atari::Atari;
use retro_core::{Machine, format};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let formats = Atari::default().formats();
    for p in &paths {
        let bytes = match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => {
                println!("ERROR  {p}: {e}");
                continue;
            }
        };
        let f = format::detect(formats, &bytes);
        let result = f.decode(&bytes);
        if paths.len() > 1 {
            match &result {
                Ok(d) if format::same(f, formats[0]) => {
                    println!("OK     {} lines  {p}", d.lines.len())
                }
                _ => {
                    // Say why it isn't Atari BASIC.
                    let why = formats[0].decode(&bytes).err().unwrap_or_default();
                    println!("NOT    {p}: {why}");
                }
            }
            continue;
        }
        match result {
            Ok(d) => {
                eprintln!("{}: {}", p, f.name());
                for l in d.lines {
                    let s: String = l
                        .iter()
                        .map(|&b| match b {
                            0x20..=0x7E => (b as char).to_string(),
                            _ => format!("{{${b:02X}}}"),
                        })
                        .collect();
                    println!("{s}");
                }
            }
            Err(e) => println!("ERROR  {p}: {e}"),
        }
    }
}
