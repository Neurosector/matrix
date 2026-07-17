//! Matrix Sperre — App #20, der Sperrschirm.
//!
//! Das loginwindow/LockScreen-Extrakt: ein ECHTER Sperrschirm über das
//! Wayland-Protokoll ext-session-lock (iced_sessionlock) — der
//! Compositor sperrt den Bildschirm auf Protokoll-Ebene, kein Fenster
//! kann darüber erscheinen, und selbst ein Absturz des Sperr-Clients
//! hält den Schirm gesperrt (Sicherheitsgarantie des Protokolls).
//! Entsperrt wird ausschließlich durch PAM-Prüfung des angemeldeten
//! Nutzers (src/pam.rs, Dienst /etc/pam.d/matrix-sperre → system-auth).
//!
//! Modi:
//!   (ohne)   echter Sperrschirm (Sitzungssperre)
//!   --demo   normales Fenster, KEINE Sperre, Passwort „demo" — Ansicht
//!   --check <pw>  nur PAM prüfen, Exit 0/1 — kein UI, kein Lock

mod pam;

use iced::widget::{column, container, image, text_input, Space};
use iced::{Color, Element, Font, Length, Subscription, Task};
use iced_sessionlock::to_session_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use matrixkit_widgets::color;

fn view_lock(app: &App, _id: iced::window::Id) -> Element<'_, Msg> {
    app.bild()
}

fn nutzer() -> String {
    std::env::var("USER").unwrap_or_else(|_| String::from("nicolas"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // --check <passwort>: reiner PAM-Test (sicher, kein Lock).
    if args.get(1).map(|s| s.as_str()) == Some("--check") {
        let pw = args.get(2).cloned().unwrap_or_default();
        let ok = pam::pruefen(&nutzer(), &pw);
        println!("{}", if ok { "ok" } else { "abgelehnt" });
        std::process::exit(if ok { 0 } else { 1 });
    }

    // Echter Sperrschirm (ext-session-lock). --demo sperrt ebenfalls
    // ECHT, entsperrt aber nach 8 s garantiert von selbst und nimmt
    // „demo" als Passwort — ein gefahrloser Voll-Test der Pipeline.
    let _ = iced_sessionlock::build_pattern::application(App::new, App::update, view_lock)
        .subscription(App::subscription)
        .style(|_s, _t| iced::theme::Style {
            background_color: Color::TRANSPARENT,
            text_color: Color::WHITE,
        })
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run();
}

fn wallpaper() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let raw = std::fs::read_to_string(format!(
        "{home}/.local/state/DankMaterialShell/session.json"
    ))
    .ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok().or_else(|| {
        // ohne serde_json-Dep: simpler Feldgriff
        None
    })?;
    let _ = &v;
    None
}

#[to_session_message]
#[derive(Debug, Clone)]
enum Msg {
    Passwort(String),
    Absenden,
    Ergebnis(bool),
    Tick,
}

struct App {
    palette: mk::Palette,
    demo: bool,
    nutzer: String,
    anzeige: String,
    passwort: String,
    prueft: bool,
    fehler: bool,
    avatar: Option<image::Handle>,
    wallpaper: Option<image::Handle>,
    uhr: String,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let demo = std::env::args().any(|a| a == "--demo");
        let palette = mk::Palette::load().unwrap_or_default();
        let nutzer = nutzer();
        let anzeige = {
            let mut c = nutzer.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_else(|| nutzer.clone())
        };
        let avatar = std::env::var("HOME")
            .ok()
            .and_then(|h| std::fs::read(format!("{h}/.face")).ok())
            .map(image::Handle::from_bytes)
            .or_else(|| matrixkit_icons::avatar_png(&palette).map(image::Handle::from_bytes));
        let wallpaper = wallpaper().map(image::Handle::from_path).or_else(|| {
            let p = "/usr/share/backgrounds/matrix/matrix-standard.jpg";
            std::path::Path::new(p).exists().then(|| image::Handle::from_path(p))
        });
        let mut app = App {
            palette,
            demo,
            nutzer,
            anzeige,
            passwort: String::new(),
            prueft: false,
            fehler: false,
            avatar,
            wallpaper,
            uhr: String::new(),
        };
        app.uhr_lesen();
        // Demo-Sicherung: nach 8 s zwingend entsperren (kein Stranden).
        let auf = mkw::suche_fokussieren();
        if demo {
            return (
                app,
                Task::batch([
                    auf,
                    Task::perform(
                        async { std::thread::sleep(std::time::Duration::from_secs(8)); },
                        |_| Msg::UnLock,
                    ),
                ]),
            );
        }
        (app, auf)
    }

    fn uhr_lesen(&mut self) {
        if let Some(z) = mk::befehl::erste_zeile("date", &["+%H:%M"]) {
            self.uhr = z;
        }
    }

    fn absenden(&mut self) -> Task<Msg> {
        if self.prueft {
            return Task::none();
        }
        self.prueft = true;
        self.fehler = false;
        let nutzer = self.nutzer.clone();
        let pw = std::mem::take(&mut self.passwort);
        let demo = self.demo;
        Task::perform(
            async move {
                if demo {
                    pw == "demo"
                } else {
                    pam::pruefen(&nutzer, &pw)
                }
            },
            Msg::Ergebnis,
        )
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Passwort(p) => {
                if !self.prueft {
                    self.passwort = p;
                }
            }
            Msg::Absenden => return self.absenden(),
            Msg::Ergebnis(true) => {
                if self.demo {
                    std::process::exit(0);
                }
                // Echte Sperre: das Macro wandelt UnLock in die
                // Entsperr-Aktion — der Compositor gibt den Schirm frei.
                return Task::done(Msg::UnLock);
            }
            Msg::Ergebnis(false) => {
                self.prueft = false;
                self.fehler = true;
                mk::feedback::fehler();
                return mkw::suche_fokussieren();
            }
            Msg::Tick => self.uhr_lesen(),
            // vom to_session_message-Macro ergänzt: UnLock
            _ => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        mkw::tick("sperre", std::time::Duration::from_secs(10)).map(|_| Msg::Tick)
    }

}

