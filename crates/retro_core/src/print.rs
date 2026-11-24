//! Printing: the document laid out on pages as a PDF.
//!
//! The machine's characters are drawn from its own 8x8 glyphs, so a
//! printout shows exactly what's on screen (graphics, inverse video) with
//! no font to install. The glyphs become a PDF Type 3 font: each one a few
//! filled squares, and each printed row one string of codes. A ToUnicode
//! table maps the codes to the clipboard's characters, so text copied out
//! of the PDF is the same as text copied in the editor.
//!
//! The optional header (file name and page number) and line numbers use
//! Helvetica, one of the fonts every PDF reader has.

use crate::machine::Machine;

/// Paper sizes, in points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    Letter,
    A4,
}

impl Paper {
    pub fn size(self) -> (f32, f32) {
        match self {
            Paper::Letter => (612.0, 792.0),
            Paper::A4 => (595.28, 841.89),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrintSettings {
    pub paper: Paper,
    /// Characters per printed row: `Some(n)` for a fixed width (40, as on
    /// the screen), `None` to fill the page.
    pub columns: Option<usize>,
    /// Character height (and width: they're square), in points.
    pub size: f32,
    /// File name and page number at the top of each page.
    pub header: bool,
    /// Each line's number (counting from 1) left of its first row.
    pub line_numbers: bool,
}

/// Space around the printed area, in points (half an inch).
const MARGIN: f32 = 36.0;
/// Header text size, and the space the header takes above the text.
const HEADER_SIZE: f32 = 9.0;
const HEADER_SPACE: f32 = 24.0;
/// Line numbers take this many characters' width: five digits (Helvetica's
/// are 0.556 of its size, at 85% of a character's) and a space.
const NUMBER_CHARS: f32 = 4.0;
const NUMBER_SCALE: f32 = 0.85;
const DIGIT_WIDTH: f32 = 0.556;

/// The size of the printed area: characters per row and rows per page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageLayout {
    pub columns: usize,
    pub rows: usize,
}

impl PrintSettings {
    /// The most characters a row can hold at this size.
    pub fn fit_columns(&self) -> usize {
        let (w, _) = self.paper.size();
        ((w - 2.0 * MARGIN - self.number_width()) / self.size)
            .floor()
            .max(1.0) as usize
    }

    /// The width of the line-number column, in points (0 without one).
    fn number_width(&self) -> f32 {
        if self.line_numbers {
            NUMBER_CHARS * self.size
        } else {
            0.0
        }
    }

    pub fn layout(&self) -> PageLayout {
        let (_, h) = self.paper.size();
        let header = if self.header { HEADER_SPACE } else { 0.0 };
        let fit = self.fit_columns();
        PageLayout {
            columns: self.columns.map_or(fit, |n| n.min(fit)),
            rows: ((h - 2.0 * MARGIN - header) / self.size).floor().max(1.0) as usize,
        }
    }
}

/// The printed rows: each line split into rows of `columns` characters
/// (an empty line is one empty row), with the line's index on its first.
pub fn rows(lines: &[Vec<u8>], columns: usize) -> Vec<(Option<usize>, &[u8])> {
    let mut v = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            v.push((Some(i), &line[..]));
        } else {
            v.extend(
                line.chunks(columns)
                    .enumerate()
                    .map(|(r, row)| ((r == 0).then_some(i), row)),
            );
        }
    }
    v
}

/// How many pages the document takes.
pub fn page_count(lines: &[Vec<u8>], s: &PrintSettings) -> usize {
    let l = s.layout();
    rows(lines, l.columns).len().div_ceil(l.rows).max(1)
}

