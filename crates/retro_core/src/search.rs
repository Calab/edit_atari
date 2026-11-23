//! Finding text in a document, for Find, Replace and Go to Line.
//!
//! Matches lie within one line. Without "match case", letters match in
//! either case; without "match inverse", characters match in normal or
//! inverse video.

use crate::document::Pos;

/// What has to match exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Match {
    /// Letter case (else `a` matches `A`).
    pub case: bool,
    /// Inverse video (else a character matches its inverse).
    pub inverse: bool,
}

/// A code as compared under `m`.
fn key(c: u8, m: Match) -> u8 {
    let c = if m.inverse { c } else { c & 0x7F };
    if !m.case && (c & 0x7F).is_ascii_lowercase() {
        c - 0x20
    } else {
        c
    }
}

fn matches_at(line: &[u8], col: usize, pat: &[u8], m: Match) -> bool {
    line.get(col..col + pat.len())
        .is_some_and(|s| s.iter().zip(pat).all(|(&a, &b)| key(a, m) == key(b, m)))
}

/// Where `pat` next occurs, searching forward from `from` (a match may
/// start there) or backward from it (a match must start before it),
/// wrapping round the document. `None` if it doesn't occur.
pub fn find(lines: &[Vec<u8>], pat: &[u8], from: Pos, forward: bool, m: Match) -> Option<Pos> {
    if pat.is_empty() || lines.is_empty() {
        return None;
    }
    let n = lines.len();
    let start = from.line.min(n - 1);
    let hit = |line: usize, col: usize| matches_at(&lines[line], col, pat, m);
    if forward {
        // The rest of the start line, the following lines, then round from
        // the top to the start line again.
        for i in 0..=n {
            let line = (start + i) % n;
            let first = if i == 0 { from.col } else { 0 };
            let last = if i == n { from.col } else { lines[line].len() };
            if let Some(col) = (first..=last.min(lines[line].len())).find(|&c| hit(line, c)) {
                return Some(Pos { line, col });
            }
        }
    } else {
        for i in 0..=n {
            let line = (start + n - i % n) % n;
            let len = lines[line].len();
            let below = if i == 0 { from.col } else { len + 1 };
            let above = if i == n { from.col } else { 0 };
            if let Some(col) = (above..below.min(len + 1)).rev().find(|&c| hit(line, c)) {
                return Some(Pos { line, col });
            }
        }
    }
    None
}

/// Every non-overlapping match, in order.
pub fn find_all(lines: &[Vec<u8>], pat: &[u8], m: Match) -> Vec<Pos> {
    let mut v = Vec::new();
    if pat.is_empty() {
        return v;
    }
    for (line, text) in lines.iter().enumerate() {
        let mut col = 0;
        while col + pat.len() <= text.len() {
            if matches_at(text, col, pat, m) {
                v.push(Pos { line, col });
                col += pat.len();
            } else {
                col += 1;
            }
        }
    }
    v
}

/// Whether the text from `at` is `pat`.
pub fn is_match(lines: &[Vec<u8>], at: Pos, pat: &[u8], m: Match) -> bool {
    !pat.is_empty()
        && lines
            .get(at.line)
            .is_some_and(|l| matches_at(l, at.col, pat, m))
}

/// The line number a listing line starts with (after any spaces).
fn line_number(line: &[u8]) -> Option<u32> {
    let text = &line[line.iter().take_while(|&&c| c == b' ').count()..];
    let digits = text.iter().take_while(|c| c.is_ascii_digit()).count();
    std::str::from_utf8(&text[..digits]).ok()?.parse().ok()
}

/// Where Go to Line went, besides the line asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoTo {
    /// The line asked for.
    Found,
    /// No such line number: the next one up, numbered this.
    Next(u32),
    /// Past the highest line number (this one): just after its line.
    After(u32),
    /// Past the end of the file: its last line.
    Last,
}

