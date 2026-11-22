//! Read Atari BASIC programs, tokenize their listings again and compare
//! with the original statement bytes, line by line.
//!
//!     cargo run -p atari --example retokenize -- [-v] [--as=ab|tb|ax|al|xl|xe|ap] FILE.BAS...

use atari::basic::{
    ALTIRRA_BASIC, ALTIRRA_BASIC_FILE, ALTIRRA_EXTENDED_BASIC, ALTIRRA_EXTENDED_BASIC_FILE,
    ATARI_BASIC, ATARI_BASIC_FILE, BASIC_A_PLUS, BASIC_A_PLUS_FILE, BASIC_XE, BASIC_XE_FILE,
    BASIC_XL, BASIC_XL_FILE, TURBO_BASIC_XL, TURBO_BASIC_XL_FILE,
};
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
    let (dialect, file) = match std::env::args().find(|a| a.starts_with("--as=")).as_deref() {
        Some("--as=tb") => (&TURBO_BASIC_XL, &TURBO_BASIC_XL_FILE),
        Some("--as=ax") => (&ALTIRRA_EXTENDED_BASIC, &ALTIRRA_EXTENDED_BASIC_FILE),
        Some("--as=al") => (&ALTIRRA_BASIC, &ALTIRRA_BASIC_FILE),
        Some("--as=xl") => (&BASIC_XL, &BASIC_XL_FILE),
        Some("--as=xe") => (&BASIC_XE, &BASIC_XE_FILE),
        Some("--as=ap") => (&BASIC_A_PLUS, &BASIC_A_PLUS_FILE),
        _ => (&ATARI_BASIC, &ATARI_BASIC_FILE),
    };
    let (mut files_ok, mut total_lines, mut bad_lines) = (0, 0, 0);
    for path in std::env::args()
        .skip(1)
        .filter(|a| a != "-v" && !a.starts_with("--as="))
    {
        let bytes = std::fs::read(&path).unwrap();
        let Ok(d) = file.decode(&bytes) else {
            continue;
        };
        let (names, orig) = program_parts(&bytes).unwrap();
        let p = match tokenize_with_variables(dialect, &d.lines, names) {
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