/// The document as a PDF. `title` goes in the header and the PDF's
/// details, with `producer` (the program's name).
pub fn pdf(
    machine: &dyn Machine,
    lines: &[Vec<u8>],
    title: &str,
    producer: &str,
    s: &PrintSettings,
) -> Vec<u8> {
    let layout = s.layout();
    let all_rows = rows(lines, layout.columns);
    let pages: Vec<&[(Option<usize>, &[u8])]> = if all_rows.is_empty() {
        vec![&[]]
    } else {
        all_rows.chunks(layout.rows).collect()
    };
    let (pw, ph) = s.paper.size();

    let mut w = Writer::default();
    // Objects 1-5: catalog, page tree, the two fonts, the ToUnicode map.
    // Then the 256 glyph procedures, then each page and its contents.
    let glyph0 = 6;
    let page0 = glyph0 + 256;
    let page_ids: Vec<usize> = (0..pages.len()).map(|i| page0 + i * 2).collect();

    w.object(1, "<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = page_ids.iter().map(|id| format!("{id} 0 R")).collect();
    w.object(
        2,
        &format!(
            "<< /Type /Pages /Count {} /Kids [{}] /MediaBox [0 0 {pw} {ph}] >>",
            pages.len(),
            kids.join(" ")
        ),
    );
    let procs: Vec<String> = (0..256)
        .map(|c| format!("/g{c:02X} {} 0 R", glyph0 + c))
        .collect();
    let names: Vec<String> = (0..256).map(|c| format!("/g{c:02X}")).collect();
    w.object(
        3,
        &format!(
            "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 8 8] \
             /FontMatrix [0.125 0 0 0.125 0 0] /CharProcs << {} >> \
             /Encoding << /Type /Encoding /Differences [0 {}] >> \
             /FirstChar 0 /LastChar 255 /Widths [{}] /Resources << >> \
             /ToUnicode 5 0 R >>",
            procs.join(" "),
            names.join(" "),
            ["8"; 256].join(" ")
        ),
    );
    w.object(
        4,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
    w.stream(5, "", to_unicode(machine).as_bytes());
    for code in 0..=255u8 {
        w.stream(
            glyph0 + code as usize,
            "",
            &glyph_proc(&machine.glyph(code)),
        );
    }

    let header = latin1(title);
    for (n, rows) in pages.iter().enumerate() {
        let mut c = String::new();
        let top = ph - MARGIN;
        let mut y = top;
        if s.header {
            let page = format!("Page {} of {}", n + 1, pages.len());
            let right = pw - MARGIN - helvetica_width(&page) * HEADER_SIZE;
            c += &format!(
                "BT /F2 {HEADER_SIZE} Tf {MARGIN} {} Td ({}) Tj ET\n\
                 BT /F2 {HEADER_SIZE} Tf {right:.2} {} Td ({page}) Tj ET\n\
                 0.5 w {MARGIN} {} m {} {} l S\n",
                top - HEADER_SIZE,
                escape(&header),
                top - HEADER_SIZE,
                top - HEADER_SIZE - 5.0,
                pw - MARGIN,
                top - HEADER_SIZE - 5.0,
            );
            y -= HEADER_SPACE;
        }
        // Each row's baseline is its bottom edge (the glyphs fill the
        // whole 8x8 cell).
        let size = s.size;
        let left = MARGIN + s.number_width();
        c += &format!("BT /F1 {size} Tf {size} TL {left} {} Td\n", y - size);
        for (i, (_, row)) in rows.iter().enumerate() {
            if i > 0 {
                c += "T* ";
            }
            let hex: String = row.iter().map(|b| format!("{b:02X}")).collect();
            c += &format!("<{hex}> Tj\n");
        }
        c += "ET\n";
        if s.line_numbers {
            // Grey, right-aligned a character's width before the text, on
            // the characters' baseline (their bottom pixel row is the
            // space between lines).
            let n_size = size * NUMBER_SCALE;
            c += &format!("BT 0.45 g /F2 {n_size:.2} Tf\n");
            for (i, (line, _)) in rows.iter().enumerate() {
                if let Some(line) = line {
                    let n = (line + 1).to_string();
                    let x = left - size - n.len() as f32 * DIGIT_WIDTH * n_size;
                    let base = y - (i + 1) as f32 * size + size / 8.0;
                    c += &format!("1 0 0 1 {x:.2} {base:.2} Tm ({n}) Tj\n");
                }
            }
            c += "ET\n";
        }
        let id = page_ids[n];
        w.object(
            id,
            &format!(
                "<< /Type /Page /Parent 2 0 R /Contents {} 0 R \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> >>",
                id + 1
            ),
        );
        w.stream(id + 1, "", c.as_bytes());
    }
    let info = page0 + pages.len() * 2;
    w.object(
        info,
        &format!(
            "<< /Title ({}) /Producer ({}) >>",
            escape(&header),
            escape(&latin1(producer))
        ),
    );
    w.finish(info)
}

/// A glyph as a Type 3 procedure: each run of set pixels in a row is one
/// filled rectangle, a hair bigger than the pixels so neighbours overlap
/// and no hairline gaps show between them.
fn glyph_proc(g: &[u8; 8]) -> Vec<u8> {
    let mut s = String::from("8 0 0 0 8 8 d1\n");
    for (r, &bits) in g.iter().enumerate() {
        let y = 7 - r as i32;
        let mut x = 0;
        while x < 8 {
            if bits & (0x80 >> x) == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < 8 && bits & (0x80 >> x) != 0 {
                x += 1;
            }
            s += &format!(
                "{} {} {} {} re\n",
                start as f32 - 0.02,
                y as f32 - 0.02,
                (x - start) as f32 + 0.04,
                1.04
            );
        }
    }
    s += "f\n";
    s.into_bytes()
}

/// The ToUnicode map: each code to the character the clipboard uses.
fn to_unicode(machine: &dyn Machine) -> String {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def /CMapType 2 def\n\
         1 begincodespacerange <00> <FF> endcodespacerange\n",
    );
    for block in (0..=255u8).collect::<Vec<_>>().chunks(64) {
        s += &format!("{} beginbfchar\n", block.len());
        for &code in block {
            let mut utf16 = [0u16; 2];
            let units = machine.char_for_code(code).encode_utf16(&mut utf16);
            let hex: String = units.iter().map(|u| format!("{u:04X}")).collect();
            s += &format!("<{code:02X}> <{hex}>\n");
        }
        s += "endbfchar\n";
    }
    s += "endcmap CMapName currentdict /CMap defineresource pop end end\n";
    s
}

/// Text for Helvetica's WinAnsi encoding: Latin-1 characters as their
/// byte, anything else as `?`.
fn latin1(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c as u32 {
            0x20..=0x7E | 0xA0..=0xFF => c as u8,
            _ => b'?',
        })
        .collect()
}

