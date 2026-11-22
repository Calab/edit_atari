//! Atari Microsoft BASIC programs (`SAVE "D:PROG"`), read into LIST text.
//!
//! A different family from Atari BASIC: there is no variable table, and
//! a line is stored as typed (names, numbers and spacing included) with
//! only the keywords replaced by tokens.
//!
//! File layout: `$00`, the length of what follows (2 bytes), then lines:
//! the address of the next line in memory (2 bytes; `$00 $00` ends the
//! program), the line number (2), the text, and `$00`.
//!
//! In the text, bytes `$80`-`$F8` are keywords ([`KEYWORDS`]) and `$FF`
//! followed by `$A3`-`$C5` is a function ([`FUNCTIONS`]). Strings and the
//! text of comments are stored as characters. Some things are stored
//! differently from how they're typed (see [`list_line`]).
//!
//! `SAVE "D:PROG" LOCK` writes a protected copy: the first byte is `$01`
//! instead of `$00`, and every byte after the 3-byte header is stored as
//! `$54` minus its value (see [`unlock`]). The editor reads both.
//!
//! Tables from the Microsoft BASIC II 2.0 cartridge ROM; checked against
//! a test program SAVEd and LISTed with it, and DIR from the Microsoft
//! BASIC 1.0 disk.

use retro_core::{Decoded, FileFormat};

/// A token number Microsoft BASIC doesn't use.
const INVALID: &str = "";

/// Keywords, statements and operators, from token `$80`.
const KEYWORDS: [&str; 0x79] = [
    "END",
    "FOR",
    "NEXT",
    "READ",
    "DATA",
    "RESTORE",
    "LINE",
    "INPUT", // $80
    "DEFSTR",
    "DEFINT",
    "DEFDBL",
    "DEFSNG",
    "OPTION",
    "DIM",
    "COMMON",
    "DEF", // $88
    "LET",
    "RANDOMIZE",
    "GOTO",
    "GOSUB",
    "RETURN",
    "GO",
    "RUN",
    "IF", // $90
    "REM",
    INVALID,
    "'",
    "!",
    "ELSE",
    "THEN",
    "STOP",
    "CONT", // $98
    "RESUME",
    "ERROR",
    "ON",
    "AFTER",
    "POKE",
    "WAIT",
    "MOVE",
    "LOAD", // $A0
    // $AB is `?`, which lists as `PRINT ` (with its space).
    "SAVE",
    "VERIFY",
    "MERGE",
    "PRINT ",
    "PRINT",
    "LIST",
    "AUTO",
    "DEL", // $A8
    "TRON",
    "TROFF",
    "RENUM",
    "OPEN",
    "CLOSE",
    "GET",
    "PUT",
    "NOTE", // $B0
    "LOCK",
    "UNLOCK",
    "KILL",
    "NAME",
    "GRAPHICS",
    "COLOR",
    "PLOT",
    "FILL", // $B8
    "SETCOLOR",
    "SOUND",
    "CLS",
    "DOS",
    "CLEAR",
    "NEW",
    "CLOAD",
    "CSAVE", // $C0
    INVALID,
    INVALID,
    INVALID,
    INVALID,
    INVALID,
    INVALID,
    INVALID,
    INVALID, // $C8
    INVALID,
    "BASE",
    "TAB(",
    "TO",
    "SUB",
    INVALID,
    "SPC(",
    "NOT", // $D0
    "STEP",
    "ALL",
    "USING",
    "RESERVE",
    "UPDATE",
    "OUTPUT",
    "APPEND",
    "AT(", // $D8
    "AT ",
    "PLM0",
    "PLM2",
    "PLM1",
    "CHR0",
    "CHR1",
    "CHR2",
    INVALID, // $E0
    INVALID,
    INVALID,
    INVALID,
    INVALID,
    INVALID,
    INVALID,
    "+",
    "-", // $E8
    "*",
    "/",
    "^",
    "AND",
    "OR",
    "XOR",
    ">",
    "=", // $F0
    "<", // $F8
];

