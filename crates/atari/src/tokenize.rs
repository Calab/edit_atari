//! Atari BASIC tokenizer: LIST text back into a `SAVE` file.
//!
//! Follows Atari BASIC's own rules, so a program read from a `.BAS` file
//! and written back comes out the same. The parts that depend on context:
//!
//! * `=` is an assignment (`$2D`, or `$2E` for a string) straight after
//!   the variable in LET and FOR; elsewhere a comparison, numeric (`$22`)
//!   or string (`$34`) depending on what's to its left. So are `<`, `>`,
//!   `<=`, `>=` and `<>`.
//! * `(` is `$37` after a string variable (substring), `$38` after an
//!   array, `$39`/`$3B` in DIM, `$3A` after a function, otherwise `$2B`.
//! * `,` is `$3C` inside any of those brackets, otherwise `$12`.
//! * `+` and `-` are unary (`$35`, `$36`) where a value is expected.
//!
//! A line that doesn't parse is stored as Atari BASIC stores it: an
//! `ERROR-` statement holding the text, with the character where the
//! error was found in inverse video.

use crate::basic::{ALTIRRA_EXTENDED_BASIC, Dialect, TURBO_BASIC_XL};

const NUMBER: u8 = 0x0E;
const STRING: u8 = 0x0F;
const COMMA: u8 = 0x12;
const EOS: u8 = 0x14;
const SEMICOLON: u8 = 0x15;
const EOL_TOKEN: u8 = 0x16;
const GOTO: u8 = 0x17;
const GOSUB: u8 = 0x18;
const TO: u8 = 0x19;
const STEP: u8 = 0x1A;
const THEN: u8 = 0x1B;
const HASH: u8 = 0x1C;
const NOT: u8 = 0x28;
const OR: u8 = 0x29;
const AND: u8 = 0x2A;
const OPEN_PAREN: u8 = 0x2B;
const CLOSE_PAREN: u8 = 0x2C;
const ASSIGN_NUM: u8 = 0x2D;
const ASSIGN_STR: u8 = 0x2E;
const UNARY_PLUS: u8 = 0x35;
const UNARY_MINUS: u8 = 0x36;
const STRING_PAREN: u8 = 0x37;
const ARRAY_PAREN: u8 = 0x38;
const DIM_ARRAY_PAREN: u8 = 0x39;
const FUNCTION_PAREN: u8 = 0x3A;
const DIM_STRING_PAREN: u8 = 0x3B;
const ARRAY_COMMA: u8 = 0x3C;
const FIRST_VARIABLE: u8 = 0x80;
const EOL: u8 = 0x9B;

/// Statement tokens with their own syntax.
const REM: u8 = 0x00;
const DATA: u8 = 0x01;
const LET: u8 = 0x06;
const IF: u8 = 0x07;
const FOR: u8 = 0x08;
const COM: u8 = 0x10;
const DIM: u8 = 0x14;
const ON: u8 = 0x1E;
const IMPLIED_LET: u8 = 0x36;
const ERROR: u8 = 0x37;

/// Comparisons: (text, numeric token, string token). Longest first.
const RELATIONS: [(&[u8], u8, u8); 6] = [
    (b"<=", 0x1D, 0x2F),
    (b"<>", 0x1E, 0x30),
    (b">=", 0x1F, 0x31),
    (b"<", 0x20, 0x32),
    (b">", 0x21, 0x33),
    (b"=", 0x22, 0x34),
];

/// Arithmetic operators.
const ARITHMETIC: [(u8, u8); 5] = [
    (b'^', 0x23),
    (b'*', 0x24),
    (b'+', 0x25),
    (b'-', 0x26),
    (b'/', 0x27),
];

/// Functions, in table order, and whether they return a string.
const FUNCTIONS: [(&str, u8, bool); 24] = [
    ("STR$", 0x3D, true),
    ("CHR$", 0x3E, true),
    ("USR", 0x3F, false),
    ("ASC", 0x40, false),
    ("VAL", 0x41, false),
    ("LEN", 0x42, false),
    ("ADR", 0x43, false),
    ("ATN", 0x44, false),
    ("COS", 0x45, false),
    ("PEEK", 0x46, false),
    ("SIN", 0x47, false),
    ("RND", 0x48, false),
    ("FRE", 0x49, false),
    ("EXP", 0x4A, false),
    ("LOG", 0x4B, false),
    ("CLOG", 0x4C, false),
    ("SQR", 0x4D, false),
    ("SGN", 0x4E, false),
    ("ABS", 0x4F, false),
    ("INT", 0x50, false),
    ("PADDLE", 0x51, false),
    ("STICK", 0x52, false),
    ("PTRIG", 0x53, false),
    ("STRIG", 0x54, false),
];

