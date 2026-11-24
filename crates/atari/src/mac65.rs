//! MAC/65 tokenized source files (`SAVE "D:PROG.M65"`), read into LIST
//! text.
//!
//! File layout: `$FE $FE`, the length of what follows (2 bytes), then
//! lines: line number (2 bytes), line length including these 3 bytes (1),
//! then the line's tokens:
//!
//! * an optional label: `$80 + length`, then its characters;
//! * a statement token (a directive or opcode, see [`STATEMENTS`]);
//! * operand tokens (see [`operand`]). Names and strings are stored as
//!   `$80 + length` and their characters.
//!
//! Token tables from the MAC/65 1.01 cartridge ROM; checked against the
//! sources on the MAC/65 and MAC/65 Toolkit disks.

use retro_core::{Decoded, FileFormat};

/// Statement tokens: directives and 6502/65C02 opcodes, by token.
const STATEMENTS: [&str; 0x60] = [
    "ERROR - ", ".IF", ".ELSE", ".ENDIF", ".MACRO", ".ENDM", ".TITLE",
    "", // $00 ($07: macro call)
    ".PAGE", ".WORD", ".ERROR", ".BYTE", ".SBYTE", ".DBYTE", ".END", ".OPT", // $08
    ".TAB", ".INCLUDE", ".DS", ".ORG", ".EQU", "BRA", "TRB", "TSB", // $10
    ".FLOAT", ".CBYTE", ";", ".LOCAL", ".SET", "*=", "=", ".=", // $18
    "JSR", "JMP", "DEC", "INC", "LDX", "LDY", "STX", "STY", // $20
    "CPX", "CPY", "BIT", "BRK", "CLC", "CLD", "CLI", "CLV", // $28
    "DEX", "DEY", "INX", "INY", "NOP", "PHA", "PHP", "PLA", // $30
    "PLP", "RTI", "RTS", "SEC", "SED", "SEI", "TAX", "TAY", // $38
    "TSX", "TXA", "TXS", "TYA", "BCC", "BCS", "BEQ", "BMI", // $40
    "BNE", "BPL", "BVC", "BVS", "ORA", "AND", "EOR", "ADC", // $48
    "STA", "LDA", "CMP", "SBC", "ASL", "ROL", "LSR", "ROR", // $50
    ";", "STZ", "DEA", "INA", "PHX", "PHY", "PLX", "PLY", // $58
];

/// A line that didn't assemble: the rest is the text as typed.
const ERROR: u8 = 0x00;
/// A macro call: the macro's name (a string) follows.
const MACRO_CALL: u8 = 0x07;
/// .MACRO: the macro's name (a string) follows.
const MACRO_DEF: u8 = 0x04;
const OPT: u8 = 0x0F;
/// The string quote and the accumulator operand.
const QUOTE: u8 = 0x41;
const ACCUMULATOR: u8 = 0x3F;
/// A comment in the instruction column; the rest is its text (with `;`).
const COMMENT: u8 = 0x1A;
/// A comment starting in the first column; the rest is its text.
const COMMENT_LINE: u8 = 0x58;

/// Operand tokens with their own data.
const HEX_WORD: u8 = 0x05;
const HEX_BYTE: u8 = 0x06;
const DEC_WORD: u8 = 0x07;
const DEC_BYTE: u8 = 0x08;
const CHAR: u8 = 0x0A;
/// A comment after the operand; the rest of the line is its text.
const OPERAND_COMMENT: u8 = 0x3B;

