use gtk::cairo::{Context, FontSlant, FontWeight, LinearGradient, RadialGradient};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::f64::consts::{PI, TAU};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

const STYLE: &str = include_str!("style.css");

// ---------------------------------------------------------------------------
// Themes
// ---------------------------------------------------------------------------

type Rgb = (u8, u8, u8);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Retro,
    Terminator,
    Joker,
}

struct Palette {
    grad: [Rgb; 3],
    hover: [Rgb; 3],
    acc: Rgb,
    line: Rgb,
    bg: Rgb,
    dlg: [Rgb; 3],
    glow: Rgb,
    sub: Rgb,
    txt: Rgb,
    card: Rgb,
    font: &'static str,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Retro, Theme::Terminator, Theme::Joker];

    pub fn id(self) -> &'static str {
        match self {
            Theme::Retro => "retro",
            Theme::Terminator => "terminator",
            Theme::Joker => "joker",
        }
    }

    pub fn from_id(id: &str) -> Theme {
        match id.trim() {
            "terminator" => Theme::Terminator,
            "joker" => Theme::Joker,
            _ => Theme::Retro,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Theme::Retro => "Retrowave",
            Theme::Terminator => "Protocolo",
            Theme::Joker => "Coringa",
        }
    }

    pub fn badge(self) -> &'static str {
        match self {
            Theme::Retro => "LOOK4 VIRUS",
            Theme::Terminator => "VIRUS TERMINATION PROCOT0L",
            Theme::Joker => "Search 4 Virus",
        }
    }

    fn palette(self) -> Palette {
        match self {
            Theme::Retro => Palette {
                grad: [(0xc6, 0x4d, 0xff), (0x7c, 0x5c, 0xff), (0x3e, 0xb2, 0xff)],
                hover: [(0xdb, 0x6d, 0xff), (0x91, 0x74, 0xff), (0x5c, 0xc4, 0xff)],
                acc: (0x7f, 0xe6, 0xff),
                line: (170, 140, 255),
                bg: (0x0b, 0x09, 0x18),
                dlg: [(0x18, 0x11, 0x2e), (0x0e, 0x0c, 0x20), (0x0a, 0x13, 0x24)],
                glow: (124, 92, 255),
                sub: (0xa9, 0x9c, 0xc9),
                txt: (0xf1, 0xec, 0xff),
                card: (16, 12, 34),
                font: "\"Inter\", \"Cantarell\", \"Ubuntu\", sans-serif",
            },
            Theme::Terminator => Palette {
                grad: [(0xff, 0x3b, 0x30), (0xc4, 0x0d, 0x14), (0xff, 0x9a, 0x8c)],
                hover: [(0xff, 0x5c, 0x50), (0xdc, 0x20, 0x28), (0xff, 0xb4, 0xa8)],
                acc: (0xff, 0xe3, 0xe0),
                line: (255, 90, 80),
                bg: (0x09, 0x04, 0x05),
                dlg: [(0x1d, 0x0a, 0x0c), (0x10, 0x06, 0x08), (0x09, 0x04, 0x05)],
                glow: (255, 40, 40),
                sub: (0xc0, 0xa0, 0xa0),
                txt: (0xff, 0xf1, 0xf0),
                card: (22, 8, 10),
                font: "\"JetBrains Mono\", \"DejaVu Sans Mono\", \"Ubuntu Mono\", monospace",
            },
            Theme::Joker => Palette {
                grad: [(0xa4, 0x3c, 0xff), (0x6a, 0x3f, 0xd6), (0x39, 0xe0, 0x7a)],
                hover: [(0xb8, 0x62, 0xff), (0x82, 0x5f, 0xee), (0x5c, 0xf0, 0x95)],
                acc: (0x7d, 0xff, 0x9b),
                line: (150, 110, 230),
                bg: (0x08, 0x06, 0x0f),
                dlg: [(0x1a, 0x0f, 0x2e), (0x0d, 0x0a, 0x18), (0x07, 0x14, 0x0d)],
                glow: (130, 70, 230),
                sub: (0xa9, 0x9d, 0xc6),
                txt: (0xf3, 0xee, 0xff),
                card: (18, 12, 32),
                font: "\"Inter\", \"Cantarell\", \"Ubuntu\", sans-serif",
            },
        }
    }
}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

