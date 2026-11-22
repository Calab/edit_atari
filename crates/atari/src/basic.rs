//! Tokenized BASIC programs (`SAVE "D:PROG.BAS"`), read into LIST text.
//!
//! The file layout is shared by Atari BASIC and its descendants; what
//! differs is the token tables, which live in a [`Dialect`]. Adding a
//! dialect (Turbo-BASIC XL, Altirra BASIC...) means adding its tables and
//! listing its [`TokenizedBasic`] format in the machine's format list,
//! narrowest dialect first.
//!
//! File layout: a 14-byte header of seven little-endian pointers (LOMEM,
//! VNTP, VNTD, VVTP, STMTAB, STMCUR, STARP), then memory from VNTP up to
//! STARP. A pointer's file offset is therefore `ptr - VNTP + 14`.
//!
//! * Variable names, VNTP..VNTD: last character of each has bit 7 set;
//!   arrays end in `(`, strings in `$`.
//! * Variable values, VVTP..STMTAB: 8 bytes each (not needed here).
//! * Program lines, STMTAB..STMCUR: line number (2 bytes), offset to the
//!   next line (1), then statements: offset to the next statement (1),
//!   statement token (1), then expression tokens.
//! * The immediate-mode line (number 32768) follows at STMCUR.

use retro_core::{Decoded, FileFormat};

/// One BASIC dialect's token tables. A dialect can extend another: its
/// tables then continue where the base's end, and anything it doesn't
/// define comes from the base.
pub struct Dialect {
    pub name: &'static str,
    base: Option<&'static Dialect>,
    /// Statement keywords by token, from token `first_statement`. "" is
    /// the implied LET (`A=1`).
    first_statement: u8,
    statements: &'static [&'static str],
    /// Statements whose rest is raw text rather than tokens (REM, DATA,
    /// ERROR-).
    raw_statements: &'static [u8],
    /// Expression tokens from `first_operator` on. "" prints nothing
    /// (the `(` of an array, whose name already ends in `(`).
    first_operator: u8,
    operators: &'static [&'static str],
    /// Expression tokens listed with a space either side (TO, THEN...).
    spaced: &'static [u8],
    /// Token for a number listed in hex (`$D01F`), followed by 6 BCD bytes.
    hex_number: Option<u8>,
    /// Token for variables 128-255, followed by one byte: the variable
    /// number minus 128. (Variables 0-127 are tokens $80-$FF.)
    high_variable: Option<u8>,
}

impl Dialect {
    fn statement(&self, t: u8) -> Option<&'static str> {
        t.checked_sub(self.first_statement)
            .and_then(|i| self.statements.get(i as usize).copied())
            .or_else(|| self.base?.statement(t))
    }

    fn operator(&self, t: u8) -> Option<&'static str> {
        t.checked_sub(self.first_operator)
            .and_then(|i| self.operators.get(i as usize).copied())
            .or_else(|| self.base?.operator(t))
    }

    fn is_raw(&self, t: u8) -> bool {
        self.raw_statements.contains(&t) || self.base.is_some_and(|b| b.is_raw(t))
    }

    fn is_spaced(&self, t: u8) -> bool {
        self.spaced.contains(&t) || self.base.is_some_and(|b| b.is_spaced(t))
    }

    fn hex_number(&self) -> Option<u8> {
        self.hex_number.or_else(|| self.base?.hex_number())
    }

    fn high_variable(&self) -> Option<u8> {
        self.high_variable.or_else(|| self.base?.high_variable())
    }
}

/// Expression tokens with their own data, below the operator table.
const NUMBER: u8 = 0x0E;
const STRING: u8 = 0x0F;
const FIRST_OPERATOR: u8 = 0x12;
const END_OF_LINE: u8 = 0x16;
/// Tokens at and above this are variables.
const FIRST_VARIABLE: u8 = 0x80;
const EOL: u8 = 0x9B;