/// How a function's arguments are written.
#[derive(Clone, Copy, PartialEq)]
enum Args {
    /// `NAME(...)`: the function token, then the function `(` token.
    Paren,
    /// The token's name includes the `(` (`BUMP(`).
    OwnParen,
    /// No arguments (`INKEY$`, `TIME`).
    None,
}

/// A dialect's grammar: Atari BASIC's, plus what the dialect adds.
pub(crate) struct Grammar {
    /// Functions: (name, token, returns a string, arguments). Longest
    /// names first, and a name with brackets before the same name without.
    functions: Vec<(&'static str, u8, bool, Args)>,
    /// Binary operators beyond Atari BASIC's (`DIV`, `&`).
    operators: Vec<(&'static str, u8)>,
    /// What can follow `ON expression`.
    on_targets: Vec<(&'static str, u8)>,
    /// Variable names may contain `_`.
    underscore: bool,
    /// `IF condition` without THEN starts a block.
    if_without_then: bool,
    /// Token for a hex constant (`$FF`).
    hex_number: Option<u8>,
    /// Token for variables 128 and up (followed by the number - 128).
    high_variable: Option<u8>,
    max_variables: usize,
    /// Statements taking an optional `+` or `-` (TRACE, *B ...).
    signed: &'static [u8],
    /// The `--` statement (listed as a line of dashes).
    dashes: Option<u8>,
}

pub(crate) fn grammar(d: &Dialect) -> Grammar {
    let mut g = Grammar {
        functions: FUNCTIONS
            .iter()
            .map(|&(n, t, s)| (n, t, s, Args::Paren))
            .collect(),
        operators: Vec::new(),
        on_targets: vec![("GOTO", GOTO), ("GOSUB", GOSUB)],
        underscore: false,
        if_without_then: false,
        hex_number: None,
        high_variable: None,
        max_variables: 128,
        signed: &[],
        dashes: None,
    };
    if d.extends(&TURBO_BASIC_XL) {
        g.functions.extend([
            ("DPEEK", 0x55, false, Args::Paren),
            ("INSTR", 0x58, false, Args::Paren),
            ("INKEY$", 0x59, true, Args::None),
            ("HEX$", 0x5B, true, Args::Paren),
            ("DEC", 0x5C, false, Args::Paren),
            ("FRAC", 0x5E, false, Args::Paren),
            ("TIME$", 0x5F, true, Args::None),
            ("TIME", 0x60, false, Args::None),
            ("RND", 0x63, false, Args::None),
            ("RAND", 0x64, false, Args::Paren),
            ("TRUNC", 0x65, false, Args::Paren),
            ("%0", 0x66, false, Args::None),
            ("%1", 0x67, false, Args::None),
            ("%2", 0x68, false, Args::None),
            ("%3", 0x69, false, Args::None),
            ("UINSTR", 0x6B, false, Args::Paren),
            ("ERR", 0x6C, false, Args::None),
            ("ERL", 0x6D, false, Args::None),
        ]);
        g.operators = vec![
            ("&", 0x56),
            ("!", 0x57),
            ("EXOR", 0x5A),
            ("DIV", 0x5D),
            ("MOD", 0x61),
        ];
        g.on_targets.extend([("EXEC", 0x62), ("GO#", 0x6A)]);
        g.underscore = true;
        g.if_without_then = true;
        g.hex_number = Some(0x0D);
        g.high_variable = Some(0x00);
        g.max_variables = 256;
        g.signed = &[0x3B, 0x53, 0x58, 0x5E]; // *F, *L, TRACE, *B
        g.dashes = Some(0x54);
    }
    if d.extends(&ALTIRRA_EXTENDED_BASIC) {
        g.functions.extend([
            ("BUMP(", 0x6E, false, Args::OwnParen),
            ("VSTICK", 0x6F, false, Args::Paren),
            ("HSTICK", 0x70, false, Args::Paren),
            ("PMADR", 0x71, false, Args::Paren),
            ("ERR", 0x72, false, Args::Paren),
        ]);
    }
    g.functions
        .sort_by_key(|&(name, _, _, args)| (std::cmp::Reverse(name.len()), args != Args::Paren));
    g
}

/// A tokenized program.
#[derive(Debug)]
pub struct Program {
    /// The whole file, ready to write.
    pub bytes: Vec<u8>,
    /// Lines that didn't parse, stored as ERROR- lines.
    pub error_lines: Vec<u16>,
}

/// Tokenize a program's LIST text (one line per entry, as the editor
/// holds it). Blank lines are skipped; lines are sorted by number, a
/// later line replacing an earlier one with the same number, as when
/// typing them in.
pub fn tokenize(d: &Dialect, lines: &[Vec<u8>]) -> Result<Program, String> {
    tokenize_with_variables(d, lines, Vec::new())
}

/// As [`tokenize`], with the variable table starting with `names` (so a
/// program read from a file keeps its variable numbers). Other variables
/// are added in order of first use.
pub fn tokenize_with_variables(
    d: &Dialect,
    lines: &[Vec<u8>],
    names: Vec<Vec<u8>>,
) -> Result<Program, String> {
    let mut numbered: Vec<(u16, &[u8])> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let text = trim_start(line);
        if text.is_empty() {
            continue;
        }
        let digits = text.iter().take_while(|c| c.is_ascii_digit()).count();
        let number = std::str::from_utf8(&text[..digits])
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|&n| n < 32768)
            .ok_or_else(|| format!("line {} of the text has no line number (0-32767)", i + 1))?;
        numbered.push((number as u16, trim_start(&text[digits..])));
    }
    numbered.sort_by_key(|&(n, _)| n); // stable: the later of two equal lines comes last
    numbered.dedup_by(|later, earlier| {
        if later.0 == earlier.0 {
            *earlier = *later;
            true
        } else {
            false
        }
    });

    let g = grammar(d);
    let mut vars = Variables { names };
    let mut statements = Vec::new();
    let mut error_lines = Vec::new();
    for &(number, text) in &numbered {
        let body = match Parser::new(d, &g, text, &mut vars).line() {
            Ok(body) => body,
            Err(at) => {
                error_lines.push(number);
                error_line(text, at)
            }
        };
        let len = body.len() + 3;
        if len > 255 {
            return Err(format!(
                "line {number} is too long to tokenize ({len} bytes; the limit is 255)"
            ));
        }
        statements.extend_from_slice(&number.to_le_bytes());
        statements.push(len as u8);
        statements.extend(body);
    }
    if vars.names.len() > g.max_variables {
        return Err(format!(
            "too many variables ({}; the limit is {})",
            vars.names.len(),
            g.max_variables
        ));
    }
    Ok(Program {
        bytes: assemble(&vars.names, &statements),
        error_lines,
    })
}

fn trim_start(s: &[u8]) -> &[u8] {
    let n = s.iter().take_while(|&&c| c == b' ').count();
    &s[n..]
}

/// An ERROR- line: the text as typed, the character at `at` in inverse.
fn error_line(text: &[u8], at: usize) -> Vec<u8> {
    let mut raw = text.to_vec();
    if let Some(c) = raw.get_mut(at) {
        *c |= 0x80;
    }
    let mut body = vec![0, ERROR];
    body.extend(raw);
    body.push(EOL);
    body[0] = body.len() as u8 + 3;
    body
}

/// The SAVE file: header, variable names, variable values, statements,
/// and an immediate-mode line (`SAVE "D:"`) as Atari BASIC leaves there.
fn assemble(names: &[Vec<u8>], statements: &[u8]) -> Vec<u8> {
    let mut vnt = Vec::new();
    for n in names {
        let (last, rest) = n.split_last().expect("names aren't empty");
        vnt.extend_from_slice(rest);
        vnt.push(last | 0x80);
    }
    vnt.push(0);
    let mut vvt = Vec::new();
    for (i, n) in names.iter().enumerate() {
        let kind = match n.last() {
            Some(b'$') => 0x80,
            Some(b'(') => 0x40,
            _ => 0x00,
        };
        vvt.extend_from_slice(&[kind, i as u8, 0, 0, 0, 0, 0, 0]);
    }
    let immediate = [
        0x00, 0x80, 0x0B, 0x0B, 0x19, STRING, 2, b'D', b':', EOL_TOKEN,
    ];
    let immediate = &immediate[..];
    let vntp = 0x0100usize;
    let vntd = vntp + vnt.len() - 1;
    let vvtp = vntd + 1;
    let stmtab = vvtp + vvt.len();
    let stmcur = stmtab + statements.len();
    let starp = stmcur + immediate.len();
    let mut f = Vec::new();
    for p in [0, vntp, vntd, vvtp, stmtab, stmcur, starp] {
        f.extend_from_slice(&(p as u16).to_le_bytes());
    }
    f.extend(vnt);
    f.extend(vvt);
    f.extend_from_slice(statements);
    f.extend_from_slice(immediate);
    f
}

/// The variable name table being built.
struct Variables {
    names: Vec<Vec<u8>>,
}

impl Variables {
    /// A variable's token: `$80` + its number, or for numbers 128 and up
    /// the grammar's two-byte form.
    fn token(&mut self, name: &[u8], g: &Grammar) -> Result<Vec<u8>, ()> {
        let i = match self.names.iter().position(|n| n == name) {
            Some(i) => i,
            None => {
                self.names.push(name.to_vec());
                self.names.len() - 1
            }
        };
        match (i, g.high_variable) {
            (0..128, _) => Ok(vec![FIRST_VARIABLE + i as u8]),
            (128..256, Some(t)) if i < g.max_variables => Ok(vec![t, (i - 128) as u8]),
            _ => Err(()),
        }
    }
}

/// Parses one line's text into statement bytes. Errors are the position
/// where parsing failed.
struct Parser<'a, 'v> {
    d: &'a Dialect,
    g: &'a Grammar,
    s: &'a [u8],
    i: usize,
    out: Vec<u8>,
    vars: &'v mut Variables,
}

type Fail = usize;

impl<'a, 'v> Parser<'a, 'v> {
    fn new(d: &'a Dialect, g: &'a Grammar, s: &'a [u8], vars: &'v mut Variables) -> Self {
        Parser {
            d,
            g,
            s,
            i: 0,
            out: Vec::new(),
            vars,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek() == Some(b' ') {
            self.i += 1;
        }
    }

    fn at_end(&self) -> bool {
        self.i >= self.s.len()
    }

    fn fail<T>(&self) -> Result<T, Fail> {
        Err(self.i)
    }

    /// If the text continues with `word` (after spaces), consume it.
    fn eat(&mut self, word: &[u8]) -> bool {
        let save = self.i;
        self.skip_spaces();
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            true
        } else {
            self.i = save;
            false
        }
    }