fn rgb_list(c: Rgb) -> String {
    format!("{}, {}, {}", c.0, c.1, c.2)
}

fn unit(c: Rgb) -> (f64, f64, f64) {
    (c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0)
}

thread_local! {
    static CURRENT: Cell<Theme> = const { Cell::new(Theme::Retro) };
    static PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    static LISTENERS: RefCell<Vec<Box<dyn Fn(Theme)>>> = RefCell::new(Vec::new());
}

pub fn current() -> Theme {
    CURRENT.with(|current| current.get())
}

fn config_path() -> PathBuf {
    glib::user_config_dir().join("look4").join("theme")
}

fn load_saved_theme() -> Theme {
    std::fs::read_to_string(config_path())
        .map(|text| Theme::from_id(&text))
        .unwrap_or(Theme::Retro)
}

fn save_theme(theme: Theme) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, theme.id());
}

fn build_css(theme: Theme) -> String {
    let p = theme.palette();
    let replacements: Vec<(&str, String)> = vec![
        ("%G1%", hex(p.grad[0])),
        ("%G2%", hex(p.grad[1])),
        ("%G3%", hex(p.grad[2])),
        ("%H1%", hex(p.hover[0])),
        ("%H2%", hex(p.hover[1])),
        ("%H3%", hex(p.hover[2])),
        ("%ACC_RGB%", rgb_list(p.acc)),
        ("%ACC%", hex(p.acc)),
        ("%LINE_RGB%", rgb_list(p.line)),
        ("%GLOW_RGB%", rgb_list(p.glow)),
        ("%CARD_RGB%", rgb_list(p.card)),
        ("%SUB_RGB%", rgb_list(p.sub)),
        ("%SUB%", hex(p.sub)),
        ("%TXT%", hex(p.txt)),
        ("%BG%", hex(p.bg)),
        ("%D1%", hex(p.dlg[0])),
        ("%D2%", hex(p.dlg[1])),
        ("%D3%", hex(p.dlg[2])),
        ("%FONT%", p.font.to_string()),
    ];
    let mut css = STYLE.to_string();
    for (token, value) in replacements {
        css = css.replace(token, &value);
    }
    css
}

pub fn install_style(display: &gtk::gdk::Display) {
    let theme = load_saved_theme();
    CURRENT.with(|current| current.set(theme));
    let provider = gtk::CssProvider::new();
    provider.load_from_data(&build_css(theme));
    // USER priority so the look4 rules win over a user-installed GTK theme (e.g. Dracula).
    gtk::style_context_add_provider_for_display(
        display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
    PROVIDER.with(|slot| *slot.borrow_mut() = Some(provider));
}

pub fn set_theme(theme: Theme) {
    CURRENT.with(|current| current.set(theme));
    save_theme(theme);
    PROVIDER.with(|slot| {
        if let Some(provider) = slot.borrow().as_ref() {
            provider.load_from_data(&build_css(theme));
        }
    });
    LISTENERS.with(|listeners| {
        for listener in listeners.borrow().iter() {
            listener(theme);
        }
    });
}

pub fn on_theme_changed(listener: impl Fn(Theme) + 'static) {
    LISTENERS.with(|listeners| listeners.borrow_mut().push(Box::new(listener)));
}

// ---------------------------------------------------------------------------
// Backgrounds
// ---------------------------------------------------------------------------

pub fn background() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    let started = Instant::now();
    area.set_draw_func(move |_, context, width, height| {
        let (w, h) = (width as f64, height as f64);
        let time = started.elapsed().as_secs_f64();
        match current() {
            Theme::Retro => draw_retro_background(context, w, h),
            Theme::Terminator => draw_terminator_background(context, w, h, time),
            Theme::Joker => draw_joker_background(context, w, h, time),
        }
    });
    let last_theme = Cell::new(None::<Theme>);
    let last_draw = Cell::new(0_i64);
    area.add_tick_callback(move |widget, clock| {
        let theme = current();
        let now = clock.frame_time();
        let changed = last_theme.get() != Some(theme);
        // Animated themes are throttled to ~30 fps to keep CPU usage low.
        if changed || (theme != Theme::Retro && now - last_draw.get() > 33_000) {
            last_theme.set(Some(theme));
            last_draw.set(now);
            widget.queue_draw();
        }
        glib::ControlFlow::Continue
    });
    area
}