pub static ATARI_BASIC: Dialect = Dialect {
    name: "Atari BASIC",
    base: None,
    first_statement: 0,
    statements: &[
        "REM", "DATA", "INPUT", "COLOR", "LIST", "ENTER", "LET", "IF", // $00
        "FOR", "NEXT", "GOTO", "GO TO", "GOSUB", "TRAP", "BYE", "CONT", // $08
        "COM", "CLOSE", "CLR", "DEG", "DIM", "END", "NEW", "OPEN", // $10
        "LOAD", "SAVE", "STATUS", "NOTE", "POINT", "XIO", "ON", "POKE", // $18
        "PRINT", "RAD", "READ", "RESTORE", "RETURN", "RUN", "STOP", "POP", // $20
        "?", "GET", "PUT", "GRAPHICS", "PLOT", "POSITION", "DOS", "DRAWTO", // $28
        "SETCOLOR", "LOCATE", "SOUND", "LPRINT", "CSAVE", "CLOAD", "", "ERROR-", // $30
    ],
    raw_statements: &[0x00, 0x01, 0x37],
    first_operator: FIRST_OPERATOR,
    operators: &[
        ",", "$", ":", ";", "", "GOTO", "GOSUB", "TO", "STEP", "THEN", "#", // $12
        "<=", "<>", ">=", "<", ">", "=", "^", "*", "+", "-", "/", // $1D
        "NOT", "OR", "AND", "(", ")", "=", "=", // $28
        "<=", "<>", ">=", "<", ">", "=", "+", "-", // $2F: string compares, unary
        "(", "", "", "(", "(",
        ",", // $37: string, array, DIM array, function, DIM string ( and array ,
        "STR$", "CHR$", "USR", "ASC", "VAL", "LEN", "ADR", "ATN", // $3D
        "COS", "PEEK", "SIN", "RND", "FRE", "EXP", "LOG", "CLOG", // $45
        "SQR", "SGN", "ABS", "INT", "PADDLE", "STICK", "PTRIG", "STRIG", // $4D
    ],
    spaced: &[0x17, 0x18, 0x19, 0x1A, 0x1B, 0x28, 0x29, 0x2A],
    hex_number: None,
    high_variable: None,
};

/// Turbo-BASIC XL's `--` statement lists as a line of 30 dashes.
const DASHES: &str = "------------------------------";

/// Turbo-BASIC XL 1.5: Atari BASIC plus statements from $38 and
/// functions from $55. (Altirra Extended BASIC uses the same tokens.)
pub static TURBO_BASIC_XL: Dialect = Dialect {
    name: "Turbo-BASIC XL",
    base: Some(&ATARI_BASIC),
    first_statement: 0x38,
    statements: &[
        "DPOKE", "MOVE", "-MOVE", "*F", "REPEAT", "UNTIL", "WHILE", "WEND", // $38
        "ELSE", "ENDIF", "BPUT", "BGET", "FILLTO", "DO", "LOOP", "EXIT", // $40
        "DIR", "LOCK", "UNLOCK", "RENAME", "DELETE", "PAUSE", "TIME$=", "PROC", // $48
        "EXEC", "ENDPROC", "FCOLOR", "*L", DASHES, "RENUM", "DEL", "DUMP", // $50
        "TRACE", "TEXT", "BLOAD", "BRUN", "GO#", "#", "*B", "PAINT", // $58
        "CLS", "DSOUND", "CIRCLE", "%PUT", "%GET", // $60
    ],
    raw_statements: &[0x54],
    first_operator: 0x55,
    operators: &[
        "DPEEK", "&", "!", "INSTR", "INKEY$", "EXOR", "HEX$", "DEC", // $55
        "DIV", "FRAC", "TIME$", "TIME", "MOD", "EXEC", "RND", "RAND", // $5D
        "TRUNC", "%0", "%1", "%2", "%3", "GO#", "UINSTR", "ERR", // $65
        "ERL", // $6D
    ],
    spaced: &[0x5A, 0x5D, 0x61, 0x62, 0x6A],
    hex_number: Some(0x0D),
    high_variable: Some(0x00),
};

/// A tokenized BASIC file in one dialect.
pub struct TokenizedBasic {
    pub dialect: &'static Dialect,
}

pub static ATARI_BASIC_FILE: TokenizedBasic = TokenizedBasic {
    dialect: &ATARI_BASIC,
};

pub static TURBO_BASIC_XL_FILE: TokenizedBasic = TokenizedBasic {
    dialect: &TURBO_BASIC_XL,
};

