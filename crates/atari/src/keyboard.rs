//! Atari keyboard rules on a PC keyboard.
//!
//! * Printable keys follow the PC layout (Shift+2 gives `@` on a US PC).
//! * The Atari CONTROL key (RIGHT-CTRL by default, chosen by the front end)
//!   types the control graphics: CONTROL+A..Z = $01..$1A, CONTROL+, = $00
//!   (heart), CONTROL+. = $60 (diamond), CONTROL+; = $7B (spade).
//! * ESC makes the next key insert its ATASCII code literally instead of
//!   acting, exactly as on the Atari (ESC then cursor-up shows the arrow).
//! * The inverse key toggles inverse video for typed characters.

use retro_core::{EditAction, Key, KeyInput, KeyboardRules};

use crate::atascii::*;

#[derive(Debug, Default, Clone)]
pub struct AtariKeyboard {
    /// ESC was pressed; the next key is inserted literally.
    pub escape_pending: bool,
    /// Inverse video is on for typed characters.
    pub inverse: bool,
}

/// ATASCII code for a printable PC character, or `None` if the Atari has no
/// such key (`` ` ``, `{`, `}`, `~` and non-ASCII characters).
pub fn printable(c: char) -> Option<u8> {
    match c {
        '`' | '{' | '}' | '~' => None,
        ' '..='z' | '|' => Some(c as u8),
        _ => None,
    }
}

/// ATASCII code for CONTROL plus a key, if that combination types a graphic.
pub fn control_graphic(c: char) -> Option<u8> {
    match c.to_ascii_lowercase() {
        c @ 'a'..='z' => Some(c as u8 - b'a' + 1),
        ',' => Some(0x00),
        '.' => Some(0x60),
        ';' => Some(0x7B),
        _ => None,
    }
}

impl AtariKeyboard {
    pub fn new() -> Self {
        Self::default()
    }

    fn typed(&self, code: u8) -> EditAction {
        EditAction::Insert(if self.inverse { code ^ 0x80 } else { code })
    }

    /// The ATASCII code a key sends when it is an Atari editing key, used
    /// for ESC-literal mode. `None` for keys that are not editing keys.
    fn literal_code(input: KeyInput) -> Option<u8> {
        let KeyInput { key, mods } = input;
        Some(match key {
            Key::Escape => ESC,
            Key::Up => CURSOR_UP,
            Key::Down => CURSOR_DOWN,
            Key::Left => CURSOR_LEFT,
            Key::Right => CURSOR_RIGHT,
            Key::Home if mods.shift || mods.control => CLEAR,
            Key::Backspace if mods.shift => DELETE_LINE,
            Key::Backspace => BACKSPACE,
            Key::Delete if mods.shift => DELETE_LINE,
            Key::Delete => DELETE_CHAR,
            Key::Insert if mods.shift => INSERT_LINE,
            Key::Insert => INSERT_CHAR,
            Key::Tab if mods.shift => SET_TAB,
            Key::Tab if mods.control => CLEAR_TAB,
            Key::Tab => TAB,
            Key::Control('2') => BELL,
            Key::Control('-') => CURSOR_UP,
            Key::Control('=') => CURSOR_DOWN,
            _ => return None,
        })
    }

    /// Translate one key press into an editing action.
    pub fn handle(&mut self, input: KeyInput) -> EditAction {
        let KeyInput { key, mods } = input;

        if self.escape_pending {
            self.escape_pending = false;
            if let Some(code) = Self::literal_code(input) {
                return EditAction::Insert(code);
            }
            // Not an editing key: fall through and type it normally.
        }

        match key {
            Key::Char(c) => printable(c).map_or(EditAction::None, |b| self.typed(b)),
            Key::Control(c) => match c {
                '-' => EditAction::MoveUp,
                '=' => EditAction::MoveDown,
                _ => control_graphic(c).map_or(EditAction::None, |b| self.typed(b)),
            },
            Key::Escape => {
                self.escape_pending = true;
                EditAction::EscapePending
            }
            Key::Inverse => {
                self.inverse = !self.inverse;
                EditAction::ToggleInverse
            }
            Key::Up => EditAction::MoveUp,
            Key::Down => EditAction::MoveDown,
            Key::Left => EditAction::MoveLeft,
            Key::Right => EditAction::MoveRight,
            Key::Home => EditAction::LineStart,
            Key::End => EditAction::LineEnd,
            Key::PageUp => EditAction::PageUp,
            Key::PageDown => EditAction::PageDown,
            Key::Enter => EditAction::NewLine,
            Key::Backspace if mods.shift => EditAction::DeleteLine,
            Key::Backspace => EditAction::Backspace,
            Key::Delete if mods.shift => EditAction::DeleteLine,
            Key::Delete => EditAction::DeleteForward,
            Key::Insert if mods.shift => EditAction::InsertLine,
            Key::Insert if mods.control => EditAction::InsertSpace,
            Key::Insert => EditAction::ToggleOverwrite,
            Key::Tab => EditAction::NextTabStop,
        }
    }
}