    /// The whole line: statements separated by `:`.
    fn line(mut self) -> Result<Vec<u8>, Fail> {
        loop {
            self.skip_spaces();
            let start = self.out.len();
            self.out.push(0); // offset to the next statement, filled in below
            let last = self.statement()?;
            let end = self.out.len() + 3;
            if end > 255 {
                // Leave it to the caller to report the length.
                self.out[start] = 255;
            } else {
                self.out[start] = end as u8;
            }
            if last {
                return Ok(self.out);
            }
        }
    }

    /// One statement, including its `:` or end of line. Returns true if
    /// it was the last on the line.
    fn statement(&mut self) -> Result<bool, Fail> {
        let token = self.statement_keyword();
        self.out.push(token);
        match token {
            REM | DATA => {
                self.skip_spaces();
                self.out.extend_from_slice(&self.s[self.i..]);
                self.out.push(EOL);
                self.i = self.s.len();
                return Ok(true);
            }
            LET | IMPLIED_LET => self.assignment()?,
            FOR => {
                self.target()?;
                self.expect(b"=", ASSIGN_NUM)?;
                self.expression()?;
                self.expect(b"TO", TO)?;
                self.expression()?;
                if self.eat(b"STEP") {
                    self.out.push(STEP);
                    self.expression()?;
                }
            }
            IF => {
                self.expression()?;
                if !self.eat(b"THEN") {
                    if self.g.if_without_then {
                        // A block IF: the condition, then `:` or the end.
                        return self.end_of_statement();
                    }
                    self.skip_spaces();
                    return self.fail();
                }
                self.out.push(THEN);
                self.skip_spaces();
                if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.number()?;
                } else if self.at_end() {
                    return self.fail();
                } else {
                    // THEN starts a new statement without a `:`.
                    return Ok(false);
                }
            }
            ON => {
                self.expression()?;
                let target = self.g.on_targets.iter().find(|(w, _)| {
                    let save = self.i;
                    let found = self.eat(w.as_bytes());
                    self.i = save;
                    found
                });
                let Some(&(word, token)) = target else {
                    self.skip_spaces();
                    return self.fail();
                };
                self.eat(word.as_bytes());
                self.out.push(token);
                self.list()?;
            }
            t if Some(t) == self.g.dashes => {
                self.skip_spaces();
                return if self.at_end() { Ok(true) } else { self.fail() };
            }
            t if self.g.signed.contains(&t) => {
                self.skip_spaces();
                match self.peek() {
                    Some(b'+') => {
                        self.i += 1;
                        self.out.push(0x25);
                    }
                    Some(b'-') => {
                        self.i += 1;
                        self.out.push(0x26);
                    }
                    _ => {}
                }
            }
            DIM | COM => self.dimensions()?,
            _ => self.list()?,
        }
        self.end_of_statement()
    }

