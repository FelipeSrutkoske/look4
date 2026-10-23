mod credentials;
mod redirect;
mod tray;
mod virustotal;
mod visuals;

use adw::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const DEFAULT_HINT_VIRUS: &str = "Cole um link acima e clique em verificar";
const DEFAULT_HINT_REDIRECT: &str = "Cole um link acima para descobrir o destino final";
const INVALID_URL_HINT: &str = "Digite uma URL válida iniciada com http:// ou https://";

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScanMode {
    Virus,
    Redirect,
}

thread_local! {
    static QUICK_POPUP: RefCell<Option<gtk::Window>> = const { RefCell::new(None) };
}

fn main() {
    let hold_guard = std::rc::Rc::new(std::cell::RefCell::new(None));
    let primary_hold_guard = hold_guard.clone();
    let app = adw::Application::builder()
        .application_id("br.com.look4.LinkChecker")
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_startup(move |app| {
        *primary_hold_guard.borrow_mut() = Some(app.hold());
        tray::start();
    });
    app.connect_activate(|app| build_ui(app, true));
    app.connect_command_line(|app, command_line| {
        let arguments: Vec<String> = command_line
            .arguments()
            .iter()
            .skip(1)
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        let has = |flag: &str| arguments.iter().any(|argument| argument == flag);

        if has("--quit") {
            app.activate_action("quit", None);
            return 0;
        }
        // The quick check must NOT bring the main window up: only the small popup is shown.
        if has("--quick-check") {
            if app.windows().is_empty() {
                build_ui(app, false);
            }
            app.activate_action("quick-check", None);
            return 0;
        }
        app.activate();
        if has("--scan-clipboard") {
            app.activate_action("scan-clipboard", None);
        } else if has("--trace-clipboard")
            || has("--redirect-clipboard")
            || has("--final-link-clipboard")
        {
            app.activate_action("trace-clipboard", None);
        }
        0
    });
    app.run();
}

/// Widgets of the virus result card, bundled so they can be updated together.
#[derive(Clone)]
struct ResultView {
    card: gtk::Box,
    chart: visuals::SharedChart,
    gauge: gtk::DrawingArea,
    bar: gtk::DrawingArea,
    verdict: gtk::Label,
    subtitle: gtk::Label,
    url_chip: gtk::Label,
    n_malicious: gtk::Label,
    n_suspicious: gtk::Label,
    n_harmless: gtk::Label,
    n_undetected: gtk::Label,
    meta: gtk::Label,
    report: gtk::LinkButton,
}

/// Widgets of the redirect result card.
#[derive(Clone)]
struct RedirectResultView {
    card: gtk::Box,
    verdict: gtk::Label,
    subtitle: gtk::Label,
    url_chip: gtk::Label,
    dest_tag: gtk::Label,
    dest_url: gtk::Label,
    n_hops: gtk::Label,
    status_code: gtk::Label,
    proto: gtk::Label,
    hops_title: gtk::Label,
    hops_scrolled: gtk::ScrolledWindow,
    hops_box: gtk::Box,
    meta: gtk::Label,
    copy_dest_button: gtk::Button,
    open_dest_button: gtk::LinkButton,
    scan_dest_button: gtk::Button,
}

#[derive(Clone, Copy)]
enum HintKind {
    Normal,
    Error,
    Ok,
}

fn set_hint(label: &gtk::Label, text: &str, kind: HintKind) {
    label.set_text(text);
    label.remove_css_class("look4-hint-error");
    label.remove_css_class("look4-hint-ok");
    match kind {
        HintKind::Error => label.add_css_class("look4-hint-error"),
        HintKind::Ok => label.add_css_class("look4-hint-ok"),
        HintKind::Normal => {}
    }
}

fn is_valid_url(raw: &str) -> Option<url::Url> {
    match url::Url::parse(raw.trim()) {
        Ok(u) if (u.scheme() == "http" || u.scheme() == "https") && u.host_str().is_some() => {
            Some(u)
        }
        _ => None,
    }
}

fn styled_headerbar(title: Option<&str>, layout: &str) -> gtk::HeaderBar {
    let bar = gtk::HeaderBar::new();
    bar.set_show_title_buttons(true);
    bar.set_decoration_layout(Some(layout));
    bar.add_css_class("look4-headerbar");
    let label = gtk::Label::new(title);
    label.add_css_class("look4-brand");
    bar.set_title_widget(Some(&label));
    bar
}

fn icon_label_widgets(
    icon: &str,
    text: &str,
    class: &str,
) -> (gtk::Button, gtk::Image, gtk::Label) {
    let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    inner.set_halign(gtk::Align::Center);
    let image = gtk::Image::from_icon_name(icon);
    let label = gtk::Label::new(Some(text));
    inner.append(&image);
    inner.append(&label);
    let button = gtk::Button::builder().child(&inner).build();
    button.add_css_class(class);
    (button, image, label)
}

fn icon_label_button(icon: &str, text: &str, class: &str) -> gtk::Button {
    let (button, _, _) = icon_label_widgets(icon, text, class);
    button
}

fn stat_tile(class: &str, label: &str) -> (gtk::Box, gtk::Label) {
    let tile = gtk::Box::new(gtk::Orientation::Vertical, 4);
    tile.add_css_class("look4-tile");
    tile.add_css_class(class);
    tile.set_hexpand(true);
    let number = gtk::Label::new(Some("0"));
    number.add_css_class("look4-stat-num");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::Center);
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("look4-dot");
    dot.set_valign(gtk::Align::Center);
    let caption = gtk::Label::new(Some(label));
    caption.add_css_class("look4-stat-label");
    row.append(&dot);
    row.append(&caption);
    tile.append(&number);
    tile.append(&row);
    (tile, number)
}

