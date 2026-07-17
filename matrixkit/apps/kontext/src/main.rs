//! Matrix Kontext — das Desktop-Kontextmenü der Leinwand.
//!
//! Der Compositor ruft `matrix-kontext leinwand <x> <y>` beim
//! Rechtsklick auf den Leerraum; dieses Popup (Overlay, volle Fläche
//! als Schließ-Schleier) zeigt die Menü-Pille exakt an der Maus.
//! Klick auf einen Eintrag startet die App, Klick daneben schließt —
//! ein zweiter Rechtsklick ersetzt das Menü (mkw::leiste_toggle).

use iced::widget::{column, container, mouse_area, row, Space};
use iced::{Color, Element, Font, Length, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;

fn main() -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};

    // Zweiter Rechtsklick: das alte Menü geht, das neue kommt.
    let _ = mkw::leiste_toggle();

    iced_layershell::application(
        App::new,
        || String::from("matrix-kontext"),
        App::update,
        App::view,
    )
    .style(|_state, _theme| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .settings(Settings {
        layer_settings: LayerShellSettings {
            size: Some((0, 0)),
            anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
            layer: Layer::Overlay,
            keyboard_interactivity: KeyboardInteractivity::None,
            exclusive_zone: -1,
            ..Default::default()
        },
        default_font: Font::with_name("Inter Variable"),
        fonts: mkw::symbol_font_laden().into_iter().collect(),
        ..Default::default()
    })
    .run()
}

/// Die Einträge des Leinwand-Menüs: (Symbol, Titel, App).
const EINTRAEGE: [(char, &str, &str); 4] = [
    (mkw::symbol::APPS, "Apps …", "matrix-start"),
    ('\u{e894}', "Neues Browserfenster", "matrix-web"),
    (mkw::symbol::PALETTE, "Hintergrund ändern …", "matrix-einstellungen hintergrund"),
    (mkw::symbol::TUNE, "Einstellungen …", "matrix-einstellungen"),
];

#[to_layer_message]
#[derive(Debug, Clone)]
enum Msg {
    Starten(usize),
    PinWechsel,
    Beenden,
    /// Zwischenablage-Verlauf: Eintrag i zurück in die Zwischenablage.
    Clip(usize),
    ClipsLeeren,
    Zu,
}

/// Der Zwischenablage-Verlauf, den das Dock pflegt (JSON-Array, neueste
/// zuerst) — flüchtig im Runtime-Verzeichnis, wie die Abzeichen.
fn clips_pfad() -> std::path::PathBuf {
    let basis = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| String::from("/tmp"));
    std::path::PathBuf::from(basis).join("matrix-zwischenablage.json")
}