/// Functions, `$FF` then a byte from `$A3`.
const FUNCTIONS: [&str; 35] = [
    "SGN", "INT", "ABS", "FRE", "POS", "SQR", "LOG", "EXP", // $FF $A3
    "COS", "SIN", "TAN", "ATN", "PEEK", "LEN", "STR$", "VAL", // $FF $AB
    "ASC", "CHR$", "STATUS", "EOF", "LEFT$", "RIGHT$", "MID$", "INSTR", // $FF $B3
    "STRING$", "VARPTR", "USR", "SCRN$", "RND", "TIME$", "TIME", "INKEY$", // $FF $BB
    "ERR", "ERL", "STACK", // $FF $C3
];

const FIRST_KEYWORD: u8 = 0x80;
const FUNCTION_PREFIX: u8 = 0xFF;
const FIRST_FUNCTION: u8 = 0xA3;

const DATA: u16 = 0x84;
const REM: u8 = 0x98;
const APOSTROPHE: u8 = 0x9A;
const BANG: u8 = 0x9B;
const ELSE: u8 = 0x9C;
/// A string's closing quote is stored as `|`.
const CLOSE_QUOTE: u8 = b'|';

/// First byte of a program file.
const PLAIN: u8 = 0x00;
/// First byte of a program saved with `SAVE ... LOCK`.
const LOCKED: u8 = 0x01;
/// A locked program stores each byte as this minus its value.
const LOCK_KEY: u8 = 0x54;

/// A locked program's bytes as an ordinary program file.
fn unlock(bytes: &[u8]) -> Vec<u8> {
    let mut v = bytes.to_vec();
    v[0] = PLAIN;
    for b in v.iter_mut().skip(3) {
        *b = LOCK_KEY.wrapping_sub(*b);
    }
    v
}

/// Microsoft BASIC program.
pub struct MicrosoftBasic;

impl FileFormat for MicrosoftBasic {
    fn name(&self) -> &'static str {
        "Microsoft BASIC"
    }

    fn extension(&self) -> &'static str {
        "AMB"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["AMB"]
    }

    fn shared_extensions(&self) -> &'static [&'static str] {
        &["BAS"]
    }

    fn detect(&self, bytes: &[u8]) -> bool {
        self.looks_like(bytes) && self.decode(bytes).is_ok()
    }

    /// `$00` (or `$01`, locked) and a length that matches the file.
    fn looks_like(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 3
            && matches!(bytes[0], PLAIN | LOCKED)
            && u16::from_le_bytes([bytes[1], bytes[2]]) as usize + 3 == bytes.len()
    }

    fn decode(&self, bytes: &[u8]) -> Result<Decoded, String> {
        let unlocked;
        let bytes = match bytes.first() {
            Some(&PLAIN) => bytes,
            Some(&LOCKED) => {
                unlocked = unlock(bytes);
                &unlocked[..]
            }
            _ => return Err("not a Microsoft BASIC file".into()),
        };
        if bytes.len() < 3 {
            return Err("not a Microsoft BASIC file".into());
        }
        let size = u16::from_le_bytes([bytes[1], bytes[2]]) as usize;
        let program = bytes
            .get(3..3 + size)
            .ok_or("file is shorter than its header says")?;
        let mut lines = Vec::new();
        let mut pos = 0;
        loop {
            let next = program
                .get(pos..pos + 2)
                .ok_or("program ends part way through a line")?;
            if next == [0, 0] {
                break;
            }
            let number = program
                .get(pos + 2..pos + 4)
                .map(|n| u16::from_le_bytes([n[0], n[1]]))
                .ok_or("program ends part way through a line")?;
            let rest = &program[pos + 4..];
            let end = rest
                .iter()
                .position(|&b| b == 0)
                .ok_or_else(|| format!("line {number} has no end"))?;
            let mut text = number.to_string().into_bytes();
            text.push(b' ');
            list_line(&rest[..end], &mut text).map_err(|e| format!("line {number}: {e}"))?;
            lines.push(text);
            pos += 4 + end + 1;
        }
        if lines.is_empty() {
            return Err("no program lines".into());
        }
        Ok(Decoded {
            lines,
            final_eol: true,
            converted_from: None,
        })
    }

    fn can_encode(&self) -> bool {
        true
    }

    fn encode(&self, doc: &retro_core::Document) -> Result<retro_core::Encoded, String> {
        Ok(retro_core::Encoded::clean(encode(doc.lines())?))
    }
}

