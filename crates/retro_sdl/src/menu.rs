//! The menu bar: menu definitions, drawing and mouse hit-testing.

use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::Canvas;
use sdl2::video::Window;

use crate::source::Source;
use crate::ui_font::UiFont;

/// Everything a menu item can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    New,
    Open,
    OpenFromImage,
    SaveToImage,
    ReopenAs,
    Import,
    Save,
    SaveAs,
    SaveListingAs,
    Export,
    Print,
    /// The Recent Files submenu (it opens; it does nothing itself).
    RecentFiles,
    /// Open the `n`th recent file.
    OpenRecent(u8),
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    FindNext,
    FindPrevious,
    Replace,
    GoToLine,
    ToggleWrap,
    ToggleLineNumbers,
    ZoomIn,
    ZoomOut,
    ToggleClick,
    ToggleCursorFlash,
    Fonts,
    /// The Atari CONTROL key: Ctrl or Alt.
    ToggleControlAlt,
    /// The Atari CONTROL key: left or right.
    ToggleControlSide,
    Preferences,
    ToggleCharset,
    KeyHelp,
    FileTypes,
    Licensing,
    About,
}

pub struct Item {
    pub label: std::borrow::Cow<'static, str>,
    pub shortcut: &'static str,
    /// `None` draws a separator line.
    pub command: Option<Command>,
    /// The items of a submenu that opens beside this one (empty: none).
    pub submenu: Vec<Item>,
}

pub struct Menu {
    /// Title with `&` before the letter that opens it with Alt.
    pub title: &'static str,
    pub items: Vec<Item>,
}

/// A shortcut as this system shows it: "Ctrl+Shift+S" on Windows and
/// Linux; on macOS, with the Command key, "⇧⌘S" (and Redo as ⇧⌘Z).
pub fn shortcut_text(s: &str) -> String {
    if !cfg!(target_os = "macos") {
        return s.to_string();
    }
    // Mac equivalents of keys that aren't simply Ctrl -> Command.
    match s {
        "F3" => return "\u{2318}G".into(),
        "Shift+F3" => return "\u{21E7}\u{2318}G".into(),
        "Ctrl+H" => return "\u{2325}\u{2318}F".into(),
        "Ctrl+G" => return "\u{2318}L".into(),
        _ => {}
    }
    let Some(rest) = s.strip_prefix("Ctrl+") else {
        return s.to_string();
    };
    if rest == "Y" {
        return "\u{21E7}\u{2318}Z".into();
    }
    match rest.strip_prefix("Shift+") {
        Some(key) => format!("\u{21E7}\u{2318}{key}"),
        None => format!("\u{2318}{rest}"),
    }
}

const fn item(label: &'static str, shortcut: &'static str, c: Command) -> Item {
    Item {
        label: std::borrow::Cow::Borrowed(label),
        shortcut,
        command: Some(c),
        submenu: Vec::new(),
    }
}

const SEP: Item = Item {
    label: std::borrow::Cow::Borrowed(""),
    shortcut: "",
    command: None,
    submenu: Vec::new(),
};