    /// `:` (more to come) or the end of the line.
    fn end_of_statement(&mut self) -> Result<bool, Fail> {
        self.skip_spaces();
        if self.at_end() {
            self.out.push(EOL_TOKEN);
            Ok(true)
        } else if self.peek() == Some(b':') {
            self.i += 1;
            self.out.push(EOS);
            Ok(false)
        } else {
            self.fail()
        }
    }

    fn expect(&mut self, word: &[u8], token: u8) -> Result<(), Fail> {
        if self.eat(word) {
            self.out.push(token);
            Ok(())
        } else {
            self.skip_spaces();
            self.fail()
        }
    }

    /// The statement's keyword (or an abbreviation ending in `.`), or the
    /// implied LET if none matches: the first in table order that the
    /// text starts with, as Atari BASIC searches.
    fn statement_keyword(&mut self) -> u8 {
        let rest = &self.s[self.i..];
        let mut best: Option<(u8, usize)> = None;
        for t in 0..=u8::MAX {
            if t == IMPLIED_LET || t == ERROR {
                continue;
            }
            if Some(t) == self.g.dashes {
                // Typed as `--`, listed as a line of dashes.
                if rest.starts_with(b"--") {
                    self.i += rest.iter().take_while(|&&c| c == b'-').count();
                    return t;
                }
                continue;
            }
            // The longest keyword the text starts with: ENDIF, not END.
            let Some(name) = self.d.statement(t).filter(|n| !n.is_empty()) else {
                continue;
            };
            let name = name.as_bytes();
            if rest.starts_with(name) && best.is_none_or(|(_, len)| name.len() > len) {
                best = Some((t, name.len()));
            }
        }
        if let Some((t, len)) = best {
            self.i += len;
            return t;
        }
        // Abbreviations: a prefix of a keyword, then a period.
        if let Some(dot) = rest.iter().position(|&c| c == b'.')
            && rest[..dot].iter().all(|c| c.is_ascii_uppercase())
        {
            let prefix = &rest[..dot];
            for t in (0..=u8::MAX).filter(|&t| t != IMPLIED_LET && t != ERROR) {
                if let Some(name) = self.d.statement(t).filter(|n| !n.is_empty())
                    && name.as_bytes().starts_with(prefix)
                {
                    self.i += dot + 1;
                    return t;
                }
            }
        }
        IMPLIED_LET
    }