fn draw_retro_background(context: &Context, width: f64, height: f64) {
    let gradient = LinearGradient::new(0.0, 0.0, width, height);
    gradient.add_color_stop_rgb(0.0, 0.035, 0.028, 0.095);
    gradient.add_color_stop_rgb(0.48, 0.075, 0.035, 0.16);
    gradient.add_color_stop_rgb(1.0, 0.018, 0.045, 0.11);
    let _ = context.set_source(&gradient);
    let _ = context.paint();

    let glow = RadialGradient::new(
        width * 0.53,
        height * 0.47,
        8.0,
        width * 0.53,
        height * 0.47,
        width.min(height) * 0.72,
    );
    glow.add_color_stop_rgba(0.0, 0.46, 0.13, 0.68, 0.17);
    glow.add_color_stop_rgba(0.52, 0.18, 0.2, 0.62, 0.08);
    glow.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    let _ = context.set_source(&glow);
    let _ = context.paint();

    let horizon = height * 0.73;
    context.set_line_width(1.0);
    context.set_source_rgba(0.36, 0.59, 1.0, 0.13);
    for line in 0..13 {
        let progress = line as f64 / 12.0;
        let y = horizon + (height - horizon) * progress.powf(1.65);
        context.move_to(0.0, y);
        context.line_to(width, y);
    }
    for line in -8..=8 {
        let x = width * 0.5 + line as f64 * width * 0.105;
        context.move_to(width * 0.5, horizon);
        context.line_to(x, height);
    }
    let _ = context.stroke();

    for (x, y, radius) in [
        (0.12, 0.18, 1.2),
        (0.24, 0.31, 0.8),
        (0.82, 0.2, 1.1),
        (0.91, 0.38, 0.8),
        (0.7, 0.11, 0.7),
        (0.08, 0.45, 0.7),
    ] {
        context.arc(width * x, height * y, radius, 0.0, TAU);
        context.set_source_rgba(0.76, 0.71, 1.0, 0.75);
        let _ = context.fill();
    }
}

