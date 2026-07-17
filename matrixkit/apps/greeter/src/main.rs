//! Matrix Greeter — App #16, der Login-Screen in MatrixKit.
//!
//! Läuft als Vollbild-Overlay (exklusive Tastatur) in einer eigenen
//! niri-Kiosk-Session unter greetd: Wallpaper aus dem Greeter-Slot mit
//! ruhigem Schleier, der lebende Matrix-Avatar, ein Passwortfeld,
//! die Sitzungs-Wahl (gemerkt), der Login-Schlüssel (Stick-Erkennung
//! mit Wach-Klang 13) und der Wächter: „Passwort vergessen?" ruft
//! laut (Klang 14) und startet die passwortlose wache-Session.
//!
//! `matrix-greeter --demo` läuft OHNE greetd in der normalen Session
//! (Passwort „demo" gelingt) — zum gefahrlosen Ansehen und Testen.

mod greetd;

use iced::widget::{button, column, container, image, row, text_input, Space};
use iced::{Color, Element, Font, Length, Subscription, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use matrixkit_widgets::{color, tick};

/// Klänge des Wächters (Slot-Kopien im Greeter-HOME als Fallback).
const KLANG_SCHLUESSEL: &str = "13-schluessel-erkannt.wav";
const KLANG_RUF: &str = "14-waechter-ruf.wav";
/// Das Sitzungskommando der Wiederherstellung (wache-Account).
/// Recovery-Session: der Compositor der Fassade im Kiosk-Modus.
fn recovery_cmd() -> String {
    format!("{} -c /etc/matrix/recovery-niri.kdl", mk::leinwand::BINARY)
}

fn main() -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};
    iced_layershell::application(
        App::new,
        || String::from("matrix-greeter"),
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
            // Alle vier Anker = die ganze Fläche.
            size: Some((0, 0)),
            anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
            layer: Layer::Overlay,
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            exclusive_zone: -1,
            ..Default::default()
        },
        default_font: Font::with_name("Inter Variable"),
        fonts: mkw::symbol_font_laden().into_iter().collect(),
        ..Default::default()
    })
    .run()
}

// ------------------------------------------------------------ Umgebung

/// Der anzumeldende Mensch: erster „echter" Account (UID ≥ 1000, kein
/// nologin) — Matrix ist ein Ein-Personen-System je Gerät.
fn nutzer_finden() -> (String, String) {
    if let Ok(passwd) = std::fs::read_to_string("/etc/passwd") {
        for l in passwd.lines() {
            let f: Vec<&str> = l.split(':').collect();
            if f.len() >= 7 {
                let uid: u32 = f[2].parse().unwrap_or(0);
                if (1000..60000).contains(&uid)
                    && !f[6].contains("nologin")
                    && f[0] != "wache"
                {
                    let gecos = f[4].split(',').next().unwrap_or("").trim();
                    let anzeige = if gecos.is_empty() {
                        gross(f[0])
                    } else {
                        gross(gecos)
                    };
                    return (f[0].to_string(), anzeige);
                }
            }
        }
    }
    (String::from("nicolas"), String::from("Nutzer"))
}

fn gross(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_else(|| s.to_string())
}

/// Wallpaper aus dem Greeter-Slot (session.json des Slot-Syncs).
fn wallpaper_finden() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    for kandidat in [
        format!("{home}/.local/state/DankMaterialShell/session.json"),
        format!("{home}/session.json"),
    ] {
        if let Ok(raw) = std::fs::read_to_string(&kandidat) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                for feld in ["wallpaperPath", "wallpaperPathDark", "wallpaperPathLight"] {
                    if let Some(p) = v[feld].as_str() {
                        if std::path::Path::new(p).exists() {
                            return Some(p.to_string());
                        }
                    }
                }
            }
        }
    }
    // Notnagel: der Versions-Standard.
    let falke = "/usr/share/backgrounds/matrix/matrix-standard.jpg";
    std::path::Path::new(falke)
        .exists()
        .then(|| falke.to_string())
}

