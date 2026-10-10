//! Tiled on-screen keyboards: one tile per connected physical keyboard, keys
//! light up while held. Works while the window is unfocused.

mod layout;
mod layouts;
mod prefs;
mod typing;

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Shape as Paint, Stroke, StrokeKind, Vec2};
use layout::{Dir, Layout, Shape};
use layouts::FormFactor;
use manykey::{Event, Key, KeyState, KeyboardId, KeyboardInfo, Listener, Options, Scope};
use prefs::Prefs;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use typing::EnterMode;

const FADE: Duration = Duration::from_millis(160);
/// How long a "key not in this layout" notice lingers after the key is released.
const OUTSIDE_LINGER: Duration = Duration::from_millis(2000);
const MIN_TILE_WIDTH: f32 = 440.0;
const WARN: Color32 = Color32::from_rgb(235, 140, 40);
const DISCONNECTED: Color32 = Color32::from_rgb(210, 100, 100);

struct Tile {
    info: KeyboardInfo,
    connected: bool,
    factor: FormFactor,
    down: HashSet<Key>,
    released_at: HashMap<Key, Instant>,
    /// Native scancode of each held key, for the optional OS check.
    native: HashMap<Key, u32>,
    /// Keys pressed that this tile's form factor doesn't draw, and when.
    outside: HashMap<(Key, u32), Instant>,
    /// Caps Lock as inferred from key presses seen since startup (the OS's own
    /// state isn't visible to us, so this can start out wrong).
    caps: bool,
    typed: String,
}

impl Tile {
    fn shift(&self) -> bool {
        self.down.contains(&Key::LeftShift) || self.down.contains(&Key::RightShift)
    }

    fn command(&self) -> bool {
        [Key::LeftCtrl, Key::RightCtrl, Key::LeftAlt, Key::RightAlt, Key::LeftGui, Key::RightGui]
            .iter()
            .any(|k| self.down.contains(k))
    }

    /// Names of keys pressed outside the drawn layout that are held or recent.
    fn outside_now(&self, now: Instant) -> Vec<String> {
        let mut v: Vec<String> = self
            .outside
            .iter()
            .filter(|((key, _), t)| self.down.contains(key) || now.duration_since(**t) < OUTSIDE_LINGER)
            .map(|((key, native), _)| key_label(*key, *native))
            .collect();
        v.sort();
        v.dedup();
        v
    }
}

struct App {
    rx: mpsc::Receiver<Event>,
    tiles: Vec<Tile>,
    layouts: HashMap<FormFactor, Layout>,
    prefs: Prefs,
    unit: f32,
    overflows: u64,
    show_info: bool,
    enter_mode: EnterMode,
    /// Width of the action-button group last frame, so it can wrap as one unit.
    buttons_width: f32,
}

impl App {
    fn new(rx: mpsc::Receiver<Event>) -> Self {
        let layouts = FormFactor::ALL.into_iter().map(|f| (f, Layout::new(f))).collect();
        Self {
            rx,
            tiles: Vec::new(),
            layouts,
            prefs: Prefs::load(),
            unit: 24.0,
            overflows: 0,
            show_info: false,
            enter_mode: EnterMode::NewLine,
            buttons_width: 0.0,
        }
    }

    fn tile_index(&self, id: KeyboardId) -> Option<usize> {
        self.tiles.iter().position(|t| t.info.id == id)
    }