/// Skynet-style HUD: red grid, rotating reticle, radar sweep, scanlines and terminal text.
fn draw_terminator_background(cx: &Context, w: f64, h: f64, t: f64) {
    let base = LinearGradient::new(0.0, 0.0, 0.0, h);
    base.add_color_stop_rgb(0.0, 0.03, 0.008, 0.012);
    base.add_color_stop_rgb(1.0, 0.078, 0.01, 0.016);
    let _ = cx.set_source(&base);
    let _ = cx.paint();

    let (rx, ry) = (w * 0.5, h * 0.46);
    let glow = RadialGradient::new(rx, ry, 6.0, rx, ry, w.min(h) * 0.78);
    glow.add_color_stop_rgba(0.0, 0.9, 0.05, 0.05, 0.22);
    glow.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    let _ = cx.set_source(&glow);
    let _ = cx.paint();

    // tactical grid
    cx.set_line_width(1.0);
    cx.set_source_rgba(1.0, 0.12, 0.12, 0.05);
    let step = 44.0;
    let mut x = 0.5;
    while x < w {
        cx.move_to(x, 0.0);
        cx.line_to(x, h);
        x += step;
    }
    let mut y = 0.5;
    while y < h {
        cx.move_to(0.0, y);
        cx.line_to(w, y);
        y += step;
    }
    let _ = cx.stroke();

    // reticle
    let rr = w.min(h) * 0.34;
    cx.set_line_width(1.5);
    cx.set_source_rgba(1.0, 0.2, 0.2, 0.2);
    cx.arc(rx, ry, rr, 0.0, TAU);
    let _ = cx.stroke();
    cx.arc(rx, ry, rr * 0.66, 0.0, TAU);
    let _ = cx.stroke();
    cx.set_line_width(2.0);
    cx.set_dash(&[3.0, 9.0], -t * 16.0);
    cx.set_source_rgba(1.0, 0.3, 0.25, 0.32);
    cx.arc(rx, ry, rr * 1.12, 0.0, TAU);
    let _ = cx.stroke();
    cx.set_dash(&[], 0.0);
    cx.set_line_width(1.5);
    cx.set_source_rgba(1.0, 0.3, 0.25, 0.35);
    for k in 0..4 {
        let a = k as f64 * PI / 2.0;
        cx.move_to(rx + a.cos() * rr * 0.9, ry + a.sin() * rr * 0.9);
        cx.line_to(rx + a.cos() * rr * 1.28, ry + a.sin() * rr * 1.28);
    }
    let _ = cx.stroke();

    // radar sweep
    let angle = t * 0.9;
    cx.move_to(rx, ry);
    cx.arc(rx, ry, rr, angle - 0.55, angle);
    cx.close_path();
    cx.set_source_rgba(1.0, 0.1, 0.1, 0.09);
    let _ = cx.fill();
    cx.move_to(rx, ry);
    cx.line_to(rx + angle.cos() * rr, ry + angle.sin() * rr);
    cx.set_line_width(1.5);
    cx.set_source_rgba(1.0, 0.25, 0.2, 0.45);
    let _ = cx.stroke();

    // moving scan band
    let band_y = ((t * 0.09).rem_euclid(1.25) - 0.12) * h;
    let band = LinearGradient::new(0.0, band_y - 55.0, 0.0, band_y + 55.0);
    band.add_color_stop_rgba(0.0, 1.0, 0.1, 0.1, 0.0);
    band.add_color_stop_rgba(0.5, 1.0, 0.1, 0.1, 0.10);
    band.add_color_stop_rgba(1.0, 1.0, 0.1, 0.1, 0.0);
    let _ = cx.set_source(&band);
    cx.rectangle(0.0, band_y - 55.0, w, 110.0);
    let _ = cx.fill();

    // scanlines
    cx.set_source_rgba(0.0, 0.0, 0.0, 0.2);
    let mut y = 0.0;
    while y < h {
        cx.rectangle(0.0, y, w, 1.0);
        y += 3.0;
    }
    let _ = cx.fill();

    // corner brackets
    cx.set_line_width(2.0);
    cx.set_source_rgba(1.0, 0.25, 0.2, 0.6);
    let (m, l) = (16.0, 26.0);
    for (cx0, cy0, dx, dy) in [(m, m, 1.0, 1.0), (w - m, m, -1.0, 1.0), (m, h - m, 1.0, -1.0), (w - m, h - m, -1.0, -1.0)] {
        cx.move_to(cx0 + dx * l, cy0);
        cx.line_to(cx0, cy0);
        cx.line_to(cx0, cy0 + dy * l);
    }
    let _ = cx.stroke();

    // terminal readout
    cx.select_font_face("monospace", FontSlant::Normal, FontWeight::Normal);
    cx.set_font_size(10.5);
    cx.set_source_rgba(1.0, 0.35, 0.3, 0.6);
    let cursor = if (t * 1.6) as i64 % 2 == 0 { "_" } else { " " };
    cx.move_to(30.0, h - 24.0);
    let _ = cx.show_text(&format!("SYS://LOOK4  PROTOCOLO ATIVO{cursor}"));
    let label = "THREAT.SCAN v2.9";
    if let Ok(extents) = cx.text_extents(label) {
        cx.move_to(w - 30.0 - extents.width(), h - 24.0);
        let _ = cx.show_text(label);
    }

    // vignette
    let vignette = RadialGradient::new(rx, h * 0.5, w.min(h) * 0.45, rx, h * 0.5, w.max(h) * 0.75);
    vignette.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
    vignette.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.55);
    let _ = cx.set_source(&vignette);
    let _ = cx.paint();
}

fn hash(value: f64) -> f64 {
    let x = (value * 12.9898).sin() * 43758.5453;
    x - x.floor()
}