impl KeyboardRules for AtariKeyboard {
    fn handle(&mut self, input: KeyInput) -> EditAction {
        AtariKeyboard::handle(self, input)
    }

    fn indicators(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.inverse {
            v.push("INV");
        }
        if self.escape_pending {
            v.push("ESC");
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use retro_core::Key::*;

    fn press(kb: &mut AtariKeyboard, k: Key) -> EditAction {
        kb.handle(KeyInput::plain(k))
    }

    #[test]
    fn printable_characters() {
        let mut kb = AtariKeyboard::new();
        assert_eq!(press(&mut kb, Char('A')), EditAction::Insert(b'A'));
        assert_eq!(press(&mut kb, Char('a')), EditAction::Insert(b'a'));
        assert_eq!(press(&mut kb, Char('|')), EditAction::Insert(b'|'));
        assert_eq!(press(&mut kb, Char('{')), EditAction::None);
        assert_eq!(press(&mut kb, Char('é')), EditAction::None);
    }

    #[test]
    fn control_graphics() {
        let mut kb = AtariKeyboard::new();
        assert_eq!(press(&mut kb, Control('a')), EditAction::Insert(0x01));
        assert_eq!(press(&mut kb, Control('z')), EditAction::Insert(0x1A));
        assert_eq!(press(&mut kb, Control(',')), EditAction::Insert(0x00));
        assert_eq!(press(&mut kb, Control('.')), EditAction::Insert(0x60));
        assert_eq!(press(&mut kb, Control(';')), EditAction::Insert(0x7B));
        assert_eq!(press(&mut kb, Control('-')), EditAction::MoveUp);
    }

    #[test]
    fn inverse_applies_to_typed_characters() {
        let mut kb = AtariKeyboard::new();
        press(&mut kb, Inverse);
        assert_eq!(press(&mut kb, Char('A')), EditAction::Insert(0xC1));
        assert_eq!(press(&mut kb, Control('a')), EditAction::Insert(0x81));
        press(&mut kb, Inverse);
        assert_eq!(press(&mut kb, Char('A')), EditAction::Insert(b'A'));
    }

    #[test]
    fn escape_inserts_control_codes_literally() {
        let mut kb = AtariKeyboard::new();
        let cases: &[(KeyInput, u8)] = &[
            (KeyInput::plain(Up), 0x1C),
            (KeyInput::plain(Down), 0x1D),
            (KeyInput::plain(Left), 0x1E),
            (KeyInput::plain(Right), 0x1F),
            (KeyInput::plain(Escape), 0x1B),
            (KeyInput::shifted(Home), 0x7D),
            (KeyInput::plain(Backspace), 0x7E),
            (KeyInput::plain(Tab), 0x7F),
            (KeyInput::shifted(Delete), 0x9C),
            (KeyInput::shifted(Insert), 0x9D),
            (KeyInput::control(Tab), 0x9E),
            (KeyInput::shifted(Tab), 0x9F),
            (KeyInput::plain(Control('2')), 0xFD),
            (KeyInput::plain(Delete), 0xFE),
            (KeyInput::plain(Insert), 0xFF),
        ];
        for &(input, code) in cases {
            assert_eq!(
                kb.handle(KeyInput::plain(Escape)),
                EditAction::EscapePending
            );
            assert_eq!(kb.handle(input), EditAction::Insert(code), "{input:?}");
            assert!(!kb.escape_pending);
        }
    }

    #[test]
    fn escape_literals_ignore_inverse() {
        let mut kb = AtariKeyboard::new();
        press(&mut kb, Inverse);
        press(&mut kb, Escape);
        assert_eq!(press(&mut kb, Up), EditAction::Insert(0x1C));
    }

    #[test]
    fn escape_before_normal_key_types_it() {
        let mut kb = AtariKeyboard::new();
        press(&mut kb, Escape);
        assert_eq!(press(&mut kb, Char('Q')), EditAction::Insert(b'Q'));
        assert!(!kb.escape_pending);
    }

    #[test]
    fn editing_keys_act_without_escape() {
        let mut kb = AtariKeyboard::new();
        assert_eq!(press(&mut kb, Up), EditAction::MoveUp);
        assert_eq!(press(&mut kb, Backspace), EditAction::Backspace);
        assert_eq!(kb.handle(KeyInput::shifted(Delete)), EditAction::DeleteLine);
        assert_eq!(kb.handle(KeyInput::shifted(Insert)), EditAction::InsertLine);
        assert_eq!(press(&mut kb, Insert), EditAction::ToggleOverwrite);
        assert_eq!(press(&mut kb, Enter), EditAction::NewLine);
    }
}