fn build_ui(app: &adw::Application, present: bool) {
    if let Some(window) = app.windows().first() {
        if present {
            window.present();
        }
        return;
    }

    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
    if let Some(display) = gtk::gdk::Display::default() {
        visuals::install_style(&display);
    }

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("look4")
        .default_width(620)
        .default_height(720)
        .build();
    window.add_css_class("look4-window");

    let titlebar = styled_headerbar(Some("LOOK4"), ":minimize,maximize,close");
    let settings_button = gtk::Button::builder()
        .icon_name("emblem-system-symbolic")
        .tooltip_text("Configurações")
        .build();
    settings_button.add_css_class("look4-icon-button");
    titlebar.pack_end(&settings_button);
    window.set_titlebar(Some(&titlebar));

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&visuals::background()));
    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_top(24);
    content.set_margin_bottom(22);
    content.set_margin_start(40);
    content.set_margin_end(40);
    content.set_hexpand(true);
    content.set_vexpand(true);

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(320)
        .hhomogeneous(false)
        .vhomogeneous(false)
        .vexpand(true)
        .build();

    let current_mode = Rc::new(Cell::new(ScanMode::Virus));

    // ---------- input page ----------
    let input_view = gtk::Box::new(gtk::Orientation::Vertical, 0);
    input_view.set_halign(gtk::Align::Fill);
    input_view.set_valign(gtk::Align::Center);

    let badge = gtk::Label::new(Some(visuals::current().badge()));
    badge.add_css_class("look4-hero-badge");
    badge.set_halign(gtk::Align::Center);
    let badge_theme = badge.clone();
    let mode_for_theme = current_mode.clone();
    visuals::on_theme_changed(move |theme| {
        if mode_for_theme.get() == ScanMode::Virus {
            badge_theme.set_text(theme.badge());
        } else {
            badge_theme.set_text(theme.redirect_badge());
        }
    });

    let hero_title = gtk::Label::new(Some("Security protocol."));
    hero_title.add_css_class("look4-hero-title");
    hero_title.set_margin_top(14);
    let hero_sub = gtk::Label::new(Some("Insira o link abaixo."));
    hero_sub.add_css_class("look4-hero-sub");
    hero_sub.set_wrap(true);
    hero_sub.set_justify(gtk::Justification::Center);
    hero_sub.set_margin_top(6);
    hero_sub.set_margin_bottom(18);

    // Segmented Mode Switcher (Alternador)
    let mode_switcher = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    mode_switcher.add_css_class("look4-mode-switcher");
    mode_switcher.set_halign(gtk::Align::Center);
    mode_switcher.set_homogeneous(true);

    let mode_virus_btn = gtk::ToggleButton::new();
    mode_virus_btn.add_css_class("look4-mode-tab");
    let virus_tab_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    virus_tab_box.set_halign(gtk::Align::Center);
    virus_tab_box.append(&gtk::Image::from_icon_name("channel-secure-symbolic"));
    virus_tab_box.append(&gtk::Label::new(Some("Segurança (Vírus)")));
    mode_virus_btn.set_child(Some(&virus_tab_box));

    let mode_redirect_btn = gtk::ToggleButton::new();
    mode_redirect_btn.add_css_class("look4-mode-tab");
    let redirect_tab_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    redirect_tab_box.set_halign(gtk::Align::Center);
    redirect_tab_box.append(&gtk::Image::from_icon_name("insert-link-symbolic"));
    redirect_tab_box.append(&gtk::Label::new(Some("Link Final")));
    mode_redirect_btn.set_child(Some(&redirect_tab_box));

    mode_redirect_btn.set_group(Some(&mode_virus_btn));
    mode_virus_btn.set_active(true);
    mode_switcher.append(&mode_virus_btn);
    mode_switcher.append(&mode_redirect_btn);

    let url_entry = gtk::Entry::builder()
        .placeholder_text("https://seulink.com")
        .hexpand(true)
        .build();
    url_entry.set_icon_from_icon_name(
        gtk::EntryIconPosition::Primary,
        Some("web-browser-symbolic"),
    );
    url_entry.set_icon_from_icon_name(
        gtk::EntryIconPosition::Secondary,
        Some("edit-paste-symbolic"),
    );
    url_entry.set_icon_tooltip_text(
        gtk::EntryIconPosition::Secondary,
        Some("Colar da área de transferência"),
    );
    url_entry.add_css_class("look4-entry");
    url_entry.set_activates_default(true);
    let url_frame = url_input_frame(&url_entry);

    let (scan, scan_icon, scan_label) =
        icon_label_widgets("system-search-symbolic", "Consultar vírus", "look4-primary");
    scan.set_halign(gtk::Align::Fill);
    scan.set_margin_top(16);

    let input_hint = gtk::Label::new(Some(DEFAULT_HINT_VIRUS));
    input_hint.add_css_class("look4-hint");
    input_hint.set_wrap(true);
    input_hint.set_justify(gtk::Justification::Center);
    input_hint.set_margin_top(18);

    let shortcut_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    shortcut_row.set_halign(gtk::Align::Center);
    shortcut_row.set_margin_top(8);
    let shortcut_text = gtk::Label::new(Some("ou verifique a área de transferência com"));
    shortcut_text.add_css_class("look4-subtle");
    let kbd = gtk::Label::new(Some("Ctrl + Alt + V"));
    kbd.add_css_class("look4-kbd");
    shortcut_row.append(&shortcut_text);
    shortcut_row.append(&kbd);

    input_view.append(&badge);
    input_view.append(&hero_title);
    input_view.append(&hero_sub);
    input_view.append(&mode_switcher);
    input_view.append(&url_frame);
    input_view.append(&scan);
    input_view.append(&input_hint);
    input_view.append(&shortcut_row);
    stack.add_named(&input_view, Some("input"));

    // Mode Switcher handlers
    let mode_for_virus = current_mode.clone();
    let badge_v = badge.clone();
    let hero_title_v = hero_title.clone();
    let hero_sub_v = hero_sub.clone();
    let scan_icon_v = scan_icon.clone();
    let scan_label_v = scan_label.clone();
    let entry_v = url_entry.clone();
    let shortcut_text_v = shortcut_text.clone();
    let kbd_v = kbd.clone();
    let hint_v = input_hint.clone();
    mode_virus_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            mode_for_virus.set(ScanMode::Virus);
            badge_v.set_text(visuals::current().badge());
            hero_title_v.set_text("Security protocol.");
            hero_sub_v.set_text("Insira o link abaixo.");
            scan_icon_v.set_icon_name(Some("system-search-symbolic"));
            scan_label_v.set_text("Consultar vírus");
            entry_v.set_placeholder_text(Some("https://seulink.com"));
            shortcut_text_v.set_text("ou verifique a área de transferência com");
            kbd_v.set_text("Ctrl + Alt + V");
            set_hint(&hint_v, DEFAULT_HINT_VIRUS, HintKind::Normal);
        }
    });

    let mode_for_redirect = current_mode.clone();
    let badge_r = badge.clone();
    let hero_title_r = hero_title.clone();
    let hero_sub_r = hero_sub.clone();
    let scan_icon_r = scan_icon.clone();
    let scan_label_r = scan_label.clone();
    let entry_r = url_entry.clone();
    let shortcut_text_r = shortcut_text.clone();
    let kbd_r = kbd.clone();
    let hint_r = input_hint.clone();
    mode_redirect_btn.connect_toggled(move |btn| {
        if btn.is_active() {
            mode_for_redirect.set(ScanMode::Redirect);
            badge_r.set_text(visuals::current().redirect_badge());
            hero_title_r.set_text("Destino do link.");
            hero_sub_r.set_text("Rastreie todos os redirecionamentos até o link final.");
            scan_icon_r.set_icon_name(Some("insert-link-symbolic"));
            scan_label_r.set_text("Rastrear link final");
            entry_r.set_placeholder_text(Some("https://linkencurtado.com/exemplo"));
            shortcut_text_r.set_text("ou rastreie a área de transferência com");
            kbd_r.set_text("Ctrl + Alt + R");
            set_hint(&hint_r, DEFAULT_HINT_REDIRECT, HintKind::Normal);
        }
    });

    // ---------- progress page ----------
    let progress_view = gtk::Box::new(gtk::Orientation::Vertical, 18);
    progress_view.set_halign(gtk::Align::Center);
    progress_view.set_valign(gtk::Align::Center);
    let orb = visuals::animated_orb();
    let progress_status = gtk::Label::new(Some("Consultando os mecanismos de segurança…"));
    progress_status.add_css_class("look4-status");
    progress_status.set_wrap(true);
    progress_status.set_justify(gtk::Justification::Center);
    let back_button = icon_label_button("go-previous-symbolic", "Voltar", "look4-ghost");
    back_button.set_halign(gtk::Align::Center);
    back_button.set_visible(false);
    progress_view.append(&orb);
    progress_view.append(&progress_status);
    progress_view.append(&back_button);
    stack.add_named(&progress_view, Some("progress"));

    // ---------- virus result page ----------
    let result_view = gtk::Box::new(gtk::Orientation::Vertical, 0);
    result_view.set_halign(gtk::Align::Fill);
    result_view.set_valign(gtk::Align::Center);
    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("look4-card");
    let (gauge, bar, chart) = visuals::result_charts();
    let verdict = gtk::Label::new(Some("Análise concluída"));
    verdict.add_css_class("look4-verdict");
    verdict.set_margin_top(2);
    let subtitle = gtk::Label::new(None);
    subtitle.add_css_class("look4-result-sub");
    subtitle.set_wrap(true);
    subtitle.set_justify(gtk::Justification::Center);
    subtitle.set_max_width_chars(46);
    let url_chip = gtk::Label::new(None);
    url_chip.add_css_class("look4-url-chip");
    url_chip.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    url_chip.set_max_width_chars(46);
    url_chip.set_halign(gtk::Align::Center);
    url_chip.set_selectable(true);
    bar.set_margin_top(8);

    let tiles = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    tiles.set_homogeneous(true);
    tiles.set_margin_top(6);
    let (tile_mal, n_malicious) = stat_tile("look4-tile-mal", "Maliciosos");
    let (tile_sus, n_suspicious) = stat_tile("look4-tile-sus", "Suspeitos");
    let (tile_har, n_harmless) = stat_tile("look4-tile-har", "Inofensivos");
    let (tile_und, n_undetected) = stat_tile("look4-tile-und", "Sem detecção");
    for tile in [&tile_mal, &tile_sus, &tile_har, &tile_und] {
        tiles.append(tile);
    }
    let meta = gtk::Label::new(None);
    meta.add_css_class("look4-meta");
    meta.set_margin_top(4);

    card.append(&gauge);
    card.append(&verdict);
    card.append(&subtitle);
    card.append(&url_chip);
    card.append(&bar);
    card.append(&tiles);
    card.append(&meta);

    let result_actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    result_actions.set_halign(gtk::Align::Center);
    result_actions.set_margin_top(20);
    result_actions.set_homogeneous(true);
    let new_scan = icon_label_button("view-refresh-symbolic", "Nova consulta", "look4-ghost");
    let report_inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    report_inner.set_halign(gtk::Align::Center);
    report_inner.append(&gtk::Label::new(Some("Relatório completo")));
    report_inner.append(&gtk::Image::from_icon_name("adw-external-link-symbolic"));
    let report = gtk::LinkButton::new("https://www.virustotal.com");
    report.set_child(Some(&report_inner));
    report.add_css_class("look4-primary");
    report.add_css_class("look4-small");
    result_actions.append(&new_scan);
    result_actions.append(&report);
    result_view.append(&card);
    result_view.append(&result_actions);
    stack.add_named(&result_view, Some("result"));

    let result_widgets = ResultView {
        card,
        chart,
        gauge,
        bar,
        verdict,
        subtitle,
        url_chip,
        n_malicious,
        n_suspicious,
        n_harmless,
        n_undetected,
        meta,
        report,
    };

    // ---------- redirect result page ----------
    let redirect_view = gtk::Box::new(gtk::Orientation::Vertical, 0);
    redirect_view.set_halign(gtk::Align::Fill);
    redirect_view.set_valign(gtk::Align::Center);

    let r_card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    r_card.add_css_class("look4-card");

    let r_verdict = gtk::Label::new(Some("Destino localizado"));
    r_verdict.add_css_class("look4-verdict");
    r_verdict.set_margin_top(4);

    let r_subtitle = gtk::Label::new(None);
    r_subtitle.add_css_class("look4-result-sub");
    r_subtitle.set_wrap(true);
    r_subtitle.set_justify(gtk::Justification::Center);
    r_subtitle.set_max_width_chars(42);

    let r_url_chip = gtk::Label::new(None);
    r_url_chip.add_css_class("look4-url-chip");
    r_url_chip.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    r_url_chip.set_max_width_chars(38);
    r_url_chip.set_halign(gtk::Align::Center);
    r_url_chip.set_selectable(true);

    // Final URL Dest Card
    let dest_card = gtk::Box::new(gtk::Orientation::Vertical, 6);
    dest_card.add_css_class("look4-redirect-dest-card");

    let dest_tag_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    dest_tag_row.set_halign(gtk::Align::Center);
    let dest_tag = gtk::Label::new(Some("DESTINO FINAL"));
    dest_tag.add_css_class("look4-redirect-dest-tag");
    dest_tag_row.append(&dest_tag);

    let dest_url = gtk::Label::new(None);
    dest_url.add_css_class("look4-redirect-dest-url");
    dest_url.set_wrap(true);
    dest_url.set_wrap_mode(gtk::pango::WrapMode::Char);
    dest_url.set_justify(gtk::Justification::Center);
    dest_url.set_selectable(true);
    dest_url.set_max_width_chars(38);

    let dest_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    dest_actions.set_halign(gtk::Align::Center);
    dest_actions.set_margin_top(8);

    let copy_dest_button = icon_label_button("edit-copy-symbolic", "Copiar", "look4-ghost");
    copy_dest_button.add_css_class("look4-small");

    let open_inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    open_inner.set_halign(gtk::Align::Center);
    open_inner.append(&gtk::Label::new(Some("Abrir")));
    open_inner.append(&gtk::Image::from_icon_name("adw-external-link-symbolic"));
    let open_dest_button = gtk::LinkButton::new("https://example.com");
    open_dest_button.set_child(Some(&open_inner));
    open_dest_button.add_css_class("look4-ghost");
    open_dest_button.add_css_class("look4-small");

    dest_actions.append(&copy_dest_button);
    dest_actions.append(&open_dest_button);

    dest_card.append(&dest_tag_row);
    dest_card.append(&dest_url);
    dest_card.append(&dest_actions);

    let r_tiles = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    r_tiles.set_homogeneous(true);
    r_tiles.set_margin_top(6);
    let (tile_hops, n_hops) = stat_tile("look4-tile-sus", "Saltos");
    let (tile_status, status_code) = stat_tile("look4-tile-har", "Status HTTP");
    let (tile_proto, proto) = stat_tile("look4-tile-und", "Protocolo");
    r_tiles.append(&tile_hops);
    r_tiles.append(&tile_status);
    r_tiles.append(&tile_proto);

    let hops_title = gtk::Label::new(Some("ROTA DOS REDIRECIONAMENTOS"));
    hops_title.add_css_class("look4-field-label");
    hops_title.set_halign(gtk::Align::Start);
    hops_title.set_margin_top(6);

    let hops_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let hops_scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .max_content_height(130)
        .propagate_natural_height(true)
        .child(&hops_box)
        .build();
    hops_scrolled.add_css_class("look4-hops-scrolled");

    let r_meta = gtk::Label::new(None);
    r_meta.add_css_class("look4-meta");
    r_meta.set_margin_top(4);

    r_card.append(&r_verdict);
    r_card.append(&r_subtitle);
    r_card.append(&r_url_chip);
    r_card.append(&dest_card);
    r_card.append(&r_tiles);
    r_card.append(&hops_title);
    r_card.append(&hops_scrolled);
    r_card.append(&r_meta);

    let r_result_actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    r_result_actions.set_halign(gtk::Align::Center);
    r_result_actions.set_margin_top(20);
    r_result_actions.set_homogeneous(true);
    let new_trace_btn = icon_label_button("view-refresh-symbolic", "Nova consulta", "look4-ghost");
    let scan_dest_button = icon_label_button(
        "channel-secure-symbolic",
        "Verificar vírus",
        "look4-primary",
    );
    scan_dest_button.add_css_class("look4-small");
    r_result_actions.append(&new_trace_btn);
    r_result_actions.append(&scan_dest_button);

    redirect_view.append(&r_card);
    redirect_view.append(&r_result_actions);
    stack.add_named(&redirect_view, Some("redirect-result"));

    let redirect_widgets = RedirectResultView {
        card: r_card,
        verdict: r_verdict,
        subtitle: r_subtitle,
        url_chip: r_url_chip,
        dest_tag,
        dest_url,
        n_hops,
        status_code,
        proto,
        hops_title,
        hops_scrolled,
        hops_box,
        meta: r_meta,
        copy_dest_button,
        open_dest_button,
        scan_dest_button,
    };

    content.append(&stack);
    overlay.add_overlay(&content);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&visuals::rgb_bar());
    root.append(&overlay);
    window.set_child(Some(&root));

    // ---------- behaviour ----------
    url_entry.connect_icon_release(|entry, position| {
        if position != gtk::EntryIconPosition::Secondary {
            return;
        }
        let target = entry.clone();
        entry
            .clipboard()
            .read_text_async(None::<&gio::Cancellable>, move |res| {
                if let Ok(Some(text)) = res {
                    target.set_text(text.trim());
                    target.set_position(-1);
                }
            });
    });

    let stack_back = stack.clone();
    let entry_back = url_entry.clone();
    back_button.connect_clicked(move |_| {
        stack_back.set_visible_child_name("input");
        entry_back.grab_focus();
    });

    let stack_new = stack.clone();
    let entry_new = url_entry.clone();
    let hint_new = input_hint.clone();
    let mode_for_new = current_mode.clone();
    new_scan.connect_clicked(move |_| {
        entry_new.set_text("");
        let def_hint = if mode_for_new.get() == ScanMode::Virus {
            DEFAULT_HINT_VIRUS
        } else {
            DEFAULT_HINT_REDIRECT
        };
        set_hint(&hint_new, def_hint, HintKind::Normal);
        stack_new.set_visible_child_name("input");
        entry_new.grab_focus();
    });

    let stack_new_trace = stack.clone();
    let entry_new_trace = url_entry.clone();
    let hint_new_trace = input_hint.clone();
    let mode_for_new_trace = current_mode.clone();
    new_trace_btn.connect_clicked(move |_| {
        entry_new_trace.set_text("");
        let def_hint = if mode_for_new_trace.get() == ScanMode::Virus {
            DEFAULT_HINT_VIRUS
        } else {
            DEFAULT_HINT_REDIRECT
        };
        set_hint(&hint_new_trace, def_hint, HintKind::Normal);
        stack_new_trace.set_visible_child_name("input");
        entry_new_trace.grab_focus();
    });

    // Copy destination URL button
    let dest_label_copy = redirect_widgets.dest_url.clone();
    let copy_btn_ref = redirect_widgets.copy_dest_button.clone();
    redirect_widgets.copy_dest_button.connect_clicked(move |_| {
        let text = dest_label_copy.text().to_string();
        if let Some(display) = gtk::gdk::Display::default() {
            display.clipboard().set_text(&text);
            copy_btn_ref.set_label("✓ Copiado!");
            let btn_reset = copy_btn_ref.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(1600), move || {
                btn_reset.set_label("Copiar destino");
            });
        }
    });

    // Scan destination URL for viruses button
    let dest_label_scan = redirect_widgets.dest_url.clone();
    let entry_dest_scan = url_entry.clone();
    let mode_virus_tab = mode_virus_btn.clone();
    let scan_dest_trigger = scan.clone();
    redirect_widgets.scan_dest_button.connect_clicked(move |_| {
        let text = dest_label_scan.text().to_string();
        entry_dest_scan.set_text(&text);
        mode_virus_tab.set_active(true);
        scan_dest_trigger.emit_clicked();
    });

    let settings_parent = window.clone();
    settings_button.connect_clicked(move |_| open_settings(&settings_parent));

    let entry_activate = scan.clone();
    url_entry.connect_activate(move |_| entry_activate.emit_clicked());

    // Set by the quick popup: the main window stays hidden until the scan has an outcome.
    let quiet = Rc::new(Cell::new(false));

    let status_scan = progress_status.clone();
    let result_scan = result_widgets.clone();
    let redirect_result_scan = redirect_widgets.clone();
    let url_scan = url_entry.clone();
    let stack_scan = stack.clone();
    let back_scan = back_button.clone();
    let hint_scan = input_hint.clone();
    let window_scan = window.clone();
    let quiet_scan = quiet.clone();
    let mode_for_scan = current_mode.clone();

    scan.connect_clicked(move |_| {
        let reveal = quiet_scan.replace(false);
        let Some(parsed) = is_valid_url(&url_scan.text()) else {
            set_hint(&hint_scan, INVALID_URL_HINT, HintKind::Error);
            if reveal {
                window_scan.present();
            }
            return;
        };

        match mode_for_scan.get() {
            ScanMode::Virus => {
                let key = credentials::entry()
                    .ok()
                    .and_then(|entry| entry.get_password().ok())
                    .unwrap_or_default();
                if key.is_empty() {
                    set_hint(
                        &hint_scan,
                        "Configure sua chave da API na engrenagem (canto superior) para continuar.",
                        HintKind::Error,
                    );
                    if reveal {
                        window_scan.present();
                    }
                    return;
                }
                set_hint(&hint_scan, DEFAULT_HINT_VIRUS, HintKind::Normal);
                back_scan.set_visible(false);
                stack_scan.set_visible_child_name("progress");
                start_scan(
                    parsed.to_string(),
                    key,
                    reveal,
                    window_scan.clone(),
                    stack_scan.clone(),
                    status_scan.clone(),
                    result_scan.clone(),
                    back_scan.clone(),
                );
            }
            ScanMode::Redirect => {
                set_hint(&hint_scan, DEFAULT_HINT_REDIRECT, HintKind::Normal);
                back_scan.set_visible(false);
                stack_scan.set_visible_child_name("progress");
                start_trace(
                    parsed.to_string(),
                    reveal,
                    window_scan.clone(),
                    stack_scan.clone(),
                    status_scan.clone(),
                    redirect_result_scan.clone(),
                    back_scan.clone(),
                );
            }
        }
    });

    // Global action: scan clipboard for viruses (Ctrl + Alt + V)
    let win_v = window.clone();
    let url_v = url_entry.clone();
    let hint_v = input_hint.clone();
    let scan_v = scan.clone();
    let mode_v_toggle = mode_virus_btn.clone();
    let action_virus = gio::SimpleAction::new("scan-clipboard", None);
    action_virus.connect_activate(move |_, _| {
        win_v.present();
        mode_v_toggle.set_active(true);
        let clipboard = gtk::prelude::WidgetExt::display(&win_v).clipboard();
        let target = url_v.clone();
        let hint = hint_v.clone();
        let scan = scan_v.clone();
        clipboard.read_text_async(None::<&gio::Cancellable>, move |res| match res {
            Ok(Some(text)) => {
                target.set_text(text.trim());
                set_hint(
                    &hint,
                    "Link recebido da área de transferência.",
                    HintKind::Ok,
                );
                scan.emit_clicked();
            }
            _ => set_hint(
                &hint,
                "Não foi possível ler a área de transferência.",
                HintKind::Error,
            ),
        });
    });
    app.add_action(&action_virus);
    app.set_accels_for_action("app.scan-clipboard", &["<Control><Alt>v"]);

    // Global action: trace clipboard for final destination (Ctrl + Alt + R)
    let win_r = window.clone();
    let url_r = url_entry.clone();
    let hint_r = input_hint.clone();
    let scan_r = scan.clone();
    let mode_r_toggle = mode_redirect_btn.clone();
    let action_trace = gio::SimpleAction::new("trace-clipboard", None);
    action_trace.connect_activate(move |_, _| {
        win_r.present();
        mode_r_toggle.set_active(true);
        let clipboard = gtk::prelude::WidgetExt::display(&win_r).clipboard();
        let target = url_r.clone();
        let hint = hint_r.clone();
        let scan = scan_r.clone();
        clipboard.read_text_async(None::<&gio::Cancellable>, move |res| match res {
            Ok(Some(text)) => {
                target.set_text(text.trim());
                set_hint(
                    &hint,
                    "Link recebido para rastreamento de destino.",
                    HintKind::Ok,
                );
                scan.emit_clicked();
            }
            _ => set_hint(
                &hint,
                "Não foi possível ler a área de transferência.",
                HintKind::Error,
            ),
        });
    });
    app.add_action(&action_trace);
    app.set_accels_for_action("app.trace-clipboard", &["<Control><Alt>r"]);

    let quick_window = window.clone();
    let quick_entry = url_entry.clone();
    let quick_scan = scan.clone();
    let quick_flag = quiet.clone();
    let quick_mode_v = mode_virus_btn.clone();
    let quick_mode_r = mode_redirect_btn.clone();
    let quick_action = gio::SimpleAction::new("quick-check", None);
    quick_action.connect_activate(move |_, _| {
        open_quick_popup(
            &quick_window,
            &quick_entry,
            &quick_scan,
            &quick_flag,
            &quick_mode_v,
            &quick_mode_r,
        );
    });
    app.add_action(&quick_action);

    let quit_action = gio::SimpleAction::new("quit", None);
    let app_quit = app.clone();
    quit_action.connect_activate(move |_, _| app_quit.quit());
    app.add_action(&quit_action);

    window.connect_close_request(|window| {
        window.set_visible(false);
        glib::Propagation::Stop
    });

    if present {
        window.present();
        url_entry.grab_focus();
    }
}