/// Slow rising smoke built from soft purple and green radial puffs.
fn draw_joker_background(cx: &Context, w: f64, h: f64, t: f64) {
    let base = LinearGradient::new(0.0, 0.0, w, h);
    base.add_color_stop_rgb(0.0, 0.035, 0.02, 0.07);
    base.add_color_stop_rgb(0.55, 0.045, 0.02, 0.085);
    base.add_color_stop_rgb(1.0, 0.015, 0.05, 0.04);
    let _ = cx.set_source(&base);
    let _ = cx.paint();

    let size = w.min(h);
    for i in 0..16 {
        let seed = i as f64;
        let speed = 0.022 + hash(seed + 3.0) * 0.026;
        let u = (t * speed + hash(seed) ).rem_euclid(1.0);
        let base_x = 0.12 + hash(seed + 7.0) * 0.76;
        let x = w * (base_x + 0.10 * (t * 0.35 + seed * 1.3 + u * 4.0).sin());
        let y = h * (1.2 - 1.45 * u);
        let radius = size * (0.26 + 0.34 * u) * (0.9 + 0.2 * hash(seed + 11.0));
        let fade = (PI * u).sin().powf(0.8);
        let alpha = 0.17 * fade;
        let (r, g, b) = if i % 2 == 0 { (0.55, 0.2, 0.95) } else { (0.2, 0.9, 0.5) };
        let puff = RadialGradient::new(x, y, 0.0, x, y, radius);
        puff.add_color_stop_rgba(0.0, r, g, b, alpha);
        puff.add_color_stop_rgba(0.55, r, g, b, alpha * 0.45);
        puff.add_color_stop_rgba(1.0, r, g, b, 0.0);
        let _ = cx.set_source(&puff);
        cx.arc(x, y, radius, 0.0, TAU);
        let _ = cx.fill();
    }

    let vignette = RadialGradient::new(w * 0.5, h * 0.5, size * 0.4, w * 0.5, h * 0.5, w.max(h) * 0.78);
    vignette.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
    vignette.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.5);
    let _ = cx.set_source(&vignette);
    let _ = cx.paint();
}

// ---------------------------------------------------------------------------
// Animated borders
// ---------------------------------------------------------------------------

fn border_stops(theme: Theme) -> &'static [(f64, f64, f64, f64)] {
    match theme {
        Theme::Retro => &[
            (0.0, 0.0, 1.0, 0.8),
            (0.2, 0.26, 0.53, 1.0),
            (0.4, 0.66, 0.23, 1.0),
            (0.6, 1.0, 0.18, 0.47),
            (0.8, 1.0, 0.81, 0.29),
            (1.0, 0.0, 1.0, 0.8),
        ],
        Theme::Terminator => &[
            (0.0, 1.0, 0.1, 0.1),
            (0.25, 1.0, 0.42, 0.35),
            (0.5, 1.0, 1.0, 1.0),
            (0.75, 1.0, 0.42, 0.35),
            (1.0, 1.0, 0.1, 0.1),
        ],
        Theme::Joker => &[
            (0.0, 0.64, 0.24, 1.0),
            (0.5, 0.22, 1.0, 0.53),
            (1.0, 0.64, 0.24, 1.0),
        ],
    }
}

pub fn rgb_bar() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_height(4);
    area.set_hexpand(true);
    area.add_css_class("look4-rgb-bar");
    let started = Instant::now();
    area.set_draw_func(move |_, context, width, height| {
        let period = width.max(1) as f64;
        let phase = (started.elapsed().as_secs_f64() / 5.0).rem_euclid(1.0) * period;
        let gradient = rainbow_gradient(period, phase);
        context.rectangle(0.0, 0.0, width as f64, height as f64);
        let _ = context.set_source(&gradient);
        let _ = context.fill();
    });
    area.add_tick_callback(|widget, _| {
        widget.queue_draw();
        glib::ControlFlow::Continue
    });
    area
}

pub fn rgb_outline() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_height(58);
    area.set_hexpand(true);
    let started = Instant::now();
    area.set_draw_func(move |_, context, width, height| {
        draw_rgb_outline(
            context,
            width as f64,
            height as f64,
            started.elapsed().as_secs_f64(),
        );
    });
    area.add_tick_callback(|widget, _| {
        widget.queue_draw();
        glib::ControlFlow::Continue
    });
    area
}

fn draw_rgb_outline(context: &Context, width: f64, height: f64, time: f64) {
    let period = width.max(1.0);
    let phase = (time / 5.0).rem_euclid(1.0) * period;
    let gradient = rainbow_gradient(period, phase);
    let (gr, gg, gb) = unit(current().palette().glow);
    rounded_rectangle(context, 3.0, 3.0, width - 6.0, height - 6.0, 16.0);
    context.set_line_width(7.0);
    context.set_source_rgba(gr, gg, gb, 0.18);
    let _ = context.stroke_preserve();
    context.set_line_width(2.0);
    let _ = context.set_source(&gradient);
    let _ = context.stroke();
}