    /// `var = expression`, the `=` being an assignment.
    fn assignment(&mut self) -> Result<(), Fail> {
        let string = self.target()?;
        self.expect(b"=", if string { ASSIGN_STR } else { ASSIGN_NUM })?;
        self.expression()?;
        Ok(())
    }

    /// A variable to assign to, with any subscript. Returns true if it's
    /// a string.
    fn target(&mut self) -> Result<bool, Fail> {
        self.skip_spaces();
        if !self.peek().is_some_and(|c| c.is_ascii_uppercase()) {
            return self.fail();
        }
        let string = self.variable(false)?;
        Ok(string)
    }

    /// Expressions separated by `,` or `;`, with `#` allowed before any of
    /// them (PRINT #6;"X", OPEN #1,4,0,"D:X", GOTO 100, nothing at all...).
    fn list(&mut self) -> Result<(), Fail> {
        loop {
            self.skip_spaces();
            match self.peek() {
                None | Some(b':') => return Ok(()),
                Some(b',') => {
                    self.i += 1;
                    self.out.push(COMMA);
                }
                Some(b';') => {
                    self.i += 1;
                    self.out.push(SEMICOLON);
                }
                Some(b'#') => {
                    self.i += 1;
                    self.out.push(HASH);
                    self.expression()?;
                }
                _ => {
                    self.expression()?;
                }
            }
        }
    }