fn styled_dialog(parent: &impl IsA<gtk::Window>, title: &str, width: i32) -> gtk::Window {
    let dialog = gtk::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .default_width(width)
        .resizable(false)
        .build();
    dialog.add_css_class("look4-dialog");
    let bar = styled_headerbar(None, ":close");
    bar.add_css_class("look4-flat");
    dialog.set_titlebar(Some(&bar));
    let escape = gtk::EventControllerKey::new();
    let dialog_escape = dialog.clone();
    escape.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            dialog_escape.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    dialog.add_controller(escape);
    dialog
}

fn dialog_body() -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_margin_top(4);
    content.set_margin_bottom(26);
    content.set_margin_start(28);
    content.set_margin_end(28);
    content
}

fn dialog_header(icon: &str, title: &str, subtitle: &str) -> gtk::Box {
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(22);
    image.add_css_class("look4-icon-button");
    image.set_size_request(44, 44);
    image.set_valign(gtk::Align::Center);
    let texts = gtk::Box::new(gtk::Orientation::Vertical, 2);
    texts.set_valign(gtk::Align::Center);
    let title_label = gtk::Label::new(Some(title));
    title_label.add_css_class("look4-panel-title");
    title_label.set_halign(gtk::Align::Start);
    let subtitle_label = gtk::Label::new(Some(subtitle));
    subtitle_label.add_css_class("look4-subtle");
    subtitle_label.set_halign(gtk::Align::Start);
    subtitle_label.set_wrap(true);
    subtitle_label.set_xalign(0.0);
    texts.append(&title_label);
    texts.append(&subtitle_label);
    header.append(&image);
    header.append(&texts);
    header.set_margin_bottom(12);
    header
}