/// Operand tokens without data, as LIST shows them.
fn operand_token(t: u8) -> Option<&'static str> {
    Some(match t {
        0x0B => "%$", // macro string parameter: %$1
        0x0C => "%",  // macro parameter: %1
        0x0D => "*",  // the location counter
        0x12 => "+",
        0x13 => "-",
        0x14 => "*",
        0x15 => "/",
        0x16 => "&",
        0x18 => "=",
        0x19 => "<=",
        0x1A => ">=",
        0x1B => "<>",
        0x1C => ">",
        0x1D => "<",
        0x1E => "-", // unary minus
        0x1F => "[",
        0x20 => "]",
        0x24 => "!",
        0x25 => "^",
        0x27 => "\\",
        0x2F => ".REF ",
        0x30 => ".DEF ",
        0x31 => ".NOT ",
        0x32 => " .AND ",
        0x33 => " .OR ",
        0x34 => " <", // low byte, as in `LDA # <ADDR`
        0x35 => " >", // high byte
        0x36 => ",X)",
        0x37 => "),Y",
        0x38 => ",Y",
        0x39 => ",X",
        0x3A => ")",
        0x3D => ",",
        0x3E => "#",
        0x3F => "A",
        0x40 => "(",
        0x41 => "\"",
        0x45 => "NO ",
        0x46 => "OBJ",
        0x47 => "ERR",
        0x48 => "EJECT",
        0x49 => "LIST",
        0x4A => "XREF",
        0x4B => "MLIST",
        0x4C => "CLIST",
        0x4D => "NUM",
        _ => return None,
    })
}

/// MAC/65 source.
pub struct Mac65;

impl FileFormat for Mac65 {
    fn name(&self) -> &'static str {
        "MAC/65"
    }

    fn extension(&self) -> &'static str {
        "M65"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["M65"]
    }

    fn detect(&self, bytes: &[u8]) -> bool {
        bytes.starts_with(&[0xFE, 0xFE]) && self.decode(bytes).is_ok()
    }

    fn looks_like(&self, bytes: &[u8]) -> bool {
        bytes.starts_with(&[0xFE, 0xFE]) && bytes.len() >= 4
    }

    fn decode(&self, bytes: &[u8]) -> Result<Decoded, String> {
        let bad = || "not a MAC/65 file".to_string();
        if !bytes.starts_with(&[0xFE, 0xFE]) || bytes.len() < 4 {
            return Err(bad());
        }
        let size = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
        let program = bytes
            .get(4..4 + size)
            .ok_or("file is shorter than its header says")?;
        let mut lines = Vec::new();
        let mut pos = 0;
        let mut depth = 0;
        while pos < program.len() {
            let rest = &program[pos..];
            if rest.len() < 3 {
                return Err("program ends part way through a line".into());
            }
            let number = u16::from_le_bytes([rest[0], rest[1]]);
            let len = rest[2] as usize;
            if len < 3 || len > rest.len() {
                return Err(format!("line {number}: bad length"));
            }
            let mut text = line_number(number).into_bytes();
            text.push(b' ');
            list_line(&rest[3..len], &mut text, &mut depth)
                .map_err(|e| format!("line {number}: {e}"))?;
            lines.push(text);
            pos += len;
        }
        if lines.is_empty() {
            lines.push(Vec::new());
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
        let (bytes, errors) = encode(doc.lines())?;
        Ok(retro_core::Encoded {
            bytes,
            problem_lines: errors.into_iter().map(|(n, i)| (u32::from(n), i)).collect(),
        })
    }
}

/// Lines that didn't parse: (line number, index in the editor).
pub type ErrorLines = Vec<(u16, usize)>;