/// The menus. `alt_charset` names the machine's second character set, if
/// it has one, for a toggle in the Options menu; `images` adds the disk
/// image items to the File menu. In labels, `&` marks the hot key
/// (underlined; pressing it chooses the item while the menu is open).
pub fn menus(alt_charset: Option<&'static str>, recent: &[String], images: bool) -> Vec<Menu> {
    use Command::*;
    let mut m = vec![
        Menu {
            title: "&File",
            items: vec![
                item("&New", "Ctrl+N", New),
                item("&Open...", "Ctrl+O", Open),
                item("Open from I&mage...", "", OpenFromImage),
                item("Recent &Files", "", RecentFiles),
                item("&Reopen As...", "", ReopenAs),
                item("&Import...", "", Import),
                SEP,
                item("&Save", "Ctrl+S", Save),
                item("Save &As...", "Ctrl+Shift+S", SaveAs),
                item("Save &to Image...", "", SaveToImage),
                item("Save &Listing As...", "", SaveListingAs),
                item("&Export...", "", Export),
                SEP,
                item("&Print to PDF...", "Ctrl+P", Print),
                SEP,
                // (Recent files are inserted here.)
                item("&Quit", "Ctrl+Q", Quit),
            ],
        },
        Menu {
            title: "&Edit",
            items: edit_items(),
        },
        Menu {
            title: "&Options",
            items: vec![
                item("&40 Columns", "F4", ToggleWrap),
                item("&Line Numbers", "", ToggleLineNumbers),
                item("Zoom &In", "Ctrl++", ZoomIn),
                item("Zoom &Out", "Ctrl+-", ZoomOut),
                SEP,
                item("&Key Click", "F5", ToggleClick),
                item("&Flashing Cursor", "", ToggleCursorFlash),
                item("&Right-Side CONTROL Key", "F7", ToggleControlSide),
                item(
                    if cfg!(target_os = "macos") {
                        "OP&TION as CONTROL Key"
                    } else {
                        "&ALT as CONTROL Key"
                    },
                    "F8",
                    ToggleControlAlt,
                ),
                SEP,
                item("Fo&nts...", "", Fonts),
                item("&Preferences...", "Ctrl+,", Preferences),
            ],
        },
        Menu {
            title: "&Help",
            items: vec![
                item("&Keyboard Help", "F1", KeyHelp),
                item("&File Types", "", FileTypes),
                SEP,
                item("&Licensing", "", Licensing),
                item("&About", "", About),
            ],
        },
    ];
    if let Some(label) = alt_charset {
        m[2].items.insert(4, item(label, "F6", ToggleCharset));
    }
    // Find and Go to Line, in the Edit menu only (not the right-click one).
    m[1].items.extend([
        SEP,
        item("&Find...", "Ctrl+F", Find),
        item("Find &Next", "F3", FindNext),
        item("Find Pre&vious", "Shift+F3", FindPrevious),
        item("R&eplace...", "Ctrl+H", Replace),
        item("&Go to Line...", "Ctrl+G", GoToLine),
    ]);
    if !images {
        m[0].items
            .retain(|i| !matches!(i.command, Some(OpenFromImage | SaveToImage)));
    }
    // The recent files, numbered, in the Recent Files submenu.
    let recent: Vec<Item> = recent_labels(recent)
        .into_iter()
        .enumerate()
        .map(|(i, label)| Item {
            label: std::borrow::Cow::Owned(label),
            shortcut: "",
            command: Some(OpenRecent(i as u8)),
            submenu: Vec::new(),
        })
        .collect();
    if let Some(it) = m[0]
        .items
        .iter_mut()
        .find(|it| it.command == Some(RecentFiles))
    {
        it.submenu = recent;
    }
    m
}

/// "&1 PROG.BAS" for each recent file ("&2 MYDISK.ATR > SAMPLE.LST" for
/// a file in a disk image), adding the folder's name where two share a
/// name.
fn recent_labels(recent: &[String]) -> Vec<String> {
    let sources: Vec<Source> = recent.iter().map(|p| Source::from_recent(p)).collect();
    sources
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let n = s.label();
            let shared = sources.iter().filter(|q| q.label() == n).count() > 1;
            // `&&` shows a literal & in a label... names rarely have one;
            // drop it rather than mark a hot key.
            let n = n.replace('&', "");
            if shared {
                let dir = s
                    .host()
                    .parent()
                    .and_then(|d| d.file_name())
                    .map(|d| d.to_string_lossy().into_owned())
                    .unwrap_or_default();
                format!("&{} {n}  ({dir})", i + 1)
            } else {
                format!("&{} {n}", i + 1)
            }
        })
        .collect()
}

fn edit_items() -> Vec<Item> {
    use Command::*;
    vec![
        item("&Undo", "Ctrl+Z", Undo),
        item("&Redo", "Ctrl+Y", Redo),
        SEP,
        item("Cu&t", "Ctrl+X", Cut),
        item("&Copy", "Ctrl+C", Copy),
        item("&Paste", "Ctrl+V", Paste),
        SEP,
        item("Select &All", "Ctrl+A", SelectAll),
    ]
}

/// A label without its `&` hot-key marker.
pub fn plain(label: &str) -> String {
    label.replacen('&', "", 1)
}

/// The hot key marked with `&` in a label, lower case.
pub fn hot_key(label: &str) -> Option<char> {
    let i = label.find('&')?;
    label[i + 1..]
        .chars()
        .next()
        .map(|c| c.to_ascii_lowercase())
}

