//! ATASCII to and from PC text: UTF-8 for the EightBit Atari font, and
//! plain ASCII with `{tags}`.
//!
//! UTF-8 uses the same characters as the clipboard: printable ASCII as
//! itself, everything else U+E000 + code (the Private Use Area, where the
//! EightBit Atari font has the Atari's characters), and with the
//! international character set real accented letters (inverse ones as
//! U+E100 + code).
//!
//! ASCII uses printable ASCII as itself and a tag for everything else:
//! `{CTRL-A}`..`{CTRL-Z}`, `{CTRL-,}` `{CTRL-.}` `{CTRL-;}`, `{ESC}`,
//! `{UP}` `{DOWN}` `{LEFT}` `{RIGHT}`, `{CLEAR}`, `{BACK}`, `{TAB}`, the
//! screen-editor codes `{DEL LINE}` `{INS LINE}` `{CLR TAB}` `{SET TAB}`
//! `{BELL}` `{DEL CHAR}` `{INS CHAR}`, and `{INV}`...`{/INV}` around
//! inverse characters. ATASCII has no `{` or `}` of its own (those codes
//! are the spade and CLEAR), so a brace always starts a tag and the text
//! reads back exactly.

use crate::atascii;

/// Tags for codes `$00`-`$7F` that aren't printable ASCII.
fn tag(code: u8) -> Option<&'static str> {
    const CTRL: [&str; 27] = [
        "{CTRL-,}", "{CTRL-A}", "{CTRL-B}", "{CTRL-C}", "{CTRL-D}", "{CTRL-E}", "{CTRL-F}",
        "{CTRL-G}", "{CTRL-H}", "{CTRL-I}", "{CTRL-J}", "{CTRL-K}", "{CTRL-L}", "{CTRL-M}",
        "{CTRL-N}", "{CTRL-O}", "{CTRL-P}", "{CTRL-Q}", "{CTRL-R}", "{CTRL-S}", "{CTRL-T}",
        "{CTRL-U}", "{CTRL-V}", "{CTRL-W}", "{CTRL-X}", "{CTRL-Y}", "{CTRL-Z}",
    ];
    Some(match code {
        0x00..=0x1A => CTRL[code as usize],
        atascii::ESC => "{ESC}",
        atascii::CURSOR_UP => "{UP}",
        atascii::CURSOR_DOWN => "{DOWN}",
        atascii::CURSOR_LEFT => "{LEFT}",
        atascii::CURSOR_RIGHT => "{RIGHT}",
        0x60 => "{CTRL-.}",
        0x7B => "{CTRL-;}",
        atascii::CLEAR => "{CLEAR}",
        atascii::BACKSPACE => "{BACK}",
        atascii::TAB => "{TAB}",
        _ => return None,
    })
}

/// Tags for the screen-editor codes among the inverse characters.
fn special_tag(code: u8) -> Option<&'static str> {
    Some(match code {
        atascii::DELETE_LINE => "{DEL LINE}",
        atascii::INSERT_LINE => "{INS LINE}",
        atascii::CLEAR_TAB => "{CLR TAB}",
        atascii::SET_TAB => "{SET TAB}",
        atascii::BELL => "{BELL}",
        atascii::DELETE_CHAR => "{DEL CHAR}",
        atascii::INSERT_CHAR => "{INS CHAR}",
        _ => return None,
    })
}

/// Whether a code is printable ASCII (shown as itself).
fn plain(code: u8) -> bool {
    matches!(code, 0x20..=0x5F | 0x61..=0x7A | 0x7C)
}

/// One line as plain ASCII with tags.
pub fn to_ascii(line: &[u8]) -> String {
    let mut s = String::new();
    let mut inverse = false;
    for &code in line {
        if let Some(t) = special_tag(code) {
            if inverse {
                s.push_str("{/INV}");
                inverse = false;
            }
            s.push_str(t);
            continue;
        }
        let on = code >= 0x80;
        if on != inverse {
            s.push_str(if on { "{INV}" } else { "{/INV}" });
            inverse = on;
        }
        let c = code & 0x7F;
        match tag(c) {
            Some(t) => s.push_str(t),
            None => s.push(c as char),
        }
    }
    if inverse {
        s.push_str("{/INV}");
    }
    s
}

