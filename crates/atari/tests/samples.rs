//! Real programs from `samples/`, one folder per language.
//!
//! The samples are other people's programs and are kept out of git, so
//! these tests pass (with a note) when a folder is missing, as in CI.

use std::path::PathBuf;

use atari::basic::ATARI_BASIC_FILE;
use retro_core::FileFormat;

fn samples(dir: &str) -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(dir);
    let Ok(entries) = std::fs::read_dir(&root) else {
        eprintln!("skipping: no samples in {}", root.display());
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = entries
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_file())
        .collect();
    v.sort();
    v
}

#[test]
fn atari_basic_programs_list() {
    for p in samples("Atari BASIC") {
        let bytes = std::fs::read(&p).unwrap();
        if let Err(e) = ATARI_BASIC_FILE.decode(&bytes) {
            panic!("{}: {e}", p.display());
        }
        assert!(ATARI_BASIC_FILE.detect(&bytes), "{}", p.display());
    }
}

#[test]
fn damaged_files_are_not_claimed() {
    for p in samples("Atari BASIC/damaged") {
        let bytes = std::fs::read(&p).unwrap();
        assert!(!ATARI_BASIC_FILE.detect(&bytes), "{}", p.display());
    }
}

#[test]
fn other_dialects_are_not_atari_basic() {
    for p in samples("Turbo BASIC XL") {
        let bytes = std::fs::read(&p).unwrap();
        assert!(!ATARI_BASIC_FILE.detect(&bytes), "{}", p.display());
    }
}
