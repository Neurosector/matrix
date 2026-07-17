//! Matrix Systemmonitor — die erste MatrixKit-App.
//!
//! Beweisziel: Design-Tokens + Live-Palette (Wallpaper-reaktiv!) in purem Rust.
//! Eine Karte im DMS-Look: CPU, RAM, Datentraeger — aktualisiert alle 2 s,
//! Farben wechseln live mit dem System-Theme.

use iced::widget::{column, container, row, text, Space};
use iced::{Color, Element, Font, Length, Subscription, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mk::motion::Spring;
use mkw::{color, tick};
use std::time::Duration;
use sysinfo::{Disks, ProcessesToUpdate, System};

/// CPU-Verlaufsfenster: 60 Messwerte im 2-s-Takt = die letzten 2 Minuten.
const VERLAUF_LEN: usize = 60;

#[derive(Clone, Copy, Debug, PartialEq)]
enum WidgetSize {
    S, // 160x160 — nur CPU
    M, // 336x160 — drei Metriken kompakt
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let widget_mode = args.iter().any(|a| a == "--widget");
    // Mailbox nur im FENSTER-Modus (fluessiges Live-Resize). Das Widget
    // rendert nur bei Zustandsaenderung — dort reicht der Vsync-Standard.
    if !widget_mode && std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    if !widget_mode && !mk::fenster::einzelinstanz("matrix-sysmon") {
        return Ok(());
    }
    if widget_mode {
        let size = if args.iter().any(|a| a == "--size=s" || a == "-s") {
            WidgetSize::S
        } else {
            WidgetSize::M
        };
        widget_main(size)?;
        return Ok(());
    }
    window_main()?;
    Ok(())
}

/// Desktop-Widget: Layer-Shell-Flaeche, verankert oben rechts, UNTER den Fenstern.
fn widget_main(size: WidgetSize) -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};
    let (w, h) = match size {
        WidgetSize::S => (160, 160),
        WidgetSize::M => (336, 160),
    };
    iced_layershell::application(
        move || App::new_widget(size),
        || String::from("matrix-widget"),
        App::update,
        App::view,
    )
    .subscription(App::subscription)
    .style(|_state, _theme| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .settings(Settings {
        layer_settings: LayerShellSettings {
            size: Some((w, h)),
            anchor: Anchor::Top | Anchor::Right,
            margin: (48, 24, 0, 0),
            layer: Layer::Bottom,
            keyboard_interactivity: KeyboardInteractivity::None,
            exclusive_zone: 0,
            ..Default::default()
        },
        default_font: Font::with_name("Inter Variable"),
        ..Default::default()
    })
    .run()
}

fn window_main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .window(iced::window::Settings {
            // Mindestgroesse (Nutzer-Direktive) bleibt; gemerkte Groesse gewinnt
            size: {
                let (b, h) = mk::fenster::groesse_lesen("matrix-sysmon")
                    .map(|(gb, gh)| (gb.max(380.0), gh.max(384.0)))
                    .unwrap_or((380.0, 384.0));
                iced::Size::new(b, h)
            },
            min_size: Some(iced::Size::new(380.0, 384.0)),
            decorations: false, // eigener Header statt grauer winit-Notloesung
            platform_specific: iced::window::settings::PlatformSpecific {
                // Wayland-App-ID = Name der .desktop-Datei — nur so ordnen
                // Dock & Launcher dem Fenster das richtige (lebende) Icon zu.
                application_id: String::from("matrix-sysmon"),
                ..Default::default()
            },
            ..Default::default()
        })
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