fn rounded_rectangle(context: &Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let right = x + width;
    let bottom = y + height;
    context.new_path();
    context.move_to(x + radius, y);
    context.line_to(right - radius, y);
    context.arc(right - radius, y + radius, radius, -PI / 2.0, 0.0);
    context.line_to(right, bottom - radius);
    context.arc(right - radius, bottom - radius, radius, 0.0, PI / 2.0);
    context.line_to(x + radius, bottom);
    context.arc(x + radius, bottom - radius, radius, PI / 2.0, PI);
    context.line_to(x, y + radius);
    context.arc(x + radius, y + radius, radius, PI, 3.0 * PI / 2.0);
    context.close_path();
}

fn rainbow_gradient(period: f64, phase: f64) -> LinearGradient {
    let gradient = LinearGradient::new(0.0, 0.0, period, 0.0);
    for &(offset, r, g, b) in border_stops(current()) {
        gradient.add_color_stop_rgb(offset, r, g, b);
    }
    gradient.set_extend(gtk::cairo::Extend::Repeat);
    gradient.set_matrix(gtk::cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, phase, 0.0));
    gradient
}

// ---------------------------------------------------------------------------
// Scan animation
// ---------------------------------------------------------------------------

pub fn animated_orb() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(210);
    area.set_content_height(210);
    area.set_halign(gtk::Align::Center);
    area.set_valign(gtk::Align::Center);
    area.add_css_class("look4-orb");
    let started = Instant::now();
    area.set_draw_func(move |_, context, width, height| {
        draw_scanner(
            context,
            width as f64,
            height as f64,
            started.elapsed().as_secs_f64(),
        );
    });
    area.add_tick_callback(|widget, _| {
        widget.queue_draw();
        glib::ControlFlow::Continue
    });
    area
}

fn mix(a: (f64, f64, f64), b: (f64, f64, f64), t: f64) -> (f64, f64, f64) {
    (
        a.0 + (b.0 - a.0) * t,
        a.1 + (b.1 - a.1) * t,
        a.2 + (b.2 - a.2) * t,
    )
}

/// Radar-like loader: pulsing core with a lens, expanding ripples and two counter-rotating comet arcs.
fn draw_scanner(cx: &Context, width: f64, height: f64, time: f64) {
    let palette = current().palette();
    let a = unit(palette.grad[0]);
    let b = unit(palette.grad[2]);
    let acc = unit(palette.acc);
    let (mx, my) = (width * 0.5, height * 0.5);
    let radius = width.min(height) * 0.5 - 4.0;
    let core = radius * 0.38 * (1.0 + 0.05 * (time * 3.2).sin());

    // soft aura (clipped to a circle so no rectangular edges are visible)
    let aura = RadialGradient::new(mx, my, core * 0.6, mx, my, radius);
    aura.add_color_stop_rgba(0.0, a.0, a.1, a.2, 0.30);
    aura.add_color_stop_rgba(1.0, b.0, b.1, b.2, 0.0);
    let _ = cx.set_source(&aura);
    cx.arc(mx, my, radius, 0.0, TAU);
    let _ = cx.fill();

    // ripples
    cx.set_line_width(1.6);
    for k in 0..3 {
        let phase = (time * 0.5 + k as f64 / 3.0).rem_euclid(1.0);
        let ring = core * 1.05 + (radius * 0.9 - core * 1.05) * phase;
        cx.set_source_rgba(acc.0, acc.1, acc.2, (1.0 - phase) * 0.5);
        cx.arc(mx, my, ring, 0.0, TAU);
        let _ = cx.stroke();
    }

    // comet arcs
    comet(cx, mx, my, radius * 0.72, time * 2.6, 1.9, a, b, 5.0);
    comet(cx, mx, my, radius * 0.88, -time * 1.7 + 1.0, 1.4, b, a, 3.5);

    // core
    let sphere = RadialGradient::new(mx - core * 0.35, my - core * 0.4, core * 0.05, mx, my, core);
    let hi = mix(a, (1.0, 1.0, 1.0), 0.55);
    sphere.add_color_stop_rgb(0.0, hi.0, hi.1, hi.2);
    sphere.add_color_stop_rgb(0.55, a.0, a.1, a.2);
    sphere.add_color_stop_rgb(1.0, b.0 * 0.6, b.1 * 0.6, b.2 * 0.6);
    cx.arc(mx, my, core, 0.0, TAU);
    let _ = cx.set_source(&sphere);
    let _ = cx.fill();

    // lens icon
    let bob = (time * 2.0).sin() * core * 0.04;
    cx.set_line_cap(gtk::cairo::LineCap::Round);
    cx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    cx.set_line_width(core * 0.11);
    cx.arc(mx - core * 0.08, my - core * 0.08 + bob, core * 0.34, 0.0, TAU);
    let _ = cx.stroke();
    cx.move_to(mx + core * 0.17, my + core * 0.17 + bob);
    cx.line_to(mx + core * 0.46, my + core * 0.46 + bob);
    let _ = cx.stroke();
    cx.set_line_cap(gtk::cairo::LineCap::Butt);
}

