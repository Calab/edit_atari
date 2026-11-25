//! The New Image pop-up on the Save to Image page: choose a file system,
//! a disk size (or a custom one), the kind of image file and a volume
//! name, then Create... asks where to save the blank image.

use retro_core::disk::{DiskImages, DiskSize, NewDisk};
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, Texture};
use sdl2::video::Window;

use crate::menu::THEME;
use crate::render::Grid;
use crate::ui_font::UiFont;
use crate::widgets::{Op, Ui};

/// The controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    System,
    /// The sizes, in two rows of radio buttons.
    Sizes(usize),
    Sectors,
    SectorSize,
    Container,
    Volume,
    Create,
    Cancel,
}

/// Sizes in each row of radio buttons.
const PER_ROW: usize = 3;

/// The longest volume name.
const VOLUME_MAX: usize = 8;

/// One file system's choices, from the machine.
struct System {
    name: &'static str,
    sizes: &'static [DiskSize],
    custom: &'static [usize],
    volume_name: bool,
}

/// What a key or click asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Create,
    Cancel,
}

pub struct NewImagePopup {
    systems: Vec<System>,
    /// The image file types ("ATR", "XFD"), and the sector sizes each
    /// holds.
    containers: Vec<(&'static str, Vec<usize>)>,
    system: usize,
    size: usize,
    /// The custom size: sectors as typed, and which sector size.
    sectors: String,
    custom_size: usize,
    container: usize,
    volume: Vec<u8>,
    focus: Field,
    /// Why the last Create failed.
    pub error: Option<String>,
    hits: Vec<(Rect, Field, Op)>,
}

impl NewImagePopup {
    pub fn new(disks: &dyn DiskImages) -> Self {
        let systems = (0..disks.new_systems().len())
            .map(|i| System {
                name: disks.new_systems()[i],
                sizes: disks.new_sizes(i),
                custom: disks.custom_sector_sizes(i),
                volume_name: disks.has_volume_name(i),
            })
            .collect();
        let containers = disks
            .extensions()
            .iter()
            .enumerate()
            .map(|(c, &name)| {
                let holds = [128, 256, 512]
                    .into_iter()
                    .filter(|&s| disks.container_holds(c, s))
                    .collect();
                (name, holds)
            })
            .collect();
        Self {
            systems,
            containers,
            system: 0,
            size: 0,
            sectors: String::new(),
            custom_size: 1,
            container: 0,
            volume: Vec::new(),
            focus: Field::System,
            error: None,
            hits: Vec::new(),
        }
    }

    fn sys(&self) -> &System {
        &self.systems[self.system]
    }

    fn is_custom(&self) -> bool {
        self.sys().sizes[self.size].sector_size.is_none()
    }

    /// Bytes per sector of the disk chosen.
    fn sector_size(&self) -> usize {
        self.sys().sizes[self.size].sector_size.unwrap_or_else(|| {
            self.sys()
                .custom
                .get(self.custom_size)
                .copied()
                .unwrap_or(256)
        })
    }

    /// Whether image type `c` can hold the disk chosen.
    fn holds(&self, c: usize) -> bool {
        self.containers[c].1.contains(&self.sector_size())
    }

    /// After a change: keep the choices ones that go together.
    fn settle(&mut self) {
        self.size = self.size.min(self.sys().sizes.len() - 1);
        let custom = self.sys().custom.len();
        self.custom_size = self.custom_size.min(custom.saturating_sub(1));
        if !self.holds(self.container) {
            self.container = 0;
        }
        self.error = None;
    }

    /// The disk to make, or why the sectors typed won't do.
    pub fn request(&self) -> Result<NewDisk, String> {
        let sectors = if self.is_custom() {
            match self.sectors.parse::<u32>() {
                Ok(n) => n,
                Err(_) => return Err("Type how many sectors the disk has".into()),
            }
        } else {
            0
        };
        Ok(NewDisk {
            system: self.system,
            size: self.size,
            sectors,
            sector_size: self.sector_size(),
            container: self.container,
            volume_name: if self.sys().volume_name {
                self.volume.clone()
            } else {
                Vec::new()
            },
        })
    }