/// LIST text back into a SAVE file, and the lines that didn't parse
/// (stored as ERROR - lines, as MAC/65 stores them). Blank lines are
/// skipped; lines are sorted by number, a later line replacing an
/// earlier one with the same number.
pub fn encode(lines: &[Vec<u8>]) -> Result<(Vec<u8>, ErrorLines), String> {
    let mut numbered: Vec<(u16, usize, &[u8])> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.iter().all(|&c| c == b' ') {
            continue;
        }
        let digits = line.iter().take_while(|c| c.is_ascii_digit()).count();
        let number = std::str::from_utf8(&line[..digits])
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(|| format!("editor line {} has no line number (0-65535)", i + 1))?;
        numbered.push((number, i, line));
    }
    numbered.sort_by_key(|&(n, ..)| n);
    numbered.reverse();
    numbered.dedup_by_key(|&mut (n, ..)| n); // keeps the later of two equal lines
    numbered.reverse();

    let mut body = Vec::new();
    let mut errors = Vec::new();
    let mut too_long = Vec::new();
    for &(number, index, line) in &numbered {
        let tokens = match tokenize_line(line) {
            Ok(t) => t,
            Err(at) => {
                errors.push((number, index));
                error_line(line, at)
            }
        };
        let len = tokens.len() + 3;
        if len > 255 {
            too_long.push((number, index, len));
            continue;
        }
        body.extend_from_slice(&number.to_le_bytes());
        body.push(len as u8);
        body.extend(tokens);
    }
    if !too_long.is_empty() {
        return Err(crate::tokenize::too_long_message(&too_long));
    }
    let size = u16::try_from(body.len()).map_err(|_| "program too large".to_string())?;
    let mut f = vec![0xFE, 0xFE];
    f.extend_from_slice(&size.to_le_bytes());
    f.extend(body);
    Ok((f, errors))
}

/// Where a line's text starts: after the line number and its space.
fn text_start(line: &[u8]) -> usize {
    let digits = line.iter().take_while(|c| c.is_ascii_digit()).count();
    (digits + 1).min(line.len())
}

/// An ERROR - line: the text as typed, the character at `at` (an index
/// into the whole line) in inverse video.
fn error_line(line: &[u8], at: usize) -> Vec<u8> {
    let start = text_start(line);
    let mut text = line[start..].to_vec();
    let skip = text.iter().take_while(|&&c| c == b' ').count();
    text.drain(..skip);
    if let Some(c) = at.checked_sub(start + skip).and_then(|i| text.get_mut(i)) {
        *c |= 0x80;
    }
    let mut v = vec![ERROR];
    v.extend(text);
    v
}

/// Characters that can be part of a name: labels, symbols, macro names
/// and file names (`D:SYSEQU.M65`).
fn name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'@' | b'?' | b'.' | b'_' | b':')
}

/// A name or string stored as `$80 + length` and its characters.
fn counted_bytes(s: &[u8]) -> Vec<u8> {
    let mut v = vec![0x80 + s.len() as u8];
    v.extend_from_slice(s);
    v
}

/// One line as LIST shows it back into tokens; errors are the position
/// in `line` where parsing failed.
fn tokenize_line(line: &[u8]) -> Result<Vec<u8>, usize> {
    let mut i = text_start(line);
    let mut out = Vec::new();
    let rest = &line[i..];
    if rest.starts_with(b"ERROR - ") {
        // A line LIST showed as an error goes back as one.
        out.push(ERROR);
        out.extend_from_slice(&rest[8..]);
        return Ok(out);
    }
    if rest.first() == Some(&b';') {
        out.push(COMMENT_LINE);
        out.extend_from_slice(rest);
        return Ok(out);
    }
    // A label starts straight after the line number.
    if rest.first().is_some_and(|&c| c != b' ') {
        let n = rest.iter().take_while(|&&c| name_char(c)).count();
        if n == 0 {
            return Err(i);
        }
        out.extend(counted_bytes(&rest[..n]));
        i += n;
    }
    let skip = |i: &mut usize| {
        while line.get(*i) == Some(&b' ') {
            *i += 1;
        }
    };
    skip(&mut i);
    if i >= line.len() {
        return Ok(out); // a label on its own
    }
    if line[i] == b';' {
        // A comment in the opcode column: `;`, then the rest of its text
        // from the comment column.
        out.push(COMMENT);
        out.push(b';');
        i += 1;
        while i < COMMENT_COLUMN && line.get(i) == Some(&b' ') {
            i += 1;
        }
        out.extend_from_slice(&line[i.min(line.len())..]);
        return Ok(out);
    }
    // The opcode, directive or macro name: up to a space.
    let start = i;
    while line.get(i).is_some_and(|&c| c != b' ') {
        i += 1;
    }
    let word = &line[start..i];
    let token = STATEMENTS
        .iter()
        .position(|s| !s.is_empty() && *s != ";" && s.as_bytes().eq_ignore_ascii_case(word))
        .filter(|&t| t as u8 != ERROR && t as u8 != COMMENT_LINE);
    match token {
        Some(t) => out.push(t as u8),
        None => {
            if !word.iter().all(|&c| name_char(c)) {
                return Err(start);
            }
            // A macro call: the name, keeping the space after it.
            out.push(MACRO_CALL);
            let mut name = word.to_vec();
            name.push(b' ');
            out.extend(counted_bytes(&name));
        }
    }
    skip(&mut i);
    if token == Some(MACRO_DEF as usize) {
        // .MACRO NAME: the name keeps the space after it too.
        let start = i;
        while line.get(i).is_some_and(|&c| name_char(c)) {
            i += 1;
        }
        let mut name = line[start..i].to_vec();
        name.push(b' ');
        out.extend(counted_bytes(&name));
        skip(&mut i);
    }
    operand_tokens(line, i, token == Some(OPT as usize), &mut out)?;
    Ok(out)
}