fn theme_picker() -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.set_homogeneous(true);
    let mut first: Option<gtk::ToggleButton> = None;
    for theme in visuals::Theme::ALL {
        let button = gtk::ToggleButton::new();
        button.add_css_class("look4-theme-card");
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let swatch = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        swatch.add_css_class("look4-swatch");
        swatch.add_css_class(&format!("look4-swatch-{}", theme.id()));
        let name = gtk::Label::new(Some(theme.label()));
        name.add_css_class("look4-theme-name");
        inner.append(&swatch);
        inner.append(&name);
        button.set_child(Some(&inner));
        match &first {
            Some(group) => button.set_group(Some(group)),
            None => first = Some(button.clone()),
        }
        button.set_active(theme == visuals::current());
        button.connect_toggled(move |button| {
            if button.is_active() {
                visuals::set_theme(theme);
            }
        });
        row.append(&button);
    }
    row
}

fn open_settings(parent: &gtk::ApplicationWindow) {
    let dialog = styled_dialog(parent, "Configurações", 460);
    let content = dialog_body();

    let header = dialog_header(
        "dialog-password-symbolic",
        "Configurações",
        "Aparência e conexão com o VirusTotal.",
    );
    let theme_label = gtk::Label::new(Some("TEMA"));
    theme_label.add_css_class("look4-field-label");
    theme_label.set_halign(gtk::Align::Start);
    let picker = theme_picker();

    let field_label = gtk::Label::new(Some("CHAVE DA API"));
    field_label.add_css_class("look4-field-label");
    field_label.set_halign(gtk::Align::Start);
    field_label.set_margin_top(10);
    let key_input = gtk::PasswordEntry::builder()
        .placeholder_text("Cole sua chave aqui")
        .show_peek_icon(true)
        .activates_default(true)
        .build();
    key_input.add_css_class("look4-entry");
    key_input.add_css_class("look4-plain");
    let state = gtk::Label::new(None);
    state.add_css_class("look4-hint");
    state.set_halign(gtk::Align::Start);
    state.set_wrap(true);
    state.set_xalign(0.0);
    let configured = credentials::entry()
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .is_some();
    if configured {
        set_hint(
            &state,
            "✓ Chave configurada no chaveiro seguro. Informe outra para substituí-la.",
            HintKind::Ok,
        );
    } else {
        set_hint(
            &state,
            "A chave fica armazenada no chaveiro do sistema.",
            HintKind::Normal,
        );
    }
    let get_key = gtk::LinkButton::with_label(
        "https://www.virustotal.com/gui/my-apikey",
        "Onde encontro minha chave?",
    );
    get_key.set_halign(gtk::Align::Start);
    get_key.add_css_class("look4-ghost");

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.set_halign(gtk::Align::End);
    actions.set_margin_top(14);
    let close = gtk::Button::with_label("Fechar");
    close.add_css_class("look4-ghost");
    let save = icon_label_button("object-select-symbolic", "Salvar chave", "look4-primary");
    save.add_css_class("look4-small");
    actions.append(&close);
    actions.append(&save);
    content.append(&header);
    content.append(&theme_label);
    content.append(&picker);
    content.append(&field_label);
    content.append(&key_input);
    content.append(&state);
    content.append(&get_key);
    content.append(&actions);
    dialog.set_child(Some(&content));
    dialog.set_default_widget(Some(&save));

    let dialog_close = dialog.clone();
    close.connect_clicked(move |_| dialog_close.close());
    let state_save = state.clone();
    let key_save = key_input.clone();
    save.connect_clicked(move |_| {
        let key = key_save.text().to_string();
        if key.trim().is_empty() {
            set_hint(
                &state_save,
                "Digite uma chave para salvar ou substituir a atual.",
                HintKind::Error,
            );
            return;
        }
        match credentials::entry()
            .and_then(|entry| entry.set_password(key.trim()).map_err(|e| e.to_string()))
        {
            Ok(()) => {
                key_save.set_text("");
                set_hint(&state_save, "✓ Chave salva no sistema.", HintKind::Ok);
            }
            Err(error) => set_hint(
                &state_save,
                &format!("Não foi possível salvar a chave: {error}"),
                HintKind::Error,
            ),
        }
    });
    dialog.present();
}