    fn apply(&mut self, ev: Event) {
        match ev {
            Event::KeyboardAdded(info) => match self.tile_index(info.id) {
                Some(i) => {
                    self.tiles[i].info = info;
                    self.tiles[i].connected = true;
                }
                None => {
                    let factor = self.prefs.factor_for(&info).unwrap_or(FormFactor::DEFAULT);
                    self.tiles.push(Tile {
                        info,
                        connected: true,
                        factor,
                        down: HashSet::new(),
                        released_at: HashMap::new(),
                        outside: HashMap::new(),
                        native: HashMap::new(),
                        caps: false,
                        typed: String::new(),
                    });
                }
            },
            Event::KeyboardRemoved(id) => {
                if let Some(i) = self.tile_index(id) {
                    self.tiles[i].connected = false;
                    self.tiles[i].down.clear();
                }
            }
            Event::Key(k) => {
                let Some(i) = self.tile_index(k.keyboard) else { return };
                let inside = self.layouts[&self.tiles[i].factor].has(k.key);
                let enter = self.enter_mode;
                let t = &mut self.tiles[i];
                match k.state {
                    KeyState::Pressed | KeyState::Repeat => {
                        t.down.insert(k.key);
                        t.native.insert(k.key, k.native_code);
                        if !inside {
                            t.outside.insert((k.key, k.native_code), Instant::now());
                        }
                        if k.key == Key::CapsLock && k.state == KeyState::Pressed {
                            t.caps = !t.caps;
                        }
                        let (shift, caps, command) = (t.shift(), t.caps, t.command());
                        typing::type_key(&mut t.typed, k.key, shift, caps, command, enter);
                    }
                    KeyState::Released => {
                        if t.down.remove(&k.key) {
                            t.released_at.insert(k.key, Instant::now());
                        }
                        if !inside {
                            t.outside.insert((k.key, k.native_code), Instant::now());
                        }
                    }
                }
            }
            // Events were lost: a release may be missing. Reset rather than show stuck keys.
            Event::Overflow { .. } => {
                self.overflows += 1;
                for t in &mut self.tiles {
                    t.down.clear();
                }
            }
        }
    }
}

/// A key's name, or for keys manykey doesn't know, its raw scancode so it can be identified.
fn key_label(key: Key, native: u32) -> String {
    if key.is_unknown() {
        format!("?{native:#06x}")
    } else {
        key.name()
    }
}

/// "Not in this layout: A B C +4 more", capped so many unknown keys can't overflow the tile.
fn outside_text(names: &[String]) -> String {
    const SHOWN: usize = 5;
    let mut shown: Vec<&str> = names.iter().take(SHOWN).map(String::as_str).collect();
    let more = names.len().saturating_sub(SHOWN);
    let tail = if more > 0 { format!(" +{more} more") } else { String::new() };
    shown.dedup();
    format!("Not in this layout: {}{tail}", shown.join(" "))
}

fn tile_title(info: &KeyboardInfo) -> String {
    format!("{} (#{})", info.name, info.id.0)
}

fn usb_ids(info: &KeyboardInfo) -> String {
    match (info.vendor_id, info.product_id) {
        (Some(v), Some(p)) => format!("{v:04x}:{p:04x}"),
        _ => "no USB id".into(),
    }
}

/// Everything we know about a keyboard, for the header tooltip.
fn details_text(info: &KeyboardInfo) -> String {
    format!(
        "id: {}\nname: {}\nUSB vendor:product: {}\nlocation: {}\npath: {}",
        info.id.0,
        info.name,
        usb_ids(info),
        info.location.as_deref().unwrap_or("(not reported)"),
        info.path,
    )
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        while let Ok(ev) = self.rx.try_recv() {
            self.apply(ev);
        }
        let now = Instant::now();
        let mut animating = false;
        let mut changes: Vec<(usize, FormFactor)> = Vec::new();

        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("manykey viewer");
                ui.separator();
                ui.label("Size:");
                ui.add(egui::Slider::new(&mut self.unit, 12.0..=40.0));
                ui.separator();
                ui.checkbox(&mut self.show_info, "Show advanced keyboard information");
                ui.separator();
                ui.label("Enter key:");
                egui::ComboBox::from_id_salt("enter_mode").selected_text(self.enter_mode.label()).show_ui(ui, |ui| {
                    for m in EnterMode::ALL {
                        ui.selectable_value(&mut self.enter_mode, m, m.label());
                    }
                });
                ui.separator();
                // The three action buttons wrap together: if they don't fit on what's left of
                // this row, start a new row first. (Their width is measured on the previous frame.)
                if ui.available_size_before_wrap().x < self.buttons_width {
                    ui.end_row();
                }
                let buttons = ui.horizontal(|ui| {
                    if ui.button("Clear text").clicked() {
                        for t in &mut self.tiles {
                            t.typed.clear();
                        }
                    }
                    if ui
                        .button("Remove disconnected")
                        .on_hover_text(
                            "Remove tiles for keyboards that are no longer connected, along with their typed text.",
                        )
                        .clicked()
                    {
                        self.tiles.retain(|t| t.connected);
                    }
                    if ui.button("Release stuck keys").on_hover_text(STUCK_HELP).clicked() {
                        for t in &mut self.tiles {
                            release_stuck(t);
                        }
                    }
                });
                self.buttons_width = buttons.response.rect.width();
                if self.overflows > 0 {
                    ui.separator();
                    ui.label(format!("input overflowed {}×", self.overflows));
                }
            });
            ui.separator();
            if self.tiles.is_empty() {
                ui.label("Waiting for keyboards… press a key on any keyboard.");
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (i, tile) in self.tiles.iter().enumerate() {
                        let layout = &self.layouts[&tile.factor];
                        let r = draw_tile(ui, tile, layout, self.unit, self.show_info, now);
                        animating |= r.animating;
                        if let Some(ff) = r.new_factor {
                            changes.push((i, ff));
                        }
                    }
                });
            });
        });

        for (i, ff) in changes {
            let t = &mut self.tiles[i];
            t.factor = ff;
            t.outside.clear();
            self.prefs.set_factor(&t.info, ff);
        }

        // Keep polling for the next event; repaint quickly only while fading.
        ui.request_repaint_after(if animating { Duration::from_millis(16) } else { Duration::from_millis(30) });
    }
}