    /// DIM and COM: `A(10),B$(20),C(2,3)`.
    fn dimensions(&mut self) -> Result<(), Fail> {
        loop {
            self.skip_spaces();
            if !self.peek().is_some_and(|c| c.is_ascii_uppercase()) {
                return self.fail();
            }
            self.variable(true)?;
            if !self.eat(b",") {
                return Ok(());
            }
            self.out.push(COMMA);
        }
    }

    /// A variable reference, with its subscript if it has one. In DIM the
    /// brackets are DIM's own. Returns true if the result is a string.
    fn variable(&mut self, in_dim: bool) -> Result<bool, Fail> {
        let start = self.i;
        let underscore = self.g.underscore;
        while self.peek().is_some_and(|c| {
            c.is_ascii_uppercase() || c.is_ascii_digit() || (underscore && c == b'_')
        }) {
            self.i += 1;
        }
        let mut name = self.s[start..self.i].to_vec();
        let string = self.peek() == Some(b'$');
        if string {
            self.i += 1;
            name.push(b'$');
        }
        let subscript = self.peek() == Some(b'(');
        if subscript && !string {
            name.push(b'(');
        }
        let token = self.vars.token(&name, self.g).or_else(|_| self.fail())?;
        self.out.extend(token);
        if subscript {
            self.i += 1;
            let paren = match (string, in_dim) {
                (true, false) => STRING_PAREN,
                (true, true) => DIM_STRING_PAREN,
                (false, false) => ARRAY_PAREN,
                (false, true) => DIM_ARRAY_PAREN,
            };
            self.out.push(paren);
            self.inside()?;
        }
        Ok(string)
    }

    /// The rest of a bracket: expressions separated by `,` (the array
    /// comma), then `)`.
    fn inside(&mut self) -> Result<(), Fail> {
        loop {
            self.expression()?;
            self.skip_spaces();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                    self.out.push(ARRAY_COMMA);
                }
                Some(b')') => {
                    self.i += 1;
                    self.out.push(CLOSE_PAREN);
                    return Ok(());
                }
                _ => return self.fail(),
            }
        }
    }

    /// An expression. Returns true if it's a string. Stops (without
    /// consuming) at anything that can't continue it: `,`, `;`, `)`, `:`,
    /// THEN, TO, STEP, GOTO, GOSUB or the end of the line.
    fn expression(&mut self) -> Result<bool, Fail> {
        let mut string = self.operand()?;
        loop {
            self.skip_spaces();
            let rest = &self.s[self.i..];
            if let Some(&(op, num, st)) = RELATIONS.iter().find(|(op, ..)| rest.starts_with(op)) {
                self.i += op.len();
                self.out.push(if string { st } else { num });
                self.operand()?;
                string = false;
                continue;
            }
            if let Some(&(_, t)) = ARITHMETIC.iter().find(|(c, _)| rest.first() == Some(c)) {
                self.i += 1;
                self.out.push(t);
                self.operand()?;
                string = false;
                continue;
            }
            if let Some(&(op, t)) = self
                .g
                .operators
                .iter()
                .find(|(op, _)| rest.starts_with(op.as_bytes()))
            {
                self.i += op.len();
                self.out.push(t);
                self.operand()?;
                string = false;
                continue;
            }
            if rest.starts_with(b"AND") {
                self.i += 3;
                self.out.push(AND);
            } else if rest.starts_with(b"OR") {
                self.i += 2;
                self.out.push(OR);
            } else {
                return Ok(string);
            }
            // What follows decides the next comparison: A$="N" OR A$="Y".
            string = self.operand()?;
        }
    }

    /// One value, with any unary operators and NOT before it. Returns
    /// true if it's a string.
    fn operand(&mut self) -> Result<bool, Fail> {
        self.skip_spaces();
        let rest = &self.s[self.i..];
        match rest.first() {
            Some(b'+') => {
                self.i += 1;
                self.out.push(UNARY_PLUS);
                self.operand()
            }
            Some(b'-') => {
                self.i += 1;
                self.out.push(UNARY_MINUS);
                self.operand()
            }
            Some(b'(') => {
                self.i += 1;
                self.out.push(OPEN_PAREN);
                let string = self.expression()?;
                if !self.eat(b")") {
                    self.skip_spaces();
                    return self.fail();
                }
                self.out.push(CLOSE_PAREN);
                Ok(string)
            }
            Some(b'"') => {
                self.string()?;
                Ok(true)
            }
            Some(c) if c.is_ascii_digit() || *c == b'.' => {
                self.number()?;
                Ok(false)
            }
            Some(b'$') if self.g.hex_number.is_some() => {
                self.hex()?;
                Ok(false)
            }
            Some(_) if rest.starts_with(b"NOT") => {
                // NOT binds less tightly than a comparison, so what
                // follows decides it: NOT A$="X".
                self.i += 3;
                self.out.push(NOT);
                self.operand()
            }
            Some(&c) => {
                let g = self.g;
                for &(name, token, string, args) in &g.functions {
                    if !rest.starts_with(name.as_bytes()) {
                        continue;
                    }
                    let after = trim_start(&rest[name.len()..]);
                    if args == Args::Paren && after.first() != Some(&b'(') {
                        continue;
                    }
                    self.i += name.len();
                    self.out.push(token);
                    match args {
                        Args::Paren => {
                            self.skip_spaces();
                            self.i += 1;
                            self.out.push(FUNCTION_PAREN);
                            self.inside()?;
                        }
                        Args::OwnParen => self.inside()?,
                        Args::None => {}
                    }
                    return Ok(string);
                }
                if c.is_ascii_uppercase() {
                    self.variable(false)
                } else {
                    self.fail()
                }
            }
            _ => self.fail(),
        }
    }

    /// A hex constant (`$FF`): the hex token, then the value in BCD.
    fn hex(&mut self) -> Result<(), Fail> {
        self.i += 1;
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
            self.i += 1;
        }
        let digits = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
        let Ok(value) = u32::from_str_radix(digits, 16) else {
            return self.fail();
        };
        let (bcd, _) = parse_number(value.to_string().as_bytes()).ok_or(self.i)?;
        self.out
            .push(self.g.hex_number.expect("checked by the caller"));
        self.out.extend_from_slice(&bcd);
        Ok(())
    }

    /// A string constant: `"` to `"` (or the end of the line).
    fn string(&mut self) -> Result<(), Fail> {
        self.i += 1;
        let start = self.i;
        while self.peek().is_some_and(|c| c != b'"') {
            self.i += 1;
        }
        let text = &self.s[start..self.i];
        if self.peek() == Some(b'"') {
            self.i += 1;
        }
        let len = u8::try_from(text.len()).or_else(|_| self.fail())?;
        self.out.push(STRING);
        self.out.push(len);
        self.out.extend_from_slice(text);
        Ok(())
    }

    /// A numeric constant.
    fn number(&mut self) -> Result<(), Fail> {
        match parse_number(&self.s[self.i..]) {
            Some((bcd, len)) => {
                self.i += len;
                self.out.push(NUMBER);
                self.out.extend_from_slice(&bcd);
                Ok(())
            }
            None => self.fail(),
        }
    }
}

