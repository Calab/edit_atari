//! Machine-independent core of the retro text editors.
//!
//! Text is stored as raw 8-bit *machine codes* (ATASCII for the Atari,
//! PETSCII for a future Commodore editor). Everything that depends on a
//! particular computer -- its character glyphs, its file line endings, its
//! keyboard -- lives behind the [`machine::Machine`] trait or in the
//! machine's own crate.

pub mod document;
pub mod editor;
pub mod input;
pub mod layout;
pub mod machine;
pub mod tabs;

pub use document::{Document, Pos};
pub use editor::Editor;
pub use input::{EditAction, Key, KeyInput, KeyboardRules, Mods};
pub use layout::{Layout, WrapMode};
pub use machine::{Decoded, Machine};
pub use tabs::TabStops;