/// Compact floating input used by the tray: it never shows the main window.
/// The main window only appears once the scan has a result (or needs attention).
fn open_quick_popup(
    main_window: &gtk::ApplicationWindow,
    target_entry: &gtk::Entry,
    scan_button: &gtk::Button,
    quiet: &Rc<Cell<bool>>,
    mode_virus_btn: &gtk::ToggleButton,
    mode_redirect_btn: &gtk::ToggleButton,
) {
    if let Some(existing) = QUICK_POPUP.with(|slot| slot.borrow().clone()) {
        existing.present();
        return;
    }

    let popup = gtk::Window::builder()
        .title("Look4")
        .decorated(false)
        .resizable(false)
        .default_width(560)
        .build();
    popup.add_css_class("look4-popup");

    let shell = gtk::Box::new(gtk::Orientation::Vertical, 14);
    shell.add_css_class("look4-popup-box");

    let top = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let badge = gtk::Image::from_icon_name("system-search-symbolic");
    badge.set_pixel_size(18);
    badge.add_css_class("look4-icon-button");
    badge.set_size_request(36, 36);
    let title = gtk::Label::new(Some("Consulta rápida"));
    title.add_css_class("look4-panel-title");
    title.set_hexpand(true);
    title.set_halign(gtk::Align::Start);
    let close = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .build();
    close.add_css_class("look4-fab");
    top.append(&badge);
    top.append(&title);
    top.append(&close);

    // Mode Switcher in Quick Popup
    let popup_switcher = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    popup_switcher.add_css_class("look4-mode-switcher");
    popup_switcher.set_halign(gtk::Align::Center);
    popup_switcher.set_homogeneous(true);

    let pop_btn_v = gtk::ToggleButton::with_label("Segurança (Vírus)");
    pop_btn_v.add_css_class("look4-mode-tab");
    let pop_btn_r = gtk::ToggleButton::with_label("Link Final");
    pop_btn_r.add_css_class("look4-mode-tab");
    pop_btn_r.set_group(Some(&pop_btn_v));
    if mode_redirect_btn.is_active() {
        pop_btn_r.set_active(true);
    } else {
        pop_btn_v.set_active(true);
    }
    popup_switcher.append(&pop_btn_v);
    popup_switcher.append(&pop_btn_r);

    let entry = gtk::Entry::builder()
        .placeholder_text("Cole o link que deseja verificar")
        .hexpand(true)
        .build();
    entry.set_icon_from_icon_name(
        gtk::EntryIconPosition::Primary,
        Some("web-browser-symbolic"),
    );
    entry.add_css_class("look4-entry");
    let frame = url_input_frame(&entry);
    frame.set_hexpand(true);
    let submit = icon_label_button("system-search-symbolic", "Verificar", "look4-primary");
    submit.add_css_class("look4-small");
    submit.set_valign(gtk::Align::Center);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.append(&frame);
    row.append(&submit);

    let hint = gtk::Label::new(Some("O resultado aparece assim que a análise terminar."));
    hint.add_css_class("look4-hint");
    hint.set_halign(gtk::Align::Start);

    shell.append(&top);
    shell.append(&popup_switcher);
    shell.append(&row);
    shell.append(&hint);
    popup.set_child(Some(&shell));

    let go = {
        let popup = popup.clone();
        let entry = entry.clone();
        let hint = hint.clone();
        let target = target_entry.clone();
        let scan = scan_button.clone();
        let quiet = quiet.clone();
        let pop_btn_r = pop_btn_r.clone();
        let main_mode_v = mode_virus_btn.clone();
        let main_mode_r = mode_redirect_btn.clone();
        let main_window = main_window.clone();
        let _ = &main_window;
        Rc::new(move || {
            let text = entry.text().trim().to_string();
            if is_valid_url(&text).is_none() {
                set_hint(&hint, INVALID_URL_HINT, HintKind::Error);
                return;
            }
            if pop_btn_r.is_active() {
                main_mode_r.set_active(true);
            } else {
                main_mode_v.set_active(true);
            }
            target.set_text(&text);
            quiet.set(true);
            popup.close();
            scan.emit_clicked();
        })
    };
    let go_click = go.clone();
    submit.connect_clicked(move |_| go_click());
    let go_enter = go.clone();
    entry.connect_activate(move |_| go_enter());
    let popup_close = popup.clone();
    close.connect_clicked(move |_| popup_close.close());

    let escape = gtk::EventControllerKey::new();
    let popup_escape = popup.clone();
    escape.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            popup_escape.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    popup.add_controller(escape);

    // Close when the user clicks elsewhere (only after the popup has been focused once).
    let armed = Rc::new(Cell::new(false));
    popup.connect_is_active_notify(move |window| {
        if window.is_active() {
            armed.set(true);
        } else if armed.get() {
            window.close();
        }
    });
    popup.connect_close_request(|_| {
        QUICK_POPUP.with(|slot| slot.borrow_mut().take());
        glib::Propagation::Proceed
    });

    QUICK_POPUP.with(|slot| *slot.borrow_mut() = Some(popup.clone()));
    popup.present();
    entry.grab_focus();

    // Pre-fill with the clipboard when it already holds a link.
    let prefill = entry.clone();
    entry
        .clipboard()
        .read_text_async(None::<&gio::Cancellable>, move |res| {
            if let Ok(Some(text)) = res {
                if is_valid_url(&text).is_some() && prefill.text().is_empty() {
                    prefill.set_text(text.trim());
                    prefill.select_region(0, -1);
                }
            }
        });
}