/// Go to Line: the `n`th line in the editor, or (with `by_number`, in a
/// numbered listing) the line numbered `n`. A line number that isn't
/// there goes to the next one up; above the highest, to just after that
/// line (the start of the next line, or the end of that one if it's the
/// last). An editor line past the end goes to the last line.
pub fn go_to_line(lines: &[Vec<u8>], n: u32, by_number: bool) -> (Pos, GoTo) {
    let last = lines.len().saturating_sub(1);
    let start = |line: usize| Pos { line, col: 0 };
    let numbered: Vec<(usize, u32)> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| line_number(l).map(|m| (i, m)))
        .collect();
    if !by_number || numbered.is_empty() {
        return match n.max(1) as usize - 1 {
            i if i <= last => (start(i), GoTo::Found),
            _ => (start(last), GoTo::Last),
        };
    }
    if let Some(&(i, _)) = numbered.iter().find(|&&(_, m)| m == n) {
        return (start(i), GoTo::Found);
    }
    if let Some(&(i, m)) = numbered
        .iter()
        .filter(|&&(_, m)| m > n)
        .min_by_key(|&&(_, m)| m)
    {
        return (start(i), GoTo::Next(m));
    }
    let &(i, m) = numbered.iter().max_by_key(|&&(_, m)| m).expect("not empty");
    let pos = if i < last {
        start(i + 1)
    } else {
        Pos {
            line: i,
            col: lines[i].len(),
        }
    };
    (pos, GoTo::After(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Vec<Vec<u8>> {
        text.lines().map(|l| l.as_bytes().to_vec()).collect()
    }

    fn pos(line: usize, col: usize) -> Pos {
        Pos { line, col }
    }

    const EXACT: Match = Match {
        case: true,
        inverse: true,
    };
    const ANY: Match = Match {
        case: false,
        inverse: false,
    };

    #[test]
    fn finds_forward_and_back_with_wrapping() {
        let d = doc("10 PRINT X\n20 print y\n30 GOTO 10");
        assert_eq!(find(&d, b"PRINT", pos(0, 0), true, EXACT), Some(pos(0, 3)));
        assert_eq!(find(&d, b"PRINT", pos(0, 4), true, EXACT), Some(pos(0, 3)));
        assert_eq!(find(&d, b"PRINT", pos(0, 4), true, ANY), Some(pos(1, 3)));
        assert_eq!(find(&d, b"10", pos(0, 1), true, EXACT), Some(pos(2, 8)));
        assert_eq!(find(&d, b"10", pos(2, 8), false, EXACT), Some(pos(0, 0)));
        assert_eq!(find(&d, b"10", pos(0, 0), false, EXACT), Some(pos(2, 8)));
        assert_eq!(find(&d, b"NOPE", pos(1, 2), true, ANY), None);
    }

    #[test]
    fn inverse_matches_unless_asked() {
        let d = vec![vec![b'x', 0xC1, b'a']]; // x, inverse A, normal a
        assert_eq!(find(&d, b"a", pos(0, 0), true, ANY), Some(pos(0, 1)));
        let case_only = Match {
            case: true,
            inverse: false,
        };
        assert_eq!(find(&d, b"A", pos(0, 0), true, case_only), Some(pos(0, 1)));
        let inverse_only = Match {
            case: false,
            inverse: true,
        };
        assert_eq!(
            find(&d, &[0xE1], pos(0, 0), true, inverse_only),
            Some(pos(0, 1))
        );
        assert_eq!(
            find(&d, b"A", pos(0, 0), true, inverse_only),
            Some(pos(0, 2))
        );
    }

    #[test]
    fn finds_all_without_overlap() {
        let d = doc("AAAA\nxAAx");
        assert_eq!(
            find_all(&d, b"AA", EXACT),
            [pos(0, 0), pos(0, 2), pos(1, 1)]
        );
    }

    #[test]
    fn goes_to_numbered_lines() {
        let d = doc("10 REM\n  20 X=1\n40 END");
        assert_eq!(go_to_line(&d, 20, true), (pos(1, 0), GoTo::Found));
        // Missing: the next one up.
        assert_eq!(go_to_line(&d, 1, true), (pos(0, 0), GoTo::Next(10)));
        assert_eq!(go_to_line(&d, 30, true), (pos(2, 0), GoTo::Next(40)));
        // Past the highest, on the last line: the end of it.
        assert_eq!(go_to_line(&d, 99, true), (pos(2, 6), GoTo::After(40)));
        // ... or the start of the line after it.
        let e = doc("10 REM\n20 END\n");
        let e = [e, vec![Vec::new()]].concat();
        assert_eq!(go_to_line(&e, 99, true), (pos(2, 0), GoTo::After(20)));
        // Editor lines.
        assert_eq!(go_to_line(&d, 2, false), (pos(1, 0), GoTo::Found));
        assert_eq!(go_to_line(&d, 7, false), (pos(2, 0), GoTo::Last));
        let t = doc("one\ntwo\nthree");
        assert_eq!(go_to_line(&t, 2, true), (pos(1, 0), GoTo::Found));
        assert_eq!(go_to_line(&t, 9, true), (pos(2, 0), GoTo::Last));
    }
}
