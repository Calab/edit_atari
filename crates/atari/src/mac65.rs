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
}

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
