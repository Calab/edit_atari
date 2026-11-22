//! ATASCII text files: lines terminated by EOL ($9B).
//!
//! Atari BASIC `LIST "D:FILE.LST"` output, Action! source, Assembler Editor
//! source, etc. are all plain ATASCII in this format.

use retro_core::Decoded;

/// End of line.
pub const EOL: u8 = 0x9B;
pub const ESC: u8 = 0x1B;
pub const CURSOR_UP: u8 = 0x1C;
pub const CURSOR_DOWN: u8 = 0x1D;
pub const CURSOR_LEFT: u8 = 0x1E;
pub const CURSOR_RIGHT: u8 = 0x1F;
pub const CLEAR: u8 = 0x7D;
pub const BACKSPACE: u8 = 0x7E;
pub const TAB: u8 = 0x7F;
pub const DELETE_LINE: u8 = 0x9C;
pub const INSERT_LINE: u8 = 0x9D;
pub const CLEAR_TAB: u8 = 0x9E;
pub const SET_TAB: u8 = 0x9F;
pub const BELL: u8 = 0xFD;
pub const DELETE_CHAR: u8 = 0xFE;
pub const INSERT_CHAR: u8 = 0xFF;

/// The XL/XE international character set: the codes whose glyphs differ
/// from standard ATASCII, and the character each one shows. (Their inverse
/// versions, code + $80, differ too.)
pub const INTERNATIONAL: [(u8, char); 29] = [
    (0x00, 'á'),
    (0x01, 'ù'),
    (0x02, 'Ñ'),
    (0x03, 'É'),
    (0x04, 'ç'),
    (0x05, 'ô'),
    (0x06, 'ò'),
    (0x07, 'ì'),
    (0x08, '£'),
    (0x09, 'ï'),
    (0x0A, 'ü'),
    (0x0B, 'ä'),
    (0x0C, 'Ö'),
    (0x0D, 'ú'),
    (0x0E, 'ó'),
    (0x0F, 'ö'),
    (0x10, 'Ü'),
    (0x11, 'â'),
    (0x12, 'û'),
    (0x13, 'î'),
    (0x14, 'é'),
    (0x15, 'è'),
    (0x16, 'ñ'),
    (0x17, 'ê'),
    (0x18, 'å'),
    (0x19, 'à'),
    (0x1A, 'Å'),
    (0x60, '¡'),
    (0x7B, 'Ä'),
];

/// The international-set code for an accented character, if it has one.
pub fn international_code(c: char) -> Option<u8> {
    INTERNATIONAL
        .iter()
        .find(|&&(_, ch)| ch == c)
        .map(|&(code, _)| code)
}

/// The accented character an international-set code shows, if any.
pub fn international_char(code: u8) -> Option<char> {
    INTERNATIONAL
        .iter()
        .find(|&&(c, _)| c == code)
        .map(|&(_, ch)| ch)
}

/// Decode an ATASCII file, byte for byte: every file is Atari bytes here,
/// whatever its line endings. PC text is read by [`import`].
pub fn decode(bytes: &[u8]) -> Decoded {
    if bytes.is_empty() {
        // One empty line with no EOL, so it saves back as an empty file.
        return Decoded {
            lines: vec![Vec::new()],
            final_eol: false,
            converted_from: None,
        };
    }
    let final_eol = bytes.last() == Some(&EOL);
    let body = if final_eol {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    };
    let lines = body.split(|&b| b == EOL).map(<[u8]>::to_vec).collect();
    Decoded {
        lines,
        final_eol,
        converted_from: None,
    }
}

/// Whether a file looks like PC text rather than ATASCII: line breaks and
/// no EOLs.
pub fn looks_like_pc_text(bytes: &[u8]) -> bool {
    !bytes.contains(&EOL) && bytes.iter().any(|&b| b == b'\n' || b == b'\r')
}

/// Import a PC text file (File > Import): lines split at LF, CR+LF or CR,
/// and each line converted by [`crate::pctext::from_text`].
pub fn import(bytes: &[u8]) -> Decoded {
    // UTF-8 (with or without a byte-order mark) is read as characters.
    // Anything else keeps its bytes: a byte above $7F is passed through as
    // the Private Use character for that code, so it reads back as itself.
    let body = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let (text, from) = match std::str::from_utf8(body) {
        Ok(s) if !s.is_ascii() => (s.to_string(), "UTF-8 text"),
        Ok(s) => (s.to_string(), "PC text"),
        Err(_) => (
            body.iter()
                .map(|&b| match b {
                    0x00..=0x7F => b as char,
                    _ => char::from_u32(0xE000 + b as u32).unwrap(),
                })
                .collect(),
            "PC text",
        ),
    };
    let mut lines = Vec::new();
    let mut unknown = 0;
    let mut rest = text.as_str();
    while !rest.is_empty() {
        let (line, next) = match rest.find(['\r', '\n']) {
            Some(n) => {
                let skip = if rest[n..].starts_with("\r\n") { 2 } else { 1 };
                (&rest[..n], &rest[n + skip..])
            }
            None => (rest, ""),
        };
        let (codes, bad) = crate::pctext::from_text(line);
        lines.push(codes);
        unknown += bad;
        rest = next;
    }
    let mut from = from.to_string();
    if unknown > 0 {
        let s = if unknown == 1 { "" } else { "s" };
        from += &format!(" ({unknown} character{s} with no Atari equivalent, shown as ?)");
    }
    // Converted files are always written back with a final EOL.
    Decoded {
        lines,
        final_eol: true,
        converted_from: Some(from),
    }
}

