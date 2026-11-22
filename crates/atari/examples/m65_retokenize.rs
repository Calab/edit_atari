//! Read MAC/65 sources, write their listings back and compare with the
//! originals, line by line.
//!
//!     cargo run -p atari --example m65_retokenize -- [-v] FILE...

use atari::mac65::{Mac65, encode};
use retro_core::FileFormat;

/// (line number, line bytes) for each line of a file.
fn lines_of(f: &[u8]) -> Vec<(u16, Vec<u8>)> {
    let mut v = Vec::new();
    let mut i = 4;
    while i + 3 <= f.len() {
        let len = f[i + 2] as usize;
        v.push((u16::from_le_bytes([f[i], f[i + 1]]), f[i..i + len].to_vec()));
        i += len.max(1);
    }
    v
}

fn main() {
    let verbose = std::env::args().any(|a| a == "-v");
    let (mut identical, mut total, mut bad) = (0, 0, 0);
    for path in std::env::args().skip(1).filter(|a| a != "-v") {
        let bytes = std::fs::read(&path).unwrap();
        let Ok(d) = Mac65.decode(&bytes) else {
            continue;
        };
        let (ours, errors) = encode(&d.lines).unwrap();
        let (a, b) = (lines_of(&bytes), lines_of(&ours));
        let mut diffs = 0;
        for ((n, x), (_, y)) in a.iter().zip(&b) {
            total += 1;
            if x != y {
                diffs += 1;
                if verbose && diffs <= 4 {
                    let l = d.lines.iter().find(|l| {
                        let digits: Vec<u8> = l
                            .iter()
                            .take_while(|c| c.is_ascii_digit())
                            .copied()
                            .collect();
                        std::str::from_utf8(&digits)
                            .ok()
                            .and_then(|s| s.parse::<u16>().ok())
                            == Some(*n)
                    });
                    println!("  {}", String::from_utf8_lossy(l.unwrap()));
                    println!("    orig {:02X?}\n    ours {:02X?}", &x[3..], &y[3..]);
                }
            }
        }
        bad += diffs;
        if ours == bytes {
            identical += 1;
        } else {
            println!(
                "DIFF {path}: {diffs} of {} lines; errors {errors:?}",
                a.len()
            );
        }
    }
    println!("{identical} files identical; {bad} of {total} lines differ");
}
