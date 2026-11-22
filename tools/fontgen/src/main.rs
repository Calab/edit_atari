//! Converts a pixel-style TrueType font into a fixed 8x8 bitmap character set.
//!
//! The source fonts we care about (e.g. "EightBit Atari") are really bitmap
//! fonts stored as square outlines. This tool samples each glyph at the centre
//! of every pixel cell and checks that every outline point lies exactly on the
//! pixel grid, so a stray point can't silently corrupt a glyph.
//!
//! Usage:
//!   fontgen info  <font.ttf> <first-codepoint-hex> <count>
//!   fontgen build <font.ttf> <first-codepoint-hex> <count> <out.bin> [sheet.png] [--lenient]
//!
//! `--lenient` writes output even when glyphs are off the pixel grid (for previews).
//!
//! Output: `count` glyphs x 8 bytes, one byte per row, bit 7 = leftmost pixel.

use std::process::ExitCode;

use ttf_parser::{Face, GlyphId, OutlineBuilder};

const CELL: usize = 8;

/// Collects outline segments as straight lines; any curve is recorded so we
/// can refuse fonts that aren't purely made of pixels.
#[derive(Default)]
struct Collector {
    contours: Vec<Vec<(f32, f32)>>,
    has_curves: bool,
}

impl OutlineBuilder for Collector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.contours.push(vec![(x, y)]);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        if let Some(c) = self.contours.last_mut() {
            c.push((x, y));
        }
    }
    fn quad_to(&mut self, _: f32, _: f32, x: f32, y: f32) {
        self.has_curves = true;
        self.line_to(x, y);
    }
    fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, x: f32, y: f32) {
        self.has_curves = true;
        self.line_to(x, y);
    }
    fn close(&mut self) {}
}