/// Styled replacement for the native `AlertDialog`; resolves to `true` when accepted.
async fn confirm(parent: &gtk::ApplicationWindow, title: &str, detail: &str, accept: &str) -> bool {
    let dialog = styled_dialog(parent, title, 440);
    let content = dialog_body();
    content.append(&dialog_header("dialog-question-symbolic", title, detail));
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    actions.set_halign(gtk::Align::End);
    actions.set_margin_top(8);
    let cancel = gtk::Button::with_label("Cancelar");
    cancel.add_css_class("look4-ghost");
    let ok = icon_label_button("document-send-symbolic", accept, "look4-primary");
    ok.add_css_class("look4-small");
    actions.append(&cancel);
    actions.append(&ok);
    content.append(&actions);
    dialog.set_child(Some(&content));

    let (sender, receiver) = futures_channel::oneshot::channel::<bool>();
    let sender = Rc::new(RefCell::new(Some(sender)));
    let resolve = move |value: bool| {
        if let Some(sender) = sender.borrow_mut().take() {
            let _ = sender.send(value);
        }
    };
    let resolve_ok = resolve.clone();
    let dialog_ok = dialog.clone();
    ok.connect_clicked(move |_| {
        resolve_ok(true);
        dialog_ok.close();
    });
    let dialog_cancel = dialog.clone();
    cancel.connect_clicked(move |_| dialog_cancel.close());
    dialog.connect_close_request(move |_| {
        resolve(false);
        glib::Propagation::Proceed
    });
    dialog.present();
    ok.grab_focus();
    receiver.await.unwrap_or(false)
}

