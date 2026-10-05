use gtk::cairo::{Context, Format, ImageSurface};
use ksni::blocking::TrayMethods;
use ksni::menu::{MenuItem, StandardItem};
use std::{process::Command, thread};

struct Look4Tray;

impl ksni::Tray for Look4Tray {
    fn id(&self) -> String {
        "br.com.look4.LinkChecker".into()
    }

    fn title(&self) -> String {
        "Look4 — Verificador de links".into()
    }

    // Empty name so the panel uses the drawn robot pixmap below.
    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        [64, 48, 32, 22].into_iter().map(robot_icon).collect()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        launch("--quick-check");
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Consulta rápida".into(),
                icon_name: "system-search-symbolic".into(),
                activate: Box::new(|_| launch("--quick-check")),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Abrir Look4".into(),
                icon_name: "window-new-symbolic".into(),
                activate: Box::new(|_| launch("--show-window")),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Sair".into(),
                icon_name: "application-exit-symbolic".into(),
                activate: Box::new(|_| launch("--quit")),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Draws a friendly robot head and converts it to the ARGB32 (network order) format of StatusNotifier.
fn robot_icon(size: i32) -> ksni::Icon {
    let mut surface =
        ImageSurface::create(Format::ARgb32, size, size).expect("cairo surface");
    {
        let cx = Context::new(&surface).expect("cairo context");
        cx.scale(size as f64 / 64.0, size as f64 / 64.0);
        draw_robot(&cx);
    }
    surface.flush();
    let stride = surface.stride() as usize;
    let pixels = surface.data().expect("surface data");
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size as usize {
        for x in 0..size as usize {
            let offset = y * stride + x * 4;
            let px = u32::from_ne_bytes([
                pixels[offset],
                pixels[offset + 1],
                pixels[offset + 2],
                pixels[offset + 3],
            ]);
            let a = (px >> 24) & 0xff;
            let unpremultiply = |c: u32| -> u8 {
                if a == 0 {
                    0
                } else {
                    ((c * 255 + a / 2) / a).min(255) as u8
                }
            };
            data.extend_from_slice(&[
                a as u8,
                unpremultiply((px >> 16) & 0xff),
                unpremultiply((px >> 8) & 0xff),
                unpremultiply(px & 0xff),
            ]);
        }
    }
    ksni::Icon {
        width: size,
        height: size,
        data,
    }
}

fn rounded(cx: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    use std::f64::consts::PI;
    cx.new_path();
    cx.arc(x + w - r, y + r, r, -PI / 2.0, 0.0);
    cx.arc(x + w - r, y + h - r, r, 0.0, PI / 2.0);
    cx.arc(x + r, y + h - r, r, PI / 2.0, PI);
    cx.arc(x + r, y + r, r, PI, 1.5 * PI);
    cx.close_path();
}

fn draw_robot(cx: &Context) {
    use std::f64::consts::TAU;
    // antenna
    cx.set_source_rgb(0.78, 0.82, 0.96);
    cx.set_line_width(3.0);
    cx.move_to(32.0, 15.0);
    cx.line_to(32.0, 7.0);
    let _ = cx.stroke();
    cx.set_source_rgb(1.0, 0.36, 0.55);
    cx.arc(32.0, 6.0, 4.0, 0.0, TAU);
    let _ = cx.fill();

    // ears
    cx.set_source_rgb(0.55, 0.62, 0.9);
    rounded(cx, 3.0, 26.0, 8.0, 14.0, 3.0);
    let _ = cx.fill();
    rounded(cx, 53.0, 26.0, 8.0, 14.0, 3.0);
    let _ = cx.fill();

    // head
    rounded(cx, 9.0, 15.0, 46.0, 36.0, 11.0);
    let head = gtk::cairo::LinearGradient::new(0.0, 15.0, 0.0, 51.0);
    head.add_color_stop_rgb(0.0, 0.96, 0.97, 1.0);
    head.add_color_stop_rgb(1.0, 0.68, 0.74, 0.95);
    let _ = cx.set_source(&head);
    let _ = cx.fill();

    // visor
    rounded(cx, 14.0, 21.0, 36.0, 22.0, 8.0);
    cx.set_source_rgb(0.07, 0.07, 0.17);
    let _ = cx.fill();

    // eyes with glow
    for ex in [24.5, 39.5] {
        let glow = gtk::cairo::RadialGradient::new(ex, 32.0, 0.0, ex, 32.0, 7.0);
        glow.add_color_stop_rgba(0.0, 0.3, 0.95, 1.0, 0.55);
        glow.add_color_stop_rgba(1.0, 0.3, 0.95, 1.0, 0.0);
        let _ = cx.set_source(&glow);
        cx.arc(ex, 32.0, 7.0, 0.0, TAU);
        let _ = cx.fill();
        cx.set_source_rgb(0.35, 0.97, 1.0);
        cx.arc(ex, 32.0, 3.6, 0.0, TAU);
        let _ = cx.fill();
    }

    // grille / neck
    cx.set_source_rgb(0.07, 0.07, 0.17);
    cx.set_line_width(1.8);
    for gx in [26.0, 32.0, 38.0] {
        cx.move_to(gx, 46.0);
        cx.line_to(gx, 48.5);
    }
    let _ = cx.stroke();
    cx.set_source_rgb(0.62, 0.69, 0.93);
    rounded(cx, 21.0, 52.0, 22.0, 8.0, 3.0);
    let _ = cx.fill();
}

pub fn start() {
    thread::spawn(|| {
        let tray = Look4Tray;
        if let Err(error) = tray.spawn() {
            eprintln!("Não foi possível iniciar o ícone do painel: {error}");
        }
        loop {
            thread::park();
        }
    });
}

fn launch(argument: &str) {
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    if let Err(error) = Command::new(executable).arg(argument).spawn() {
        eprintln!("Não foi possível executar a ação do painel: {error}");
    }
}