#[cfg(windows)]
const STUCK_HELP: &str = "Ask Windows which keys are really down and un-light any it says are up.\nOnly runs when you click; manykey itself never polls key state.";
#[cfg(not(windows))]
const STUCK_HELP: &str = "Un-light every key. A key that is really still held relights on its next event.";

/// Windows: drop held keys the OS says are up (a release we never heard about,
/// e.g. it happened on the lock screen or in an elevated window). Done here, on
/// request, rather than in the library: polling key state is not something a
/// library should do behind its host's back.
#[cfg(windows)]
fn release_stuck(t: &mut Tile) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, MapVirtualKeyW, MAPVK_VSC_TO_VK_EX};
    let stuck: Vec<Key> = t
        .down
        .iter()
        .copied()
        .filter(|k| {
            let Some(&native) = t.native.get(k) else { return false };
            let vk = unsafe { MapVirtualKeyW(native, MAPVK_VSC_TO_VK_EX) };
            vk != 0 && (unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000) == 0
        })
        .collect();
    for k in stuck {
        t.down.remove(&k);
        t.released_at.insert(k, Instant::now());
    }
}

#[cfg(not(windows))]
fn release_stuck(t: &mut Tile) {
    t.down.clear();
}

struct TileResult {
    animating: bool,
    new_factor: Option<FormFactor>,
}