/// Draw a label at (x, y), underlining its hot key.
fn draw_label(
    canvas: &mut Canvas<Window>,
    font: &mut UiFont,
    x: i32,
    y: i32,
    label: &str,
    color: Color,
) {
    font.draw(canvas, x, y, &plain(label), color);
    if let Some(i) = label.find('&')
        && let Some(c) = label[i + 1..].chars().next()
    {
        let before = font.measure(&label[..i]);
        let w = font.measure(&c.to_string());
        let uy = y + font.baseline() + 2;
        canvas.set_draw_color(color);
        let _ = canvas.draw_line((x + before, uy), (x + before + w - 1, uy));
    }
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
    /// A menu-bar menu.
    pub open: Option<usize>,
    /// The right-click menu, opened at this pixel position.
    pub context: Option<(i32, i32)>,
    pub hover: Option<usize>,
    /// The hovered item's submenu is showing.
    pub sub_open: bool,
    /// The highlighted item in that submenu.
    pub sub_hover: Option<usize>,
}

impl MenuState {
    pub fn is_open(&self) -> bool {
        self.open.is_some() || self.context.is_some()
    }
}

/// The right-click menu.
pub fn context_menu() -> Menu {
    Menu {
        title: "",
        items: edit_items(),
    }
}

/// Where things were drawn, for mouse hit-testing.
#[derive(Debug, Default, Clone)]
pub struct Geometry {
    pub bar_height: i32,
    pub titles: Vec<Rect>,
    pub dropdown: Option<Rect>,
    pub items: Vec<Rect>,
    /// The open submenu, and its items.
    pub sub_dropdown: Option<Rect>,
    pub sub_items: Vec<Rect>,
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

    pub fn in_sub_dropdown(&self, x: i32, y: i32) -> bool {
        self.sub_dropdown.is_some_and(|r| r.contains_point((x, y)))
    }

    pub fn sub_item_at(&self, x: i32, y: i32) -> Option<usize> {
        self.sub_items.iter().position(|r| r.contains_point((x, y)))
    }
}

/// Item state supplied by the app when drawing.
pub struct ItemState {
    pub enabled: bool,
    pub checked: Option<bool>,
}

/// Draw the bar, and the open menu or right-click menu if any. `pad` is
/// the basic spacing unit in pixels. Returns the geometry for hit-testing.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    canvas: &mut Canvas<Window>,
    font: &mut UiFont,
    menus: &[Menu],
    context: &Menu,
    state: &MenuState,
    (width, height): (u32, u32),
    pad: i32,
    item_state: &dyn Fn(Command) -> ItemState,
) -> Geometry {
    let t = &THEME;
    let bar_h = font.line_height() + pad;
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
        let w = font.measure(&plain(m.title)) + pad * 2;
        let r = Rect::new(x, 0, w as u32, (bar_h - 1) as u32);
        let open = state.open == Some(i);
        if open {
            canvas.set_draw_color(t.highlight);
            let _ = canvas.fill_rect(r);
        }
        let color = if open { t.highlight_text } else { t.text };
        draw_label(canvas, font, x + pad, pad / 2, m.title, color);
        geom.titles.push(r);
        x += w;
    }

    // A menu-bar menu always opens under the bar, never over its title
    // (where releasing the click that opened it would choose an item).
    let (menu, left, top, fixed) = match (state.open, state.context) {
        (Some(i), _) => (&menus[i], geom.titles[i].x(), bar_h - 1, true),
        (None, Some((x, y))) => (context, x, y, false),
        (None, None) => return geom,
    };
    let (drop, items) = draw_dropdown(
        canvas,
        font,
        &menu.items,
        state.hover,
        (left, top, fixed),
        (width as i32, height as i32),
        pad,
        item_state,
    );
    // The hovered item's submenu, beside it.
    if state.sub_open
        && let Some(h) = state.hover
        && let Some(it) = menu.items.get(h).filter(|it| !it.submenu.is_empty())
    {
        let (sub, sub_items) = draw_dropdown(
            canvas,
            font,
            &it.submenu,
            state.sub_hover,
            (drop.right() - 2, items[h].y() - pad / 4, false),
            (width as i32, height as i32),
            pad,
            item_state,
        );
        geom.sub_dropdown = Some(sub);
        geom.sub_items = sub_items;
    }
    geom.dropdown = Some(drop);
    geom.items = items;
    geom
}