impl App {
    fn bild(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let avatar: Element<'_, Msg> = match &self.avatar {
            Some(h) => image(h.clone())
                .width(Length::Fixed(96.0))
                .height(Length::Fixed(96.0))
                .into(),
            None => Space::new().into(),
        };
        let hinweis = if self.fehler {
            mkw::txt("Falsches Passwort", mk::typo::HINWEIS, p.error)
        } else if self.prueft {
            mkw::txt("Einen Moment …", mk::typo::HINWEIS, p.on_surface_variant)
        } else {
            mkw::txt("", mk::typo::HINWEIS, p.on_surface_variant)
        };
        // familien-ausnahme: Sperr-Heldenfeld: Autofokus-Id + Hero-Maße, Zwilling des Greeters
        let feld = text_input("Passwort", &self.passwort)
            .id(iced::advanced::widget::Id::new("mkw-suchfeld"))
            .secure(true)
            .on_input(Msg::Passwort)
            .on_submit(Msg::Absenden)
            .padding([mk::spacing::S as u16, mk::spacing::M as u16])
            .size(mk::font_size::MEDIUM)
            .width(Length::Fixed(280.0))
            .style(move |_, status| {
                if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                    // R58: auch der Sperrschirm ruft die Bildschirmtastatur.
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

        let mitte = column![
            avatar,
            mkw::txt(&self.anzeige, mk::typo::TITEL, p.on_surface),
            Space::new().height(mk::spacing::S),
            feld,
            hinweis,
        ]
        .spacing(mk::spacing::S)
        .align_x(iced::Alignment::Center);

        let inhalt = column![
            container(mkw::txt(&self.uhr, mk::typo::GROSSTITEL, p.on_surface.mit_alpha(0.9)))
                .center_x(Length::Fill)
                .padding(mk::spacing::XXL),
            Space::new().height(Length::Fill),
            container(mitte).center_x(Length::Fill),
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill);

        let schleier = container(inhalt)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(color(p.surface.mit_alpha(0.55)).into()),
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