/// A number as Atari BASIC stores it, and how many characters it took:
/// 6-byte BCD, a base-100 exponent then 10 digits, extra digits dropped.
pub fn parse_number(s: &[u8]) -> Option<([u8; 6], usize)> {
    let mut i = 0;
    let mut digits = Vec::new();
    let mut before_point = None;
    while let Some(&c) = s.get(i) {
        if c.is_ascii_digit() {
            digits.push(c - b'0');
        } else if c == b'.' && before_point.is_none() {
            before_point = Some(digits.len());
        } else {
            break;
        }
        i += 1;
    }
    if digits.is_empty() {
        return None;
    }
    // value = 0.DIGITS x 10^exp10
    let mut exp10 = before_point.unwrap_or(digits.len()) as i32;
    if s.get(i) == Some(&b'E') {
        let mut j = i + 1;
        let negative = s.get(j) == Some(&b'-');
        if matches!(s.get(j), Some(b'-' | b'+')) {
            j += 1;
        }
        let start = j;
        while s.get(j).is_some_and(|c| c.is_ascii_digit()) {
            j += 1;
        }
        if j > start {
            let e: i32 = std::str::from_utf8(&s[start..j]).ok()?.parse().ok()?;
            exp10 += if negative { -e } else { e };
            i = j;
        }
    }
    let leading = digits.iter().take_while(|&&d| d == 0).count();
    if leading == digits.len() {
        return Some(([0; 6], i));
    }
    digits.drain(..leading);
    exp10 -= leading as i32;
    // Base-100 pairs: make the digits before the point an even number.
    if exp10.rem_euclid(2) == 1 {
        digits.insert(0, 0);
        exp10 += 1;
    }
    let exp100 = exp10 / 2 - 1;
    if !(-64..=63).contains(&exp100) {
        return None;
    }
    digits.resize(10, 0);
    let mut n = [0u8; 6];
    n[0] = (exp100 + 64) as u8;
    for k in 0..5 {
        n[k + 1] = digits[2 * k] << 4 | digits[2 * k + 1];
    }
    Some((n, i))
}

