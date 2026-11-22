//! The interface each target computer implements.

use crate::document::Document;
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

    /// Decode a file's bytes into lines.
    fn decode(&self, bytes: &[u8]) -> Decoded;

    /// Encode a document back into the machine's native file format.
    fn encode(&self, doc: &Document) -> Vec<u8>;
}