/// Where Microsoft BASIC II keeps the first program line in memory; the
/// next-line addresses in a file count from here.
const PROGRAM_START: u16 = 0x0701;

/// LIST text back into a SAVE file. Blank lines are skipped; lines are
/// sorted by number, a later line replacing an earlier one with the same
/// number, as when typing them in.
pub fn encode(lines: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let mut numbered: Vec<(u16, Vec<u8>)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let start = line.iter().take_while(|&&c| c == b' ').count();
        let text = &line[start..];
        if text.is_empty() {
            continue;
        }
        let digits = text.iter().take_while(|c| c.is_ascii_digit()).count();
        let number = std::str::from_utf8(&text[..digits])
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .filter(|&n| n <= 63999)
            .ok_or_else(|| format!("line {} of the text has no line number (0-63999)", i + 1))?;
        let rest = &text[digits..];
        let rest = &rest[rest.iter().take_while(|&&c| c == b' ').count()..];
        numbered.push((number, crunch(rest)));
    }
    numbered.sort_by_key(|(n, _)| *n); // stable: the later of two equal lines comes last
    numbered.reverse();
    numbered.dedup_by_key(|(n, _)| *n); // keeps the first, which is now the later one
    numbered.reverse();

    let mut body = Vec::new();
    let mut addr = PROGRAM_START as usize;
    for (number, text) in &numbered {
        addr += 4 + text.len() + 1;
        let next = u16::try_from(addr).map_err(|_| "program too large".to_string())?;
        body.extend_from_slice(&next.to_le_bytes());
        body.extend_from_slice(&number.to_le_bytes());
        body.extend_from_slice(text);
        body.push(0);
    }
    body.extend_from_slice(&[0, 0]);
    let size = u16::try_from(body.len()).map_err(|_| "program too large".to_string())?;
    let mut f = vec![PLAIN];
    f.extend_from_slice(&size.to_le_bytes());
    f.extend(body);
    Ok(f)
}

/// One line's text with keywords crunched to tokens, the reverse of
/// [`list_line`]: a keyword counts only where it isn't part of a longer
/// name (PUTIOCB stays as typed); `?` is PRINT; a string's closing quote
/// is stored as `|`; `'`, `!` and ELSE get their hidden `:`; the rest of
/// a REM, `'` or `!` line, and a DATA statement, stay as typed.
fn crunch(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    let name_char = |c: u8| c.is_ascii_alphanumeric();
    while i < text.len() {
        let c = text[i];
        if c == b'"' {
            out.push(b'"');
            i += 1;
            while i < text.len() && text[i] != b'"' {
                out.push(text[i]);
                i += 1;
            }
            if i < text.len() {
                out.push(CLOSE_QUOTE);
                i += 1;
            }
            continue;
        }
        let after_name = i > 0 && (name_char(text[i - 1]) || text[i - 1] == b'.');
        let rest = &text[i..];
        // `!` straight after a name is a type suffix (X!), not a comment.
        let token = if c == b'?' {
            Some((0xAB_u16, 1))
        } else if c == b'\'' || (c == b'!' && !after_name) {
            Some((u16::from(if c == b'!' { BANG } else { APOSTROPHE }), 1))
        } else if after_name && c.is_ascii_alphabetic() {
            None
        } else {
            keyword(rest)
        };
        let Some((token, len)) = token else {
            out.push(c);
            i += 1;
            continue;
        };
        let is = |t: u8| token == u16::from(t);
        if is(APOSTROPHE) || is(BANG) || is(ELSE) {
            out.push(b':');
        }
        out.extend_from_slice(&token_bytes(token));
        i += len;
        if is(REM) || is(APOSTROPHE) || is(BANG) {
            out.extend_from_slice(&text[i..]);
            return out;
        }
        if token == DATA {
            // As typed up to the end of the statement.
            let mut quoted = false;
            while i < text.len() && (quoted || text[i] != b':') {
                quoted ^= text[i] == b'"';
                out.push(text[i]);
                i += 1;
            }
        }
    }
    out
}