    /// The image file's extension, e.g. "ATR".
    pub fn extension(&self) -> &'static str {
        self.containers[self.container].0
    }

    /// A name for the image file: the volume name, or NEWDISK.
    pub fn file_name(&self) -> String {
        let base: String = self
            .volume
            .iter()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|&c| c as char)
            .collect();
        let base = if base.is_empty() {
            "NEWDISK".into()
        } else {
            base
        };
        format!("{base}.{}", self.extension())
    }

    /// The controls in Tab order, for the choices made.
    fn order(&self) -> Vec<Field> {
        let mut v = vec![Field::System, Field::Sizes(0)];
        if self.sys().sizes.len() > PER_ROW {
            v.push(Field::Sizes(1));
        }
        if self.is_custom() {
            v.push(Field::Sectors);
            if self.sys().custom.len() > 1 {
                v.push(Field::SectorSize);
            }
        }
        v.push(Field::Container);
        if self.sys().volume_name {
            v.push(Field::Volume);
        }
        v.extend([Field::Create, Field::Cancel]);
        v
    }

    fn choose(&mut self, f: Field, i: usize) {
        match f {
            Field::System if i < self.systems.len() => self.system = i,
            Field::Sizes(row) => {
                let i = row * PER_ROW + i;
                if i < self.sys().sizes.len() {
                    self.size = i;
                }
            }
            Field::SectorSize => self.custom_size = i,
            Field::Container if i < self.containers.len() && self.holds(i) => self.container = i,
            _ => {}
        }
        self.settle();
    }

    /// Typed text, in machine codes: digits into Sectors, the volume name
    /// into its box.
    pub fn type_codes(&mut self, codes: &[u8]) {
        match self.focus {
            Field::Sectors => {
                for &c in codes {
                    if c.is_ascii_digit() && self.sectors.len() < 5 {
                        self.sectors.push(c as char);
                    }
                }
            }
            Field::Volume => {
                for &c in codes {
                    if (0x21..=0x7E).contains(&c) && self.volume.len() < VOLUME_MAX {
                        self.volume.push(c);
                    }
                }
            }
            _ => return,
        }
        self.error = None;
    }

    pub fn key(&mut self, k: Keycode, shift: bool) -> Action {
        let order = self.order();
        let at = order.iter().position(|&f| f == self.focus).unwrap_or(0);
        match k {
            Keycode::ESCAPE => return Action::Cancel,
            Keycode::TAB => {
                let n = order.len() as isize;
                let step = if shift { -1 } else { 1 };
                self.focus = order[(at as isize + step).rem_euclid(n) as usize];
            }
            Keycode::RETURN | Keycode::KP_ENTER => {
                return if self.focus == Field::Cancel {
                    Action::Cancel
                } else {
                    Action::Create
                };
            }
            Keycode::SPACE => match self.focus {
                Field::Create => return Action::Create,
                Field::Cancel => return Action::Cancel,
                _ => {}
            },
            Keycode::BACKSPACE => {
                match self.focus {
                    Field::Sectors => {
                        self.sectors.pop();
                    }
                    Field::Volume => {
                        self.volume.pop();
                    }
                    _ => {}
                }
                self.error = None;
            }
            Keycode::LEFT | Keycode::RIGHT => {
                let d: isize = if k == Keycode::LEFT { -1 } else { 1 };
                let step = |i: usize, n: usize| (i as isize + d).clamp(0, n as isize - 1) as usize;
                match self.focus {
                    Field::System => self.system = step(self.system, self.systems.len()),
                    Field::Sizes(_) => {
                        self.size = step(self.size, self.sys().sizes.len());
                        // The focus follows into the size's row.
                        self.focus = Field::Sizes(self.size / PER_ROW);
                    }
                    Field::SectorSize => {
                        self.custom_size = step(self.custom_size, self.sys().custom.len())
                    }
                    Field::Container => {
                        let c = step(self.container, self.containers.len());
                        if self.holds(c) {
                            self.container = c;
                        }
                    }
                    _ => {}
                }
                self.settle();
            }
            _ => {}
        }
        Action::None
    }

    pub fn click(&mut self, x: i32, y: i32) -> Action {
        let Some(&(_, f, op)) = self.hits.iter().find(|(r, ..)| r.contains_point((x, y))) else {
            return Action::None;
        };
        self.focus = f;
        match (f, op) {
            (Field::Create, _) => Action::Create,
            (Field::Cancel, _) => Action::Cancel,
            (_, Op::Choose(i)) => {
                self.choose(f, i);
                Action::None
            }
            _ => Action::None,
        }
    }

    /// Draw the pop-up in the middle of the window.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        font: &mut UiFont,
        atlas: &mut Texture,
        (width, height): (u32, u32),
        pad: i32,
        cell: i32,
        warning: Color,
    ) {
        let t = &THEME;
        canvas.set_blend_mode(BlendMode::BLEND);
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 60));
        let _ = canvas.fill_rect(None);
        canvas.set_blend_mode(BlendMode::NONE);

        let lh = font.line_height();
        let row = lh.max(cell) + pad * 2;
        let labels = ["DOS:", "Disk:", "Sectors:", "Image type:", "Volume name:"];
        let label_w = labels.iter().map(|l| font.measure(l)).max().unwrap_or(0);
        let radio = |font: &mut UiFont, opts: &[&str]| -> i32 {
            opts.iter()
                .map(|o| lh * 2 / 3 + pad + font.measure(o) + pad * 5)
                .sum()
        };
        // The chosen system's choices ('static, so no borrow of self).
        let (sizes, custom, volume_name) = {
            let s = self.sys();
            (s.sizes, s.custom, s.volume_name)
        };
        let size_labels: Vec<&str> = sizes.iter().map(|s| s.label).collect();
        let system_names: Vec<&str> = self.systems.iter().map(|s| s.name).collect();
        let mut longest = radio(font, &system_names);
        for r in size_labels.chunks(PER_ROW) {
            longest = longest.max(radio(font, r));
        }
        let w = (pad * 8 + label_w + pad * 3 + longest).min(width as i32 - pad * 4);
        // A long reason for failing, in lines that fit.
        let error_lines = match &self.error {
            Some(e) => wrap(font, e, w - pad * 8),
            None => Vec::new(),
        };
        let title_h = lh + pad * 2;
        let button_h = lh + pad * 2;
        let size_rows = sizes.len().div_ceil(PER_ROW) as i32;
        let rows = 1 + size_rows + i32::from(self.is_custom()) + 1 + i32::from(volume_name);
        let notes = error_lines.len().max(1) as i32;
        let h = title_h + pad * 3 + rows * row + notes * lh + pad * 3 + button_h + pad * 3;
        let x0 = (width as i32 - w) / 2;
        let y0 = ((height as i32 - h) / 2).max(0);
        let frame = Rect::new(x0, y0, w as u32, h as u32);
        canvas.set_draw_color(Color::RGB(0xFF, 0xFF, 0xFF));
        let _ = canvas.fill_rect(frame);
        canvas.set_draw_color(t.bar);
        let _ = canvas.fill_rect(Rect::new(x0, y0, w as u32, title_h as u32));
        canvas.set_draw_color(t.border);
        let _ = canvas.draw_line((x0, y0 + title_h), (x0 + w - 1, y0 + title_h));
        let _ = canvas.draw_rect(frame);
        font.draw(canvas, x0 + pad * 4, y0 + pad, "New Image", t.text);

        let lx = x0 + pad * 4;
        let fx = lx + label_w + pad * 3;
        let mut y = y0 + title_h + pad * 3;
        let text_y = |y: i32| y + (row - lh) / 2;
        let mut glyphs = None;
        let mut hits = std::mem::take(&mut self.hits);
        hits.clear();
        let holds: Vec<bool> = (0..self.containers.len()).map(|c| self.holds(c)).collect();
        let custom_labels: Vec<String> = custom.iter().map(|s| format!("{s} bytes")).collect();
        {
            let mut ui = Ui {
                canvas: &mut *canvas,
                font: &mut *font,
                pad,
                lh,
                focus: self.focus,
                hits: &mut hits,
                rings: Vec::new(),
                in_row: |_| false,
            };
            ui.font.draw(ui.canvas, lx, text_y(y), "DOS:", t.text);
            ui.choice(Field::System, fx, text_y(y), &system_names, self.system);
            y += row;
            ui.font.draw(ui.canvas, lx, text_y(y), "Disk:", t.text);
            for (r, chunk) in size_labels.chunks(PER_ROW).enumerate() {
                // Only the row holding the choice shows it selected.
                let sel = if self.size / PER_ROW == r {
                    self.size % PER_ROW
                } else {
                    usize::MAX
                };
                ui.choice(Field::Sizes(r), fx, text_y(y), chunk, sel);
                y += row;
            }
            if self.is_custom() {
                ui.font.draw(ui.canvas, lx, text_y(y), "Sectors:", t.text);
                let sw = ui.font.measure("000000") + pad * 2;
                ui.text_field(Field::Sectors, fx, text_y(y), sw, &self.sectors);
                let mut cx = fx + sw + pad * 4;
                if custom_labels.len() > 1 {
                    let opts: Vec<&str> = custom_labels.iter().map(String::as_str).collect();
                    cx = ui.choice(Field::SectorSize, cx, text_y(y), &opts, self.custom_size)
                        + pad * 4;
                } else if let Some(only) = custom_labels.first() {
                    cx += ui.font.draw(ui.canvas, cx, text_y(y), only, t.text) + pad * 4;
                }
                let _ = cx;
                y += row;
            }
            ui.font
                .draw(ui.canvas, lx, text_y(y), "Image type:", t.text);
            let names: Vec<&str> = self.containers.iter().map(|c| c.0).collect();
            let end = ui.choice(Field::Container, fx, text_y(y), &names, self.container);
            if let Some(c) = holds.iter().position(|&h| !h) {
                let note = format!(
                    "({} holds only {}-byte sectors)",
                    self.containers[c].0,
                    self.containers[c].1.first().copied().unwrap_or(128)
                );
                ui.font
                    .draw(ui.canvas, end + pad * 4, text_y(y), &note, t.dim);
            }
            y += row;
            if volume_name {
                ui.font
                    .draw(ui.canvas, lx, text_y(y), "Volume name:", t.text);
                let fw = cell * VOLUME_MAX as i32 + cell + pad * 2;
                let r = Rect::new(
                    fx,
                    y + (row - (cell + pad)) / 2,
                    fw as u32,
                    (cell + pad) as u32,
                );
                ui.field_box(Field::Volume, r);
                let gx = fx + pad;
                let gy = r.y() + pad / 2;
                if self.focus == Field::Volume {
                    let cx = gx + self.volume.len() as i32 * cell;
                    ui.canvas.set_draw_color(t.text);
                    let _ = ui
                        .canvas
                        .fill_rect(Rect::new(cx, gy, cell as u32, cell as u32));
                }
                glyphs = Some((gx, gy, self.volume.clone()));
                y += row;
            }
            for (i, line) in error_lines.iter().enumerate() {
                ui.font
                    .draw(ui.canvas, lx, y + pad + i as i32 * lh, line, warning);
            }

            // [Create...] [Cancel]
            let by = y0 + h - pad * 3 - button_h;
            let bw = ui.font.measure("Create...").max(ui.font.measure("Cancel")) + pad * 6;
            let mut bx = x0 + w - pad * 4;
            for (f, label) in [(Field::Cancel, "Cancel"), (Field::Create, "Create...")] {
                bx -= bw;
                ui.button_w(f, bx, by, bw, label);
                bx -= pad * 2;
            }
            ui.draw_focus();
        }
        self.hits = hits;
        if let Some((x, y, codes)) = glyphs {
            let mut grid = Grid {
                canvas,
                atlas,
                cell: cell as u32,
                origin_x: x,
                origin_y: y,
            };
            grid.codes(0, 0, &codes, t.text);
        }
    }
}

/// `text` in lines no wider than `w` pixels, broken between words.
fn wrap(font: &mut UiFont, text: &str, w: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let tried = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && font.measure(&tried) > w {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = tried;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
