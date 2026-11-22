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
    /// Set when the file was not in the machine's native format and was
    /// converted (e.g. a PC text file with LF line endings). The string is a
    /// short human-readable description for the status bar.
    pub converted_from: Option<&'static str>,
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