/// Sitzungen aus den wayland-sessions-Verzeichnissen (Name, Exec).
fn sitzungen_finden() -> Vec<(String, String)> {
    let mut liste = Vec::new();
    for dir in [
        "/usr/local/share/wayland-sessions",
        "/usr/share/wayland-sessions",
    ] {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for e in rd.flatten() {
            let Ok(inhalt) = std::fs::read_to_string(e.path()) else {
                continue;
            };
            let (mut name, mut exec) = (None, None);
            for z in inhalt.lines() {
                if let Some(v) = z.strip_prefix("Name=") {
                    name.get_or_insert(v.trim().to_string());
                }
                if let Some(v) = z.strip_prefix("Exec=") {
                    exec.get_or_insert(v.trim().to_string());
                }
            }
            if let (Some(n), Some(x)) = (name, exec) {
                if !liste.iter().any(|(ln, _): &(String, String)| ln == &n) {
                    liste.push((n, x));
                }
            }
        }
    }
    if liste.is_empty() {
        liste.push((String::from("Niri"), String::from("niri-session")));
    }
    liste
}

fn klang(name: &str) {
    let home = std::env::var("HOME").unwrap_or_default();
    let system = format!("/usr/share/matrix/klaenge/{name}");
    let slot = format!("{home}/{name}");
    let datei = if std::path::Path::new(&system).exists() {
        system
    } else {
        slot
    };
    std::thread::spawn(move || {
        // Boot-Rennen (Nutzer-Fund 15.7.): Beim Kaltstart feuert der
        // Gong, bevor PipeWire der greeter-Session steht — EIN Versuch
        // verhallte lautlos. Also geduldig: bis ~6 s mit Backoff, der
        // erste erfolgreiche Spieler gewinnt.
        for _ in 0..15 {
            for spieler in ["pw-play", "paplay"] {
                if mk::befehl::still(spieler, &[&datei]) {
                    return;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
    });
}

fn stick_da() -> bool {
    std::path::Path::new("/dev/disk/by-label/MATRIXKEY").exists()
}

// ----------------------------------------------------------------- App

#[derive(Debug, Clone, Copy, PartialEq)]
enum Lage {
    Ruhe,
    Prueft,
    /// greetd hat übernommen — wir verabschieden uns gleich.
    Erfolg,
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Msg {
    Passwort(String),
    Anmelden,
    Ergebnis(Result<(), String>),
    Sitzung(usize),
    StickPuls,
    Wiederherstellung,
    Uhr,
}

struct App {
    palette: mk::Palette,
    demo: bool,
    nutzer: String,
    anzeige_name: String,
    passwort: String,
    lage: Lage,
    fehler: Option<String>,
    wallpaper: Option<image::Handle>,
    avatar: Option<image::Handle>,
    sitzungen: Vec<(String, String)>,
    gewaehlt: usize,
    stick: bool,
    uhr: String,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let demo = std::env::args().any(|a| a == "--demo");
        // Der Matrix-Gong wohnt seit R53 im SYSTEM-Dienst
        // matrix-gong.service — wie beim Referenzsystem klingt er aus der
        // „Firmware" (Boot), bevor irgendeine UI steht. Der Greeter
        // spielt nur noch die Wächter-Klänge.
        let _ = demo;
        let palette = mk::Palette::load().unwrap_or_default();
        let (nutzer, anzeige_name) = nutzer_finden();
        let sitzungen = sitzungen_finden();
        // Gemerkte Wahl (Einstellungs-Kultur im Greeter-HOME).
        let gewaehlt = mk::einstellung::lesen("greeter-sitzung")
            .and_then(|n| sitzungen.iter().position(|(name, _)| *name == n))
            .unwrap_or(0);
        // Bytes statt Pfad: eliminiert Lade-Pfad-Fragen im Kiosk-Kontext.
        let avatar_slot = std::env::var("HOME")
            .ok()
            .and_then(|h| std::fs::read(format!("{h}/.face")).ok())
            .map(image::Handle::from_bytes);
        let avatar = avatar_slot.or_else(|| {
            matrixkit_icons::avatar_png(&palette).map(image::Handle::from_bytes)
        });
        let mut app = App {
            palette,
            demo,
            nutzer,
            anzeige_name,
            passwort: String::new(),
            lage: Lage::Ruhe,
            fehler: None,
            wallpaper: wallpaper_finden().map(image::Handle::from_path),
            avatar,
            sitzungen,
            gewaehlt,
            stick: stick_da(),
            uhr: String::new(),
        };
        app.uhr_lesen();
        (app, mkw::suche_fokussieren())
    }

    fn uhr_lesen(&mut self) {
        if let Some(z) = mk::befehl::erste_zeile("date", &["+%H:%M"]) {
            self.uhr = z;
        }
    }

    fn anmelden(&mut self) -> Task<Msg> {
        if self.lage != Lage::Ruhe {
            return Task::none();
        }
        self.lage = Lage::Prueft;
        self.fehler = None;
        let nutzer = self.nutzer.clone();
        let passwort = self.passwort.clone();
        let cmd = self.sitzungen[self.gewaehlt].1.clone();
        let demo = self.demo;
        Task::perform(
            async move {
                if demo {
                    std::thread::sleep(std::time::Duration::from_millis(600));
                    if passwort == "demo" {
                        Ok(())
                    } else {
                        Err(String::from("Demo: Passwort ist „demo“"))
                    }
                } else {
                    greetd::einloggen(&nutzer, &passwort, &cmd)
                }
            },
            Msg::Ergebnis,
        )
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Passwort(p) => {
                if self.lage == Lage::Ruhe {
                    self.passwort = p;
                }
            }
            Msg::Anmelden => return self.anmelden(),
            Msg::Ergebnis(Ok(())) => {
                self.lage = Lage::Erfolg;
                // Erfolgs-Marker: der Wrapper darf einen SCHNELLEN Erfolg
                // (Recovery-Klick!) nicht als Crash-Fallback deuten.
                let _ = std::fs::write("/tmp/matrix-greeter-erfolg", "1");
                // greetd wechselt zur Session, sobald der Greeter endet.
                std::process::exit(0);
            }
            Msg::Ergebnis(Err(f)) => {
                self.lage = Lage::Ruhe;
                self.passwort.clear();
                self.fehler = Some(f);
                mk::feedback::fehler(); // sensoryFeedback(.error)
                return mkw::suche_fokussieren();
            }
            Msg::Sitzung(i) => {
                if i < self.sitzungen.len() {
                    self.gewaehlt = i;
                    mk::einstellung::schreiben("greeter-sitzung", &self.sitzungen[i].0);
                }
            }
            Msg::StickPuls => {
                let jetzt = stick_da();
                if jetzt && !self.stick {
                    klang(KLANG_SCHLUESSEL); // bewusst NICHT abschaltbar
                }
                self.stick = jetzt;
            }
            Msg::Wiederherstellung => {
                if self.lage == Lage::Ruhe {
                    klang(KLANG_RUF); // laut — Übernahme nie heimlich
                    self.lage = Lage::Prueft;
                    let demo = self.demo;
                    return Task::perform(
                        async move {
                            if demo {
                                Err(String::from("Demo: Wiederherstellung nur am echten Greeter"))
                            } else {
                                greetd::einloggen("wache", "", &recovery_cmd())
                            }
                        },
                        Msg::Ergebnis,
                    );
                }
            }
            Msg::Uhr => self.uhr_lesen(),
            // vom to_layer_message-Makro ergänzte Varianten
            _ => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch([
            tick("stick", std::time::Duration::from_secs(2)).map(|_| Msg::StickPuls),
            tick("uhr", std::time::Duration::from_secs(20)).map(|_| Msg::Uhr),
        ])
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.palette;

        // ---- Mitte: Avatar, Name, Passwort, Hinweise.
        let avatar: Element<'_, Msg> = match &self.avatar {
            Some(h) => image(h.clone())
                .width(Length::Fixed(96.0))
                .height(Length::Fixed(96.0))
                .into(),
            None => Space::new().into(),
        };

        let hinweis = if let Some(f) = &self.fehler {
            mkw::txt(f.clone(), mk::typo::HINWEIS, p.error)
        } else if self.lage == Lage::Prueft {
            mkw::txt("Einen Moment …", mk::typo::HINWEIS, p.on_surface_variant)
        } else if self.stick {
            mkw::txt(
                "Login-Schlüssel erkannt — Enter genügt",
                mk::typo::HINWEIS,
                p.primary,
            )
        } else {
            mkw::txt("", mk::typo::HINWEIS, p.on_surface_variant)
        };

        // familien-ausnahme: Greeter-Heldenfeld: Autofokus-Id + Hero-Maße, Zwilling der Sperre
        let feld = text_input("Passwort", &self.passwort)
            .id(iced::advanced::widget::Id::new("mkw-suchfeld"))
            .secure(true)
            .on_input(Msg::Passwort)
            .on_submit(Msg::Anmelden)
            .padding([mk::spacing::S as u16, mk::spacing::M as u16])
            .size(mk::font_size::MEDIUM)
            .width(Length::Fixed(280.0))
            .style(move |_, status| {
                if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                    // R58: das Passwortfeld ruft die Bildschirmtastatur —
                    // ohne Type Cover tippt man den Login auf dem Schirm.
                    mkw::tastatur::funken();
                }
                iced::widget::text_input::Style {
                    background: color(p.surface_container.mit_alpha(0.85)).into(),
                    border: iced::Border {
                        color: color(p.outline.mit_alpha(0.4)),
                        width: 1.0,
                        radius: mk::radius::KLEIN.into(),
                    },
                    icon: color(p.on_surface_variant),
                    placeholder: color(p.on_surface_variant.mit_alpha(0.7)),
                    value: color(p.on_surface),
                    selection: color(p.primary.mit_alpha(0.4)),
                }
            });

        let vergessen = button(mkw::txt(
            "Passwort vergessen?",
            mk::typo::KLEIN,
            p.on_surface_variant,
        ))
        .padding([2, mk::spacing::S as u16])
        .style(move |_, status| mkw::leiste::knopf_stil(p, status, mk::radius::KLEIN))
        .on_press(Msg::Wiederherstellung);

        let mitte = column![
            avatar,
            mkw::txt(&self.anzeige_name, mk::typo::TITEL, p.on_surface),
            Space::new().height(mk::spacing::S),
            feld,
            hinweis,
            Space::new().height(mk::spacing::S),
            vergessen,
        ]
        .spacing(mk::spacing::S)
        .align_x(iced::Alignment::Center);

        // ---- Unten: die Sitzungs-Wahl als Segmente.
        let mut wahl = row![].spacing(mk::spacing::XS);
        for (i, (name, _)) in self.sitzungen.iter().enumerate() {
            let aktiv = i == self.gewaehlt;
            let farbe = if aktiv { p.on_surface } else { p.on_surface_variant };
            wahl = wahl.push(
                button(mkw::txt(name.clone(), mk::typo::KLEIN, farbe))
                    .padding([4, mk::spacing::M as u16])
                    .style(move |_, status| {
                        let mut stil = mkw::leiste::knopf_stil(p, status, mk::radius::KLEIN);
                        if aktiv {
                            stil.background = Some(
                                color(p.on_surface.over(p.surface_container, mkw::leiste::HOVER))
                                    .into(),
                            );
                        }
                        stil
                    })
                    .on_press(Msg::Sitzung(i)),
            );
        }

        let inhalt = column![
            container(mkw::txt(&self.uhr, mk::typo::GROSSTITEL, p.on_surface.mit_alpha(0.9)))
                .center_x(Length::Fill)
                .padding(mk::spacing::XXL),
            Space::new().height(Length::Fill),
            container(mitte).center_x(Length::Fill),
            Space::new().height(Length::Fill),
            container(wahl).center_x(Length::Fill).padding(mk::spacing::XL),
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        // ---- Fläche: Wallpaper + ruhiger Schleier, darüber der Inhalt.
        let schleier = container(inhalt)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(
                    color(p.surface.mit_alpha(if mk::transparenz_reduziert() {
                        0.9
                    } else {
                        0.45
                    }))
                    .into(),
                ),
                ..Default::default()
            });

        match &self.wallpaper {
            Some(w) => iced::widget::stack![
                image(w.clone())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .content_fit(iced::ContentFit::Cover),
                schleier,
            ]
            .into(),
            None => schleier.into(),
        }
    }
}