/// Encode lines as ATASCII with $9B after each line (and after the last
/// line only if `final_eol`).
pub fn encode(lines: &[Vec<u8>], final_eol: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(lines.iter().map(|l| l.len() + 1).sum());
    for (i, l) in lines.iter().enumerate() {
        out.extend_from_slice(l);
        if i + 1 < lines.len() || final_eol {
            out.push(EOL);
        }
    }
    out
}

/// ATASCII text with $9B line ends: LIST files, assembler source and so
/// on. Accepts any file, so it goes last in the
/// machine's format list.
pub struct AtasciiText;

impl retro_core::FileFormat for AtasciiText {
    fn name(&self) -> &'static str {
        "ATASCII text"
    }

    fn status_name(&self) -> &'static str {
        "ATASCII"
    }

    fn extension(&self) -> &'static str {
        "LST"
    }

    /// Text in other languages: source for MAC/65 and the Assembler
    /// Editor, Logo, PILOT, FastBASIC and Action!.
    fn shared_extensions(&self) -> &'static [&'static str] {
        &["TXT", "ASM", "LGO", "PIL", "FB", "ACT"]
    }

    fn detect(&self, _bytes: &[u8]) -> bool {
        true
    }

    fn decode(&self, bytes: &[u8]) -> Result<Decoded, String> {
        Ok(decode(bytes))
    }

    fn can_encode(&self) -> bool {
        true
    }

    fn encode(&self, doc: &retro_core::Document) -> Result<retro_core::Encoded, String> {
        Ok(retro_core::Encoded::clean(encode(
            doc.lines(),
            doc.final_eol,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_native_files() {
        for bytes in [
            &b"10 PRINT \"HI\"\x9b20 GOTO 10\x9b"[..],
            &b"NO FINAL EOL\x9bLAST"[..],
            &b"\x9b\x9b"[..],
            &b"\x00\x7d\xff\x9b"[..],
        ] {
            let d = decode(bytes);
            assert_eq!(d.converted_from, None);
            assert_eq!(encode(&d.lines, d.final_eol), bytes, "{bytes:?}");
        }
    }

    #[test]
    fn splits_lines() {
        let d = decode(b"A\x9bBC\x9b");
        assert_eq!(d.lines, vec![b"A".to_vec(), b"BC".to_vec()]);
        assert!(d.final_eol);
    }

    #[test]
    fn converts_utf8_and_tags() {
        // A heart in the Private Use Area, an inverse A as a tag, and a
        // character the Atari doesn't have.
        let d = import("\u{FEFF}\u{E000}{INV}A{/INV}\t\u{2603}\r\n".as_bytes());
        assert_eq!(d.lines, vec![vec![0x00, 0xC1, TAB, b'?']]);
        assert_eq!(
            d.converted_from.as_deref(),
            Some("UTF-8 text (1 character with no Atari equivalent, shown as ?)")
        );
        // A $9B byte inside a character doesn't make it ATASCII.
        assert_eq!(import("\u{E0DB}\n".as_bytes()).lines, vec![vec![0xDB]]);
        // Not UTF-8: bytes are kept.
        let d = import(b"A\x9cB\n");
        assert_eq!(d.lines, vec![vec![b'A', 0x9C, b'B']]);
    }

    #[test]
    fn converts_pc_text() {
        let d = import(b"10 REM\r\n20 END\n30 X\r");
        assert_eq!(d.converted_from.as_deref(), Some("PC text"));
        assert_eq!(
            d.lines,
            vec![b"10 REM".to_vec(), b"20 END".to_vec(), b"30 X".to_vec()]
        );
        assert_eq!(
            encode(&d.lines, d.final_eol),
            b"10 REM\x9b20 END\x9b30 X\x9b"
        );
        let d = import(b"A\nB");
        assert_eq!(d.lines, vec![b"A".to_vec(), b"B".to_vec()]);
    }

    #[test]
    fn open_keeps_pc_line_ends() {
        // LF is CTRL-J to Open.
        let d = decode(b"A\nB\x9b");
        assert_eq!(d.lines, vec![b"A\nB".to_vec()]);
        assert_eq!(d.converted_from, None);
        assert!(looks_like_pc_text(b"A\nB"));
        assert!(!looks_like_pc_text(b"A\nB\x9b"));
    }

    #[test]
    fn empty_file() {
        let d = decode(b"");
        assert_eq!(encode(&d.lines, d.final_eol), b"");
        // A single empty line is preserved.
        let d = decode(b"\x9b");
        assert_eq!(encode(&d.lines, d.final_eol), b"\x9b");
    }
}
