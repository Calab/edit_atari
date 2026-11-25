//! Machine-independent core of the retro text editors.
//!
//! Text is stored as raw 8-bit *machine codes* (ATASCII for the Atari,
//! PETSCII for a future Commodore editor). Everything that depends on a
//! particular computer -- its character glyphs, its file line endings, its
//! keyboard -- lives behind the [`machine::Machine`] trait or in the
//! machine's own crate.

pub mod clipboard;
pub mod disk;
pub mod document;
pub mod editor;
pub mod format;
pub mod input;
pub mod layout;
pub mod machine;
pub mod print;
pub mod search;
pub mod tabs;

pub use document::{Document, Pos, Snapshot};
pub use editor::Editor;
pub use format::{Encoded, FileFormat};
pub use input::{EditAction, Key, KeyInput, KeyboardRules, Mods};
pub use layout::{Layout, WrapMode};
pub use machine::{CaseOffer, Decoded, Machine};
pub use tabs::TabStops;
