//! The interface fonts for menus, the status bar, dialogs and text pages.
//!
//! Uses the operating system's own UI font when it can be found (Segoe UI
//! on Windows, the system font on macOS, fontconfig's sans-serif on Linux),
//! falling back to the built-in Noto Sans so the program never depends on
//! installed fonts.

use std::collections::HashMap;

use fontdue::{Font, FontSettings};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

/// Noto Sans and Noto Sans Mono, (c) Google, SIL Open Font License 1.1.
static FALLBACK: &[u8] = include_bytes!("../../../assets/fonts/ui/NotoSans-Regular.ttf");
static FALLBACK_MONO: &[u8] = include_bytes!("../../../assets/fonts/ui/NotoSansMono-Regular.ttf");

/// The fallback fonts' licence, for the Licensing page.
pub const FALLBACK_LICENSE: &str = include_str!("../../../assets/fonts/ui/OFL.txt");

/// Proportional for menus and dialogs; monospace for pages of plain text
/// such as licences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Proportional,
    Monospace,
}

struct Glyph<'t> {
    tex: Option<Texture<'t>>,
    width: u32,
    height: u32,
    xmin: i32,
    ymin: i32,
    advance: f32,
}

pub struct UiFont<'t> {
    font: Font,
    px: f32,
    ascent: f32,
    line_height: f32,
    creator: &'t TextureCreator<WindowContext>,
    glyphs: HashMap<char, Glyph<'t>>,
}

/// Candidate system font files for a style, best first.
fn system_font_paths(style: Style) -> Vec<std::path::PathBuf> {
    let mono = style == Style::Monospace;
    let mut v: Vec<std::path::PathBuf> = Vec::new();
    if cfg!(windows) {
        let dir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
        let fonts = std::path::Path::new(&dir).join("Fonts");
        let names: &[&str] = if mono {
            &["consola.ttf", "cour.ttf"]
        } else {
            &["segoeui.ttf", "tahoma.ttf"]
        };
        v.extend(names.iter().map(|n| fonts.join(n)));
    } else if cfg!(target_os = "macos") {
        let names: &[&str] = if mono {
            &[
                "/System/Library/Fonts/SFNSMono.ttf",
                "/System/Library/Fonts/Menlo.ttc",
                "/System/Library/Fonts/Monaco.ttf",
            ]
        } else {
            &[
                "/System/Library/Fonts/SFNS.ttf",
                "/System/Library/Fonts/Helvetica.ttc",
                "/Library/Fonts/Arial.ttf",
            ]
        };
        v.extend(names.iter().map(Into::into));
    } else {
        // Ask fontconfig for the desktop's font.
        let family = if mono { "monospace" } else { "sans-serif" };
        if let Ok(out) = std::process::Command::new("fc-match")
            .args(["-f", "%{file}", family])
            .output()
            && out.status.success()
        {
            v.push(String::from_utf8_lossy(&out.stdout).trim().into());
        }
        let names: &[&str] = if mono {
            &[
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
                "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
            ]
        } else {
            &[
                "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
                "/usr/share/fonts/TTF/DejaVuSans.ttf",
            ]
        };
        v.extend(names.iter().map(Into::into));
    }
    v
}

fn load_font(style: Style) -> Font {
    for path in system_font_paths(style) {
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(font) = Font::from_bytes(bytes, FontSettings::default())
            && font.lookup_glyph_index('A') != 0
        {
            return font;
        }
    }
    let fallback = match style {
        Style::Proportional => FALLBACK,
        Style::Monospace => FALLBACK_MONO,
    };
    Font::from_bytes(fallback, FontSettings::default()).expect("built-in font is valid")
}

impl<'t> UiFont<'t> {
    /// Load the system font of the given style at `px` pixels.
    pub fn new(creator: &'t TextureCreator<WindowContext>, px: f32, style: Style) -> Self {
        let font = load_font(style);
        let (ascent, line_height) = match font.horizontal_line_metrics(px) {
            Some(m) => (m.ascent, m.new_line_size),
            None => (px * 0.8, px * 1.2),
        };
        Self {
            font,
            px,
            ascent,
            line_height,
            creator,
            glyphs: HashMap::new(),
        }
    }

    /// Height of a line of text in pixels.
    pub fn line_height(&self) -> i32 {
        self.line_height.ceil() as i32
    }

    fn glyph(&mut self, c: char) -> &Glyph<'t> {
        let (font, px, creator) = (&self.font, self.px, self.creator);
        self.glyphs.entry(c).or_insert_with(|| {
            let (m, coverage) = font.rasterize(c, px);
            let tex = (m.width > 0 && m.height > 0)
                .then(|| {
                    let mut rgba = Vec::with_capacity(coverage.len() * 4);
                    for a in coverage {
                        rgba.extend_from_slice(&[255, 255, 255, a]);
                    }
                    let mut t = creator
                        .create_texture_static(
                            PixelFormatEnum::RGBA32,
                            m.width as u32,
                            m.height as u32,
                        )
                        .ok()?;
                    t.update(None, &rgba, m.width * 4).ok()?;
                    t.set_blend_mode(BlendMode::BLEND);
                    Some(t)
                })
                .flatten();
            Glyph {
                tex,
                width: m.width as u32,
                height: m.height as u32,
                xmin: m.xmin,
                ymin: m.ymin,
                advance: m.advance_width,
            }
        })
    }

    /// Width of `s` in pixels.
    pub fn measure(&mut self, s: &str) -> i32 {
        s.chars().map(|c| self.glyph(c).advance).sum::<f32>().ceil() as i32
    }

    /// Draw `s` with its line box's top-left at (x, y). Returns the width.
    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        x: i32,
        y: i32,
        s: &str,
        color: Color,
    ) -> i32 {
        let baseline = y as f32 + self.ascent;
        let mut pen = x as f32;
        for c in s.chars() {
            self.glyph(c); // make sure it's cached
            let g = self.glyphs.get_mut(&c).expect("cached above");
            if let Some(tex) = &mut g.tex {
                let dst = Rect::new(
                    (pen + g.xmin as f32).round() as i32,
                    (baseline - g.ymin as f32 - g.height as f32).round() as i32,
                    g.width,
                    g.height,
                );
                tex.set_color_mod(color.r, color.g, color.b);
                let _ = canvas.copy(tex, None, dst);
            }
            pen += g.advance;
        }
        (pen - x as f32).ceil() as i32
    }
}