struct App {
    sys: System,
    net: sysinfo::Networks,
    /// Bytes/s der letzten Messperiode (Down, Up)
    net_down: f64,
    net_up: f64,
    net_at: std::time::Instant,
    /// CPU-Verlauf (0..1), aeltester Wert vorn — gezeichnet als Sparkline.
    verlauf: std::collections::VecDeque<f32>,
    palette: mk::Palette,
    watcher: mk::PaletteWatcher,
    cpu: f32,
    mem_used: u64,
    mem_total: u64,
    disk_used: u64,
    disk_total: u64,
    /// Gefederte Anzeigewerte (0..1) — die Balken FEDERN zu neuen Werten.
    springs: [Spring; 3],
    /// Some(...) = Desktop-Widget-Modus (Groessenfamilie), None = App-Fenster.
    widget: Option<WidgetSize>,
    root: mkw::RootZustand,
    fenster: mkw::FensterZustand,
    /// Lebendes App-Icon fuer das "Ueber"-Panel (live gerendert).
    icon: Option<iced::widget::image::Handle>,
    rechte: mk::rechte::Berechtigungen,
    /// Prozess-Tabelle (Leitbild Table, Runde 12): (Name, CPU %, RAM Bytes).
    prozesse: Vec<(String, f32, u64)>,
    /// Sortierung wie Leitbild-UI sortOrder: aktive Spalte + Richtung.
    sort_spalte: usize,
    sort_ab: bool,
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Msg {
    Tick,
    Aktiv(bool),
    AmpelnHover(bool),
    AnimTick,
    Taste(mkw::Taste),
    Ablage,
    Maximieren,
    Groesse(iced::Size),
    DragWindow,
    CloseWindow,
    Resize(iced::window::Direction),
    OpenApp,
    RootUmschalten,
    Recht(mk::rechte::Recht, bool),
    RootPasswort(String),
    RootEntsperren,
    RootEntsperrt(bool),
    Hilfe,
    /// Tabellen-Kopf geklickt: nach Spalte i sortieren.
    Sortieren(usize),
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let mut sys = System::new_all();
        sys.refresh_all();
        let start_palette = mk::Palette::load().unwrap_or_default();
        let mut app = Self {
            icon: matrixkit_icons::render_png("matrix-sysmon", &start_palette)
                .map(iced::widget::image::Handle::from_bytes),
            sys,
            net: sysinfo::Networks::new_with_refreshed_list(),
            net_down: 0.0,
            net_up: 0.0,
            net_at: std::time::Instant::now(),
            verlauf: std::collections::VecDeque::with_capacity(VERLAUF_LEN),
            palette: mk::Palette::load().unwrap_or_default(),
            watcher: mk::PaletteWatcher::new(),
            cpu: 0.0,
            mem_used: 0,
            mem_total: 0,
            disk_used: 0,
            disk_total: 0,
            springs: [Spring::new(0.0), Spring::new(0.0), Spring::new(0.0)],
            widget: None,
            root: mkw::RootZustand::neu(),
            fenster: mkw::FensterZustand::neu(),
            rechte: mk::rechte::Berechtigungen::laden("matrix-sysmon"),
            prozesse: Vec::new(),
            sort_spalte: 1, // CPU zuerst — wie die Aktivitätsanzeige
            sort_ab: true,
        };
        app.refresh();
        app.springs[0].retarget((app.cpu / 100.0).clamp(0.0, 1.0));
        app.springs[1]
            .retarget((app.mem_used as f32 / app.mem_total.max(1) as f32).clamp(0.0, 1.0));
        app.springs[2]
            .retarget((app.disk_used as f32 / app.disk_total.max(1) as f32).clamp(0.0, 1.0));
        (app, Task::none())
    }

    fn new_widget(size: WidgetSize) -> Self {
        let (mut app, _) = Self::new();
        app.widget = Some(size);
        app
    }

    fn title(&self) -> String {
        "Matrix Monitor".into()
    }

