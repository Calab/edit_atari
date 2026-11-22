//! Platform-neutral keyboard input and the editing actions it produces.
//!
//! The front end (SDL) turns physical key events into [`KeyInput`]; the
//! machine's keyboard module turns those into [`EditAction`]s. That keeps the
//! Atari (or C64) keyboard rules testable without a window.

/// A key, independent of the windowing library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A printable character as typed on the PC keyboard layout
    /// (already shifted, e.g. `'A'` or `'"'`).
    Char(char),
    /// A letter/digit/punctuation key pressed while the machine's CONTROL
    /// modifier is held. Holds the unshifted key's character, lower case
    /// (e.g. `'a'`, `','`, `'2'`).
    Control(char),
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Enter,
    Backspace,
    Delete,
    Insert,
    Tab,
    Escape,
    /// Toggles inverse video (the Atari "Fuji"/inverse key).
    Inverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Mods {
    pub shift: bool,
    /// The machine's CONTROL key (not the PC Ctrl used for editor commands).
    pub control: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyInput {
    pub key: Key,
    pub mods: Mods,
}

impl KeyInput {
    pub fn plain(key: Key) -> Self {
        Self {
            key,
            mods: Mods::default(),
        }
    }
    pub fn shifted(key: Key) -> Self {
        Self {
            key,
            mods: Mods {
                shift: true,
                control: false,
            },
        }
    }
    pub fn control(key: Key) -> Self {
        Self {
            key,
            mods: Mods {
                shift: false,
                control: true,
            },
        }
    }
}

/// What a keystroke does to the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditAction {
    /// Insert (or overwrite with) a machine code at the cursor.
    Insert(u8),
    NewLine,
    Backspace,
    DeleteForward,
    InsertSpace,
    DeleteLine,
    InsertLine,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    LineStart,
    LineEnd,
    PageUp,
    PageDown,
    DocStart,
    DocEnd,
    NextTabStop,
    /// Set a tab stop at the cursor column.
    SetTabStop,
    /// Clear the tab stop at the cursor column.
    ClearTabStop,
    ToggleOverwrite,
    ToggleInverse,
    /// ESC was pressed: the next key is inserted literally.
    EscapePending,
    /// The key does nothing.
    None,
}

/// A machine's keyboard behaviour: turns key presses into editing actions.
pub trait KeyboardRules {
    fn handle(&mut self, input: KeyInput) -> EditAction;
    /// Short status-bar indicators for active modes (e.g. "INV", "ESC").
    fn indicators(&self) -> Vec<&'static str>;

    /// The machine's character set changed (see `Machine::set_charset`),
    /// which may change what typed characters produce.
    fn set_charset(&mut self, _index: usize) {}
}