/// Operand tokens from `i` to the end of the line.
fn operand_tokens(line: &[u8], mut i: usize, opt: bool, out: &mut Vec<u8>) -> Result<(), usize> {
    let mut value_expected = true;
    let at = |i: usize, s: &[u8]| line[i.min(line.len())..].starts_with(s);
    while i < line.len() {
        let c = line[i];
        if c == b' ' {
            i += 1;
            continue;
        }
        if c == b';' {
            out.push(OPERAND_COMMENT);
            out.extend_from_slice(&line[i..]);
            return Ok(());
        }
        if c == b'"' {
            // A string: the quote token, the counted text, the quote again.
            out.push(QUOTE);
            let start = i + 1;
            let mut j = start;
            while j < line.len() && line[j] != b'"' {
                j += 1;
            }
            out.extend(counted_bytes(&line[start..j]));
            if j < line.len() {
                out.push(QUOTE);
                j += 1;
            }
            i = j;
            value_expected = false;
            continue;
        }
        if value_expected {
            if opt
                && let Some(&(w, t)) = OPT_WORDS.iter().find(|(w, _)| {
                    at(i, w.as_bytes()) && !line.get(i + w.len()).is_some_and(|&c| name_char(c))
                })
            {
                out.push(t);
                i += w.len();
                // NO goes with the word after it (NO LIST).
                value_expected = w == "NO";
                continue;
            }
            // Unary and prefix tokens keep a value expected.
            let prefix = [
                (&b".NOT"[..], 0x31),
                (b".DEF", 0x30),
                (b".REF", 0x2F),
                (b"%$", 0x0B),
                (b"%", 0x0C),
                (b"-", 0x1E),
                (b"+", 0x12),
                (b"<", 0x34),
                (b">", 0x35),
                (b"[", 0x1F),
                (b"(", 0x40),
                (b"#", 0x3E),
            ];
            if let Some(&(w, t)) = prefix.iter().find(|(w, _)| at(i, w)) {
                out.push(t);
                i += w.len();
                continue;
            }
            if c == b'*' {
                out.push(0x0D);
                i += 1;
            } else if c == b'\'' {
                out.push(CHAR);
                out.push(*line.get(i + 1).ok_or(i)?);
                i += 2;
            } else if c == b'$' {
                let start = i + 1;
                let mut j = start;
                while line.get(j).is_some_and(|c| c.is_ascii_hexdigit()) {
                    j += 1;
                }
                let v = std::str::from_utf8(&line[start..j])
                    .ok()
                    .and_then(|h| u16::from_str_radix(h, 16).ok())
                    .ok_or(i)?;
                number(out, v, HEX_BYTE, HEX_WORD);
                i = j;
            } else if c.is_ascii_digit() {
                let start = i;
                while line.get(i).is_some_and(|c| c.is_ascii_digit()) {
                    i += 1;
                }
                let v: u16 = std::str::from_utf8(&line[start..i])
                    .ok()
                    .and_then(|d| d.parse().ok())
                    .ok_or(start)?;
                number(out, v, DEC_BYTE, DEC_WORD);
            } else if name_char(c) {
                let start = i;
                while line.get(i).is_some_and(|&c| name_char(c)) {
                    i += 1;
                }
                let name = &line[start..i];
                if name.eq_ignore_ascii_case(b"A") {
                    out.push(ACCUMULATOR);
                } else {
                    out.extend(counted_bytes(name));
                }
            } else {
                return Err(i);
            }
            value_expected = false;
            continue;
        }
        // After a value: index modes, closing brackets and operators.
        let x_or_y = |s: &[u8]| !line.get(i + s.len()).is_some_and(|&c| name_char(c));
        let after = [
            (&b",X)"[..], 0x36, false),
            (b"),Y", 0x37, false),
            (b",Y", 0x38, false),
            (b",X", 0x39, false),
            (b")", 0x3A, false),
            (b"]", 0x20, false),
            (b",", 0x3D, true),
            (b".AND", 0x32, true),
            (b".OR", 0x33, true),
            (b"<=", 0x19, true),
            (b">=", 0x1A, true),
            (b"<>", 0x1B, true),
            (b">", 0x1C, true),
            (b"<", 0x1D, true),
            (b"=", 0x18, true),
            (b"+", 0x12, true),
            (b"-", 0x13, true),
            (b"*", 0x14, true),
            (b"/", 0x15, true),
            (b"&", 0x16, true),
            (b"!", 0x24, true),
            (b"^", 0x25, true),
            (b"\\", 0x27, true),
        ];
        let found = after
            .iter()
            .find(|(w, ..)| at(i, w) && (!matches!(*w, b",Y" | b",X") || x_or_y(w)));
        let Some(&(w, t, value_next)) = found else {
            return Err(i);
        };
        out.push(t);
        i += w.len();
        value_expected = value_next;
    }
    Ok(())
}