    fn refresh(&mut self) {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.cpu = self.sys.global_cpu_usage();
        self.mem_used = self.sys.used_memory();
        self.mem_total = self.sys.total_memory();
        // Auf bootc/ostree ist "/" ein Overlay — den echten Datenträger über
        // die bekannten Mounts suchen, sonst den größten nehmen.
        let disks = Disks::new_with_refreshed_list();
        let by_mount = |m: &str| {
            disks.list().iter().find(|d| d.mount_point().to_str() == Some(m))
        };
        let chosen = by_mount("/sysroot")
            .or_else(|| by_mount("/var/home"))
            .or_else(|| by_mount("/"))
            .or_else(|| disks.list().iter().max_by_key(|d| d.total_space()));
        if let Some(d) = chosen {
            self.disk_total = d.total_space();
            self.disk_used = d.total_space() - d.available_space();
        }
        // Prozess-Tabelle: alle Prozesse, sortiert wird in view()
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.prozesse = self
            .sys
            .processes()
            .values()
            .map(|pr| {
                (
                    pr.name().to_string_lossy().to_string(),
                    pr.cpu_usage(),
                    pr.memory(),
                )
            })
            .collect();
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::AnimTick => {
                for s in &mut self.springs {
                    s.tick(1.0 / 60.0);
                }
                self.root.tick();
                self.fenster.tick();
                Task::none()
            }
            Msg::Taste(t) => {
                match t {
                    mkw::Taste::Escape => self.root.schliessen(),
                    mkw::Taste::Weiter if self.root.offen() => self.root.fokus.weiter(),
                    mkw::Taste::Zurueck if self.root.offen() => self.root.fokus.zurueck(),
                    mkw::Taste::Aktivieren if self.root.offen() => {
                        match self.root.fokus.aktuell() {
                            Some(0) if self.root.entsperrt => {
                                let r = mk::rechte::Recht::Netzwerk;
                                let neu = !self.rechte.erlaubt(r);
                                self.rechte.setzen(r, neu);
                            }
                            Some(1) => self.root.schliessen(),
                            _ => {}
                        }
                    }
                    mkw::Taste::Einstellungen => mkw::einstellungen_oeffnen(),
                    // Strg+R (Leitbild refreshable): sofort neu messen.
                    mkw::Taste::Aktualisieren => self.refresh(),
                    _ => {}
                }
                Task::none()
            }
            Msg::Aktiv(a) => {
                self.fenster.aktiv = a;
                Task::none()
            }
            Msg::AmpelnHover(h) => {
                self.fenster.ampeln_hover = h;
                Task::none()
            }
            Msg::Tick => {
                self.refresh();
                // Netzrate — BINDEND: ohne Netzwerk-Recht fasst der Rahmen
                // die Schnittstellen gar nicht erst an.
                if self.rechte.erlaubt(mk::rechte::Recht::Netzwerk) {
                    let dt = self.net_at.elapsed().as_secs_f64().max(0.1);
                    self.net_at = std::time::Instant::now();
                    self.net.refresh();
                    let (mut rx, mut tx) = (0u64, 0u64);
                    for (_, data) in self.net.iter() {
                        rx += data.received();
                        tx += data.transmitted();
                    }
                    self.net_down = rx as f64 / dt;
                    self.net_up = tx as f64 / dt;
                } else {
                    self.net_down = -1.0;
                    self.net_up = -1.0;
                }
                // CPU-Verlauf fortschreiben (2-s-Raster, ~2 Minuten Fenster)
                if self.verlauf.len() == VERLAUF_LEN {
                    self.verlauf.pop_front();
                }
                self.verlauf.push_back((self.cpu / 100.0).clamp(0.0, 1.0));
                // Neue Zielwerte — die Federn nehmen von hier uebernommen Fahrt auf
                self.springs[0].retarget((self.cpu / 100.0).clamp(0.0, 1.0));
                self.springs[1].retarget(
                    (self.mem_used as f32 / self.mem_total.max(1) as f32).clamp(0.0, 1.0),
                );
                self.springs[2].retarget(
                    (self.disk_used as f32 / self.disk_total.max(1) as f32).clamp(0.0, 1.0),
                );
                // Live-Palette: folgt Wallpaper-Wechsel & Hell/Dunkel sofort
                if self.watcher.changed() {
                    if let Some(p) = mk::Palette::load() {
                        self.palette = p;
                        self.icon = matrixkit_icons::render_png("matrix-sysmon", &p)
                            .map(iced::widget::image::Handle::from_bytes);
                    }
                }
                Task::none()
            }
            Msg::Ablage => {
                mk::fenster::ablage();
                Task::none()
            }
            Msg::Maximieren => iced::window::latest().and_then(iced::window::toggle_maximize),
            Msg::Groesse(groesse) => {
                if self.widget.is_none() {
                    mk::fenster::groesse_merken("matrix-sysmon", groesse.width, groesse.height);
                }
                Task::none()
            }
            Msg::DragWindow => iced::window::latest()
                .and_then(iced::window::drag),
            Msg::CloseWindow => iced::window::latest()
                .and_then(iced::window::close),
            // Native Compositor-Geste (xdg_toplevel.resize) — wie DMS/Qt:
            // Niri uebernimmt Cursor, Kanten-Logik und Groessenaenderung selbst.
            Msg::Resize(dir) => iced::window::latest()
                .and_then(move |id| iced::window::drag_resize(id, dir)),
            // Widget-Klick: die Voll-App oeffnen
            Msg::OpenApp => {
                if let Ok(exe) = std::env::current_exe() {
                    if let Ok(mut child) = std::process::Command::new(exe).spawn() {
                        // Kind im Hintergrund einsammeln — sonst bleibt ein Zombie
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                }
                Task::none()
            }
            Msg::RootUmschalten => {
                self.root.umschalten();
                self.root.fokus.setze_anzahl(2); // Netzwerk + Fertig
                Task::none()
            }
            Msg::RootPasswort(pw) => {
                self.root.passwort = pw;
                self.root.fehlversuch = false;
                Task::none()
            }
            Msg::RootEntsperren => {
                if self.root.passwort.is_empty() || self.root.pruefung_laeuft {
                    return Task::none();
                }
                self.root.pruefung_laeuft = true;
                let pw = self.root.passwort.clone();
                return Task::perform(
                    async move { mk::rechte::passwort_pruefen(&pw) },
                    Msg::RootEntsperrt,
                );
            }
            Msg::RootEntsperrt(ok) => {
                self.root.entsperr_ergebnis(ok);
                Task::none()
            }
            Msg::Hilfe => {
                mkw::hilfe_oeffnen("Systemmonitor");
                Task::none()
            }
            Msg::Sortieren(i) => {
                if self.sort_spalte == i {
                    self.sort_ab = !self.sort_ab;
                } else {
                    self.sort_spalte = i;
                    // Zahlen-Spalten starten absteigend, Namen aufsteigend
                    self.sort_ab = i != 0;
                }
                Task::none()
            }
            Msg::Recht(r, erlaubt) => {
                // Schloss-Modell: ohne Passwort-Bestätigung keine Änderung
                if !self.root.entsperrt {
                    return Task::none();
                }
                self.rechte.setzen(r, erlaubt);
                Task::none()
            }
            // vom to_layer_message-Makro ergaenzte Varianten (Widget-Steuerung)
            _ => Task::none(),
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        let data = tick("daten", Duration::from_secs(2)).map(|_| Msg::Tick);
        let mut alle = vec![data];
        // Esc schliesst die Root-Ebene (nur Fenster-Modus — das Widget hat
        // keine Tastatur-Interaktivitaet).
        if self.widget.is_none() {
            alle.push(mkw::tasten_abo(Msg::Taste));
            alle.push(mkw::aktiv_abo(Msg::Aktiv));
            alle.push(iced::window::resize_events().map(|(_, g)| Msg::Groesse(g)));
        }
        // Effizienz: 60-fps-Tick NUR solange sich etwas bewegt —
        // im Ruhezustand kostet die App keinerlei Animations-Zyklen.
        if !self.springs.iter().all(|s| s.is_settled()) || self.root.animiert() || self.fenster.animiert() {
            alle.push(tick("anim", Duration::from_millis(16)).map(|_| Msg::AnimTick));
        }
        Subscription::batch(alle)
    }

    fn view(&self) -> Element<'_, Msg> {
        if let Some(size) = self.widget {
            return self.view_widget(size);
        }
        let p = self.palette;


        // Prozess-Tabelle (Leitbild Table + sortOrder): App sortiert selbst,
        // Kopf-Klick wechselt Spalte/Richtung. Top 8 reichen dem Überblick.
        let mut sortiert = self.prozesse.clone();
        match self.sort_spalte {
            0 => sortiert.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase())),
            1 => sortiert.sort_by(|a, b| a.1.total_cmp(&b.1)),
            _ => sortiert.sort_by_key(|e| e.2),
        }
        if self.sort_ab {
            sortiert.reverse();
        }
        let zeilen: Vec<Vec<String>> = sortiert
            .into_iter()
            .take(8)
            .map(|(name, cpu, mem)| {
                // Einzeilig wie die Aktivitätsanzeige: lange Namen enden in …
                let kurz = if name.chars().count() > 22 {
                    let mut k: String = name.chars().take(21).collect();
                    k.push('\u{2026}');
                    k
                } else {
                    name
                };
                vec![
                    kurz,
                    format!("{cpu:.1} %"),
                    mk::format::bytes_speicher(mem),
                ]
            })
            .collect();
        const SPALTEN: [mkw::Spalte; 3] = [
            mkw::Spalte { titel: "Prozess", anteil: 3, rechts: false },
            mkw::Spalte { titel: "CPU", anteil: 1, rechts: true },
            mkw::Spalte { titel: "Speicher", anteil: 2, rechts: true },
        ];

        let card = container(
            column![
                metric_row(
                    "Prozessor",
                    format!("{:.0} %", self.springs[0].value * 100.0),
                    self.springs[0].value,
                    p,
                ),
                Space::new().height(mk::spacing::M),
                metric_row(
                    "Arbeitsspeicher",
                    mk::format::bytes_speicher_paar(
                        (self.mem_total as f64 * self.springs[1].value as f64) as u64,
                        self.mem_total,
                    ),
                    self.springs[1].value,
                    p,
                ),
                Space::new().height(mk::spacing::M),
                metric_row(
                    "Datenträger /",
                    mk::format::bytes_paar(
                        (self.disk_total as f64 * self.springs[2].value as f64) as u64,
                        self.disk_total,
                    ),
                    self.springs[2].value,
                    p,
                ),
                Space::new().height(mk::spacing::M),
                // Netzwerk: Raten sind unbegrenzt — Text statt Balken
                row![
                    mkw::txt("Netzwerk", mk::typo::FLIESS, p.on_surface_variant),
                    Space::new().width(Length::Fill),
                    if self.net_down < 0.0 {
                        Element::from(
                            mkw::txt("Berechtigung aus", mk::typo::FLIESS, p.on_surface_variant),
                        )
                    } else {
                        row![
                            mkw::symbol::<Msg>(mkw::symbol::ARROW_DOWNWARD, mk::font_size::SMALL, p.on_surface_variant),
                            mkw::txt(format!("{}  ", mk::format::rate(self.net_down)), mk::typo::FLIESS, p.on_surface),
                            mkw::symbol::<Msg>(mkw::symbol::ARROW_UPWARD, mk::font_size::SMALL, p.on_surface_variant),
                            mkw::txt(mk::format::rate(self.net_up), mk::typo::FLIESS, p.on_surface),
                        ]
                        .align_y(iced::Alignment::Center)
                        .into()
                    },
                ],
                Space::new().height(mk::spacing::M),
                // CPU-Verlauf der letzten 2 Minuten — ruhige Flaeche + Linie.
                // Beschriftet, damit die Kurve nicht der Netzwerk-Zeile
                // darueber zugeordnet wird (Nutzer-Feedback 5.7.).
                mkw::txt("Prozessor-Verlauf · 2 Minuten", mk::typo::KLEIN, p.on_surface_variant),
                Space::new().height(mk::spacing::XS),
                container(mkw::diagramm::<Msg>(
                    self.verlauf.iter().copied().collect(),
                    VERLAUF_LEN,
                    mkw::DiagrammArt::Linie,
                    p,
                ))
                .width(Length::Fill)
                .height(Length::Fixed(64.0)),
                Space::new().height(mk::spacing::M),
                mkw::txt("Prozesse", mk::typo::KLEIN, p.on_surface_variant),
                Space::new().height(mk::spacing::XS),
                mkw::tabelle(&SPALTEN, zeilen, self.sort_spalte, self.sort_ab, Msg::Sortieren, p),
            ]
            .spacing(0),
        )
        .padding(mk::spacing::L)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(color(p.surface_container).into()),
            border: iced::Border {
                radius: mk::CORNER_RADIUS.into(),
                ..Default::default()
            },
            ..Default::default()
        });

        let root = self.root.sichtbar().then(|| {
            mkw::root_ansicht(
                mkw::RootInfo {
                    name: "Systemmonitor",
                    version: env!("CARGO_PKG_VERSION"),
                    icon: self.icon.clone(),
                    beschreibung: "CPU, Arbeitsspeicher, Datenträger und Netzwerk im Blick — die erste MatrixKit-App.",
                },
                p,
                &self.rechte,
                &[mk::rechte::Recht::Netzwerk],
                Msg::Recht,
                Msg::RootUmschalten,
                &self.root,
                Msg::RootPasswort,
                Msg::RootEntsperren,
                Msg::Hilfe,
            )
        });
        mkw::app_fenster(
            "Systemmonitor",
            p,
            card.into(),
            Msg::DragWindow,
            Msg::CloseWindow,
            Msg::Resize,
            Msg::RootUmschalten,
            Msg::Ablage,
            Msg::Maximieren,
            root,
            &self.fenster,
            Msg::AmpelnHover,
        )
    }
}