/// Draw a menu's items in a box at (left, top), moved if needed to stay
/// inside the window (only sideways if `fixed`). Returns the box and
/// each item's rectangle.
#[allow(clippy::too_many_arguments)]
fn draw_dropdown(
    canvas: &mut Canvas<Window>,
    font: &mut UiFont,
    menu_items: &[Item],
    hover: Option<usize>,
    (left, top, fixed): (i32, i32, bool),
    (win_w, win_h): (i32, i32),
    pad: i32,
    item_state: &dyn Fn(Command) -> ItemState,
) -> (Rect, Vec<Rect>) {
    let t = &THEME;
    let row_h = font.line_height() + pad;
    let sep_h = pad;
    let gap = pad * 3;
    let label_w = menu_items
        .iter()
        .map(|it| font.measure(&plain(&it.label)))
        .max()
        .unwrap_or(0);
    let short_w = menu_items
        .iter()
        .map(|it| font.measure(&shortcut_text(it.shortcut)))
        .max()
        .unwrap_or(0);
    let check_w = pad * 2;
    let w = check_w + label_w + gap + short_w + pad * 2;
    let h: i32 = menu_items
        .iter()
        .map(|it| if it.command.is_some() { row_h } else { sep_h })
        .sum::<i32>()
        + pad / 2;
    let left = left.min(win_w - w).max(0);
    let top = if fixed {
        top
    } else {
        top.min(win_h - h).max(0)
    };
    let drop = Rect::new(left, top, w as u32, h as u32);
    canvas.set_draw_color(t.bar);
    let _ = canvas.fill_rect(drop);
    canvas.set_draw_color(t.border);
    let _ = canvas.draw_rect(drop);

    let mut rects = Vec::new();
    let mut y = top + pad / 4;
    for (i, it) in menu_items.iter().enumerate() {
        let Some(cmd) = it.command else {
            let mid = y + sep_h / 2;
            canvas.set_draw_color(t.border);
            let _ = canvas.draw_line((left + pad / 2, mid), (left + w - pad / 2, mid));
            rects.push(Rect::new(0, 0, 0, 0));
            y += sep_h;
            continue;
        };
        let st = item_state(cmd);
        let r = Rect::new(left + 1, y, (w - 2) as u32, row_h as u32);
        let hot = hover == Some(i) && st.enabled;
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
            let (cx, cy, u) = (left + pad / 2 + pad / 4, y + row_h / 2, (pad / 4).max(2));
            canvas.set_draw_color(color);
            for d in 0..(u / 2).max(1) {
                let _ = canvas.draw_line((cx, cy + d), (cx + u, cy + u + d));
                let _ = canvas.draw_line((cx + u, cy + u + d), (cx + u * 3, cy - u + d));
            }
        }
        draw_label(canvas, font, left + pad / 2 + check_w, ty, &it.label, color);
        let shortcut = shortcut_text(it.shortcut);
        let sw = font.measure(&shortcut);
        font.draw(canvas, left + w - pad - sw, ty, &shortcut, color);
        if !it.submenu.is_empty() || cmd == Command::RecentFiles {
            // A small triangle pointing right: there's a submenu.
            let s = (row_h / 5).max(3);
            let (ax, ay) = (left + w - pad - s, y + row_h / 2);
            canvas.set_draw_color(color);
            for i in 0..=s {
                let _ = canvas.draw_line((ax + i, ay - s + i), (ax + i, ay + s - i));
            }
        }
        rects.push(r);
        y += row_h;
    }
    (drop, rects)
}

/// The next selectable item after `from` in direction `dir` (+1 / -1).
pub fn step(items: &[Item], from: Option<usize>, dir: isize) -> Option<usize> {
    let n = items.len() as isize;
    let mut i = from.map_or(if dir > 0 { -1 } else { n }, |f| f as isize);
    for _ in 0..n {
        i = (i + dir).rem_euclid(n);
        if items[i as usize].command.is_some() {
            return Some(i as usize);
        }
    }
    None
}
