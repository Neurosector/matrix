//! Matrix Mitteilungen — App #19, der Benachrichtigungs-Daemon (Runde 2).
//!
//! Das UNUserNotificationCenter-Extrakt: EIN Systemdienst nimmt die
//! Mitteilungen aller Programme entgegen (org.freedesktop.Notifications
//! über DBus) und zeigt sie als Kit-Pillen oben rechts. Runde 2 bringt:
//!   • Verlauf — nichts geht verloren, die Glocke in der Bar öffnet ihn
//!   • Nicht stören — Popups schweigen, der Verlauf sammelt weiter
//!   • Aktionsknöpfe — Notify-actions werden Knöpfe; Klick sendet das
//!     ActionInvoked-Signal zurück (der App-Kontrakt von freedesktop)
//!
//! Der DBus-Teil läuft blockierend im Nebenthread; ein Rück-Kanal
//! (Befehl) lässt die iced-Fläche Signale emittieren.

use std::sync::mpsc;

use iced::widget::{button, column, container, image, mouse_area, row, Space};
use iced::{Color, Element, Font, Length, Subscription, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use matrixkit_widgets::{color, tick};

const BREITE: u32 = 380;
const STANDZEIT_MS: u128 = 5000;
const VERLAUF_MAX: usize = 40;

#[derive(Debug, Clone)]
struct Mitteilung {
    id: u32,
    app: String,
    titel: String,
    text: String,
    /// (Schlüssel, Beschriftung) — „default" wird nicht als Knopf gezeigt.
    aktionen: Vec<(String, String)>,
}

enum Ereignis {
    Neu(Mitteilung),
    Weg(u32),
}

/// Rück-Kanal: die App bittet den DBus-Thread, Signale zu emittieren.
enum Befehl {
    Aktion(u32, String),
    Geschlossen(u32, u32),
}

fn zeige_pfad() -> String {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| String::from("/tmp"));
    format!("{dir}/matrix-mitteilungen-zeige")
}

fn dnd_an() -> bool {
    mk::einstellung::lesen("nicht-stoeren").as_deref() == Some("an")
}

// ------------------------------------------------------------- DBus

struct Dienst {
    tx: mpsc::Sender<Ereignis>,
    naechste_id: std::sync::atomic::AtomicU32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Dienst {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        _hints: std::collections::HashMap<String, zbus::zvariant::Value<'_>>,
        _expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            self.naechste_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        };
        // actions ist flach [key1, label1, key2, label2, …] → Paare.
        let aktionen: Vec<(String, String)> = actions
            .chunks(2)
            .filter_map(|c| match c {
                [k, l] => Some((k.clone(), l.clone())),
                _ => None,
            })
            .collect();
        let _ = self.tx.send(Ereignis::Neu(Mitteilung {
            id,
            app: app_name,
            titel: summary,
            text: body,
            aktionen,
        }));
        id
    }

    fn close_notification(&self, id: u32) {
        let _ = self.tx.send(Ereignis::Weg(id));
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec![
            String::from("body"),
            String::from("actions"),
            String::from("persistence"),
        ]
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            String::from("Matrix Mitteilungen"),
            String::from("Matrix"),
            String::from(env!("CARGO_PKG_VERSION")),
            String::from("1.2"),
        )
    }
}