impl App {
    /// Desktop-Widget-Ansichten: KEIN Header, KEINE Griffe — nur die Karte.
    /// Jede Groessenfamilie ist ein eigenes Layout (Leitbild-Prinzip), keine Skalierung.
    fn view_widget(&self, size: WidgetSize) -> Element<'_, Msg> {
        let p = self.palette;
        let inner: Element<'_, Msg> = match size {
            WidgetSize::S => column![
                text("CPU")
                    .size(mk::font_size::SMALL)
                    .color(color(p.on_surface_variant)),
                Space::new().height(Length::Fill),
                text(format!("{:.0} %", self.springs[0].value * 100.0))
                    .size(34)
                    .color(color(p.on_surface)),
                Space::new().height(mk::spacing::S),
                progress_bar(
                    self.springs[0].value.clamp(0.0, 1.0),
                    p.primary,
                    p.on_surface.over(p.surface_container, 0.08),
                ),
            ]
            .into(),
            WidgetSize::M => {
                column![
                    metric_row(
                        "Prozessor",
                        format!("{:.0} %", self.springs[0].value * 100.0),
                        self.springs[0].value,
                        p,
                    ),
                    Space::new().height(mk::spacing::S),
                    metric_row(
                        "Arbeitsspeicher",
                        mk::format::bytes_speicher(
                            (self.mem_total as f64 * self.springs[1].value as f64) as u64,
                        ),
                        self.springs[1].value,
                        p,
                    ),
                    Space::new().height(mk::spacing::S),
                    metric_row(
                        "Datenträger",
                        mk::format::bytes(
                            (self.disk_total as f64 * self.springs[2].value as f64) as u64,
                        ),
                        self.springs[2].value,
                        p,
                    ),
                ]
                .into()
            }
        };
        let card = iced::widget::mouse_area(
            container(inner)
                .padding(mk::spacing::L)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container).into()),
                    border: iced::Border {
                        radius: mk::CORNER_RADIUS.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        )
        .on_press(Msg::OpenApp)
        .interaction(iced::mouse::Interaction::Pointer);
        card.into()
    }
}

