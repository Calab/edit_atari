//! Real programs from `samples/`, one folder per language.
//!
//! The samples are other people's programs and are kept out of git, so
//! these tests pass (with a note) when a folder is missing, as in CI.

use std::path::PathBuf;

use atari::basic::{
    ATARI_BASIC_FILE, BASIC_A_PLUS_FILE, BASIC_XE_FILE, BASIC_XL_FILE, TURBO_BASIC_XL_FILE,
};
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

/// The programs in a folder: everything except `.lst` files, which are
/// reference listings (`PROG.lst` is the machine's own LIST of `PROG.bas`).
fn programs(dir: &str) -> Vec<PathBuf> {
    samples(dir)
        .into_iter()
        .filter(|p| !is_listing(p))
        .collect()
}

fn is_listing(p: &std::path::Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lst"))
}

/// Each reference listing must match our listing of its program exactly.
fn check_listings(dir: &str, format: &dyn FileFormat) {
    for lst in samples(dir).into_iter().filter(|p| is_listing(p)) {
        let stem = lst.file_stem().unwrap().to_string_lossy().to_lowercase();
        // The program with the same name that this format reads (there may
        // also be a plain-text version, such as SYSEQU.ASM).
        let Some(prog) = programs(dir).into_iter().find(|p| {
            p.file_stem().unwrap().to_string_lossy().to_lowercase() == stem
                && format.detect(&std::fs::read(p).unwrap())
        }) else {
            panic!("{}: no program with the same name", lst.display());
        };
        let d = format.decode(&std::fs::read(&prog).unwrap()).unwrap();
        let bytes = std::fs::read(&lst).unwrap();
        let body = bytes.strip_suffix(&[0x9B]).unwrap_or(&bytes);
        let want: Vec<&[u8]> = body.split(|&c| c == 0x9B).collect();
        assert_eq!(d.lines.len(), want.len(), "{}: line count", prog.display());
        for (got, want) in d.lines.iter().zip(want) {
            assert_eq!(
                String::from_utf8_lossy(got),
                String::from_utf8_lossy(want),
                "{}",
                prog.display()
            );
        }
    }
}

#[test]
fn atari_basic_programs_list() {
    for p in programs("Atari BASIC") {
        let bytes = std::fs::read(&p).unwrap();
        if let Err(e) = ATARI_BASIC_FILE.decode(&bytes) {
            panic!("{}: {e}", p.display());
        }
        assert!(ATARI_BASIC_FILE.detect(&bytes), "{}", p.display());
    }
}

#[test]
fn damaged_files_are_not_claimed() {
    for p in programs("Atari BASIC/damaged") {
        let bytes = std::fs::read(&p).unwrap();
        assert!(!ATARI_BASIC_FILE.detect(&bytes), "{}", p.display());
        assert!(!TURBO_BASIC_XL_FILE.detect(&bytes), "{}", p.display());
    }
}

#[test]
fn turbo_basic_xl_programs_list() {
    for p in programs("Turbo BASIC XL") {
        let bytes = std::fs::read(&p).unwrap();
        if let Err(e) = TURBO_BASIC_XL_FILE.decode(&bytes) {
            panic!("{}: {e}", p.display());
        }
        // They use Turbo-BASIC tokens, so aren't read as Atari BASIC.
        assert!(!ATARI_BASIC_FILE.detect(&bytes), "{}", p.display());
    }
}

#[test]
fn atari_basic_programs_are_also_turbo_basic() {
    // Turbo-BASIC XL is a superset, so it can read them too; Atari BASIC
    // comes first in the format list so they're named correctly.
    for p in programs("Atari BASIC") {
        let bytes = std::fs::read(&p).unwrap();
        assert!(TURBO_BASIC_XL_FILE.detect(&bytes), "{}", p.display());
    }
}

#[test]
fn reference_listings_match() {
    check_listings("Atari BASIC", &ATARI_BASIC_FILE);
    check_listings("Turbo BASIC XL", &TURBO_BASIC_XL_FILE);
    check_listings("OSS BASIC XL-XE/BASIC XL", &BASIC_XL_FILE);
    check_listings("OSS BASIC XL-XE/BASIC XE", &BASIC_XE_FILE);
    check_listings("OSS BASIC A+", &BASIC_A_PLUS_FILE);
}

#[test]
fn oss_basic_programs_list() {
    for (dir, format) in [
        ("OSS BASIC XL-XE/BASIC XL", &BASIC_XL_FILE),
        ("OSS BASIC XL-XE/BASIC XE", &BASIC_XE_FILE),
        ("OSS BASIC A+", &BASIC_A_PLUS_FILE),
    ] {
        for p in programs(dir) {
            let bytes = std::fs::read(&p).unwrap();
            if let Err(e) = format.decode(&bytes) {
                panic!("{}: {e}", p.display());
            }
        }
    }
}

/// Each sample reads as its own dialect even without its extension (as
/// if it were called .BAS), so detection works on content alone.
#[test]
fn dialect_detected_from_content() {
    use retro_core::{Machine, format};
    let formats = atari::Atari::default().formats();
    for (dir, want) in [
        ("Atari BASIC", "Atari BASIC"),
        ("Turbo BASIC XL", "Turbo-BASIC XL"),
        ("OSS BASIC XL-XE/BASIC XL", "BASIC XL"),
        ("OSS BASIC XL-XE/BASIC XE", "BASIC XE"),
        ("OSS BASIC A+", "BASIC A+"),
    ] {
        for p in programs(dir) {
            let bytes = std::fs::read(&p).unwrap();
            let got = format::detect(formats, &bytes, None).name();
            assert_eq!(got, want, "{}", p.display());
        }
    }
}

/// MAC/65 sources (`.M65`, `.LIB` and the Toolkit's `.DEM`; not the
/// plain-text `.ASM`) read as MAC/65.
#[test]
fn mac65_sources_list() {
    use retro_core::{Machine, format};
    let formats = atari::Atari::default().formats();
    for p in programs("MAC-65") {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("asm") {
            continue;
        }
        let bytes = std::fs::read(&p).unwrap();
        if let Err(e) = atari::mac65::Mac65.decode(&bytes) {
            panic!("{}: {e}", p.display());
        }
        assert_eq!(
            format::detect(formats, &bytes, None).name(),
            "MAC/65",
            "{}",
            p.display()
        );
    }
    check_listings("MAC-65", &atari::mac65::Mac65);
}