fn dienst_starten(tx: mpsc::Sender<Ereignis>, befehl_rx: mpsc::Receiver<Befehl>) {
    std::thread::spawn(move || {
        let mach = || -> zbus::Result<zbus::blocking::Connection> {
            let conn = zbus::blocking::connection::Builder::session()?
                .serve_at(
                    "/org/freedesktop/Notifications",
                    Dienst {
                        tx: tx.clone(),
                        naechste_id: std::sync::atomic::AtomicU32::new(1),
                    },
                )?
                .build()?;
            use zbus::fdo::RequestNameFlags;
            let dbus = zbus::blocking::fdo::DBusProxy::new(&conn)?;
            let _ = dbus.request_name(
                "org.freedesktop.Notifications".try_into().unwrap(),
                RequestNameFlags::AllowReplacement | RequestNameFlags::ReplaceExisting,
            );
            Ok(conn)
        };
        match mach() {
            Ok(conn) => loop {
                // Rück-Kanal: Signale emittieren (ActionInvoked/Closed).
                while let Ok(b) = befehl_rx.try_recv() {
                    let _ = match b {
                        Befehl::Aktion(id, key) => conn.emit_signal(
                            None::<()>,
                            "/org/freedesktop/Notifications",
                            "org.freedesktop.Notifications",
                            "ActionInvoked",
                            &(id, key),
                        ),
                        Befehl::Geschlossen(id, grund) => conn.emit_signal(
                            None::<()>,
                            "/org/freedesktop/Notifications",
                            "org.freedesktop.Notifications",
                            "NotificationClosed",
                            &(id, grund),
                        ),
                    };
                }
                std::thread::sleep(std::time::Duration::from_millis(80));
            },
            Err(e) => eprintln!("matrix-mitteilungen: DBus-Start scheiterte: {e}"),
        }
    });
}

// ----------------------------------------------------------------- App

fn main() -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};
    if !mkw::leiste_toggle() {
        return Ok(());
    }
    iced_layershell::application(
        App::new,
        || String::from("matrix-mitteilung"),
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
            size: Some((1, 1)),
            anchor: Anchor::Top | Anchor::Right,
            margin: (52, 8, 0, 0),
            layer: Layer::Overlay,
            keyboard_interactivity: KeyboardInteractivity::None,
            exclusive_zone: 0,
            ..Default::default()
        },
        default_font: Font::with_name("Inter Variable"),
        fonts: mkw::symbol_font_laden().into_iter().collect(),
        ..Default::default()
    })
    .run()
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Msg {
    Puls,
    Klick(u32),
    Aktion(u32, String),
    VerlaufZu,
    AlleLoeschen,
    DndUmschalten,
}

