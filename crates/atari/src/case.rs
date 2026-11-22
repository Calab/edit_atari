//! Uppercasing imported listings.
//!
//! PC copies of Atari programs are often in lower case, which Atari BASIC
//! and the assemblers don't accept. When an imported file is a numbered
//! BASIC or assembler listing with lower-case letters, Import offers to
//! convert them, leaving alone everything the program would treat as text:
//!
//! * BASIC: strings, and the rest of the line after `REM`, `DATA`, their
//!   abbreviations `R.` `D.` `.`, and the `!` `'` `--` remarks of other
//!   dialects, at the start of a statement (after the line number, a `:`,
//!   `THEN` or `ELSE`).
//! * Assembler: strings, `'c` character constants and `;` comments.

/// What kind of listing a file is, for [`uppercase`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listing {
    Basic,
    Assembler,
}

/// BASIC statements, for recognising a listing (any dialect).
const BASIC_WORDS: &[&str] = &[
    "BYE", "CLOAD", "CLOSE", "CLR", "CLS", "COLOR", "CONT", "CSAVE", "DATA", "DEG", "DIM", "DOS",
    "DRAWTO", "END", "ENTER", "FOR", "GET", "GOSUB", "GOTO", "GRAPHICS", "IF", "INPUT", "LET",
    "LIST", "LOAD", "LOCATE", "LPRINT", "NEW", "NEXT", "NOTE", "ON", "OPEN", "PLOT", "POINT",
    "POKE", "POP", "POSITION", "PRINT", "PUT", "RAD", "READ", "REM", "RESTORE", "RETURN", "RUN",
    "SAVE", "SETCOLOR", "SOUND", "STATUS", "STOP", "TRAP", "XIO",
];

/// 6502 instructions, for recognising an assembler listing.
const MNEMONICS: &[&str] = &[
    "ADC", "AND", "ASL", "BCC", "BCS", "BEQ", "BIT", "BMI", "BNE", "BPL", "BRK", "BVC", "BVS",
    "CLC", "CLD", "CLI", "CLV", "CMP", "CPX", "CPY", "DEC", "DEX", "DEY", "EOR", "INC", "INX",
    "INY", "JMP", "JSR", "LDA", "LDX", "LDY", "LSR", "NOP", "ORA", "PHA", "PHP", "PLA", "PLP",
    "ROL", "ROR", "RTI", "RTS", "SBC", "SEC", "SED", "SEI", "STA", "STX", "STY", "TAX", "TAY",
    "TSX", "TXA", "TXS", "TYA",
];

/// The text after a line number, if the line starts with one.
fn after_number(line: &[u8]) -> Option<usize> {
    let digits = line.iter().take_while(|c| c.is_ascii_digit()).count();
    (digits > 0 && line.get(digits) == Some(&b' ')).then_some(digits + 1)
}

/// The word (letters only) at `i`, in capitals, and where it ends.
fn word_at(line: &[u8], i: usize) -> (String, usize) {
    let end = i + line[i..]
        .iter()
        .take_while(|c| c.is_ascii_alphabetic())
        .count();
    (
        String::from_utf8_lossy(&line[i..end]).to_ascii_uppercase(),
        end,
    )
}

fn skip_spaces(line: &[u8], i: usize) -> usize {
    i + line[i..].iter().take_while(|&&c| c == b' ').count()
}

/// How a numbered line votes: BASIC, assembler, or neither.
fn vote(line: &[u8]) -> Option<Listing> {
    let i = skip_spaces(line, after_number(line)?);
    let c = *line.get(i)?;
    if c == b'?' {
        return Some(Listing::Basic);
    }
    if c == b';' || (c == b'.' && line.get(i + 1).is_some_and(u8::is_ascii_alphabetic)) {
        return Some(Listing::Assembler);
    }
    let (w, end) = word_at(line, i);
    if w.is_empty() {
        return None;
    }
    // An abbreviation (PR.) is BASIC.
    if line.get(end) == Some(&b'.') || BASIC_WORDS.contains(&w.as_str()) {
        return Some(Listing::Basic);
    }
    if MNEMONICS.contains(&w.as_str()) {
        return Some(Listing::Assembler);
    }
    // A label, then an instruction or directive.
    let j = skip_spaces(line, end);
    if j > end {
        if line.get(j) == Some(&b'.') {
            return Some(Listing::Assembler);
        }
        if MNEMONICS.contains(&word_at(line, j).0.as_str()) {
            return Some(Listing::Assembler);
        }
    }
    None
}

/// What kind of listing these lines are: most non-blank lines numbered,
/// and a clear majority of those recognised as one kind.
pub fn listing_kind(lines: &[Vec<u8>]) -> Option<Listing> {
    let filled: Vec<&Vec<u8>> = lines.iter().filter(|l| !l.is_empty()).collect();
    let numbered = filled.iter().filter(|l| after_number(l).is_some()).count();
    if filled.is_empty() || numbered * 2 < filled.len() {
        return None;
    }
    let (mut basic, mut asm) = (0, 0);
    for l in &filled {
        match vote(l) {
            Some(Listing::Basic) => basic += 1,
            Some(Listing::Assembler) => asm += 1,
            None => {}
        }
    }
    if basic > asm * 3 {
        Some(Listing::Basic)
    } else if asm > basic * 3 {
        Some(Listing::Assembler)
    } else {
        None
    }
}