impl FileFormat for TokenizedBasic {
    fn name(&self) -> &'static str {
        self.dialect.name
    }

    fn extension(&self) -> &'static str {
        "BAS"
    }

    /// The header must be consistent and every token known to the dialect.
    fn detect(&self, bytes: &[u8]) -> bool {
        Header::read(bytes).is_ok() && self.decode(bytes).is_ok()
    }

    fn looks_like(&self, bytes: &[u8]) -> bool {
        Header::read(bytes).is_ok()
    }

    fn decode(&self, bytes: &[u8]) -> Result<Decoded, String> {
        let h = Header::read(bytes)?;
        let names = variable_names(&bytes[h.vnt.clone()]);
        let mut lines = Vec::new();
        let program = &bytes[h.program.clone()];
        let mut pos = 0;
        while pos < program.len() {
            let rest = &program[pos..];
            if rest.len() < 3 {
                return Err("program ends part way through a line".into());
            }
            let number = u16::from_le_bytes([rest[0], rest[1]]);
            if number >= 0x8000 {
                break; // the immediate-mode line
            }
            let len = rest[2] as usize;
            if len < 4 || len > rest.len() {
                return Err(format!("line {number}: bad length"));
            }
            let line = list_line(self.dialect, &rest[..len], &names)
                .map_err(|e| format!("line {number}: {e}"))?;
            let mut text = number.to_string().into_bytes();
            text.push(b' ');
            text.extend(line);
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

/// Where the variable names and program lines are in the file.
struct Header {
    vnt: std::ops::Range<usize>,
    program: std::ops::Range<usize>,
}

impl Header {
    fn read(bytes: &[u8]) -> Result<Header, String> {
        let word = |i: usize| -> Option<usize> {
            Some(u16::from_le_bytes([*bytes.get(i)?, *bytes.get(i + 1)?]) as usize)
        };
        let bad = || "not a tokenized BASIC file".to_string();
        let p: Vec<usize> = (0..7)
            .map(|i| word(i * 2))
            .collect::<Option<_>>()
            .ok_or_else(bad)?;
        let [_lomem, vntp, vntd, vvtp, stmtab, stmcur, starp] = p[..] else {
            return Err(bad());
        };
        let in_order =
            vntp <= vntd && vntd < vvtp && vvtp <= stmtab && stmtab <= stmcur && stmcur <= starp;
        let vars = (stmtab - vvtp.min(stmtab)) / 8;
        if !in_order || (stmtab - vvtp) % 8 != 0 || vars > 256 {
            return Err(bad());
        }
        let at = |ptr: usize| ptr - vntp + 14;
        if bytes.len() < at(stmcur) {
            return Err("file is shorter than its header says".into());
        }
        Ok(Header {
            vnt: at(vntp)..at(vntd),
            program: at(stmtab)..at(stmcur),
        })
    }
}

/// Variable names in table order. A name ends at a byte with bit 7 set.
fn variable_names(table: &[u8]) -> Vec<Vec<u8>> {
    let mut names = Vec::new();
    let mut cur = Vec::new();
    for &b in table {
        if b == 0 && cur.is_empty() {
            break;
        }
        cur.push(b & 0x7F);
        if b & 0x80 != 0 {
            names.push(std::mem::take(&mut cur));
        }
    }
    names
}

/// One program line's statements as LIST text (without the line number).
fn list_line(d: &Dialect, line: &[u8], names: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut pos = 3;
    while pos < line.len() {
        let next = line[pos] as usize;
        if next <= pos + 1 || next > line.len() {
            return Err("bad statement".into());
        }
        let token = line[pos + 1];
        let body = &line[pos + 2..next];
        let keyword = d
            .statement(token)
            .ok_or_else(|| format!("unknown statement token ${token:02X}"))?;
        out.extend_from_slice(keyword.as_bytes());
        if d.is_raw(token) {
            let text = body.strip_suffix(&[EOL]).unwrap_or(body);
            if !text.is_empty() && text[0] != b' ' {
                out.push(b' ');
            }
            out.extend_from_slice(text);
        } else {
            // LIST puts a space after every keyword, even with nothing
            // after it: `PRINT :`, `END `.
            if !keyword.is_empty() {
                out.push(b' ');
            }
            list_tokens(d, body, names, &mut out)?;
        }
        pos = next;
    }
    Ok(out)
}

/// Expression tokens as text.
fn list_tokens(
    d: &Dialect,
    body: &[u8],
    names: &[Vec<u8>],
    out: &mut Vec<u8>,
) -> Result<(), String> {
    let mut i = 0;
    while i < body.len() {
        let t = body[i];
        i += 1;
        match t {
            NUMBER => {
                let n = body.get(i..i + 6).ok_or("number cut short")?;
                out.extend(format_number(n.try_into().unwrap()).bytes());
                i += 6;
            }
            STRING => {
                let len = *body.get(i).ok_or("string cut short")? as usize;
                let s = body.get(i + 1..i + 1 + len).ok_or("string cut short")?;
                out.push(b'"');
                out.extend_from_slice(s);
                out.push(b'"');
                i += 1 + len;
            }
            END_OF_LINE => {}
            _ if Some(t) == d.hex_number() => {
                let n = body.get(i..i + 6).ok_or("number cut short")?;
                out.extend(format_hex(n.try_into().unwrap()).bytes());
                i += 6;
            }
            FIRST_VARIABLE.. => variable(names, (t - FIRST_VARIABLE) as usize, out)?,
            _ if Some(t) == d.high_variable() => {
                let n = *body.get(i).ok_or("variable cut short")? as usize;
                variable(names, 128 + n, out)?;
                i += 1;
            }
            _ => {
                let name = d
                    .operator(t)
                    .ok_or_else(|| format!("unknown token ${t:02X}"))?;
                if d.is_spaced(t) {
                    if out.last() != Some(&b' ') {
                        out.push(b' ');
                    }
                    out.extend_from_slice(name.as_bytes());
                    out.push(b' ');
                } else {
                    out.extend_from_slice(name.as_bytes());
                }
            }
        }
    }
    Ok(())
}

/// Variable `n`'s name.
fn variable(names: &[Vec<u8>], n: usize, out: &mut Vec<u8>) -> Result<(), String> {
    let name = names
        .get(n)
        .ok_or_else(|| format!("variable {n} has no name"))?;
    out.extend_from_slice(name);
    Ok(())
}

/// A whole number written in hex, as Turbo-BASIC XL lists `$D01F`, in
/// whole bytes.
fn format_hex(n: [u8; 6]) -> String {
    let exp100 = (n[0] & 0x7F) as i32 - 64;
    let mut v: u64 = 0;
    if n[0] & 0x7F != 0 {
        for (k, b) in n[1..].iter().enumerate() {
            let pair = u64::from((b >> 4).min(9) * 10 + (b & 0x0F).min(9));
            // Byte k is worth 100^(exp100 - k).
            let p = exp100 - k as i32;
            if p >= 0 {
                v += pair * 100u64.pow(p as u32);
            }
        }
    }
    // Whole bytes, as Turbo-BASIC XL lists them: $0F, $02E7, $B000.
    if v < 0x100 {
        format!("${v:02X}")
    } else if v < 0x10000 {
        format!("${v:04X}")
    } else {
        format!("${v:06X}")
    }
}

/// An Atari 6-byte BCD number as BASIC prints it: plain decimal from 0.01
/// up to 9999999999, otherwise like `1.5E-05` or `1.0E+10`.
pub fn format_number(n: [u8; 6]) -> String {
    if n[0] & 0x7F == 0 {
        return "0".into();
    }
    let negative = n[0] & 0x80 != 0;
    let exp100 = (n[0] & 0x7F) as i32 - 64;
    let mut digits: Vec<u8> = n[1..]
        .iter()
        .flat_map(|b| [b >> 4, b & 0x0F])
        .map(|d| b'0' + d.min(9))
        .collect();
    // The value is 0.DDDDDDDDDD x 10^(2 * exp100 + 2). Normalise so the
    // first digit isn't 0, giving d.ddd x 10^exp.
    let lead = digits.iter().take_while(|&&d| d == b'0').count();
    if lead == digits.len() {
        return "0".into();
    }
    digits.drain(..lead);
    while digits.last() == Some(&b'0') {
        digits.pop();
    }
    let exp = 2 * exp100 + 1 - lead as i32;
    let digits = String::from_utf8(digits).unwrap();
    let mut s = String::new();
    if negative {
        s.push('-');
    }
    if (-2..=9).contains(&exp) {
        if exp < 0 {
            s.push_str("0.");
            s.push_str(&"0".repeat((-exp - 1) as usize));
            s.push_str(&digits);
        } else {
            let int_len = exp as usize + 1;
            if digits.len() <= int_len {
                s.push_str(&digits);
                s.push_str(&"0".repeat(int_len - digits.len()));
            } else {
                s.push_str(&digits[..int_len]);
                s.push('.');
                s.push_str(&digits[int_len..]);
            }
        }
    } else {
        // At least one decimal: 1.0E-03, 1.5E-05.
        s.push_str(&digits[..1]);
        s.push('.');
        s.push_str(if digits.len() > 1 { &digits[1..] } else { "0" });
        s.push_str(&format!(
            "E{}{:02}",
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `:` between statements.
    const END_OF_STATEMENT: u8 = 0x14;

    #[test]
    fn numbers() {
        let f = |b: [u8; 6]| format_number(b);
        assert_eq!(f([0, 0, 0, 0, 0, 0]), "0");
        assert_eq!(f([0x40, 0x01, 0, 0, 0, 0]), "1");
        assert_eq!(f([0x40, 0x10, 0, 0, 0, 0]), "10");
        assert_eq!(f([0x41, 0x01, 0, 0, 0, 0]), "100");
        assert_eq!(f([0x41, 0x12, 0x34, 0x50, 0, 0]), "1234.5");
        assert_eq!(f([0xC0, 0x05, 0x25, 0, 0, 0]), "-5.25");
        assert_eq!(f([0x3F, 0x50, 0, 0, 0, 0]), "0.5");
        assert_eq!(f([0x3F, 0x01, 0, 0, 0, 0]), "0.01");
        assert_eq!(f([0x3E, 0x10, 0, 0, 0, 0]), "1.0E-03");
        assert_eq!(f([0x3D, 0x15, 0, 0, 0, 0]), "1.5E-05");
        assert_eq!(f([0x44, 0x99, 0x99, 0x99, 0x99, 0x99]), "9999999999");
        assert_eq!(f([0x45, 0x01, 0, 0, 0, 0]), "1.0E+10");
        assert_eq!(f([0x40, 0x03, 0x14, 0x15, 0x92, 0x65]), "3.14159265");
    }

    #[test]
    fn hex_numbers() {
        assert_eq!(format_hex([0x40, 0x12, 0, 0, 0, 0]), "$0C");
        assert_eq!(format_hex([0x40, 0x15, 0, 0, 0, 0]), "$0F");
        assert_eq!(format_hex([0x41, 0x02, 0x55, 0, 0, 0]), "$FF");
        assert_eq!(format_hex([0x41, 0x07, 0x43, 0, 0, 0]), "$02E7");
        assert_eq!(format_hex([0x42, 0x04, 0x50, 0x56, 0, 0]), "$B000");
        assert_eq!(format_hex([0, 0, 0, 0, 0, 0]), "$00");
    }

    #[test]
    fn turbo_basic_builds_on_atari_basic() {
        assert_eq!(TURBO_BASIC_XL.statement(0x20), Some("PRINT"));
        assert_eq!(TURBO_BASIC_XL.statement(0x4D), Some("PAUSE"));
        assert_eq!(TURBO_BASIC_XL.statement(0x64), Some("%GET"));
        assert_eq!(TURBO_BASIC_XL.statement(0x65), None);
        assert_eq!(TURBO_BASIC_XL.operator(0x46), Some("PEEK"));
        assert_eq!(TURBO_BASIC_XL.operator(0x6D), Some("ERL"));
        assert_eq!(ATARI_BASIC.statement(0x38), None);
        assert_eq!(ATARI_BASIC.operator(0x55), None);
        assert!(TURBO_BASIC_XL.is_spaced(0x19)); // TO, from Atari BASIC
    }

    /// Build a SAVE file from variable names and raw line bytes.
    fn program(names: &[&str], lines: &[&[u8]]) -> Vec<u8> {
        let mut vnt = Vec::new();
        for n in names {
            let b = n.as_bytes();
            vnt.extend_from_slice(&b[..b.len() - 1]);
            vnt.push(b[b.len() - 1] | 0x80);
        }
        vnt.push(0);
        let vntp = 0x100;
        let vntd = vntp + vnt.len() - 1;
        let vvtp = vntd + 1;
        let stmtab = vvtp + 8 * names.len();
        let body: Vec<u8> = lines.iter().flat_map(|l| l.iter().copied()).collect();
        let stmcur = stmtab + body.len();
        let immediate = [0x00, 0x80, 0x06, 0x06, 0x19, 0x16]; // 32768 SAVE
        let starp = stmcur + immediate.len();
        let mut f = Vec::new();
        for p in [0, vntp, vntd, vvtp, stmtab, stmcur, starp] {
            f.extend_from_slice(&(p as u16).to_le_bytes());
        }
        f.extend(vnt);
        f.extend(vec![0; 8 * names.len()]);
        f.extend(body);
        f.extend(immediate);
        f
    }

    fn listing(bytes: &[u8]) -> Vec<String> {
        let d = ATARI_BASIC_FILE.decode(bytes).unwrap();
        d.lines
            .iter()
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .collect()
    }

    #[test]
    fn lists_a_program() {
        let one = [NUMBER, 0x40, 0x01, 0, 0, 0, 0];
        let ten = [NUMBER, 0x40, 0x10, 0, 0, 0, 0];
        // 10 PRINT "HI";A$:REM  COMMENT
        let mut l10 = vec![
            10,
            0,
            0,
            0,
            0x20,
            STRING,
            2,
            b'H',
            b'I',
            0x15,
            0x81,
            END_OF_STATEMENT,
        ];
        l10[3] = l10.len() as u8;
        let rem_at = l10.len();
        l10.extend([0, 0x00, b' ', b'C', b'O', b'M', b'M', b'E', b'N', b'T', EOL]);
        l10[rem_at] = l10.len() as u8;
        l10[2] = l10.len() as u8;
        // 20 FOR I=1 TO 10
        let mut l20 = vec![20, 0, 0, 0, 0x08, 0x80, 0x2D];
        l20.extend(one);
        l20.push(0x19);
        l20.extend(ten);
        l20.push(END_OF_LINE);
        l20[2] = l20.len() as u8;
        l20[3] = l20.len() as u8;
        // 30 B(I)=PEEK(53279)
        let mut l30 = vec![30, 0, 0, 0, 0x36, 0x82, 0x38, 0x80, 0x2C, 0x2D, 0x46, 0x3A];
        l30.extend([NUMBER, 0x42, 0x05, 0x32, 0x79, 0, 0]);
        l30.extend([0x2C, END_OF_LINE]);
        l30[2] = l30.len() as u8;
        l30[3] = l30.len() as u8;
        // 40 END
        let l40 = [40, 0, 6, 6, 0x15, END_OF_LINE];
        let f = program(&["I", "A$", "B("], &[&l10, &l20, &l30, &l40]);
        assert!(ATARI_BASIC_FILE.detect(&f));
        assert_eq!(
            listing(&f),
            [
                "10 PRINT \"HI\";A$:REM COMMENT",
                "20 FOR I=1 TO 10",
                "30 B(I)=PEEK(53279)",
                "40 END ",
            ]
        );
    }

    #[test]
    fn turbo_basic_variables_past_127() {
        let names: Vec<String> = (0..130).map(|i| format!("V{i}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        // 10 V129=V1 : variable 129 is $00 $01, variable 1 is $81.
        let line = [10, 0, 9, 9, 0x36, 0x00, 0x01, 0x2D, 0x81, END_OF_LINE];
        let mut line = line.to_vec();
        line[2] = line.len() as u8;
        line[3] = line.len() as u8;
        let f = program(&names, &[&line]);
        assert!(!ATARI_BASIC_FILE.detect(&f));
        let d = TURBO_BASIC_XL_FILE.decode(&f).unwrap();
        assert_eq!(d.lines[0], b"10 V129=V1");
    }

    #[test]
    fn text_is_not_basic() {
        assert!(!ATARI_BASIC_FILE.detect(b"10 PRINT \"HELLO\"\x9b"));
        assert!(!ATARI_BASIC_FILE.detect(b""));
    }
}