fn draw_tile(ui: &mut egui::Ui, tile: &Tile, layout: &Layout, unit: f32, show_info: bool, now: Instant) -> TileResult {
    let pad = 8.0;
    let text_h = 58.0;
    let gap = 2.0;
    let keys_w = layout.width * unit;
    let tile_w = (keys_w + pad * 2.0).max(MIN_TILE_WIDTH);
    let vis = ui.visuals().clone();

    // Advanced info wraps to the tile width (breaking inside long device paths),
    // and the tile grows to fit it.
    let info_galley = show_info.then(|| {
        let text = format!(
            "location: {}\npath: {}",
            tile.info.location.as_deref().unwrap_or("(not reported)"),
            tile.info.path
        );
        let mut job =
            egui::text::LayoutJob::simple(text, FontId::monospace(10.0), vis.weak_text_color(), tile_w - pad * 2.0);
        job.wrap.break_anywhere = true;
        ui.painter().layout_job(job)
    });
    let info_h = info_galley.as_ref().map_or(0.0, |g| g.size().y + 4.0);
    let size = Vec2::new(tile_w, layout.height * unit + pad * 2.0 + 44.0 + info_h + text_h + 6.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let alpha = if tile.connected { 255 } else { 110 };
    let outside = tile.outside_now(now);
    let mut animating = !outside.is_empty();

    let border = if outside.is_empty() {
        Stroke::new(1.0, vis.widgets.noninteractive.bg_stroke.color)
    } else {
        Stroke::new(2.0, WARN)
    };
    painter.rect(rect, 6.0, vis.faint_bg_color, border, StrokeKind::Inside);

    // Header: title (left), form-factor dropdown (right).
    let combo_w = 210.0;
    let combo_rect =
        Rect::from_min_size(egui::pos2(rect.max.x - pad - combo_w, rect.min.y + pad - 3.0), Vec2::new(combo_w, 22.0));
    let title = tile_title(&tile.info);
    let title_color = if tile.connected { vis.strong_text_color() } else { vis.weak_text_color() };
    let title_clip = Rect::from_min_max(rect.min, egui::pos2(combo_rect.min.x - 6.0, rect.min.y + pad + 20.0));
    painter.with_clip_rect(title_clip).text(
        rect.min + Vec2::new(pad, pad),
        Align2::LEFT_TOP,
        title,
        FontId::proportional(14.0),
        title_color,
    );

    // Second line: USB ids, then a warning if a key outside the layout was used.
    let mut sub_x = 0.0;
    if !tile.connected {
        let g = painter.layout_no_wrap("disconnected".to_string(), FontId::proportional(11.0), DISCONNECTED);
        sub_x = g.size().x + 10.0;
        painter.galley(rect.min + Vec2::new(pad, pad + 18.0), g, DISCONNECTED);
    }
    let ids_galley = painter.layout_no_wrap(usb_ids(&tile.info), FontId::proportional(11.0), vis.weak_text_color());
    let ids_w = ids_galley.size().x + sub_x;
    painter.galley(rect.min + Vec2::new(pad + sub_x, pad + 18.0), ids_galley, vis.weak_text_color());
    if !outside.is_empty() {
        painter.text(
            rect.min + Vec2::new(pad + ids_w + 14.0, pad + 18.0),
            Align2::LEFT_TOP,
            outside_text(&outside),
            FontId::proportional(11.0),
            WARN,
        );
    }

    let mut top = pad + 44.0;
    if let Some(galley) = info_galley {
        painter.galley(rect.min + Vec2::new(pad, top - 4.0), galley, vis.weak_text_color());
        top += info_h;
    }
    // Hovering the title shows the full record without cluttering the tile.
    let header = Rect::from_min_max(rect.min, egui::pos2(combo_rect.min.x - 6.0, rect.min.y + pad + 36.0));
    ui.interact(header, response.id.with("header"), Sense::hover()).on_hover_text(details_text(&tile.info));

    let mut new_factor = None;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(combo_rect));
    egui::ComboBox::from_id_salt(("form_factor", tile.info.id.0))
        .selected_text(tile.factor.label())
        .width(combo_w - 8.0)
        .show_ui(&mut child, |ui| {
            for ff in FormFactor::ALL {
                if ui.selectable_label(tile.factor == ff, ff.label()).on_hover_text(ff.description()).clicked() {
                    new_factor = Some(ff);
                }
            }
        });

    // Keys, centred horizontally in the tile.
    let origin = rect.min + Vec2::new((tile_w - keys_w) / 2.0, top);
    let accent = Color32::from_rgb(70, 160, 255);
    let idle = vis.widgets.inactive.bg_fill;
    let edge = Stroke::new(1.0, vis.widgets.inactive.bg_stroke.color);
    let shift = tile.shift();

    for k in &layout.keys {
        let r = Rect::from_min_size(
            origin + Vec2::new(k.x * unit, k.y * unit),
            Vec2::new(k.w * unit - gap, k.h * unit - gap),
        );
        let (fill, text_col) = if tile.down.contains(&k.key) {
            (accent, Color32::WHITE)
        } else if let Some(t) = tile.released_at.get(&k.key).filter(|t| now.duration_since(**t) < FADE) {
            animating = true;
            let f = 1.0 - now.duration_since(*t).as_secs_f32() / FADE.as_secs_f32();
            (lerp(idle, accent, f), vis.text_color())
        } else {
            (idle, vis.text_color())
        };
        let fill = fill.gamma_multiply(alpha as f32 / 255.0);
        match k.shape {
            Shape::Round => {
                painter.circle(r.center(), r.width().min(r.height()) / 2.0, fill, edge);
            }
            _ => {
                painter.rect(r, 3.0, fill, edge, StrokeKind::Inside);
            }
        }
        if let Shape::Arrow(dir) = k.shape {
            draw_arrow(&painter, r.center(), unit * 0.22, dir, text_col);
            continue;
        }
        let label =
            typing::char_for(k.key, shift, tile.caps).map(|c| c.to_string()).unwrap_or_else(|| k.label.to_string());
        let font =
            if k.shape == Shape::Round { (unit * 0.34).clamp(7.0, 11.0) } else { (unit * 0.42).clamp(8.0, 14.0) };
        painter.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(font), text_col);
    }

    // Typed text, bottom of the tile. Newest text stays visible.
    let box_rect = Rect::from_min_size(
        egui::pos2(rect.min.x + pad, rect.max.y - pad - text_h),
        Vec2::new(rect.width() - pad * 2.0, text_h),
    );
    painter.rect(
        box_rect,
        3.0,
        vis.extreme_bg_color,
        Stroke::new(1.0, vis.widgets.noninteractive.bg_stroke.color),
        StrokeKind::Inside,
    );
    let text_painter = painter.with_clip_rect(box_rect.shrink(3.0));
    let galley =
        text_painter.layout(tile.typed.clone(), FontId::monospace(13.0), vis.text_color(), box_rect.width() - 10.0);
    // Anchor the bottom of the text to the bottom of the box so it scrolls as you type.
    let y = (box_rect.max.y - 4.0 - galley.size().y).max(box_rect.min.y + 4.0);
    text_painter.galley(egui::pos2(box_rect.min.x + 5.0, y), galley, vis.text_color());

    TileResult { animating, new_factor }
}