fn url_input_frame(entry: &gtk::Entry) -> gtk::Overlay {
    let frame = gtk::Overlay::new();
    frame.add_css_class("look4-url-frame");
    frame.set_child(Some(&visuals::rgb_outline()));
    entry.set_halign(gtk::Align::Fill);
    entry.set_valign(gtk::Align::Fill);
    entry.set_margin_top(5);
    entry.set_margin_bottom(5);
    entry.set_margin_start(5);
    entry.set_margin_end(5);
    frame.add_overlay(entry);
    frame
}

#[allow(clippy::too_many_arguments)]
fn start_scan(
    url: String,
    api_key: String,
    reveal: bool,
    window: gtk::ApplicationWindow,
    stack: gtk::Stack,
    status: gtk::Label,
    result: ResultView,
    back_button: gtk::Button,
) {
    status.set_text("Consultando o VirusTotal…");
    result.report.set_visible(false);

    glib::MainContext::default().spawn_local(async move {
        let lookup_started = std::time::Instant::now();
        let url_for_lookup = url.clone();
        let key_for_lookup = api_key.clone();
        let lookup = virustotal::run_in_background(move || {
            virustotal::lookup(&url_for_lookup, &key_for_lookup)
        })
        .await;
        let remaining = std::time::Duration::from_millis(1500)
            .saturating_sub(lookup_started.elapsed());
        if !remaining.is_zero() {
            glib::timeout_future(remaining).await;
        }

        match lookup {
            Ok(virustotal::Lookup::Report { report, report_url }) => {
                if reveal {
                    window.present();
                }
                show_result(&stack, &status, &result, &url, &report, &report_url);
            }
            Ok(virustotal::Lookup::NeedsSubmission { report_url }) => {
                if reveal {
                    window.present();
                }
                status.set_text("O VirusTotal ainda não conhece este link. Aguardando sua confirmação…");
                let accepted = confirm(
                    &window,
                    "Link sem relatório",
                    "O VirusTotal pode adicionar URLs enviadas à sua base pública. Deseja enviar este link para análise?",
                    "Enviar para análise",
                )
                .await;
                if !accepted {
                    stack.set_visible_child_name("input");
                    return;
                }

                status.set_text("URL enviada. Aguardando resultado da análise…");
                let url_for_scan = url.clone();
                let key_for_scan = api_key.clone();
                let submitted = virustotal::run_in_background(move || {
                    virustotal::submit_and_poll(&url_for_scan, &key_for_scan, &report_url)
                })
                .await;
                match submitted {
                    Ok((report, report_url)) => {
                        window.present();
                        show_result(&stack, &status, &result, &url, &report, &report_url);
                    }
                    Err(error) => {
                        window.present();
                        status.set_text(&error);
                        back_button.set_visible(true);
                    }
                }
            }
            Err(error) => {
                if reveal {
                    window.present();
                }
                status.set_text(&error);
                back_button.set_visible(true);
            }
        }
    });
}