/// Encoded functions are written as `$FF` and a byte; this marks them in
/// [`keyword`]'s result.
const FUNCTION_FLAG: u16 = 0x100;

fn token_bytes(token: u16) -> Vec<u8> {
    if token & FUNCTION_FLAG != 0 {
        vec![FUNCTION_PREFIX, token as u8]
    } else {
        vec![token as u8]
    }
}

/// The keyword `text` starts with, as (token, length): the first in table
/// order, keywords then functions, that isn't followed by a letter when it
/// ends in one (so a name like PUTIOCB isn't crunched).
fn keyword(text: &[u8]) -> Option<(u16, usize)> {
    let fits = |name: &str| {
        let n = name.as_bytes();
        text.starts_with(n)
            && !(n.last().is_some_and(|c| c.is_ascii_alphabetic())
                && text.get(n.len()).is_some_and(|c| c.is_ascii_alphabetic()))
    };
    for (k, name) in KEYWORDS.iter().enumerate() {
        if !name.is_empty() && *name != "'" && *name != "!" && fits(name) {
            return Some((u16::from(FIRST_KEYWORD) + k as u16, name.len()));
        }
    }
    for (k, name) in FUNCTIONS.iter().enumerate() {
        if fits(name) {
            return Some((
                FUNCTION_FLAG | (u16::from(FIRST_FUNCTION) + k as u16),
                name.len(),
            ));
        }
    }
    None
}

