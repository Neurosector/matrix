//! Matrix Web — App #17, jetzt als REINE MatrixKit-App (des Nutzers
//! Abnahme-Veto zur GTK-Fassung war berechtigt).
//!
//! Diese App ist die Chrome: mkw::app_fenster (Ampeln, Zieh-Griff,
//! Root-Ebene), Navigationszeile, Adressfeld, Fortschritt — alles Kit.
//! Den Web-Inhalt rendert der unsichtbare Träger (matrix-web-inhalt,
//! WebKit), den diese App über stdin/stdout steuert und über die
//! niri-Brücke pixelgenau unter ihren Inhaltsbereich koppelt: Der
//! Compositor macht die Einbettung — der Nutzer sieht EIN Fenster.

use std::io::Write;

use iced::widget::{button, column, container, row, text_input, Space};
use iced::{Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use matrixkit_widgets::{color, tick};

/// Höhe der Chrome (Ampel-Header + Navigationszeile) — der Träger
/// dockt exakt darunter an.
const KOPF: f64 = 84.0;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(1180.0, 820.0),
            min_size: Some(iced::Size::new(520.0, 360.0)),
            decorations: false,
            platform_specific: iced::window::settings::PlatformSpecific {
                application_id: String::from("matrix-web"),
                ..Default::default()
            },
            ..Default::default()
        })
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

fn zu_url(eingabe: &str) -> String {
    let e = eingabe.trim();
    if e.contains("://") {
        return e.to_string();
    }
    if e.contains('.') && !e.contains(' ') {
        return format!("https://{e}");
    }
    format!("https://duckduckgo.com/?q={}", e.replace(' ', "+"))
}

#[derive(Debug, Clone)]
enum Msg {
    Puls,
    Adresse(String),
    Laden,
    Zurueck,
    Vor,
    Neu,
    // Der Kit-Rahmen.
    Drag,
    Schliessen,
    Ablage,
    Maximieren,
    Titel,
    AmpelnHover(bool),
    Groesse(iced::Size),
    Taste(mkw::Taste),
}