/// A number as one byte (up to 255) or two.
fn number(out: &mut Vec<u8>, v: u16, byte: u8, word: u8) {
    if v <= 255 {
        out.push(byte);
        out.push(v as u8);
    } else {
        out.push(word);
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// .OPT's words.
const OPT_WORDS: [(&str, u8); 9] = [
    ("NO", 0x45),
    ("OBJ", 0x46),
    ("ERR", 0x47),
    ("EJECT", 0x48),
    ("LIST", 0x49),
    ("XREF", 0x4A),
    ("MLIST", 0x4B),
    ("CLIST", 0x4C),
    ("NUM", 0x4D),
];

/// A line number as LIST prints it: in whole bytes of decimal digits, so
/// an even number of digits (`01`, `0100`, `1000`, `010000`).
fn line_number(n: u16) -> String {
    let s = n.to_string();
    if s.len() % 2 == 1 { format!("0{s}") } else { s }
}

/// A counted string and the bytes after it.
type Split<'a> = (&'a [u8], &'a [u8]);

/// A name or string stored as `$80 + length` and its characters, if `d`
/// starts with one. Returns it and the bytes after it.
fn counted(d: &[u8]) -> Option<Result<Split<'_>, String>> {
    let &first = d.first()?;
    if first < 0x80 {
        return None;
    }
    let n = (first - 0x80) as usize;
    Some(
        d.get(1..1 + n)
            .map(|s| (s, &d[1 + n..]))
            .ok_or_else(|| "name cut short".to_string()),
    )
}

/// LIST's columns, counted from the start of the line whatever the line
/// number's width (`05       LDA #1`, `010000   LDA #1`). A label starts
/// straight after the line number and its space.
const OPCODE_COLUMN: usize = 9;
const OPERAND_COLUMN: usize = 13;
const COMMENT_COLUMN: usize = 21;
/// How far each open `.IF` moves the opcode and operand columns.
const NEST_STEP: usize = 2;

const IF: u8 = 0x01;
const ENDIF: u8 = 0x03;

/// Pad `out` with spaces to column `col`, or by one space if it's already
/// there or past it (fields never touch).
fn pad_to(out: &mut Vec<u8>, col: usize) {
    if out.len() < col {
        out.resize(col, b' ');
    } else if out.last() != Some(&b' ') {
        out.push(b' ');
    }
}

/// One line as LIST shows it: label, opcode, operand and comment in
/// columns. `depth` is the number of `.IF`s open, carried from line to
/// line: each moves the opcode and operand columns right; a `.IF` line is
/// already indented, and its `.ENDIF` still is.
fn list_line(d: &[u8], out: &mut Vec<u8>, depth: &mut usize) -> Result<(), String> {
    let mut d = d;
    if let Some(r) = counted(d) {
        let (label, rest) = r?;
        out.extend_from_slice(label);
        d = rest;
    }
    let Some((&token, rest)) = d.split_first() else {
        return Ok(()); // a label on its own
    };
    if token == COMMENT_LINE {
        out.extend_from_slice(rest);
        return Ok(());
    }
    if token == ERROR {
        // Not in columns: the line as typed, with the character where the
        // error was found in inverse video.
        out.extend_from_slice(b"ERROR - ");
        out.extend_from_slice(rest);
        return Ok(());
    }
    if token == IF {
        *depth += 1;
    }
    let indent = *depth * NEST_STEP;
    if token == ENDIF {
        *depth = depth.saturating_sub(1);
    }

    // The opcode (or directive), always followed by a space.
    let (keyword, operand_bytes, comment): (Vec<u8>, &[u8], Option<&[u8]>) = match token {
        // `;` in the opcode column, the rest of its text in the comment column.
        COMMENT => (
            b";".to_vec(),
            &[],
            Some(rest.strip_prefix(b";").unwrap_or(rest)),
        ),
        MACRO_CALL => {
            // The name keeps the space typed after it: ` OPEN `.
            let (name, rest) = counted(rest).ok_or("macro call without a name")??;
            let mut k = vec![b' '];
            k.extend_from_slice(name);
            (k, rest, None)
        }
        _ => {
            let k = STATEMENTS
                .get(token as usize)
                .ok_or_else(|| format!("unknown statement token ${token:02X}"))?;
            (k.as_bytes().to_vec(), rest, None)
        }
    };
    pad_to(out, OPCODE_COLUMN + indent);
    out.extend_from_slice(&keyword);
    out.push(b' ');

    let mut text = Vec::new();
    let inline_comment = operand(operand_bytes, &mut text)?;
    if !text.is_empty() {
        pad_to(out, OPERAND_COLUMN + indent);
        out.extend_from_slice(&text);
    }
    if let Some(c) = comment.or(inline_comment).filter(|c| !c.is_empty()) {
        pad_to(out, COMMENT_COLUMN);
        out.extend_from_slice(c);
    }
    Ok(())
}

/// Operand tokens as text. Returns the comment after them, if any (its
/// text, with its `;`).
fn operand<'a>(d: &'a [u8], out: &mut Vec<u8>) -> Result<Option<&'a [u8]>, String> {
    let mut i = 0;
    let byte = |i: usize| d.get(i).copied().ok_or("operand cut short");
    while i < d.len() {
        let t = d[i];
        i += 1;
        match t {
            0x80.. => {
                let (s, _) = counted(&d[i - 1..]).unwrap()?;
                out.extend_from_slice(s);
                i += s.len();
            }
            HEX_WORD => {
                let v = u16::from_le_bytes([byte(i)?, byte(i + 1)?]);
                out.extend(format!("${v:04X}").bytes());
                i += 2;
            }
            HEX_BYTE => {
                out.extend(format!("${:02X}", byte(i)?).bytes());
                i += 1;
            }
            DEC_WORD => {
                let v = u16::from_le_bytes([byte(i)?, byte(i + 1)?]);
                out.extend(v.to_string().bytes());
                i += 2;
            }
            DEC_BYTE => {
                out.extend(byte(i)?.to_string().bytes());
                i += 1;
            }
            CHAR => {
                out.push(b'\'');
                out.push(byte(i)?);
                i += 1;
            }
            OPERAND_COMMENT => return Ok(Some(&d[i..])),
            _ => {
                let s = operand_token(t).ok_or_else(|| format!("unknown token ${t:02X}"))?;
                out.extend_from_slice(s.as_bytes());
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_block_the_save() {
        let long = format!("10  .BYTE {}1", "1,".repeat(130)).into_bytes();
        let e = encode(&[long, b"20  RTS".to_vec()]).unwrap_err();
        assert!(e.starts_with("1 line is too long"), "{e}");
        assert!(e.contains("Line 10 (editor line 1) is "), "{e}");
    }

    /// A MAC/65 file from lines of (number, tokens).
    fn file(lines: &[(u16, &[u8])]) -> Vec<u8> {
        let mut body = Vec::new();
        for (n, t) in lines {
            body.extend_from_slice(&n.to_le_bytes());
            body.push(t.len() as u8 + 3);
            body.extend_from_slice(t);
        }
        let mut f = vec![0xFE, 0xFE];
        f.extend_from_slice(&(body.len() as u16).to_le_bytes());
        f.extend(body);
        f
    }

    fn listing(f: &[u8]) -> Vec<String> {
        let d = Mac65.decode(f).unwrap();
        d.lines
            .iter()
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .collect()
    }

    #[test]
    fn columns_count_from_the_start_of_the_line() {
        // LDA #1 ;X, as MAC/65 listed it with 2- and 6-digit line numbers.
        let lda: &[u8] = &[0x51, 0x3E, DEC_BYTE, 1, OPERAND_COMMENT, b';', b'X'];
        let f = file(&[(5, lda), (10000, lda)]);
        assert_eq!(
            listing(&f),
            ["05       LDA #1      ;X", "010000   LDA #1      ;X"]
        );
    }

    #[test]
    fn line_numbers_are_whole_bytes() {
        assert_eq!(line_number(1), "01");
        assert_eq!(line_number(100), "0100");
        assert_eq!(line_number(1000), "1000");
        assert_eq!(line_number(10000), "010000");
    }

    #[test]
    fn labels_nesting_and_comments() {
        let f = file(&[
            (
                1000,
                &[0x86, b'S', b'A', b'V', b'E', b'P', b'C', 0x1E, 0x0D],
            ), // SAVEPC = *
            (1010, &[0x01, DEC_BYTE, 1]), // .IF 1
            (1020, &[0x37]),              // PLA
            (1030, &[0x03]),              // .ENDIF
            (1040, &[COMMENT_LINE, b';', b'X']),
            (1050, &[ERROR, b'X']),
        ]);
        assert_eq!(
            listing(&f),
            [
                "1000 SAVEPC = *",
                "1010       .IF 1",
                "1020       PLA ",
                "1030       .ENDIF ",
                "1040 ;X",
                "1050 ERROR - X",
            ]
        );
    }

    #[test]
    fn not_mac65() {
        assert!(!Mac65.detect(b"10 PRINT\x9b"));
        assert!(!Mac65.detect(&[0xFE, 0xFE, 9, 0]));
    }
}