/// One line's text as LIST shows it:
///
/// * keywords spelled out, everything else as stored;
/// * a string's closing quote is stored as `|`, and listed as `"`;
/// * `'`, `!` and ELSE are stored with a `:` before them, which LIST
///   leaves out (`:'` lists as `'`);
/// * after REM, `'` or `!` the rest of the line is plain text.
fn list_line(d: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
    let mut i = 0;
    while i < d.len() {
        let b = d[i];
        i += 1;
        match b {
            b'"' => {
                // A string: characters up to the closing quote.
                out.push(b'"');
                while i < d.len() {
                    let c = d[i];
                    i += 1;
                    if c == CLOSE_QUOTE {
                        out.push(b'"');
                        break;
                    }
                    out.push(c);
                }
            }
            b':' if matches!(d.get(i), Some(&(APOSTROPHE | BANG | ELSE))) => {}
            FUNCTION_PREFIX => {
                let f = d.get(i).ok_or("function cut short")?;
                let name = f
                    .checked_sub(FIRST_FUNCTION)
                    .and_then(|n| FUNCTIONS.get(n as usize))
                    .ok_or_else(|| format!("unknown function token $FF ${f:02X}"))?;
                out.extend_from_slice(name.as_bytes());
                i += 1;
            }
            FIRST_KEYWORD.. => {
                let name = KEYWORDS
                    .get((b - FIRST_KEYWORD) as usize)
                    .filter(|n| !n.is_empty())
                    .ok_or_else(|| format!("unknown token ${b:02X}"))?;
                out.extend_from_slice(name.as_bytes());
                if matches!(b, REM | APOSTROPHE | BANG) {
                    out.extend_from_slice(&d[i..]);
                    return Ok(());
                }
            }
            _ => out.push(b),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program file from lines of (number, stored text).
    fn file(lines: &[(u16, &[u8])]) -> Vec<u8> {
        let mut body = Vec::new();
        let mut addr = 0x0700u16;
        for (n, t) in lines {
            addr += 4 + t.len() as u16 + 1;
            body.extend_from_slice(&addr.to_le_bytes());
            body.extend_from_slice(&n.to_le_bytes());
            body.extend_from_slice(t);
            body.push(0);
        }
        body.extend_from_slice(&[0, 0]);
        let mut f = vec![0];
        f.extend_from_slice(&(body.len() as u16).to_le_bytes());
        f.extend(body);
        f
    }

    fn listing(f: &[u8]) -> Vec<String> {
        let d = MicrosoftBasic.decode(f).unwrap();
        d.lines
            .iter()
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .collect()
    }

    #[test]
    fn lists_as_typed() {
        let f = file(&[
            (10, b"\x98 HELLO \xa0WORLD"),         // REM, with an inverse character
            (20, b":\x9a APOSTROPHE"),             // ' comment
            (30, b"\xac \"HI|:\xab \"Q| ::\x9bX"), // PRINT "HI":? "Q" :!X
            (40, b"\x97 A=1 \x9d \xac \"Y| :\x9c \xac \"N|"),
            (50, b"X=\xff\xaf(1536)+\xff\xc5"), // PEEK(1536)+STACK
        ]);
        assert!(MicrosoftBasic.detect(&f));
        assert_eq!(
            listing(&f),
            [
                "10 REM HELLO \u{fffd}WORLD",
                "20 ' APOSTROPHE",
                "30 PRINT \"HI\":PRINT  \"Q\" :!X",
                "40 IF A=1 THEN PRINT \"Y\" ELSE PRINT \"N\"",
                "50 X=PEEK(1536)+STACK",
            ]
        );
    }

    #[test]
    fn reads_locked_programs() {
        let plain = file(&[(10, b"\x98 HI"), (20, b"\xac \"X|")]);
        let mut locked = plain.clone();
        locked[0] = LOCKED;
        for b in locked.iter_mut().skip(3) {
            *b = LOCK_KEY.wrapping_sub(*b);
        }
        assert!(MicrosoftBasic.detect(&locked));
        assert_eq!(listing(&locked), listing(&plain));
    }

    #[test]
    fn crunches_back_the_same() {
        let f = file(&[
            (10, b"\x98 HELLO"),
            (20, b":\x9a APOSTROPHE"),
            (30, b"\xab\"HI|:\xab \"Q| ::\x9bX"),
            // PRINT followed by a space is always $AB ("PRINT "), as
            // Microsoft BASIC stores it.
            (40, b"\x97 A\xf71 \x9d \xab\"Y| :\x9c \xab\"N|"),
            (50, b"X\xf7\xff\xaf(1536)\xee\xff\xc5"),
            (60, b"PUTIOCB\xf7ADDR\xee61"),
            (70, b"\x84 1,2, THREE:\xab"),
        ]);
        let text = MicrosoftBasic.decode(&f).unwrap().lines;
        let ours = encode(&text).unwrap();
        // The same line contents (the next-line addresses depend on where
        // the program starts in memory).
        assert_eq!(MicrosoftBasic.decode(&ours).unwrap().lines, text);
        assert_eq!(lines_of(&ours), lines_of(&f));
    }

    /// Each line's number and stored text, without the addresses.
    fn lines_of(f: &[u8]) -> Vec<Vec<u8>> {
        let mut v = Vec::new();
        let mut i = 3;
        while f[i] != 0 || f[i + 1] != 0 {
            let end = i + 4 + f[i + 4..].iter().position(|&b| b == 0).unwrap();
            v.push(f[i + 2..end].to_vec());
            i = end + 1;
        }
        v
    }

    #[test]
    fn not_microsoft_basic() {
        assert!(!MicrosoftBasic.detect(b"10 PRINT\x9b"));
        // An Atari BASIC header: $00 $00 LOMEM, so no matching length.
        assert!(!MicrosoftBasic.detect(&[0, 0, 0, 1, 2, 3]));
    }
}