/// A BASIC line with lower case outside strings and remarks in capitals.
fn upper_basic(line: &[u8]) -> Vec<u8> {
    let mut out = line.to_vec();
    let Some(start) = after_number(line) else {
        return out;
    };
    let mut i = start;
    let mut statement = true;
    while i < line.len() {
        if statement {
            i = skip_spaces(line, i);
            let rest = &line[i..];
            let (w, end) = word_at(line, i);
            let remark = matches!(w.as_str(), "REM" | "DATA")
                || ((w == "R" || w == "D") && line.get(end) == Some(&b'.'))
                || rest.starts_with(b".")
                || rest.starts_with(b"!")
                || rest.starts_with(b"'")
                || rest.starts_with(b"--");
            if remark {
                // The keyword itself in capitals; the text as it is.
                out[i..end].make_ascii_uppercase();
                return out;
            }
            statement = false;
            continue;
        }
        match line[i] {
            b'"' => {
                // To the closing quote, or the end of the line.
                i += 1;
                i += line[i..].iter().take_while(|&&c| c != b'"').count();
            }
            b':' => statement = true,
            c if c.is_ascii_alphabetic() => {
                let (w, end) = word_at(line, i);
                out[i..end].make_ascii_uppercase();
                // A statement follows THEN and ELSE (unless it's a line
                // number, where it does no harm).
                if (w == "THEN" || w == "ELSE") && !line[i - 1].is_ascii_alphabetic() {
                    statement = true;
                }
                i = end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// An assembler line with lower case outside strings, character constants
/// and comments in capitals.
fn upper_assembler(line: &[u8]) -> Vec<u8> {
    let mut out = line.to_vec();
    let mut i = after_number(line).unwrap_or(0);
    while i < line.len() {
        match line[i] {
            b';' => break,
            b'"' => {
                i += 1;
                i += line[i..].iter().take_while(|&&c| c != b'"').count();
            }
            b'\'' => i += 1,
            c if c.is_ascii_lowercase() => out[i] = c.to_ascii_uppercase(),
            _ => {}
        }
        i += 1;
    }
    out
}

/// The lines uppercased as their kind allows, and how many changed, if
/// any did.
pub fn uppercase(lines: &[Vec<u8>], kind: Listing) -> Option<(Vec<Vec<u8>>, usize)> {
    let out: Vec<Vec<u8>> = lines
        .iter()
        .map(|l| match kind {
            Listing::Basic => upper_basic(l),
            Listing::Assembler => upper_assembler(l),
        })
        .collect();
    let changed = out.iter().zip(lines).filter(|(a, b)| a != b).count();
    (changed > 0).then_some((out, changed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<Vec<u8>> {
        text.lines().map(|l| l.as_bytes().to_vec()).collect()
    }

    fn upper(text: &str) -> String {
        let l = lines(text);
        let kind = listing_kind(&l).expect("a listing");
        match uppercase(&l, kind) {
            Some((out, _)) => out
                .iter()
                .map(|l| String::from_utf8_lossy(l).into_owned())
                .collect::<Vec<_>>()
                .join("\n"),
            None => text.to_string(),
        }
    }

    #[test]
    fn basic_keeps_strings_and_remarks() {
        assert_eq!(
            upper(
                "10 rem a comment: here\n\
                 20 print \"Hello\";a$:goto 10\n\
                 30 data abc,Def\n\
                 40 if x then rem keep\n\
                 50 x=1:. keep\n\
                 60 ? \"open\":d. kept"
            ),
            "10 REM a comment: here\n\
             20 PRINT \"Hello\";A$:GOTO 10\n\
             30 DATA abc,Def\n\
             40 IF X THEN REM keep\n\
             50 X=1:. keep\n\
             60 ? \"open\":D. kept"
        );
    }

    #[test]
    fn assembler_keeps_strings_and_comments() {
        assert_eq!(
            upper(
                "10 ; set up\n\
                 20 start lda #'a ; comment\n\
                 30  .byte \"Text\",$9b\n\
                 40  sta $d01a"
            ),
            "10 ; set up\n\
             20 START LDA #'a ; comment\n\
             30  .BYTE \"Text\",$9B\n\
             40  STA $D01A"
        );
    }

    #[test]
    fn only_listings_qualify() {
        assert_eq!(listing_kind(&lines("Some prose.\nMore prose.")), None);
        // Already in capitals: nothing to offer.
        let l = lines("10 PRINT \"hi\"\n20 GOTO 10");
        assert_eq!(listing_kind(&l), Some(Listing::Basic));
        assert_eq!(uppercase(&l, Listing::Basic), None);
    }
}