/// A filled triangle, drawn as a shape because egui's fonts lack arrow glyphs.
fn draw_arrow(painter: &egui::Painter, c: egui::Pos2, s: f32, dir: Dir, color: Color32) {
    let p = |dx: f32, dy: f32| c + Vec2::new(dx, dy);
    let pts = match dir {
        Dir::Up => vec![p(0.0, -s), p(-s, s * 0.8), p(s, s * 0.8)],
        Dir::Down => vec![p(0.0, s), p(-s, -s * 0.8), p(s, -s * 0.8)],
        Dir::Left => vec![p(-s, 0.0), p(s * 0.8, -s), p(s * 0.8, s)],
        Dir::Right => vec![p(s, 0.0), p(-s * 0.8, -s), p(-s * 0.8, s)],
    };
    painter.add(Paint::convex_polygon(pts, color, Stroke::NONE));
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

fn main() -> eframe::Result {
    // `--focused`: only react while this window has focus (the library drops input otherwise).
    // `--log`: also print every event to stdout.
    // `--no-release-on-unplug`, `--no-release-on-focus-loss`: turn off manykey's synthesized releases.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let log = args.iter().any(|a| a == "--log");
    let options = Options {
        scope: if args.iter().any(|a| a == "--focused") { Scope::OwnProcessFocused } else { Scope::Global },
        // The window library (winit) registers for raw keyboard input itself; take it back.
        reclaim_registration: true,
        // Show injected input too, so a macro tool's output is visible here.
        include_injected: true,
        release_on_unplug: !args.iter().any(|a| a == "--no-release-on-unplug"),
        release_on_focus_loss: !args.iter().any(|a| a == "--no-release-on-focus-loss"),
        ..Options::default()
    };
    let listener = match Listener::with_options(options) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("manykey-view: {e}");
            std::process::exit(1);
        }
    };
    let (tx, rx) = mpsc::channel();
    // Forward events to the UI thread. The UI polls on a timer, so no repaint hook is needed.
    std::thread::spawn(move || {
        for ev in listener.iter() {
            if log {
                println!("{ev:?}");
            }
            if tx.send(ev).is_err() {
                break;
            }
        }
    });
    eframe::run_native(
        "manykey viewer",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Glow,
            viewport: egui::ViewportBuilder::default().with_inner_size([1250.0, 520.0]),
            ..Default::default()
        },
        Box::new(|_cc| Ok(Box::new(App::new(rx)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_keys_are_labelled_with_their_scancode() {
        assert_eq!(key_label(Key::UNKNOWN, 0xE05B), "?0xe05b");
        assert_eq!(key_label(Key::UNKNOWN, 0x0), "?0x0000");
        assert_eq!(key_label(Key::A, 0x1E), "A");
    }
}
