//! The interface each target computer implements.

use crate::format::FileFormat;
use crate::tabs::TabStops;

/// Result of decoding a file from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// Lines of machine codes, without line terminators.
    pub lines: Vec<Vec<u8>>,
    /// Whether the last line was followed by a terminator.
    pub final_eol: bool,
    /// Set when the file was converted from PC text (File > Import). The
    /// string is a short human-readable description for the status bar.
    pub converted_from: Option<String>,
}

/// An offer, after Import, to put a listing's lower case in capitals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseOffer {
    /// What the file is, e.g. "a BASIC program".
    pub what: &'static str,
    /// What is left as it is, e.g. "Strings, REM and DATA".
    pub kept: &'static str,
    /// The converted lines.
    pub lines: Vec<Vec<u8>>,
    /// How many lines change.
    pub changed: usize,
}

pub trait Machine {
    /// Short display name, e.g. "Atari 8-bit".
    fn name(&self) -> &'static str;

    /// The 8x8 glyph shown for a machine code: one byte per row, bit 7 is
    /// the leftmost pixel. Inverse/reverse video is part of the glyph.
    fn glyph(&self, code: u8) -> [u8; 8];

    /// Names of the character sets the machine can display, e.g.
    /// `["Standard", "International"]`. The first is the default.
    fn charsets(&self) -> &'static [&'static str] {
        &["Standard"]
    }

    /// The character set in use (an index into [`Machine::charsets`]).
    fn charset(&self) -> usize {
        0
    }

    /// Switch character set. Affects [`Machine::glyph`] and the clipboard
    /// mapping; the codes in the document never change.
    fn set_charset(&mut self, _index: usize) {}

    /// Code of the blank (space) character.
    fn space(&self) -> u8;

    /// Native screen width in characters (40 for both Atari and C64).
    fn screen_columns(&self) -> usize;

    /// Blank columns at the left of the native screen (2 on the Atari).
    fn left_margin(&self) -> usize;

    /// Machine code used to display an ASCII character in the editor's own
    /// text (status bar, help). Defaults to the ASCII value itself.
    fn ascii_code(&self, c: char) -> u8 {
        if c.is_ascii() { c as u8 } else { b'?' }
    }

    /// Tab stops at start-up. The default is every 8 text columns.
    fn default_tab_stops(&self) -> TabStops {
        let m = self.left_margin();
        TabStops::every(8, m + 8, self.screen_columns(), m, 3)
    }

    /// The Unicode character used for a machine code on the system
    /// clipboard. Default: printable ASCII as itself, anything else as
    /// U+E000 + code (a private-use character).
    fn char_for_code(&self, code: u8) -> char {
        match code {
            0x20..=0x7E => code as char,
            _ => char::from_u32(0xE000 + code as u32).unwrap(),
        }
    }

    /// The machine code for a character pasted from the system clipboard,
    /// or `None` if the machine has no such character. Default: the inverse
    /// of [`Machine::char_for_code`].
    fn code_for_char(&self, c: char) -> Option<u8> {
        match c as u32 {
            0x20..=0x7E => Some(c as u8),
            n @ 0xE000..=0xE0FF => Some((n - 0xE000) as u8),
            _ => None,
        }
    }

    /// A line as UTF-8 for export: each code as [`Machine::char_for_code`],
    /// as on the clipboard.
    fn export_unicode(&self, line: &[u8]) -> String {
        line.iter().map(|&c| self.char_for_code(c)).collect()
    }

    /// A line as standard Unicode for export, readable in any font, and
    /// how many characters had no exact equivalent. Default: printable
    /// ASCII as itself, `?` (counted) for the rest.
    fn export_standard(&self, line: &[u8]) -> (String, usize) {
        let mut lost = 0;
        let s = line
            .iter()
            .map(|&c| match c {
                0x20..=0x7E => c as char,
                _ => {
                    lost += 1;
                    '?'
                }
            })
            .collect();
        (s, lost)
    }

    /// A line as plain ASCII for export. Default: printable ASCII as
    /// itself, other codes as `{$xx}`.
    fn export_ascii(&self, line: &[u8]) -> String {
        line.iter()
            .map(|&c| match c {
                0x20..=0x7E => (c as char).to_string(),
                _ => format!("{{${c:02X}}}"),
            })
            .collect()
    }

    /// Import a PC text file. Default: lines split at LF, CR+LF or CR,
    /// characters as [`Machine::code_for_char`], `?` for the rest.
    fn import_text(&self, bytes: &[u8]) -> Decoded {
        let text = String::from_utf8_lossy(bytes);
        let lines = text
            .lines()
            .map(|l| {
                l.chars()
                    .map(|c| self.code_for_char(c).unwrap_or(b'?'))
                    .collect()
            })
            .collect();
        Decoded {
            lines,
            final_eol: true,
            converted_from: Some("PC text".into()),
        }
    }

    /// After Import: whether to offer converting lower case to capitals.
    fn case_offer(&self, _lines: &[Vec<u8>]) -> Option<CaseOffer> {
        None
    }

    /// Whether a file looks like PC text that should be imported rather
    /// than opened.
    fn looks_like_pc_text(&self, _bytes: &[u8]) -> bool {
        false
    }

    /// The file formats the machine reads, most specific first. The last
    /// is the machine's text format: it accepts any file and is used for
    /// new documents (see [`crate::format`]).
    fn formats(&self) -> &'static [&'static dyn FileFormat];

    /// The machine's text format (the last of [`Machine::formats`]).
    fn text_format(&self) -> &'static dyn FileFormat {
        *self
            .formats()
            .last()
            .expect("a machine has at least one format")
    }
}