struct App {
    palette: mk::Palette,
    watcher: mk::PaletteWatcher,
    fenster: mkw::FensterZustand,
    root: mkw::RootZustand,
    kind_stdin: Option<std::process::ChildStdin>,
    ereignisse: Option<std::sync::mpsc::Receiver<String>>,
    adresse: String,
    adresse_fokus_text: String,
    titel: String,
    fortschritt: f32,
    kann_zurueck: bool,
    kann_vor: bool,
    /// Letzte gesetzte Träger-Lage — nur Abweichungen werden befohlen.
    letzte_kopplung: Option<((i64, i64), (i64, i64))>,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        // Den Träger starten: unsere Pipes sind sein Lebensfaden.
        let heim = std::env::var("HOME").unwrap_or_default();
        let kandidaten = [
            format!("{heim}/.local/bin/matrix-web-inhalt"),
            String::from("matrix-web-inhalt"),
        ];
        let mut kind_stdin = None;
        let mut ereignisse = None;
        for k in kandidaten {
            if let Ok(mut kind) = std::process::Command::new(&k)
                // WebKitGTK malt auf NVIDIA-Treibern über den DMABUF-Renderer
                // nur Schwarz; der Shared-Memory-Pfad rendert überall korrekt.
                .env("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                kind_stdin = kind.stdin.take();
                if let Some(stdout) = kind.stdout.take() {
                    let (tx, rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        use std::io::BufRead;
                        for zeile in std::io::BufReader::new(stdout).lines() {
                            match zeile {
                                Ok(z) => {
                                    if tx.send(z).is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    });
                    ereignisse = Some(rx);
                }
                std::thread::spawn(move || {
                    let _ = kind.wait(); // kein Zombie
                });
                break;
            }
        }
        (
            App {
                palette: mk::Palette::load().unwrap_or_default(),
                watcher: mk::PaletteWatcher::new(),
                fenster: mkw::FensterZustand::neu(),
                root: mkw::RootZustand::neu(),
                kind_stdin,
                ereignisse,
                adresse: String::new(),
                adresse_fokus_text: String::new(),
                titel: String::from("Matrix Web"),
                fortschritt: 0.0,
                kann_zurueck: false,
                kann_vor: false,
                letzte_kopplung: None,
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        if self.titel.is_empty() {
            String::from("Matrix Web")
        } else {
            format!("{} — Matrix Web", self.titel)
        }
    }

    fn befehl(&mut self, b: &str) {
        if let Some(stdin) = &mut self.kind_stdin {
            let _ = writeln!(stdin, "{b}");
            let _ = stdin.flush();
        }
    }

    /// Die Andock-Kopplung: der Träger sitzt exakt unter der Chrome.
    fn koppeln(&mut self) {
        let alle = mkw::leinwand::fenster();
        let ich = alle.iter().find(|f| f.app_id == "matrix-web");
        let kind = alle.iter().find(|f| f.app_id == "matrix-web-inhalt");
        let (Some(ich), Some(kind)) = (ich, kind) else { return };
        let (Some(pos), Some(gr)) = (ich.pos, ich.groesse) else { return };
        let Some(kind_pos) = kind.pos else { return };
        let ziel_pos = ((pos.0) as i64, (pos.1 + KOPF) as i64);
        let ziel_gr = ((gr.0) as i64, (gr.1 - KOPF).max(120.0) as i64);
        // Delta statt Absolut: niri deutet nackte negative Zahlen als
        // relative Schritte — mit explizitem Vorzeichen ist es eindeutig.
        let dx = ziel_pos.0 - kind_pos.0 as i64;
        let dy = ziel_pos.1 - kind_pos.1 as i64;
        if dx.abs() <= 1 && dy.abs() <= 1 && self.letzte_kopplung == Some((ziel_pos, ziel_gr)) {
            return;
        }
        if dx.abs() > 1 || dy.abs() > 1 {
            mk::leinwand::fenster_bewegen(kind.id, dx, dy);
        }
        mk::leinwand::fenster_breite(kind.id, ziel_gr.0);
        mk::leinwand::fenster_hoehe(kind.id, ziel_gr.1);
        self.letzte_kopplung = Some((ziel_pos, ziel_gr));
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Puls => {
                if self.watcher.changed() {
                    if let Some(neu) = mk::Palette::load() {
                        self.palette = neu;
                    }
                }
                // Träger-Ereignisse einsammeln.
                if let Some(rx) = &self.ereignisse {
                    while let Ok(z) = rx.try_recv() {
                        let mut teile = z.splitn(2, ' ');
                        match (teile.next(), teile.next()) {
                            (Some("titel"), Some(t)) => self.titel = t.to_string(),
                            (Some("uri"), Some(u)) => {
                                if self.adresse_fokus_text.is_empty() {
                                    self.adresse = u.to_string();
                                }
                            }
                            (Some("fortschritt"), Some(f)) => {
                                self.fortschritt = f.parse().unwrap_or(0.0)
                            }
                            (Some("nav"), Some(rest)) => {
                                let mut n = rest.split_whitespace();
                                self.kann_zurueck = n.next() == Some("1");
                                self.kann_vor = n.next() == Some("1");
                            }
                            _ => {}
                        }
                    }
                }
                self.koppeln();
            }
            Msg::Adresse(a) => {
                self.adresse_fokus_text = a.clone();
                self.adresse = a;
            }
            Msg::Laden => {
                let url = zu_url(&self.adresse);
                self.adresse_fokus_text.clear();
                self.befehl(&format!("lade {url}"));
            }
            Msg::Zurueck => self.befehl("zurueck"),
            Msg::Vor => self.befehl("vor"),
            Msg::Neu => self.befehl("neu"),
            Msg::Drag => return iced::window::latest().and_then(iced::window::drag),
            Msg::Schliessen => {
                self.befehl("ende");
                std::process::exit(0);
            }
            Msg::Ablage => {
                mk::leinwand::fenster_zur_ablage();
            }
            Msg::Maximieren => {
                return iced::window::latest().and_then(iced::window::toggle_maximize);
            }
            Msg::Titel => self.root.umschalten(),
            Msg::AmpelnHover(an) => self.fenster.ampeln_hover = an,
            Msg::Groesse(_) => self.letzte_kopplung = None, // frisch koppeln
            Msg::Taste(t) => match t {
                mkw::Taste::Escape => self.root.schliessen(),
                mkw::Taste::Suchen => {
                    return mkw::suche_fokussieren();
                }
                mkw::Taste::Aktualisieren => self.befehl("neu"),
                _ => {}
            },
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch([
            tick("web", std::time::Duration::from_millis(250)).map(|_| Msg::Puls),
            mkw::leinwand_strom().map(|_| Msg::Puls),
            mkw::tasten_abo(Msg::Taste),
            iced::window::resize_events().map(|(_, g)| Msg::Groesse(g)),
        ])
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.palette;

        // Navigationszeile — die der Referenz-Webview-Grammatik in Kit-Bausteinen.
        // MatrixUI HarnessFamily: Navigations-Knöpfe mit Lupe.
        let nav_knopf = |zeichen: char, aktivierbar: bool, msg: Msg| {
            mkw::ui::nav_knopf(zeichen, aktivierbar, msg, p)
        };
        // familien-ausnahme: Browser-Adresszeile: Autofokus-Id + eigene Dichte
        let adresse = text_input("Adresse oder Suche …", &self.adresse)
            .id(iced::advanced::widget::Id::new("mkw-suchfeld"))
            .on_input(Msg::Adresse)
            .on_submit(Msg::Laden)
            .padding([mk::spacing::XS as u16, mk::spacing::M as u16])
            .size(mk::font_size::SMALL)
            .width(Length::Fill)
            .style(move |_, _| iced::widget::text_input::Style {
                background: color(p.surface.mit_alpha(0.7)).into(),
                border: iced::Border {
                    color: color(p.outline.mit_alpha(0.4)),
                    width: 1.0,
                    radius: mk::radius::KLEIN.into(),
                },
                icon: color(p.on_surface_variant),
                placeholder: color(p.on_surface_variant.mit_alpha(0.7)),
                value: color(p.on_surface),
                selection: color(p.primary.mit_alpha(0.4)),
            });
        let nav = row![
            nav_knopf(mkw::symbol::ARROW_BACK, self.kann_zurueck, Msg::Zurueck),
            nav_knopf(mkw::symbol::CHEVRON_RIGHT, self.kann_vor, Msg::Vor),
            nav_knopf(mkw::symbol::RESTART, true, Msg::Neu),
            adresse,
        ]
        .spacing(mk::spacing::XS)
        .align_y(iced::Alignment::Center)
        .padding([mk::spacing::XS as u16, mk::spacing::S as u16]);

        // Fortschritt: die 2-px-Akzentlinie.
        let fortschritt: Element<'_, Msg> = if self.fortschritt > 0.0 && self.fortschritt < 1.0 {
            container(Space::new())
                .width(Length::FillPortion((self.fortschritt * 1000.0).max(1.0) as u16))
                .height(Length::Fixed(2.0))
                .style(move |_| container::Style {
                    background: Some(color(p.primary).into()),
                    ..Default::default()
                })
                .into()
        } else {
            Space::new().height(Length::Fixed(2.0)).into()
        };
        let fortschritt = row![fortschritt, Space::new().width(Length::Fill)];

        // Inhaltsbereich: hier sitzt (per Compositor) der Träger.
        let inhalt = container(Space::new())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(color(p.surface).into()),
                ..Default::default()
            });

        let koerper = column![nav, fortschritt, inhalt];

        mkw::app_fenster(
            "Matrix Web",
            p,
            koerper.into(),
            Msg::Drag,
            Msg::Schliessen,
            |_| Msg::Drag, // Resize übernimmt der Compositor-Rand
            Msg::Titel,
            Msg::Ablage,
            Msg::Maximieren,
            None,
            &self.fenster,
            Msg::AmpelnHover,
        )
    }
}