/// The character for a code, as on the clipboard (see the module notes).
pub fn char_for_code(code: u8, international: bool) -> char {
    if international {
        if let Some(ch) = atascii::international_char(code) {
            return ch;
        }
        if code >= 0x80 && atascii::international_char(code & 0x7F).is_some() {
            return char::from_u32(0xE100 + code as u32).unwrap();
        }
    }
    if plain(code) {
        code as char
    } else {
        char::from_u32(0xE000 + code as u32).unwrap()
    }
}

/// The code for a character, if it has one: Private Use characters, the
/// printable ASCII the Atari has, and (with `international`) accented
/// letters.
pub fn code_for_char(c: char, international: bool) -> Option<u8> {
    match c as u32 {
        n @ 0xE000..=0xE1FF => Some((n & 0xFF) as u8),
        _ => crate::keyboard::printable(c).or_else(|| {
            international
                .then(|| atascii::international_code(c))
                .flatten()
        }),
    }
}

/// One line of PC text (already split from its line ending) as ATASCII:
/// tags become their codes, a tab the Atari's TAB, other ASCII its own
/// code, and other characters as [`code_for_char`] with accented letters
/// allowed. Returns the codes and how many characters
/// had no Atari equivalent (stored as `?`).
pub fn from_text(line: &str) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut unknown = 0;
    let mut inverse = 0u8;
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        if c == '{'
            && let Some(end) = rest.find('}')
            && let Some(codes) = tag_codes(&rest[..=end])
        {
            match codes {
                Tag::InverseOn => inverse = 0x80,
                Tag::InverseOff => inverse = 0,
                Tag::Code(code) => out.push(code | inverse),
                Tag::Special(code) => out.push(code),
            }
            rest = &rest[end + 1..];
            continue;
        }
        // A PC tab is the Atari's TAB; other ASCII keeps its code (a `{`
        // that isn't a tag is the spade).
        let code = match c {
            '\t' => Some(atascii::TAB),
            _ if c.is_ascii() => Some(c as u8),
            _ => code_for_char(c, true),
        };
        match code {
            Some(code) => out.push(code | inverse),
            None => {
                out.push(b'?' | inverse);
                unknown += 1;
            }
        }
        rest = &rest[c.len_utf8()..];
    }
    (out, unknown)
}

enum Tag {
    InverseOn,
    InverseOff,
    /// A code in `$00`-`$7F`, made inverse inside `{INV}`.
    Code(u8),
    /// A screen-editor code, as is.
    Special(u8),
}

/// What a `{...}` means, if it's a tag (any case).
fn tag_codes(t: &str) -> Option<Tag> {
    let t = t.to_ascii_uppercase();
    match t.as_str() {
        "{INV}" => return Some(Tag::InverseOn),
        "{/INV}" => return Some(Tag::InverseOff),
        _ => {}
    }
    if let Some(code) = (0x00..=0x7F).find(|&c| tag(c) == Some(t.as_str())) {
        return Some(Tag::Code(code));
    }
    (0x80..=0xFF)
        .find(|&c| special_tag(c) == Some(t.as_str()))
        .map(Tag::Special)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_round_trips_every_code() {
        // Every code but EOL, normal and inverse, in one line.
        let line: Vec<u8> = (0..=0xFF).filter(|&c| c != atascii::EOL).collect();
        let text = to_ascii(&line);
        assert!(text.is_ascii());
        assert_eq!(from_text(&text), (line, 0));
    }

    #[test]
    fn unicode_round_trips_every_code() {
        for international in [false, true] {
            let line: Vec<u8> = (0..=0xFF).filter(|&c| c != atascii::EOL).collect();
            let text: String = line
                .iter()
                .map(|&c| char_for_code(c, international))
                .collect();
            assert_eq!(from_text(&text), (line, 0), "international {international}");
        }
    }

    #[test]
    fn tags_read_well() {
        assert_eq!(
            to_ascii(b"\x7d\x00A\xc8\xc9!"),
            "{CLEAR}{CTRL-,}A{INV}HI{/INV}!"
        );
        assert_eq!(to_ascii(&[atascii::BELL]), "{BELL}");
        assert_eq!(from_text("{clear}{inv}OK{/inv}").0, b"\x7d\xcf\xcb");
        // Not a tag: the brace is the spade, the rest as typed.
        assert_eq!(from_text("{NOPE}").0, b"{NOPE}");
        // No Atari equivalent.
        assert_eq!(from_text("½"), (b"?".to_vec(), 1));
    }
}
