//! The menu bar: menu definitions, drawing and mouse hit-testing.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

use crate::ui_font::UiFont;

/// Everything a menu item can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    New,
    Open,
    Save,
    SaveAs,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    ToggleWrap,
    ZoomIn,
    ZoomOut,
    ToggleClick,
    ToggleControlKey,
    LineWarning,
    ToggleCharset,
    KeyHelp,
    About,
}

pub struct Item {
    pub label: &'static str,
    pub shortcut: &'static str,
    /// `None` draws a separator line.
    pub command: Option<Command>,
}

pub struct Menu {
    pub title: &'static str,
    /// The letter that opens the menu with Alt.
    pub mnemonic: char,
    pub items: Vec<Item>,
}

const fn item(label: &'static str, shortcut: &'static str, c: Command) -> Item {
    Item {
        label,
        shortcut,
        command: Some(c),
    }
}

const SEP: Item = Item {
    label: "",
    shortcut: "",
    command: None,
};

/// The menus. `alt_charset` names the machine's second character set, if
/// it has one, for a toggle in the Options menu.
pub fn menus(alt_charset: Option<&'static str>) -> Vec<Menu> {
    use Command::*;
    let mut m = vec![
        Menu {
            title: "File",
            mnemonic: 'f',
            items: vec![
                item("New", "Ctrl+N", New),
                item("Open...", "Ctrl+O", Open),
                SEP,
                item("Save", "Ctrl+S", Save),
                item("Save As...", "Ctrl+Shift+S", SaveAs),
                SEP,
                item("Quit", "Ctrl+Q", Quit),
            ],
        },
        Menu {
            title: "Edit",
            mnemonic: 'e',
            items: vec![
                item("Undo", "Ctrl+Z", Undo),
                item("Redo", "Ctrl+Y", Redo),
                SEP,
                item("Cut", "Ctrl+X", Cut),
                item("Copy", "Ctrl+C", Copy),
                item("Paste", "Ctrl+V", Paste),
                SEP,
                item("Select All", "Ctrl+A", SelectAll),
            ],
        },
        Menu {
            title: "Options",
            mnemonic: 'o',
            items: vec![
                item("40 Columns", "F4", ToggleWrap),
                item("Zoom In", "Ctrl++", ZoomIn),
                item("Zoom Out", "Ctrl+-", ZoomOut),
                SEP,
                item("Key Click", "F7", ToggleClick),
                item("LEFT-ALT as CONTROL Key", "F8", ToggleControlKey),
                item("Line Length Warning...", "F9", LineWarning),
            ],
        },
        Menu {
            title: "Help",
            mnemonic: 'h',
            items: vec![
                item("Keyboard Help", "F1", KeyHelp),
                SEP,
                item("About", "", About),
            ],
        },
    ];
    if let Some(label) = alt_charset {
        m[2].items.insert(4, item(label, "F6", ToggleCharset));
    }
    m
}

/// Interface colours (a light theme like a standard desktop app).
pub struct Theme {
    pub bar: Color,
    pub text: Color,
    pub dim: Color,
    pub highlight: Color,
    pub highlight_text: Color,
    pub border: Color,
}

pub const THEME: Theme = Theme {
    bar: Color::RGB(0xF0, 0xF0, 0xF0),
    text: Color::RGB(0x1E, 0x1E, 0x1E),
    dim: Color::RGB(0xA0, 0xA0, 0xA0),
    highlight: Color::RGB(0x35, 0x74, 0xD4),
    highlight_text: Color::RGB(0xFF, 0xFF, 0xFF),
    border: Color::RGB(0xB8, 0xB8, 0xB8),
};

/// Which menu is open and which item is highlighted.
#[derive(Debug, Default, Clone, Copy)]
pub struct MenuState {
    pub open: Option<usize>,
    pub hover: Option<usize>,
}

/// Where things were drawn, for mouse hit-testing.
#[derive(Debug, Default, Clone)]
pub struct Geometry {
    pub bar_height: i32,
    pub titles: Vec<Rect>,
    pub dropdown: Option<Rect>,
    pub items: Vec<Rect>,
}

impl Geometry {
    pub fn title_at(&self, x: i32, y: i32) -> Option<usize> {
        self.titles.iter().position(|r| r.contains_point((x, y)))
    }

    pub fn item_at(&self, x: i32, y: i32) -> Option<usize> {
        self.items.iter().position(|r| r.contains_point((x, y)))
    }

    pub fn in_dropdown(&self, x: i32, y: i32) -> bool {
        self.dropdown.is_some_and(|r| r.contains_point((x, y)))
    }
}

/// Item state supplied by the app when drawing.
pub struct ItemState {
    pub enabled: bool,
    pub checked: Option<bool>,
}

