//! Drawing the character grid with a glyph-atlas texture.

use retro_core::Machine;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

/// Colours for one machine's look.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: (u8, u8, u8),
    pub foreground: (u8, u8, u8),
    pub border: (u8, u8, u8),
    /// Text past the line-length warning column.
    pub warning: (u8, u8, u8),
}

pub fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::RGB(r, g, b)
}

/// 16x16 grid of 8x8 glyphs, white on transparent, tinted when drawn.
pub fn build_atlas<'a>(
    creator: &'a TextureCreator<WindowContext>,
    machine: &dyn Machine,
) -> Result<Texture<'a>, String> {
    let mut tex = creator
        .create_texture_static(PixelFormatEnum::RGBA32, 128, 128)
        .map_err(|e| e.to_string())?;
    let mut px = vec![0u8; 128 * 128 * 4];
    for code in 0..=255u8 {
        let g = machine.glyph(code);
        let (gx, gy) = ((code as usize % 16) * 8, (code as usize / 16) * 8);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..8 {
                if bits & (0x80 >> col) != 0 {
                    let i = ((gy + row) * 128 + gx + col) * 4;
                    px[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }
    tex.update(None, &px, 128 * 4).map_err(|e| e.to_string())?;
    tex.set_blend_mode(BlendMode::BLEND);
    Ok(tex)
}

/// Draws cells on a grid of `cell`-pixel squares, offset by `origin_x`.
pub struct Grid<'c, 't> {
    pub canvas: &'c mut Canvas<Window>,
    pub atlas: &'c mut Texture<'t>,
    pub cell: u32,
    pub origin_x: i32,
}

impl Grid<'_, '_> {
    fn rect(&self, col: usize, row: usize, cols: usize) -> Rect {
        Rect::new(
            self.origin_x + (col as u32 * self.cell) as i32,
            (row as u32 * self.cell) as i32,
            self.cell * cols as u32,
            self.cell,
        )
    }

    pub fn fill(&mut self, col: usize, row: usize, cols: usize, color: Color) {
        let r = self.rect(col, row, cols);
        self.canvas.set_draw_color(color);
        let _ = self.canvas.fill_rect(r);
    }

    /// Draw codes starting at (col, row) in `fg` (background untouched).
    pub fn codes(&mut self, col: usize, row: usize, codes: &[u8], fg: Color) {
        self.atlas.set_color_mod(fg.r, fg.g, fg.b);
        for (i, &code) in codes.iter().enumerate() {
            let src = Rect::new((code as i32 % 16) * 8, (code as i32 / 16) * 8, 8, 8);
            let dst = self.rect(col + i, row, 1);
            let _ = self.canvas.copy(self.atlas, src, dst);
        }
    }
}