fn comet(
    cx: &Context,
    mx: f64,
    my: f64,
    radius: f64,
    angle: f64,
    sweep: f64,
    from: (f64, f64, f64),
    to: (f64, f64, f64),
    thickness: f64,
) {
    const SEGMENTS: usize = 28;
    cx.set_line_cap(gtk::cairo::LineCap::Round);
    for s in 0..SEGMENTS {
        let f0 = s as f64 / SEGMENTS as f64;
        let f1 = (s + 1) as f64 / SEGMENTS as f64;
        let color = mix(from, to, f0);
        cx.set_source_rgba(color.0, color.1, color.2, f0.powf(1.6));
        cx.set_line_width(thickness * (0.35 + 0.65 * f0));
        cx.arc(mx, my, radius, angle + sweep * f0, angle + sweep * f1 + 0.02);
        let _ = cx.stroke();
    }
    let head = angle + sweep;
    let (hx, hy) = (mx + head.cos() * radius, my + head.sin() * radius);
    let glow = RadialGradient::new(hx, hy, 0.0, hx, hy, thickness * 2.6);
    glow.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.9);
    glow.add_color_stop_rgba(1.0, to.0, to.1, to.2, 0.0);
    let _ = cx.set_source(&glow);
    cx.arc(hx, hy, thickness * 2.6, 0.0, TAU);
    let _ = cx.fill();
    cx.set_line_cap(gtk::cairo::LineCap::Butt);
}

// ---------------------------------------------------------------------------
// Result charts
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
pub struct ChartData {
    pub malicious: u32,
    pub suspicious: u32,
    pub harmless: u32,
    pub undetected: u32,
    pub timeout: u32,
    /// 0 = safe, 1 = suspicious, 2 = malicious
    pub kind: u8,
    pub started: Option<Instant>,
}

pub type SharedChart = Rc<Cell<ChartData>>;

const MALICIOUS: (f64, f64, f64) = (1.0, 0.36, 0.55);
const SUSPICIOUS: (f64, f64, f64) = (1.0, 0.78, 0.3);
const CLEAN: (f64, f64, f64) = (0.25, 0.95, 0.81);
const MUTED: (f64, f64, f64) = (0.55, 0.53, 0.66);

fn verdict_color(kind: u8) -> (f64, f64, f64) {
    match kind {
        2 => MALICIOUS,
        1 => SUSPICIOUS,
        _ => CLEAN,
    }
}

fn chart_progress(data: ChartData) -> f64 {
    match data.started {
        Some(started) => {
            let p = (started.elapsed().as_secs_f64() / 1.0).min(1.0);
            1.0 - (1.0 - p).powi(3)
        }
        None => 0.0,
    }
}

/// Returns the donut gauge, the stacked distribution bar and the shared data driving both.
pub fn result_charts() -> (gtk::DrawingArea, gtk::DrawingArea, SharedChart) {
    let data: SharedChart = Rc::new(Cell::new(ChartData::default()));

    let gauge = gtk::DrawingArea::new();
    gauge.set_content_width(140);
    gauge.set_content_height(140);
    gauge.set_halign(gtk::Align::Center);
    let shared = data.clone();
    gauge.set_draw_func(move |_, cx, w, h| draw_gauge(cx, w as f64, h as f64, shared.get()));

    let bar = gtk::DrawingArea::new();
    bar.set_content_height(10);
    bar.set_hexpand(true);
    let shared = data.clone();
    bar.set_draw_func(move |_, cx, w, h| draw_bar(cx, w as f64, h as f64, shared.get()));

    for area in [&gauge, &bar] {
        let shared = data.clone();
        area.add_tick_callback(move |widget, _| {
            if let Some(started) = shared.get().started {
                if started.elapsed().as_secs_f64() < 1.4 {
                    widget.queue_draw();
                }
            }
            glib::ControlFlow::Continue
        });
    }
    (gauge, bar, data)
}

