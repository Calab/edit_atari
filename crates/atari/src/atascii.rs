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

/// Decode an ATASCII file. A file with no EOL bytes but with LF/CR line
/// endings is treated as a PC text file and converted.
pub fn decode(bytes: &[u8]) -> Decoded {
    if bytes.is_empty() {
        // One empty line with no EOL, so it saves back as an empty file.
        return Decoded {
            lines: vec![Vec::new()],
            final_eol: false,
            converted_from: None,
        };
    }
    if !bytes.contains(&EOL) && bytes.iter().any(|&b| b == b'\n' || b == b'\r') {
        return decode_pc_text(bytes);
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

/// Split on LF, CRLF or lone CR.
fn decode_pc_text(bytes: &[u8]) -> Decoded {
    let mut lines = Vec::new();
    let mut cur = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                lines.push(std::mem::take(&mut cur));
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
            }
            b'\n' => lines.push(std::mem::take(&mut cur)),
            b => cur.push(b),
        }
        i += 1;
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    // Converted files are always written back with a final EOL.
    Decoded {
        lines,
        final_eol: true,
        converted_from: Some("PC text"),
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
/// on. Accepts any file (PC text is converted), so it goes last in the
/// machine's format list.
pub struct AtasciiText;

impl retro_core::FileFormat for AtasciiText {
    fn name(&self) -> &'static str {
        "ATASCII text"
    }

    fn extension(&self) -> &'static str {
        "LST"
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
    fn converts_pc_text() {
        let d = decode(b"10 REM\r\n20 END\n30 X\r");
        assert_eq!(d.converted_from, Some("PC text"));
        assert_eq!(
            d.lines,
            vec![b"10 REM".to_vec(), b"20 END".to_vec(), b"30 X".to_vec()]
        );
        assert_eq!(
            encode(&d.lines, d.final_eol),
            b"10 REM\x9b20 END\x9b30 X\x9b"
        );
        let d = decode(b"A\nB");
        assert_eq!(d.lines, vec![b"A".to_vec(), b"B".to_vec()]);
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
