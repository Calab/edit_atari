//! ATASCII to and from PC text: UTF-8 for the EightBit Atari font,
//! standard Unicode for any font, and plain ASCII with `{tags}`.
//!
//! Standard Unicode uses the symbols the EightBit Atari font itself gives
//! the Atari graphics (heavy box drawing, block elements, card suits,
//! arrows), so it looks right in that font too. Inverse video has no
//! Unicode equivalent except for a few block shapes (inverse space is a
//! full block); other inverse characters export as normal ones.
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

/// Standard Unicode symbols for the Atari graphics characters.
const STANDARD: [(u8, char); 35] = [
    (0x00, '\u{2665}'), // ♥
    (0x01, '\u{2523}'), // ┣
    (0x02, '\u{2595}'), // ▕
    (0x03, '\u{251B}'), // ┛
    (0x04, '\u{252B}'), // ┫
    (0x05, '\u{2513}'), // ┓
    (0x06, '\u{2571}'), // ╱
    (0x07, '\u{2572}'), // ╲
    (0x08, '\u{25E2}'), // ◢
    (0x09, '\u{2597}'), // ▗
    (0x0A, '\u{25E3}'), // ◣
    (0x0B, '\u{259D}'), // ▝
    (0x0C, '\u{2598}'), // ▘
    (0x0D, '\u{2594}'), // ▔
    (0x0E, '\u{2581}'), // ▁
    (0x0F, '\u{2596}'), // ▖
    (0x10, '\u{2663}'), // ♣
    (0x11, '\u{250F}'), // ┏
    (0x12, '\u{2501}'), // ━
    (0x13, '\u{254B}'), // ╋
    (0x14, '\u{2B24}'), // ⬤
    (0x15, '\u{2584}'), // ▄
    (0x16, '\u{258F}'), // ▏
    (0x17, '\u{2533}'), // ┳
    (0x18, '\u{253B}'), // ┻
    (0x19, '\u{258C}'), // ▌
    (0x1A, '\u{2517}'), // ┗
    (0x1B, '\u{241B}'), // ␛
    (0x1C, '\u{2191}'), // ↑
    (0x1D, '\u{2193}'), // ↓
    (0x1E, '\u{2190}'), // ←
    (0x1F, '\u{2192}'), // →
    (0x60, '\u{2666}'), // ♦
    (0x7B, '\u{2660}'), // ♠
    (0x7D, '\u{2923}'), // ⤣ (CLEAR)
];

/// Inverse characters that are themselves standard block shapes.
const STANDARD_INVERSE: [(u8, char); 9] = [
    (0xA0, '\u{2588}'), // inverse space: █
    (0x95, '\u{2580}'), // inverse ▄: ▀
    (0x99, '\u{2590}'), // inverse ▌: ▐
    (0x88, '\u{25E4}'), // inverse ◢: ◤
    (0x8A, '\u{25E5}'), // inverse ◣: ◥
    (0x89, '\u{259B}'), // inverse ▗: ▛
    (0x8B, '\u{2599}'), // inverse ▝: ▙
    (0x8C, '\u{259F}'), // inverse ▘: ▟
    (0x8F, '\u{259C}'), // inverse ▖: ▜
];

/// The standard character for a code, if it has an exact one.
fn standard_char(code: u8, international: bool) -> Option<char> {
    if let Some(&(_, ch)) = STANDARD_INVERSE.iter().find(|(c, _)| *c == code) {
        return Some(ch);
    }
    if code >= 0x80 {
        return None;
    }
    if international && let Some(ch) = atascii::international_char(code) {
        return Some(ch);
    }
    match code {
        0x7E => Some('\u{25C0}'), // ◀ backspace
        0x7F => Some('\u{25B6}'), // ▶ tab
        _ if plain(code) => Some(code as char),
        _ => STANDARD.iter().find(|(c, _)| *c == code).map(|&(_, ch)| ch),
    }
}

/// The code for a standard Unicode symbol from [`to_standard`].
fn standard_code(ch: char) -> Option<u8> {
    let pair = |&&(code, c): &&(u8, char)| (c == ch).then_some(code);
    match ch {
        '\u{25C0}' => Some(0x7E),
        '\u{25B6}' => Some(0x7F),
        _ => STANDARD
            .iter()
            .chain(&STANDARD_INVERSE)
            .find_map(|p| pair(&p)),
    }
}

/// One line as standard Unicode, for any font. Returns the text and how
/// many characters had no exact equivalent (inverse video, exported as
/// the normal character).
pub fn to_standard(line: &[u8], international: bool) -> (String, usize) {
    let mut lost = 0;
    let s = line
        .iter()
        .map(|&code| {
            standard_char(code, international).unwrap_or_else(|| {
                lost += 1;
                standard_char(code & 0x7F, international).unwrap_or('?')
            })
        })
        .collect();
    (s, lost)
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
/// allowed, or as the standard symbols of [`to_standard`]. Returns the codes and how many characters
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
            // Standard Unicode exports read back too (Import only: the
            // clipboard and keyboard leave these symbols alone).
            _ => code_for_char(c, true).or_else(|| standard_code(c)),
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
    fn standard_unicode_reads_back() {
        // Every normal character reads back exactly, and so do the inverse
        // block shapes.
        let line: Vec<u8> = (0..=0x7F)
            .chain(STANDARD_INVERSE.iter().map(|&(c, _)| c))
            .collect();
        let (text, lost) = to_standard(&line, false);
        assert_eq!(lost, 0);
        assert_eq!(from_text(&text), (line, 0));
        // Other inverse characters come out normal, and are counted.
        assert_eq!(
            to_standard(b"\xc8\xc9 \x80", false),
            ("HI \u{2665}".into(), 3)
        );
        // International letters as themselves.
        assert_eq!(to_standard(&[0x00], true).0, "\u{e1}");
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