/// A SAVE file's variable names and statement bytes (the lines, without
/// the immediate-mode line), for comparing programs.
pub fn program_parts(bytes: &[u8]) -> Option<(Vec<Vec<u8>>, Vec<u8>)> {
    let h = crate::basic::Header::read(bytes).ok()?;
    let names = crate::basic::variable_names(&bytes[h.vnt.clone()]);
    let program = &bytes[h.program.clone()];
    // Stop at the immediate-mode line if it's inside the range.
    let mut pos = 0;
    while pos + 3 <= program.len() {
        if u16::from_le_bytes([program[pos], program[pos + 1]]) >= 0x8000 {
            break;
        }
        pos += program[pos + 2].max(1) as usize;
    }
    Some((names, program[..pos.min(program.len())].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic::{ATARI_BASIC_FILE, format_number};
    use retro_core::FileFormat;

    #[test]
    fn numbers_round_trip() {
        for s in [
            "0",
            "1",
            "10",
            "100",
            "1234.5",
            "0.5",
            "0.25",
            "0.01",
            "53279",
            "3.14159265",
            "9999999999",
            "1.0E-03",
            "1.5E-05",
            "1.0E+10",
            "1.23456789E+10",
        ] {
            let (n, len) = parse_number(s.as_bytes()).unwrap();
            assert_eq!(len, s.len(), "{s}");
            assert_eq!(format_number(n), s, "{s}");
        }
        assert_eq!(
            parse_number(b".5").unwrap().0,
            parse_number(b"0.5").unwrap().0
        );
        assert_eq!(
            parse_number(b"1E10").unwrap().0,
            parse_number(b"1.0E+10").unwrap().0
        );
    }

    fn round_trip(text: &[&str]) -> Vec<String> {
        let lines: Vec<Vec<u8>> = text.iter().map(|l| l.as_bytes().to_vec()).collect();
        let p = tokenize(&crate::basic::ATARI_BASIC, &lines).unwrap();
        let d = ATARI_BASIC_FILE.decode(&p.bytes).unwrap();
        d.lines
            .iter()
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .collect()
    }

    #[test]
    fn lists_back_the_same() {
        let text = [
            "10 REM HELLO",
            "20 DIM A$(10),B(5),C(2,3)",
            "30 A$=\"HI\":B(1)=2:C(1,2)=A$(1,1)=\"H\"",
            "40 FOR I=1 TO 10 STEP 2:NEXT I",
            "50 IF A$<>\"X\" THEN PRINT -1;\" \"; NOT 0",
            "60 IF I>2 THEN 100",
            "70 ON I GOTO 10,20,30",
            "80 PRINT #6;\"X\":OPEN #1,4,0,\"D:X\"",
            "90 X=PEEK(53279)+USR(1536,1,2)*LEN(A$)",
            "100 DATA 1,2, THREE",
            "110 ? :? \"A\":END ",
        ];
        assert_eq!(
            round_trip(&text),
            [
                "10 REM HELLO",
                "20 DIM A$(10),B(5),C(2,3)",
                "30 A$=\"HI\":B(1)=2:C(1,2)=A$(1,1)=\"H\"",
                "40 FOR I=1 TO 10 STEP 2:NEXT I",
                "50 IF A$<>\"X\" THEN PRINT -1;\" \"; NOT 0",
                "60 IF I>2 THEN 100",
                "70 ON I GOTO 10,20,30",
                "80 PRINT #6;\"X\":OPEN #1,4,0,\"D:X\"",
                "90 X=PEEK(53279)+USR(1536,1,2)*LEN(A$)",
                "100 DATA 1,2, THREE",
                "110 ? :? \"A\":END ",
            ]
        );
    }

    #[test]
    fn bad_lines_become_error_lines() {
        let lines = vec![b"10 PRUNT 5".to_vec(), b"20 END".to_vec()];
        let p = tokenize(&crate::basic::ATARI_BASIC, &lines).unwrap();
        assert_eq!(p.error_lines, [10]);
        let d = ATARI_BASIC_FILE.decode(&p.bytes).unwrap();
        // The error is at the 5, which Atari BASIC shows in inverse video.
        assert_eq!(d.lines[0], b"10 ERROR-   PRUNT \xb5");
        assert_eq!(d.lines[1], b"20 END ");
    }
}
