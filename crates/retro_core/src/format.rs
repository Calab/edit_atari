//! File formats: how a machine's files are read into lines of machine codes
//! and written back.
//!
//! Each machine lists its formats (see [`crate::Machine::formats`]). A file
//! being opened is read with the first format that recognises it, so
//! specific formats (a tokenized BASIC program with a recognisable header)
//! come first and the machine's plain text format, which accepts anything,
//! comes last. Adding a format means writing one more implementation of
//! [`FileFormat`] and adding it to the machine's list.

use crate::document::Document;
use crate::machine::Decoded;

pub trait FileFormat: Sync {
    /// Short name for the status bar and messages, e.g. "Atari BASIC".
    fn name(&self) -> &'static str;

    /// The usual file-name extension, without the dot, e.g. "BAS".
    fn extension(&self) -> &'static str;

    /// Whether `bytes` look like a file in this format. Should be cheap and
    /// strict enough not to claim other formats' files.
    fn detect(&self, bytes: &[u8]) -> bool;

    /// Whether `bytes` have this format's outward signs (a header, say)
    /// even if [`FileFormat::detect`] rejects them because they're
    /// damaged. Used to explain why such a file opened as plain text.
    fn looks_like(&self, bytes: &[u8]) -> bool {
        self.detect(bytes)
    }

    /// Read a file into lines of machine codes.
    fn decode(&self, bytes: &[u8]) -> Result<Decoded, String>;

    /// Whether [`FileFormat::encode`] is available.
    fn can_encode(&self) -> bool {
        false
    }

    /// Write a document in this format.
    fn encode(&self, _doc: &Document) -> Result<Vec<u8>, String> {
        Err(format!("{} files can't be written yet", self.name()))
    }
}

/// The first format in `formats` that recognises `bytes`, or the last one
/// (the machine's text format) if none does.
pub fn detect(formats: &[&'static dyn FileFormat], bytes: &[u8]) -> &'static dyn FileFormat {
    formats
        .iter()
        .copied()
        .find(|f| f.detect(bytes))
        .unwrap_or_else(|| *formats.last().expect("a machine has at least one format"))
}

/// Whether two format references are the same format. (Compared by name:
/// formats are usually zero-sized, so their addresses can coincide.)
pub fn same(a: &dyn FileFormat, b: &dyn FileFormat) -> bool {
    a.name() == b.name()
}