/// Eine Messwert-Zeile: Label links, Wert rechts, Fortschrittsbalken darunter.
fn metric_row(label: &str, value: String, ratio: f32, p: mk::Palette) -> Element<'_, Msg> {
    let bar_bg = p.on_surface.over(p.surface_container, 0.08);
    column![
        row![
            mkw::txt(label.to_string(), mk::typo::FLIESS, p.on_surface_variant),
            Space::new().width(Length::Fill),
            mkw::txt(value, mk::typo::FLIESS, p.on_surface),
        ],
        Space::new().height(mk::spacing::XS),
        progress_bar(ratio.clamp(0.0, 1.0), p.primary, bar_bg),
    ]
    .into()
}

/// Schlanker Fortschrittsbalken im DMS-Stil (Track + primary-Füllung, Pillenform).
fn progress_bar(ratio: f32, fill: mk::Rgba, track: mk::Rgba) -> Element<'static, Msg> {
    let h = 6.0;
    let fill_units = ((ratio * 1000.0).round() as u16).clamp(1, 999);
    let rest_units = 1000 - fill_units;
    container(row![
        container(Space::new().width(Length::Fill).height(Length::Fixed(h)))
            .width(Length::FillPortion(fill_units))
            .style(move |_| container::Style {
                background: Some(color(fill).into()),
                border: iced::Border { radius: (h / 2.0).into(), ..Default::default() },
                ..Default::default()
            }),
        Space::new().width(Length::FillPortion(rest_units)).height(Length::Fixed(h)),
    ])
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(color(track).into()),
        border: iced::Border { radius: (h / 2.0).into(), ..Default::default() },
        ..Default::default()
    })
    .into()
}