fn show_result(
    stack: &gtk::Stack,
    status: &gtk::Label,
    result: &ResultView,
    url: &str,
    report: &virustotal::Report,
    report_url: &str,
) {
    use virustotal::Verdict;

    status.set_text("Consulta concluída.");
    let flagged = report.malicious + report.suspicious;
    let total = report.total();

    let (class, kind, heading, subtitle) = match report.verdict {
        Verdict::Safe => (
            "look4-safe",
            0_u8,
            "Nenhuma ameaça detectada".to_string(),
            format!("Nenhum dos {total} mecanismos apontou risco neste link."),
        ),
        Verdict::Suspicious => (
            "look4-warn",
            1,
            "Resultado suspeito".to_string(),
            format!(
                "{flagged} mecanismo{} marcou{} este link como suspeito. Prossiga com cautela.",
                if flagged == 1 { "" } else { "s" },
                if flagged == 1 { "" } else { "aram" }
            ),
        ),
        Verdict::Malicious => (
            "look4-alert",
            2,
            "Ameaça detectada".to_string(),
            format!(
                "{flagged} mecanismo{} detectou{} ameaça neste link. Não abra este endereço.",
                if flagged == 1 { "" } else { "s" },
                if flagged == 1 { "" } else { "aram" }
            ),
        ),
    };
    for existing in ["look4-safe", "look4-warn", "look4-alert"] {
        result.card.remove_css_class(existing);
    }
    result.card.add_css_class(class);
    result.verdict.set_text(&heading);
    result.subtitle.set_text(&subtitle);
    result.url_chip.set_text(url);
    result.n_malicious.set_text(&report.malicious.to_string());
    result.n_suspicious.set_text(&report.suspicious.to_string());
    result.n_harmless.set_text(&report.harmless.to_string());
    result.n_undetected.set_text(&report.undetected.to_string());

    let mut meta = format!("{total} mecanismos consultados");
    match &report.date {
        Some(date) => meta.push_str(&format!(" · analisado em {date}")),
        None => meta.push_str(" · análise recém-concluída"),
    }
    if report.timeout > 0 {
        meta.push_str(&format!(" · {} timeout", report.timeout));
    }
    result.meta.set_text(&meta);

    result.chart.set(visuals::ChartData {
        malicious: report.malicious,
        suspicious: report.suspicious,
        harmless: report.harmless,
        undetected: report.undetected,
        timeout: report.timeout,
        kind,
        started: Some(std::time::Instant::now()),
    });
    result.gauge.queue_draw();
    result.bar.queue_draw();

    result.report.set_uri(report_url);
    result.report.set_visible(true);
    stack.set_visible_child_name("result");
}

fn start_trace(
    url: String,
    reveal: bool,
    window: gtk::ApplicationWindow,
    stack: gtk::Stack,
    status: gtk::Label,
    result: RedirectResultView,
    back_button: gtk::Button,
) {
    status.set_text("Rastreando redirecionamentos do link…");

    glib::MainContext::default().spawn_local(async move {
        let lookup_started = std::time::Instant::now();
        let url_for_trace = url.clone();
        let trace_op = redirect::run_in_background(move || redirect::trace(&url_for_trace)).await;

        let remaining =
            std::time::Duration::from_millis(1000).saturating_sub(lookup_started.elapsed());
        if !remaining.is_zero() {
            glib::timeout_future(remaining).await;
        }

        match trace_op {
            Ok(trace_result) => {
                if reveal {
                    window.present();
                }
                show_redirect_result(&stack, &status, &result, &trace_result);
            }
            Err(error) => {
                if reveal {
                    window.present();
                }
                status.set_text(&error);
                back_button.set_visible(true);
            }
        }
    });
}

fn show_redirect_result(
    stack: &gtk::Stack,
    status: &gtk::Label,
    result: &RedirectResultView,
    trace: &redirect::TraceResult,
) {
    status.set_text("Rastreamento concluído.");

    let is_direct = trace.total_redirects == 0;
    let (class, title, sub) = if is_direct {
        (
            "look4-safe",
            "Link direto (Sem redirecionamentos)".to_string(),
            "Este link aponta diretamente para o destino sem intermediários.".to_string(),
        )
    } else if !trace.is_same_domain {
        (
            "look4-warn",
            format!(
                "{} redirecionamento{} (Domínio externo)",
                trace.total_redirects,
                if trace.total_redirects == 1 { "" } else { "s" },
            ),
            format!(
                "O link redireciona para um domínio diferente do original ({} salto{}).",
                trace.total_redirects,
                if trace.total_redirects == 1 { "" } else { "s" }
            ),
        )
    } else {
        (
            "look4-safe",
            format!(
                "{} redirecionamento{} identificado{}",
                trace.total_redirects,
                if trace.total_redirects == 1 { "" } else { "s" },
                if trace.total_redirects == 1 { "" } else { "s" }
            ),
            format!(
                "O link passou por {} salto{} no mesmo domínio.",
                trace.total_redirects,
                if trace.total_redirects == 1 { "" } else { "s" }
            ),
        )
    };

    for existing in ["look4-safe", "look4-warn", "look4-alert"] {
        result.card.remove_css_class(existing);
    }
    result.card.add_css_class(class);
    result.verdict.set_text(&title);
    result.subtitle.set_text(&sub);
    result
        .url_chip
        .set_text(&format!("Origem: {}", trace.initial_url));
    result.dest_url.set_text(&trace.final_url);
    result.open_dest_button.set_uri(&trace.final_url);

    result
        .n_hops
        .set_text(&format!("{}", trace.total_redirects));
    result
        .status_code
        .set_text(&format!("{}", trace.final_status));
    result
        .status_code
        .set_tooltip_text(Some(&trace.final_reason));

    if let Ok(u) = url::Url::parse(&trace.final_url) {
        result.proto.set_text(u.scheme().to_uppercase().as_str());
    } else {
        result.proto.set_text("—");
    }

    if !trace.is_same_domain && trace.total_redirects > 0 {
        result.dest_tag.set_text("DESTINO FINAL · DOMÍNIO EXTERNO");
    } else {
        result.dest_tag.set_text("DESTINO FINAL");
    }

    result.meta.set_text(&format!(
        "Rastreamento concluído · {} salto{} analisado{}",
        trace.hops.len(),
        if trace.hops.len() == 1 { "" } else { "s" },
        if trace.hops.len() == 1 { "" } else { "s" }
    ));

    // Clear previous hop rows
    while let Some(child) = result.hops_box.first_child() {
        result.hops_box.remove(&child);
    }

    if trace.hops.len() <= 1 {
        result.hops_title.set_visible(false);
        result.hops_scrolled.set_visible(false);
    } else {
        result.hops_title.set_visible(true);
        result.hops_scrolled.set_visible(true);

        for (i, hop) in trace.hops.iter().enumerate() {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.add_css_class("look4-hop-row");

            let num = gtk::Label::new(Some(&format!("{}.", i + 1)));
            num.add_css_class("look4-hop-num");

            let badge = gtk::Label::new(Some(&format!("{}", hop.status)));
            badge.add_css_class("look4-hop-badge");
            if hop.status >= 300 && hop.status < 400 {
                badge.add_css_class("look4-hop-redirect");
            } else if hop.status >= 200 && hop.status < 300 {
                badge.add_css_class("look4-hop-ok");
            } else {
                badge.add_css_class("look4-hop-error");
            }

            let url_lbl = gtk::Label::new(Some(&hop.url));
            url_lbl.add_css_class("look4-hop-url");
            url_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            url_lbl.set_max_width_chars(28);
            url_lbl.set_halign(gtk::Align::Start);
            url_lbl.set_hexpand(true);
            url_lbl.set_selectable(true);

            row.append(&num);
            row.append(&badge);
            row.append(&url_lbl);
            result.hops_box.append(&row);

            if i + 1 < trace.hops.len() {
                let arrow_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                let arrow = gtk::Label::new(Some("  ↓"));
                arrow.add_css_class("look4-hop-arrow");
                let reason_lbl = gtk::Label::new(Some(&hop.reason));
                reason_lbl.add_css_class("look4-subtle");
                arrow_row.append(&arrow);
                arrow_row.append(&reason_lbl);
                result.hops_box.append(&arrow_row);
            }
        }
    }

    stack.set_visible_child_name("redirect-result");
}
