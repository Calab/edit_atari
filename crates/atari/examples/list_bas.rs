//! List tokenized BASIC files as text, for checking the detokenizer.
//!
//!     cargo run -p atari --example list_bas -- [--as=ab|tb|xl|xe] FILE.BAS...
//!
//! With one file, prints its listing (PC line endings, ATASCII bytes
//! above $7F shown as `{$xx}`). With several, prints one summary line each:
//! the format chosen and each BASIC dialect's misfit score.

use atari::{Atari, basic};
use retro_core::{Machine, format};

fn main() {
    let mut paths: Vec<String> = std::env::args().skip(1).collect();
    // --as=ab|tb|xl|xe reads every file in that dialect.
    let forced: Option<&'static dyn retro_core::FileFormat> = paths
        .iter()
        .position(|a| a.starts_with("--as="))
        .map(|i| match paths.remove(i).trim_start_matches("--as=") {
            "ab" => &basic::ATARI_BASIC_FILE as _,
            "tb" => &basic::TURBO_BASIC_XL_FILE as _,
            "xl" => &basic::BASIC_XL_FILE as _,
            "xe" => &basic::BASIC_XE_FILE as _,
            other => panic!("unknown dialect {other}"),
        });
    let formats = Atari::default().formats();
    for p in &paths {
        let bytes = match std::fs::read(p) {
            Ok(b) => b,
            Err(e) => {
                println!("ERROR  {p}: {e}");
                continue;
            }
        };
        let ext = std::path::Path::new(p).extension().and_then(|e| e.to_str());
        let f = forced.unwrap_or_else(|| format::detect(formats, &bytes, ext));
        let result = f.decode(&bytes);
        if paths.len() > 1 {
            // One line per file: the format chosen, then each BASIC
            // dialect's misfit score ("-" = can't read it).
            let scores: Vec<String> = formats[..formats.len() - 1]
                .iter()
                .map(|f| match f.misfit(&bytes) {
                    Some(m) => format!("{}={m}", f.name()),
                    None => format!("{}=-", f.name()),
                })
                .collect();
            match &result {
                Ok(d) => println!(
                    "{:<16} {:>5} lines  [{}]  {p}",
                    f.name(),
                    d.lines.len(),
                    scores.join(" ")
                ),
                Err(e) => println!("ERROR  {p}: {e}"),
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
