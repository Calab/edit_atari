//! Read Atari BASIC programs, tokenize their listings again and compare
//! with the original statement bytes, line by line.
//!
//!     cargo run -p atari --example retokenize -- FILE.BAS...

use atari::basic::{ATARI_BASIC, ATARI_BASIC_FILE};
use atari::tokenize::{program_parts, tokenize_with_variables};
use retro_core::FileFormat;

/// Split statement bytes into (line number, line bytes).
fn lines(p: &[u8]) -> Vec<(u16, &[u8])> {
    let mut v = Vec::new();
    let mut i = 0;
    while i + 3 <= p.len() {
        let len = p[i + 2] as usize;
        v.push((u16::from_le_bytes([p[i], p[i + 1]]), &p[i..i + len]));
        i += len.max(1);
    }
    v
}

fn main() {
    let verbose = std::env::args().any(|a| a == "-v");
    let (mut files_ok, mut total_lines, mut bad_lines) = (0, 0, 0);
    for path in std::env::args().skip(1).filter(|a| a != "-v") {
        let bytes = std::fs::read(&path).unwrap();
        let Ok(d) = ATARI_BASIC_FILE.decode(&bytes) else {
            continue;
        };
        let (names, orig) = program_parts(&bytes).unwrap();
        let p = match tokenize_with_variables(&ATARI_BASIC, &d.lines, names) {
            Ok(p) => p,
            Err(e) => {
                println!("ERR  {path}: {e}");
                continue;
            }
        };
        let (_, ours) = program_parts(&p.bytes).unwrap();
        let (a, b) = (lines(&orig), lines(&ours));
        let mut diffs = 0;
        for ((n, x), (_, y)) in a.iter().zip(&b) {
            total_lines += 1;
            if x != y {
                diffs += 1;
                if verbose && diffs <= 3 {
                    let listing = d
                        .lines
                        .iter()
                        .find(|l| l.starts_with(format!("{n} ").as_bytes()));
                    println!("  line {n}: {}", String::from_utf8_lossy(listing.unwrap()));
                    println!("    orig {:02X?}", x);
                    println!("    ours {:02X?}", y);
                }
            }
        }
        bad_lines += diffs;
        if diffs == 0 && a.len() == b.len() {
            files_ok += 1;
        } else {
            println!(
                "DIFF {path}: {diffs} of {} lines (lines {} vs {})",
                a.len(),
                a.len(),
                b.len()
            );
        }
    }
    println!("{files_ok} files identical; {bad_lines} of {total_lines} lines differ");
}
