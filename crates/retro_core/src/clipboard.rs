//! Converting text between machine codes and the system clipboard.

use crate::machine::Machine;

/// Lines of machine codes as clipboard text (lines joined with `\n`).
pub fn to_text(lines: &[Vec<u8>], machine: &dyn Machine) -> String {
    let mut s = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        s.extend(line.iter().map(|&c| machine.char_for_code(c)));
    }
    s
}

/// Clipboard text as lines of machine codes. Line breaks may be LF, CRLF
/// or CR; a tab becomes a space. Returns the lines and the number of
/// characters the machine has no code for (which are left out).
pub fn from_text(text: &str, machine: &dyn Machine) -> (Vec<Vec<u8>>, usize) {
    let mut lines = vec![Vec::new()];
    let mut skipped = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                chars.next_if_eq(&'\n');
                lines.push(Vec::new());
            }
            '\n' => lines.push(Vec::new()),
            '\t' => lines.last_mut().unwrap().push(machine.space()),
            c => match machine.code_for_char(c) {
                Some(code) => lines.last_mut().unwrap().push(code),
                None => skipped += 1,
            },
        }
    }
    (lines, skipped)
}