fn center_text(cx: &Context, x: f64, y: f64, text: &str, size: f64, bold: bool, c: (f64, f64, f64, f64)) {
    cx.select_font_face(
        "Sans",
        FontSlant::Normal,
        if bold { FontWeight::Bold } else { FontWeight::Normal },
    );
    cx.set_font_size(size);
    if let Ok(extents) = cx.text_extents(text) {
        cx.move_to(x - (extents.width() / 2.0 + extents.x_bearing()), y);
        cx.set_source_rgba(c.0, c.1, c.2, c.3);
        let _ = cx.show_text(text);
    }
}

fn draw_gauge(cx: &Context, w: f64, h: f64, data: ChartData) {
    let progress = chart_progress(data);
    let verdict = verdict_color(data.kind);
    let (mx, my) = (w / 2.0, h / 2.0);
    let radius = w.min(h) / 2.0 - 14.0;

    let glow = RadialGradient::new(mx, my, radius * 0.5, mx, my, radius + 14.0);
    glow.add_color_stop_rgba(0.0, verdict.0, verdict.1, verdict.2, 0.18);
    glow.add_color_stop_rgba(1.0, verdict.0, verdict.1, verdict.2, 0.0);
    let _ = cx.set_source(&glow);
    cx.arc(mx, my, radius + 14.0, 0.0, TAU);
    let _ = cx.fill();

    cx.set_line_width(11.0);
    cx.set_source_rgba(1.0, 1.0, 1.0, 0.07);
    cx.arc(mx, my, radius, 0.0, TAU);
    let _ = cx.stroke();

    let total = (data.malicious + data.suspicious + data.harmless + data.undetected + data.timeout)
        .max(1) as f64;
    let fm = data.malicious as f64 / total;
    let fs = data.suspicious as f64 / total;
    let fc = (1.0 - fm - fs).max(0.0);
    let span = TAU * progress;
    let mut angle = -PI / 2.0;
    for (fraction, color) in [(fm, MALICIOUS), (fs, SUSPICIOUS), (fc, CLEAN)] {
        if fraction <= 0.0 {
            continue;
        }
        let length = span * fraction;
        cx.set_source_rgb(color.0, color.1, color.2);
        cx.arc(mx, my, radius, angle, angle + length);
        let _ = cx.stroke();
        angle += length;
    }

    let flagged = data.malicious + data.suspicious;
    let shown = (flagged as f64 * progress).round() as u32;
    center_text(cx, mx, my + 8.0, &shown.to_string(), 36.0, true, (verdict.0, verdict.1, verdict.2, 1.0));
    center_text(
        cx,
        mx,
        my + 28.0,
        &format!("de {} motores", total as u32),
        11.0,
        false,
        (0.72, 0.7, 0.86, 0.9),
    );
}

fn draw_bar(cx: &Context, w: f64, h: f64, data: ChartData) {
    let progress = chart_progress(data);
    rounded_rectangle(cx, 0.0, 0.0, w, h, h / 2.0);
    cx.set_source_rgba(1.0, 1.0, 1.0, 0.07);
    let _ = cx.fill_preserve();
    cx.clip();
    let total = (data.malicious + data.suspicious + data.harmless + data.undetected + data.timeout)
        .max(1) as f64;
    let mut x = 0.0;
    for (count, color, alpha) in [
        (data.malicious, MALICIOUS, 1.0),
        (data.suspicious, SUSPICIOUS, 1.0),
        (data.harmless, CLEAN, 1.0),
        (data.undetected, MUTED, 0.6),
    ] {
        if count == 0 {
            continue;
        }
        let width = w * (count as f64 / total) * progress;
        cx.set_source_rgba(color.0, color.1, color.2, alpha);
        cx.rectangle(x, 0.0, width, h);
        let _ = cx.fill();
        x += width;
    }
}