/// A PDF literal string's contents.
fn escape(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                s.push('\\');
                s.push(b as char);
            }
            0x20..=0x7E => s.push(b as char),
            _ => s += &format!("\\{b:03o}"),
        }
    }
    s
}

/// The width of "Page n of m" in Helvetica, in text-size units.
fn helvetica_width(s: &str) -> f32 {
    s.chars()
        .map(|c| match c {
            ' ' | 'f' => 278,
            'P' => 667,
            _ => 556, // digits, a, e, g, o
        })
        .sum::<u32>() as f32
        / 1000.0
}

/// Writes numbered objects and the cross-reference table.
#[derive(Default)]
struct Writer {
    out: Vec<u8>,
    offsets: Vec<(usize, usize)>,
}

impl Writer {
    fn start(&mut self) {
        if self.out.is_empty() {
            // The binary comment tells tools the file isn't plain text.
            self.out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        }
    }

    fn object(&mut self, id: usize, body: &str) {
        self.start();
        self.offsets.push((id, self.out.len()));
        self.out
            .extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
    }

    fn stream(&mut self, id: usize, dict: &str, data: &[u8]) {
        self.start();
        self.offsets.push((id, self.out.len()));
        self.out.extend_from_slice(
            format!("{id} 0 obj\n<< /Length {} {dict}>>\nstream\n", data.len()).as_bytes(),
        );
        self.out.extend_from_slice(data);
        self.out.extend_from_slice(b"\nendstream\nendobj\n");
    }

    fn finish(mut self, info: usize) -> Vec<u8> {
        self.offsets.sort();
        let count = self.offsets.len() + 1;
        let xref = self.out.len();
        let mut s = format!("xref\n0 {count}\n0000000000 65535 f \n");
        for (_, off) in &self.offsets {
            s += &format!("{off:010} 00000 n \n");
        }
        s += &format!(
            "trailer\n<< /Size {count} /Root 1 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        );
        self.out.extend_from_slice(s.as_bytes());
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(columns: Option<usize>, size: f32) -> PrintSettings {
        PrintSettings {
            paper: Paper::Letter,
            columns,
            size,
            header: true,
            line_numbers: false,
        }
    }

    #[test]
    fn layout_fits_the_page() {
        // Letter is 8.5 x 11 inches; half-inch margins leave 7.5 inches
        // (540 points) across.
        assert_eq!(settings(None, 10.0).layout().columns, 54);
        assert_eq!(settings(Some(40), 10.0).layout().columns, 40);
        // Too big for 40 across: as many as fit.
        assert_eq!(settings(Some(40), 16.0).layout().columns, 33);
        // 720 points down, less the header: 69 rows of 10 points.
        assert_eq!(settings(None, 10.0).layout().rows, 69);
    }

    #[test]
    fn long_lines_wrap() {
        let lines = vec![vec![b'A'; 100], vec![], b"B".to_vec()];
        let r = rows(&lines, 40);
        assert_eq!(
            r.iter().map(|(_, r)| r.len()).collect::<Vec<_>>(),
            [40, 40, 20, 0, 1]
        );
        // Only each line's first row has its number.
        assert_eq!(
            r.iter().map(|(n, _)| *n).collect::<Vec<_>>(),
            [Some(0), None, None, Some(1), Some(2)]
        );
    }

    /// A machine whose characters are all blank, for writing PDFs.
    struct Blank;
    impl Machine for Blank {
        fn name(&self) -> &'static str {
            "Blank"
        }
        fn glyph(&self, _code: u8) -> [u8; 8] {
            [0; 8]
        }
        fn space(&self) -> u8 {
            b' '
        }
        fn screen_columns(&self) -> usize {
            40
        }
        fn left_margin(&self) -> usize {
            0
        }
        fn formats(&self) -> &'static [&'static dyn crate::FileFormat] {
            &[]
        }
    }

    #[test]
    fn line_numbers_take_four_columns() {
        let s = PrintSettings {
            line_numbers: true,
            ..settings(None, 10.0)
        };
        assert_eq!(s.layout().columns, 50);
        let pdf =
            String::from_utf8_lossy(&pdf(&Blank, &[b"A".to_vec(), vec![b'B'; 60]], "T", "P", &s))
                .into_owned();
        assert!(pdf.contains("(1) Tj") && pdf.contains("(2) Tj"));
        assert!(!pdf.contains("(3) Tj"));
    }

    #[test]
    fn glyphs_become_rectangles() {
        let p = String::from_utf8(glyph_proc(&[0x81, 0, 0, 0, 0, 0, 0, 0xFF])).unwrap();
        assert_eq!(p.matches(" re").count(), 3);
    }
}