struct App {
    palette: mk::Palette,
    watcher: mk::PaletteWatcher,
    rx: mpsc::Receiver<Ereignis>,
    befehl: mpsc::Sender<Befehl>,
    /// Aktive Popups (Mitteilung + Geburtszeit).
    stapel: Vec<(Mitteilung, std::time::Instant)>,
    /// Verlauf — jüngste zuerst.
    verlauf: Vec<Mitteilung>,
    verlauf_offen: bool,
    dnd: bool,
    icons: std::collections::HashMap<String, Option<image::Handle>>,
    letzte_zeige: Option<std::time::SystemTime>,
    letzte_hoehe: u32,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let (tx, rx) = mpsc::channel();
        let (btx, brx) = mpsc::channel();
        dienst_starten(tx, brx);
        (
            App {
                palette: mk::Palette::load().unwrap_or_default(),
                watcher: mk::PaletteWatcher::new(),
                rx,
                befehl: btx,
                stapel: Vec::new(),
                verlauf: Vec::new(),
                verlauf_offen: false,
                dnd: dnd_an(),
                icons: std::collections::HashMap::new(),
                letzte_zeige: std::fs::metadata(zeige_pfad()).and_then(|m| m.modified()).ok(),
                letzte_hoehe: 1,
            },
            Task::none(),
        )
    }

    fn icon_backen(&mut self, app: &str) {
        let p = self.palette;
        self.icons.entry(app.to_string()).or_insert_with(|| {
            matrixkit_icons::render_png(app, &p).map(image::Handle::from_bytes)
        });
    }

    fn mass(&mut self) -> Task<Msg> {
        let (breite, hoehe) = if self.verlauf_offen {
            (BREITE, 44 + (self.verlauf.len().max(1) as u32) * 84 + 16)
        } else if !self.stapel.is_empty() {
            (BREITE, (self.stapel.len() as u32) * 108 + 8)
        } else {
            (1, 1)
        };
        if hoehe != self.letzte_hoehe {
            self.letzte_hoehe = hoehe;
            return Task::done(Msg::SizeChange((breite, hoehe.min(1400))));
        }
        Task::none()
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Puls => {
                if self.watcher.changed() {
                    if let Some(neu) = mk::Palette::load() {
                        self.palette = neu;
                    }
                    if !mk::uebergang::aktiv() {
                        self.icons.clear();
                    }
                }
                self.dnd = dnd_an();
                // Glocke: Verlaufs-Panel per Datei-Toggle öffnen/schließen.
                let zeige = std::fs::metadata(zeige_pfad()).and_then(|m| m.modified()).ok();
                if zeige.is_some() && zeige != self.letzte_zeige {
                    self.letzte_zeige = zeige;
                    self.verlauf_offen = !self.verlauf_offen;
                }
                while let Ok(e) = self.rx.try_recv() {
                    match e {
                        Ereignis::Neu(m) => {
                            self.icon_backen(&m.app);
                            self.verlauf.retain(|v| v.id != m.id);
                            self.verlauf.insert(0, m.clone());
                            self.verlauf.truncate(VERLAUF_MAX);
                            // Nicht stören: nur der Verlauf, kein Popup.
                            if !self.dnd {
                                // Nutzer-Fund (15.7.): niris Screenshot-
                                // Meldung ist ein AUSLÖSER, keine Post —
                                // sie klingt als 09, nicht als Hinweis.
                                let foto = m.titel.contains("creenshot")
                                    || m.titel.contains("ildschirmfoto")
                                    || m.app.contains("niri");
                                self.stapel.retain(|(alt, _)| alt.id != m.id);
                                self.stapel.push((m, std::time::Instant::now()));
                                if foto {
                                    mk::feedback::jetzt("screenshot", "09-screenshot.wav");
                                } else {
                                    mk::feedback::hinweis();
                                }
                            }
                        }
                        Ereignis::Weg(id) => self.stapel.retain(|(m, _)| m.id != id),
                    }
                }
                self.stapel
                    .retain(|(_, seit)| seit.elapsed().as_millis() < STANDZEIT_MS);
                return self.mass();
            }
            Msg::Klick(id) => {
                self.stapel.retain(|(m, _)| m.id != id);
                let _ = self.befehl.send(Befehl::Geschlossen(id, 2)); // dismissed
                return self.mass();
            }
            Msg::Aktion(id, key) => {
                let _ = self.befehl.send(Befehl::Aktion(id, key));
                let _ = self.befehl.send(Befehl::Geschlossen(id, 2));
                self.stapel.retain(|(m, _)| m.id != id);
                return self.mass();
            }
            Msg::VerlaufZu => {
                self.verlauf_offen = false;
                return self.mass();
            }
            Msg::AlleLoeschen => {
                self.verlauf.clear();
                return self.mass();
            }
            Msg::DndUmschalten => {
                self.dnd = !self.dnd;
                mk::einstellung::schreiben("nicht-stoeren", if self.dnd { "an" } else { "aus" });
                if self.dnd {
                    self.stapel.clear();
                }
                return self.mass();
            }
            _ => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        tick("mitteilungen", std::time::Duration::from_millis(120)).map(|_| Msg::Puls)
    }

    /// Ein App-Icon (lebend) oder Initial-Kachel.
    fn icon<'a>(&'a self, app: &str, kante: f32) -> Element<'a, Msg> {
        let p = self.palette;
        match self.icons.get(app).cloned().flatten() {
            Some(h) => image(h).width(kante).height(kante).into(),
            None => container(mkw::txt(
                app.chars().next().unwrap_or('•').to_uppercase().to_string(),
                mk::typo::KOPF,
                p.on_primary_container,
            ))
            .center_x(Length::Fixed(kante))
            .center_y(Length::Fixed(kante))
            .style(move |_| container::Style {
                background: Some(color(p.primary_container).into()),
                border: iced::border::rounded(mk::radius::KLEIN + 2.0),
                ..Default::default()
            })
            .into(),
        }
    }

    fn view(&self) -> Element<'_, Msg> {
        if self.verlauf_offen {
            self.verlauf_ansicht()
        } else {
            self.stapel_ansicht()
        }
    }

    fn stapel_ansicht(&self) -> Element<'_, Msg> {
        let _p = self.palette;
        if self.stapel.is_empty() {
            return Space::new().into();
        }
        let mut spalte = column![].spacing(mk::spacing::S);
        for (m, _) in &self.stapel {
            spalte = spalte.push(self.pille(m, true));
        }
        container(spalte)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .into()
    }

    /// Eine Mitteilungs-Pille. `mit_aktionen` = Popup (Knöpfe zeigen).
    fn pille<'a>(&'a self, m: &'a Mitteilung, mit_aktionen: bool) -> Element<'a, Msg> {
        let p = self.palette;
        let mut text_spalte = column![mkw::txt(&m.titel, mk::typo::KOPF, p.on_surface)];
        if !m.text.is_empty() {
            let kurz: String = m.text.chars().take(140).collect();
            text_spalte = text_spalte.push(mkw::txt(kurz, mk::typo::KLEIN, p.on_surface_variant));
        }
        // Aktionsknöpfe (ohne „default").
        if mit_aktionen {
            let mut knopfreihe = row![].spacing(mk::spacing::XS);
            let mut hat = false;
            for (key, label) in &m.aktionen {
                if key == "default" {
                    continue;
                }
                hat = true;
                let id = m.id;
                let k = key.clone();
                knopfreihe = knopfreihe.push(
                    button(mkw::txt(label, mk::typo::KLEIN, p.on_surface))
                        .padding([2, mk::spacing::S as u16])
                        .style(move |_, s| mkw::leiste::knopf_stil(p, s, mk::radius::KLEIN))
                        .on_press(Msg::Aktion(id, k.clone())),
                );
            }
            if hat {
                text_spalte = text_spalte
                    .push(Space::new().height(mk::spacing::XXS))
                    .push(knopfreihe);
            }
        }
        let inhalt = row![self.icon(&m.app, 36.0), text_spalte]
            .spacing(mk::spacing::M)
            .align_y(iced::Alignment::Center);
        let flaeche = mkw::ui::panel_huelle(inhalt.into(), BREITE as f32 - 8.0, p);
        mouse_area(flaeche).on_press(Msg::Klick(m.id)).into()
    }

    fn verlauf_ansicht(&self) -> Element<'_, Msg> {
        let p = self.palette;
        // MatrixUI MenuFamily: Panel-Kopf in der EINEN Sprache.
        let dnd_zeichen = if self.dnd { mkw::symbol::VOLUME_OFF } else { mkw::symbol::NOTIFICATIONS };
        let kopf = mkw::ui::panel_kopf(
            "Mitteilungen",
            vec![
                mkw::ui::nav_knopf(dnd_zeichen, true, Msg::DndUmschalten, p),
                mkw::ui::kopf_text_knopf("Leeren", Msg::AlleLoeschen, p),
                mkw::ui::nav_knopf(mkw::symbol::CLOSE, true, Msg::VerlaufZu, p),
            ],
            p,
        );
        let mut liste = column![kopf].spacing(mk::spacing::XS);
        if self.verlauf.is_empty() {
            liste = liste.push(
                container(mkw::txt("Keine Mitteilungen", mk::typo::HINWEIS, p.on_surface_variant))
                    .center_x(Length::Fill)
                    .padding(mk::spacing::L),
            );
        } else {
            for m in &self.verlauf {
                liste = liste.push(self.pille(m, false));
            }
        }
        mkw::ui::panel_huelle(liste.into(), BREITE as f32, p)
    }
}