/// Draw the bar (and the open menu, if any). `pad` is the basic spacing
/// unit in pixels. Returns the geometry for hit-testing.
pub fn draw(
    canvas: &mut Canvas<Window>,
    font: &mut UiFont,
    menus: &[Menu],
    state: &MenuState,
    width: u32,
    pad: i32,
    item_state: &dyn Fn(Command) -> ItemState,
) -> Geometry {
    let t = &THEME;
    let lh = font.line_height();
    let bar_h = lh + pad;
    let mut geom = Geometry {
        bar_height: bar_h,
        ..Default::default()
    };

    canvas.set_draw_color(t.bar);
    let _ = canvas.fill_rect(Rect::new(0, 0, width, bar_h as u32));
    canvas.set_draw_color(t.border);
    let _ = canvas.draw_line((0, bar_h - 1), (width as i32, bar_h - 1));

    let mut x = pad / 2;
    for (i, m) in menus.iter().enumerate() {
        let w = font.measure(m.title) + pad * 2;
        let r = Rect::new(x, 0, w as u32, (bar_h - 1) as u32);
        let open = state.open == Some(i);
        if open {
            canvas.set_draw_color(t.highlight);
            let _ = canvas.fill_rect(r);
        }
        let color = if open { t.highlight_text } else { t.text };
        font.draw(canvas, x + pad, pad / 2, m.title, color);
        geom.titles.push(r);
        x += w;
    }

    let Some(open) = state.open else {
        return geom;
    };
    let menu = &menus[open];
    let sep_h = pad;
    let gap = pad * 3;
    let label_w = menu
        .items
        .iter()
        .map(|it| font.measure(it.label))
        .max()
        .unwrap_or(0);
    let short_w = menu
        .items
        .iter()
        .map(|it| font.measure(it.shortcut))
        .max()
        .unwrap_or(0);
    let check_w = pad * 2;
    let w = check_w + label_w + gap + short_w + pad * 2;
    let h: i32 = menu
        .items
        .iter()
        .map(|it| if it.command.is_some() { bar_h } else { sep_h })
        .sum::<i32>()
        + pad / 2;
    let left = geom.titles[open].x();
    let drop = Rect::new(left, bar_h - 1, w as u32, h as u32);
    canvas.set_draw_color(t.bar);
    let _ = canvas.fill_rect(drop);
    canvas.set_draw_color(t.border);
    let _ = canvas.draw_rect(drop);
    geom.dropdown = Some(drop);

    let mut y = drop.y() + pad / 4;
    for (i, it) in menu.items.iter().enumerate() {
        let Some(cmd) = it.command else {
            let mid = y + sep_h / 2;
            canvas.set_draw_color(t.border);
            let _ = canvas.draw_line((left + pad / 2, mid), (left + w - pad / 2, mid));
            geom.items.push(Rect::new(0, 0, 0, 0));
            y += sep_h;
            continue;
        };
        let st = item_state(cmd);
        let r = Rect::new(left + 1, y, (w - 2) as u32, bar_h as u32);
        let hot = state.hover == Some(i) && st.enabled;
        if hot {
            canvas.set_draw_color(t.highlight);
            let _ = canvas.fill_rect(r);
        }
        let color = if !st.enabled {
            t.dim
        } else if hot {
            t.highlight_text
        } else {
            t.text
        };
        let ty = y + pad / 2;
        if st.checked == Some(true) {
            // A tick drawn with lines (not every font has a check mark).
            let (cx, cy, u) = (left + pad / 2 + pad / 4, y + bar_h / 2, (pad / 4).max(2));
            canvas.set_draw_color(color);
            for d in 0..(u / 2).max(1) {
                let _ = canvas.draw_line((cx, cy + d), (cx + u, cy + u + d));
                let _ = canvas.draw_line((cx + u, cy + u + d), (cx + u * 3, cy - u + d));
            }
        }
        font.draw(canvas, left + pad / 2 + check_w, ty, it.label, color);
        let sw = font.measure(it.shortcut);
        font.draw(canvas, left + w - pad - sw, ty, it.shortcut, color);
        geom.items.push(r);
        y += bar_h;
    }
    geom
}

/// The next selectable item after `from` in direction `dir` (+1 / -1).
pub fn step(menu: &Menu, from: Option<usize>, dir: isize) -> Option<usize> {
    let n = menu.items.len() as isize;
    let mut i = from.map_or(if dir > 0 { -1 } else { n }, |f| f as isize);
    for _ in 0..n {
        i = (i + dir).rem_euclid(n);
        if menu.items[i as usize].command.is_some() {
            return Some(i as usize);
        }
    }
    None
}
