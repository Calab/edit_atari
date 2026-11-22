//! Read Microsoft BASIC programs, write their listings back and compare
//! each line's stored text with the original.
//!
//!     cargo run -p atari --example ms_retokenize -- FILE...

use atari::msbasic::{MicrosoftBasic, encode};
use retro_core::FileFormat;

/// Each line's number and stored text.
fn lines_of(f: &[u8]) -> Vec<Vec<u8>> {
    let f = if f[0] == 1 {
        let mut v = f.to_vec();
        v[0] = 0;
        v.iter_mut()
            .skip(3)
            .for_each(|b| *b = 0x54u8.wrapping_sub(*b));
        v
    } else {
        f.to_vec()
    };
    let mut v = Vec::new();
    let mut i = 3;
    while f[i] != 0 || f[i + 1] != 0 {
        let end = i + 4 + f[i + 4..].iter().position(|&b| b == 0).unwrap();
        v.push(f[i + 2..end].to_vec());
        i = end + 1;
    }
    v
}

fn main() {
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).unwrap();
        let Ok(d) = MicrosoftBasic.decode(&bytes) else {
            continue;
        };
        let ours = encode(&d.lines).unwrap();
        let (a, b) = (lines_of(&bytes), lines_of(&ours));
        let diffs: Vec<_> = a.iter().zip(&b).filter(|(x, y)| x != y).collect();
        let exact = if ours == bytes {
            ", whole file identical"
        } else {
            ""
        };
        println!("{path}: {} lines, {} differ{exact}", a.len(), diffs.len());
        for (x, y) in diffs.iter().take(3) {
            println!("  orig {:02X?}\n  ours {:02X?}", x, y);
        }
    }
}