fn clips_lesen() -> Vec<String> {
    std::fs::read_to_string(clips_pfad())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Dock-Modus: das Kachel-Menü als eigene Overlay-Fläche — das Dock
/// selbst wächst nie mehr (der Resize-Glitch ist damit strukturell
/// unmöglich; Nutzer-Fund 8.7.).
struct DockZiel {
    app_id: String,
    gepinnt: bool,
    fenster: Vec<u64>,
    /// Kachel-Versatz von der Bildschirmmitte (das Dock ist zentriert).
    offset_x: f32,
    /// Abstand der Menü-Unterkante vom unteren Bildschirmrand.
    unten: f32,
}

/// Clips-Modus: der Zwischenablage-Verlauf als Menü über dem Dock.
struct ClipsZiel {
    offset_x: f32,
    unten: f32,
    eintraege: Vec<String>,
}

struct App {
    palette: mk::Palette,
    x: f32,
    y: f32,
    dock: Option<DockZiel>,
    clips: Option<ClipsZiel>,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let args: Vec<String> = std::env::args().collect();
        // dock-Modus: matrix-kontext dock <offset_x> <unten> <app_id>
        //             <gepinnt 0|1> <fenster-ids kommasep|->
        let dock = (args.get(1).map(String::as_str) == Some("dock")).then(|| DockZiel {
            offset_x: args.get(2).and_then(|a| a.parse().ok()).unwrap_or(0.0),
            unten: args.get(3).and_then(|a| a.parse().ok()).unwrap_or(140.0),
            app_id: args.get(4).cloned().unwrap_or_default(),
            gepinnt: args.get(5).map(String::as_str) == Some("1"),
            fenster: args
                .get(6)
                .map(|f| f.split(',').filter_map(|t| t.parse().ok()).collect())
                .unwrap_or_default(),
        });
        // clips-Modus: matrix-kontext clips <offset_x> <unten>
        let clips = (args.get(1).map(String::as_str) == Some("clips")).then(|| ClipsZiel {
            offset_x: args.get(2).and_then(|a| a.parse().ok()).unwrap_or(0.0),
            unten: args.get(3).and_then(|a| a.parse().ok()).unwrap_or(140.0),
            eintraege: clips_lesen(),
        });
        let x = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(200.0);
        let y = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(200.0);
        (
            App {
                palette: mk::Palette::load().unwrap_or_default(),
                x,
                y,
                dock,
                clips,
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Starten(i) => {
                // "name bereich" → Wunsch-Datei, dann starten (Fusion R41).
                let ziel = EINTRAEGE[i].2;
                let mut teile = ziel.splitn(2, ' ');
                let name = teile.next().unwrap_or(ziel);
                if let Some(bereich) = teile.next() {
                    let basis = std::env::var("XDG_RUNTIME_DIR")
                        .unwrap_or_else(|_| String::from("/tmp"));
                    let _ = std::fs::write(format!("{basis}/{name}-bereich"), bereich);
                }
                mkw::leiste::app_starten(name);
                std::process::exit(0);
            }
            Msg::PinWechsel => {
                if let Some(d) = &self.dock {
                    let mut pins: Vec<String> = mk::einstellung::lesen("dock-pins")
                        .unwrap_or_default()
                        .split_whitespace()
                        .map(String::from)
                        .collect();
                    if d.gepinnt {
                        pins.retain(|p| p != &d.app_id);
                    } else {
                        pins.push(d.app_id.clone());
                    }
                    mk::einstellung::schreiben("dock-pins", &pins.join(" "));
                }
                std::process::exit(0);
            }
            Msg::Beenden => {
                if let Some(d) = &self.dock {
                    for id in &d.fenster {
                        mk::leinwand::fenster_schliessen(*id);
                    }
                }
                std::process::exit(0);
            }
            Msg::Clip(i) => {
                // Der Eintrag wandert zurück in die Zwischenablage — als
                // Argument übergeben, nie durch eine Shell (keine Injektion).
                if let Some(text) = self.clips.as_ref().and_then(|c| c.eintraege.get(i)) {
                    let _ = std::process::Command::new("wl-copy").arg("--").arg(text).spawn();
                }
                std::process::exit(0);
            }
            Msg::ClipsLeeren => {
                let _ = std::fs::remove_file(clips_pfad());
                std::process::exit(0);
            }
            Msg::Zu => std::process::exit(0),
            _ => {}
        }
        Task::none()
    }

    /// Menü-Pille des Zwischenablage-Verlaufs — Platzierung wie das
    /// Dock-Menü (unten verankert, um die Mitte versetzt).
    fn clips_menue(&self, c: &ClipsZiel) -> Element<'_, Msg> {
        let p = self.palette;
        let mut eintraege: Vec<mkw::ui::MenuEintrag<Msg>> = Vec::new();
        if c.eintraege.is_empty() {
            eintraege.push(mkw::ui::MenuEintrag::Punkt {
                zeichen: Some(mkw::symbol::CONTENT_COPY),
                titel: String::from("Noch nichts kopiert"),
                farbe: None,
                msg: Msg::Zu,
            });
        }
        for (i, text) in c.eintraege.iter().enumerate() {
            // Einzeilige Vorschau: Zeilenumbrüche raus, hart gekürzt.
            let mut zeile: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if zeile.chars().count() > 34 {
                zeile = zeile.chars().take(33).collect::<String>() + "…";
            }
            eintraege.push(mkw::ui::MenuEintrag::Punkt {
                zeichen: None,
                titel: zeile,
                farbe: None,
                msg: Msg::Clip(i),
            });
        }
        if !c.eintraege.is_empty() {
            eintraege.push(mkw::ui::MenuEintrag::Trenner);
            eintraege.push(mkw::ui::MenuEintrag::Punkt {
                zeichen: None,
                titel: String::from("Verlauf leeren"),
                farbe: Some(p.error),
                msg: Msg::ClipsLeeren,
            });
        }
        let pille = mkw::ui::menu_family(None, eintraege, p);
        let versatz = c.offset_x * 2.0;
        let zeile = if versatz >= 0.0 {
            row![
                Space::new().width(Length::Fill),
                Space::new().width(Length::Fixed(versatz)),
                pille,
                Space::new().width(Length::Fill),
            ]
        } else {
            row![
                Space::new().width(Length::Fill),
                pille,
                Space::new().width(Length::Fixed(-versatz)),
                Space::new().width(Length::Fill),
            ]
        };
        mouse_area(
            container(
                column![
                    Space::new().height(Length::Fill),
                    zeile,
                    Space::new().height(Length::Fixed(c.unten)),
                ],
            )
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .on_press(Msg::Zu)
        .on_right_press(Msg::Zu)
        .into()
    }


    fn dock_menue(&self, d: &DockZiel) -> Element<'_, Msg> {
        let p = self.palette;
        let mut eintraege: Vec<mkw::ui::MenuEintrag<Msg>> = vec![mkw::ui::MenuEintrag::Punkt {
            zeichen: None,
            titel: String::from(if d.gepinnt { "Loslösen" } else { "Anpinnen" }),
            farbe: None,
            msg: Msg::PinWechsel,
        }];
        if !d.fenster.is_empty() {
            eintraege.push(mkw::ui::MenuEintrag::Punkt {
                zeichen: None,
                titel: String::from("Beenden"),
                farbe: None,
                msg: Msg::Beenden,
            });
        }
        let pille = mkw::ui::menu_family(None, eintraege, p);
        // Unten verankert, horizontal um die Bildschirmmitte versetzt:
        // ein Fixed-Space von 2·offset auf einer Seite verschiebt das
        // zentrierte Element um offset (Flex-Symmetrie-Trick).
        let versatz = d.offset_x * 2.0;
        let zeile = if versatz >= 0.0 {
            row![
                Space::new().width(Length::Fill),
                Space::new().width(Length::Fixed(versatz)),
                pille,
                Space::new().width(Length::Fill),
            ]
        } else {
            row![
                Space::new().width(Length::Fill),
                pille,
                Space::new().width(Length::Fixed(-versatz)),
                Space::new().width(Length::Fill),
            ]
        };
        mouse_area(
            container(
                column![
                    Space::new().height(Length::Fill),
                    zeile,
                    Space::new().height(Length::Fixed(d.unten)),
                ],
            )
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .on_press(Msg::Zu)
        .on_right_press(Msg::Zu)
        .into()
    }
    fn view(&self) -> Element<'_, Msg> {
        if let Some(d) = &self.dock {
            return self.dock_menue(d);
        }
        if let Some(c) = &self.clips {
            return self.clips_menue(c);
        }
        let p = self.palette;
        // Die Pille exakt an der Maus — und wie das Leitbild an den Schirm-
        // rändern geklappt, wenn der Platz nach unten/rechts nicht reicht.
        let (mx, my) = (self.x, self.y);
        let hoehe = mkw::menue_hoehe(EINTRAEGE.len(), false);
        mouse_area(iced::widget::responsive(move |flaeche| {
            let eintraege: Vec<mkw::ui::MenuEintrag<Msg>> = EINTRAEGE
                .iter()
                .enumerate()
                .map(|(i, (zeichen, titel, _))| {
                    if i == 0 {
                        mkw::ui::MenuEintrag::PunktMitKuerzel {
                            zeichen: Some(*zeichen),
                            titel: String::from(*titel),
                            kuerzel: String::from("Super+Space"),
                            farbe: None,
                            msg: Msg::Starten(i),
                        }
                    } else {
                        mkw::ui::MenuEintrag::Punkt {
                            zeichen: Some(*zeichen),
                            titel: String::from(*titel),
                            farbe: None,
                            msg: Msg::Starten(i),
                        }
                    }
                })
                .collect();
            let pille = mkw::ui::menu_family(None, eintraege, p);
            let mut x = mx;
            let mut y = my;
            if x + mkw::ui::MENU_BREITE > flaeche.width {
                x = (mx - mkw::ui::MENU_BREITE).max(0.0);
            }
            if y + hoehe > flaeche.height {
                y = (my - hoehe).max(0.0);
            }
            container(
                column![
                    Space::new().height(Length::Fixed(y)),
                    row![Space::new().width(Length::Fixed(x)), pille],
                ],
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        }))
        .on_press(Msg::Zu)
        .on_right_press(Msg::Zu)
        .into()
    }
}
