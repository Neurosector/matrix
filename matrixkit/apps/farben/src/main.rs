//! Matrix Farben — die lebende System-Palette zum Anfassen.
//!
//! Zweite MatrixKit-App und erster Nutzer des matrixkit-widgets-Crates:
//! Header, Resize-Griffe und Fensteraufbau kommen komplett aus der
//! Bibliothek — die App selbst ist nur noch Inhalt. Zeigt alle
//! Material-Rollen der aktuellen Palette, live beim Wallpaper-Wechsel;
//! ein Klick kopiert den Hex-Wert.

use iced::widget::{column, container, row, Space};
use iced::{Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::time::Duration;

const APP_ID: &str = "matrix-farben";

fn main() -> iced::Result {
    // Einzelinstanz wie das Leitbild: läuft die App schon, wird sie fokussiert
    if !mk::fenster::einzelinstanz(APP_ID) {
        return Ok(());
    }
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| String::from("Matrix Farben"))
        .subscription(App::subscription)
        .window(mkw::fenster_settings("matrix-farben", 380.0, 520.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

struct App {
    rahmen: mkw::Rahmen,
    /// Zuletzt kopierter Hex-Wert — kurze Rueckmeldung in der Fusszeile.
    kopiert: Option<String>,
    /// Hinweis, wenn eine Aktion an fehlender Berechtigung scheiterte.
    verweigert: bool,
    /// Tastatur-Fokus über die Farbrollen (Tab wandert, Enter kopiert).
    fokus: mkw::Fokus,
    /// Some(Rollen-Index) = Kontextmenü offen.
    menue: Option<usize>,
}

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Tick,
    Taste(mkw::Taste),
    Kopieren(String),
    Menue(usize),
    MenueZu,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        (
            Self {
                rahmen: mkw::Rahmen::neu(APP_ID, &[mk::rechte::Recht::Zwischenablage]),
                kopiert: None,
                verweigert: false,
                fokus: mkw::Fokus::neu(13),
                // Dev-Haken für Screenshots: Kontextmenü zur Rolle N öffnen
                menue: std::env::var("MATRIXKIT_MENUE_OFFEN")
                    .ok()
                    .and_then(|v| v.parse().ok()),
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(m) => self.rahmen.update(m).map(Msg::Rahmen),
            Msg::Tick => {
                self.rahmen.palette_geaendert();
                // Kopiert-Hinweis nach dem naechsten Takt wieder ausblenden
                self.kopiert = None;
                self.verweigert = false;
                Task::none()
            }
            Msg::Menue(i) => {
                self.menue = Some(i.min(12));
                return Task::none();
            }
            Msg::MenueZu => {
                self.menue = None;
                return Task::none();
            }
            Msg::Kopieren(hex) => {
                self.menue = None;
                // BINDEND: ohne Zwischenablage-Recht findet kein Zugriff statt
                if !self.rahmen.rechte.erlaubt(mk::rechte::Recht::Zwischenablage) {
                    self.verweigert = true;
                    self.kopiert = None;
                    return Task::none();
                }
                self.verweigert = false;
                self.kopiert = Some(hex.clone());
                iced::clipboard::write(hex)
            }
            Msg::Taste(t) => {
                // Root-Ebene zuerst; nur wenn sie die Taste nicht verbraucht,
                // steuert sie den App-eigenen Fokus über die Farbrollen.
                if self.rahmen.taste(t) {
                    return Task::none();
                }
                match t {
                    mkw::Taste::Weiter => self.fokus.weiter(),
                    mkw::Taste::Zurueck => self.fokus.zurueck(),
                    mkw::Taste::Aktivieren => {
                        if let Some(i) = self.fokus.aktuell() {
                            let hex = rollen(self.rahmen.palette)[i].1.hex();
                            return self.update(Msg::Kopieren(hex));
                        }
                    }
                    mkw::Taste::Escape => {
                        self.menue = None;
                    }
                    mkw::Taste::Suchen => {}
                    mkw::Taste::Einstellungen => {}
                    mkw::Taste::Rueckgaengig => {}
                    mkw::Taste::Aktualisieren => {}
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch([
            self.rahmen.abo().map(Msg::Rahmen),
            mkw::tick("farben", Duration::from_secs(2)).map(|_| Msg::Tick),
            mkw::tasten_abo(Msg::Taste),
        ])
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;

        let mut liste = column![].spacing(mk::spacing::XS);
        for (i, (name, farbe)) in rollen(p).into_iter().enumerate() {
            liste = liste.push(swatch(i, name, farbe, p, self.fokus.ist(i)));
        }

        let fusstext = if self.verweigert {
            String::from("Zwischenablage-Berechtigung ist aus (App-Name \u{2192} Berechtigungen)")
        } else {
            match &self.kopiert {
                Some(hex) => format!("{hex} kopiert \u{2713}"),
                None => format!(
                    "{} · Klick kopiert den Hex-Wert",
                    if p.is_light { "Hell-Modus" } else { "Dunkel-Modus" }
                ),
            }
        };

        let inhalt = container(
            column![
                self.rahmen.scrollflaeche(liste.into(), Msg::Rahmen),
                mkw::fusszeile(fusstext, p),
            ]
            .spacing(0),
        )
        .padding(mk::spacing::L)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(color(p.surface_container).into()),
            border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
            ..Default::default()
        });

        let fenster = self.rahmen.fenster(
            "Matrix Farben",
            env!("CARGO_PKG_VERSION"),
            "Die lebende System-Palette zum Anfassen — jede Rolle, live aus dem Wallpaper.",
            inhalt.into(),
            Msg::Rahmen,
        );

        // Kontextmenü an der Klickstelle — die Mausposition trackt der Rahmen.
        let mit_maus: Element<'_, Msg> = fenster;
        let punkte = self
            .menue
            .map(|i| {
                let (name, farbe) = rollen(p)[i];
                vec![
                    mkw::MenuePunkt {
                        label: "Hex kopieren",
                        symbol: Some(mkw::symbol::CONTENT_COPY),
                        destruktiv: false,
                        msg: Msg::Kopieren(farbe.hex()),
                    },
                    mkw::MenuePunkt {
                        label: "Rollenname kopieren",
                        symbol: None,
                        destruktiv: false,
                        msg: Msg::Kopieren(name.to_string()),
                    },
                ]
            })
            .unwrap_or_default();
        mkw::kontextmenue(mit_maus, punkte, self.menue.map(|_| self.rahmen.geist.maus), Msg::MenueZu, p)
    }
}

/// Alle Material-Rollen der Palette, in Anzeige-Reihenfolge.
fn rollen(p: mk::Palette) -> [(&'static str, mk::Rgba); 13] {
    [
        ("primary", p.primary),
        ("on_primary", p.on_primary),
        ("primary_container", p.primary_container),
        ("on_primary_container", p.on_primary_container),
        ("secondary", p.secondary),
        ("tertiary", p.tertiary),
        ("surface", p.surface),
        ("on_surface", p.on_surface),
        ("on_surface_variant", p.on_surface_variant),
        ("surface_container", p.surface_container),
        ("surface_container_high", p.surface_container_high),
        ("outline", p.outline),
        ("error", p.error),
    ]
}

/// Eine Farbzeile: Kreis-Swatch, Rollenname, Hex — Klick kopiert, Enter auch.
fn swatch(index: usize, name: &'static str, farbe: mk::Rgba, p: mk::Palette, im_fokus: bool) -> Element<'static, Msg> {
    let hex = farbe.hex();
    iced::widget::mouse_area(
        container(
            row![
                container(Space::new().width(Length::Fixed(24.0)).height(Length::Fixed(24.0)))
                    .style(move |_| container::Style {
                        background: Some(color(farbe).into()),
                        border: iced::Border {
                            radius: mk::radius::NORMAL.into(),
                            width: 1.0,
                            color: color(p.outline.over(p.surface_container, 0.4)),
                        },
                        ..Default::default()
                    }),
                Space::new().width(mk::spacing::M),
                mkw::txt(name, mk::typo::FLIESS, p.on_surface),
                Space::new().width(Length::Fill),
                mkw::txt(hex.clone(), mk::typo::FLIESS, p.on_surface_variant),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(iced::Padding {
            top: mk::spacing::XS,
            right: mk::spacing::L + mk::spacing::XS, // Platz fuer die Scrollleiste
            bottom: mk::spacing::XS,
            left: mk::spacing::S,
        })
        .width(Length::Fill)
        .style(move |_| container::Style {
            border: mkw::fokus_ring(im_fokus, mk::CORNER_RADIUS, p),
            ..Default::default()
        }),
    )
    .on_press(Msg::Kopieren(hex))
    .on_right_press(Msg::Menue(index))
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}