/// Non-zero winding test for point (px, py) against all contours.
fn inside(contours: &[Vec<(f32, f32)>], px: f32, py: f32) -> bool {
    let mut winding = 0i32;
    for c in contours {
        for i in 0..c.len() {
            let (x0, y0) = c[i];
            let (x1, y1) = c[(i + 1) % c.len()];
            if y0 <= py {
                if y1 > py && cross(x0, y0, x1, y1, px, py) > 0.0 {
                    winding += 1;
                }
            } else if y1 <= py && cross(x0, y0, x1, y1, px, py) < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}

fn cross(x0: f32, y0: f32, x1: f32, y1: f32, px: f32, py: f32) -> f32 {
    (x1 - x0) * (py - y0) - (px - x0) * (y1 - y0)
}

struct Grid {
    /// Font units per pixel.
    pixel: f32,
    /// Font-unit y coordinate of the top edge of pixel row 0.
    top: f32,
}

/// The cell is the advance width (monospace) split into 8 pixels; its top
/// edge is the highest outline point found among the requested glyphs.
fn grid_for(face: &Face, first: u32, count: usize) -> Grid {
    let advance = face
        .glyph_index('A')
        .and_then(|g| face.glyph_hor_advance(g))
        .unwrap_or(face.units_per_em()) as f32;
    let top = (first..first + count as u32)
        .filter_map(|cp| outline(face, cp))
        .flat_map(|(_, c)| c.contours.into_iter().flatten().map(|p| p.1))
        .reduce(f32::max)
        .unwrap_or(face.ascender() as f32);
    Grid {
        pixel: advance / CELL as f32,
        top,
    }
}

fn outline(face: &Face, cp: u32) -> Option<(GlyphId, Collector)> {
    let ch = char::from_u32(cp)?;
    let gid = face.glyph_index(ch)?;
    let mut c = Collector::default();
    face.outline_glyph(gid, &mut c);
    Some((gid, c))
}

fn parse_args(args: &[String]) -> Result<(Vec<u8>, u32, usize), String> {
    let data = std::fs::read(&args[0]).map_err(|e| format!("{}: {e}", args[0]))?;
    let first = u32::from_str_radix(args[1].trim_start_matches("0x"), 16)
        .map_err(|e| format!("bad codepoint {}: {e}", args[1]))?;
    let count: usize = args[2].parse().map_err(|e| format!("bad count: {e}"))?;
    Ok((data, first, count))
}

fn info(args: &[String]) -> Result<(), String> {
    let (data, first, count) = parse_args(args)?;
    let face = Face::parse(&data, 0).map_err(|e| e.to_string())?;
    let g = grid_for(&face, first, count);
    println!(
        "units/em={} ascender={} descender={} glyphs={} -> pixel={} units, top={}",
        face.units_per_em(),
        face.ascender(),
        face.descender(),
        face.number_of_glyphs(),
        g.pixel,
        g.top
    );
    let mut present = 0;
    let (mut minx, mut maxx, mut miny, mut maxy) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for cp in first..first + count as u32 {
        if let Some((_, c)) = outline(&face, cp) {
            present += 1;
            for &(x, y) in c.contours.iter().flatten() {
                minx = minx.min(x);
                maxx = maxx.max(x);
                miny = miny.min(y);
                maxy = maxy.max(y);
            }
        }
    }
    println!("present {present}/{count}; bbox x {minx}..{maxx} y {miny}..{maxy}");
    Ok(())
}

fn build(args: &[String], lenient: bool) -> Result<(), String> {
    let (data, first, count) = parse_args(args)?;
    let out = args.get(3).ok_or("missing output path")?;
    let face = Face::parse(&data, 0).map_err(|e| e.to_string())?;
    let g = grid_for(&face, first, count);

    let mut bytes = vec![0u8; count * CELL];
    let mut problems = Vec::new();
    let mut missing = Vec::new();

    for i in 0..count {
        let cp = first + i as u32;
        let Some((_, c)) = outline(&face, cp) else {
            missing.push(cp);
            continue;
        };
        if c.has_curves {
            problems.push(format!("U+{cp:04X}: contains curves"));
        }
        for &(x, y) in c.contours.iter().flatten() {
            let gx = x / g.pixel;
            let gy = (g.top - y) / g.pixel;
            if (gx - gx.round()).abs() > 1e-3 || (gy - gy.round()).abs() > 1e-3 {
                problems.push(format!("U+{cp:04X}: point ({x},{y}) is off the pixel grid"));
                break;
            }
            if !(-0.001..=CELL as f32 + 0.001).contains(&gx)
                || !(-0.001..=CELL as f32 + 0.001).contains(&gy)
            {
                problems.push(format!(
                    "U+{cp:04X}: point ({x},{y}) is outside the 8x8 cell"
                ));
                break;
            }
        }
        for row in 0..CELL {
            let mut b = 0u8;
            for col in 0..CELL {
                let px = (col as f32 + 0.5) * g.pixel;
                let py = g.top - (row as f32 + 0.5) * g.pixel;
                if inside(&c.contours, px, py) {
                    b |= 0x80 >> col;
                }
            }
            bytes[i * CELL + row] = b;
        }
    }

    if !missing.is_empty() {
        eprintln!(
            "warning: {} codepoints missing (left blank), e.g. {}",
            missing.len(),
            missing
                .iter()
                .take(8)
                .map(|c| format!("U+{c:04X}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    if !problems.is_empty() {
        for p in &problems {
            eprintln!("error: {p}");
        }
        if !lenient {
            return Err(format!(
                "{} glyph problem(s); no output written",
                problems.len()
            ));
        }
        eprintln!("--lenient: writing output anyway (preview only)");
    }

    std::fs::write(out, &bytes).map_err(|e| format!("{out}: {e}"))?;
    println!("wrote {count} glyphs ({} bytes) to {out}", bytes.len());

    if let Some(sheet) = args.get(4) {
        write_sheet(&bytes, count, sheet)?;
        println!("wrote test sheet {sheet}");
    }
    Ok(())
}

/// Renders all glyphs 16 per row at 4x scale with a 1-pixel grid gap.
fn write_sheet(bytes: &[u8], count: usize, path: &str) -> Result<(), String> {
    const SCALE: usize = 4;
    const PER_ROW: usize = 16;
    let cell = CELL * SCALE + 2;
    let rows = count.div_ceil(PER_ROW);
    let (w, h) = (PER_ROW * cell, rows * cell);
    let mut img = vec![60u8; w * h];
    for i in 0..count {
        let (ox, oy) = ((i % PER_ROW) * cell + 1, (i / PER_ROW) * cell + 1);
        for row in 0..CELL {
            for col in 0..CELL {
                let on = bytes[i * CELL + row] & (0x80 >> col) != 0;
                for dy in 0..SCALE {
                    for dx in 0..SCALE {
                        img[(oy + row * SCALE + dy) * w + ox + col * SCALE + dx] =
                            if on { 255 } else { 0 };
                    }
                }
            }
        }
    }
    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(&img).map_err(|e| e.to_string())?;
    Ok(())
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let lenient = args.iter().any(|a| a == "--lenient");
    args.retain(|a| a != "--lenient");
    let result = match args.first().map(String::as_str) {
        Some("info") if args.len() >= 4 => info(&args[1..]),
        Some("build") if args.len() >= 5 => build(&args[1..], lenient),
        _ => Err(
            "usage: fontgen info|build <font.ttf> <first-hex> <count> [out.bin] [sheet.png]".into(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fontgen: {e}");
            ExitCode::FAILURE
        }
    }
}
