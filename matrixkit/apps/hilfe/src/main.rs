//! Matrix Hilfe — alles Wissenswerte zu MatrixKit, für Nutzer und Entwickler.
//!
//! Aufbau im Stil einer Hilfe-App: Sidebar mit Kategorien, Karten-Übersicht
//! der Artikel, Leseansicht mit Zurück-Navigation. Die Inhalte liegen in
//! inhalte.rs — neue Artikel brauchen keinen UI-Code.

mod inhalte;

use iced::widget::{column, container, row, Space};
use iced::{Element, Font, Length, Subscription, Task};
use inhalte::KATEGORIEN;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::time::Duration;

const APP_ID: &str = "matrix-hilfe";

fn main() -> iced::Result {
    // Einzelinstanz wie macOS: läuft die App schon, wird sie fokussiert
    if !mk::fenster::einzelinstanz(APP_ID) {
        return Ok(());
    }
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| String::from("Matrix Hilfe"))
        .subscription(App::subscription)
        .window(mkw::fenster_settings("matrix-hilfe", 760.0, 540.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

struct App {
    rahmen: mkw::Rahmen,
    kategorie: usize,
    /// Some(i) = Artikel i der aktiven Kategorie ist geöffnet.
    artikel: Option<usize>,
    /// Auftritts-Feder der Leseansicht (Artikel gleiten herein).
    lese_feder: mk::motion::Spring,
    /// Tastatur-Fokus: Kategorien (0..4), dann Artikel-Karten; in der
    /// Leseansicht nur der Zurück-Knopf.
    fokus: mkw::Fokus,
    /// Verlaufsnavigation wie macOS (‹ ›): besuchte Ansichten.
    verlauf: Vec<(usize, Option<usize>)>,
    vorwaerts: Vec<(usize, Option<usize>)>,
    /// Volltextsuche über alle Kategorien (macOS .searchable).
    suche: String,
}

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Tick,
    /// Eigener 60-fps-Tick für die Lese-Feder (Artikel gleiten herein).
    LeseTick,
    VerlaufZurueck,
    VerlaufVor,
    Taste(mkw::Taste),
    Kategorie(usize),
    Artikel(usize),
    Zurueck,
    Suche(String),
    SucheLeeren,
    /// Suchtreffer öffnen: (Kategorie, Artikel).
    Treffer(usize, usize),
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        // HelpLink (macOS): `matrix-hilfe --suche <begriff>` öffnet die
        // Hilfe direkt mit gefüllter Suche — so springen die ?-Knöpfe
        // der Apps in ihr Kapitel. Dev-Haken MATRIX_HILFE_SUCHE bleibt.
        let args: Vec<String> = std::env::args().collect();
        let cli_suche = args
            .iter()
            .position(|a| a == "--suche")
            .and_then(|i| args.get(i + 1))
            .cloned();
        let suche = cli_suche
            .or_else(|| std::env::var("MATRIX_HILFE_SUCHE").ok())
            .unwrap_or_default();

        // Gedächtnis (macOS SceneStorage): die Hilfe öffnet dort, wo sie
        // geschlossen wurde — außer ein Suchauftrag übersteuert das.
        let (kategorie, artikel) = if suche.is_empty() {
            Self::zustand_lesen()
        } else {
            (0, None)
        };

        (
            Self {
                rahmen: mkw::Rahmen::neu("matrix-hilfe", &[]),
                kategorie,
                artikel,
                lese_feder: mk::motion::Spring::new(1.0),
                fokus: if artikel.is_some() {
                    mkw::Fokus::neu(1)
                } else {
                    mkw::Fokus::neu(KATEGORIEN.len() + KATEGORIEN[kategorie].artikel.len())
                },
                verlauf: Vec::new(),
                vorwaerts: Vec::new(),
                suche,
            },
            Task::none(),
        )
    }

    /// Gemerkten Ort laden ("k" oder "k,a"), gegen die aktuellen
    /// Kategorien/Artikel geklemmt — Kapitel dürfen zwischen Versionen
    /// schrumpfen, ohne dass die Wiederherstellung ins Leere zeigt.
    fn zustand_lesen() -> (usize, Option<usize>) {
        let Some(wert) = mk::einstellung::lesen("zustand-hilfe") else {
            return (0, None);
        };
        let mut teile = wert.splitn(2, ',');
        let k = teile
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&k| k < KATEGORIEN.len())
            .unwrap_or(0);
        let a = teile
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&a| a < KATEGORIEN[k].artikel.len());
        (k, a)
    }

    /// Aktuellen Ort merken — nach jeder Navigation aufgerufen.
    fn zustand_merken(&self) {
        let wert = match self.artikel {
            Some(a) => format!("{},{a}", self.kategorie),
            None => format!("{}", self.kategorie),
        };
        mk::einstellung::schreiben("zustand-hilfe", &wert);
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(m) => self.rahmen.update(m).map(Msg::Rahmen),
            Msg::Tick => {
                self.rahmen.palette_geaendert();
                Task::none()
            }
            Msg::LeseTick => {
                self.lese_feder.tick(1.0 / 60.0);
                Task::none()
            }
            Msg::Kategorie(i) => {
                self.verlauf.push((self.kategorie, self.artikel));
                self.vorwaerts.clear();
                self.kategorie = i.min(KATEGORIEN.len() - 1);
                self.artikel = None;
                self.fokus.setze_anzahl(KATEGORIEN.len() + KATEGORIEN[self.kategorie].artikel.len());
                self.zustand_merken();
                Task::none()
            }
            Msg::Artikel(i) => {
                self.verlauf.push((self.kategorie, self.artikel));
                self.vorwaerts.clear();
                self.artikel = Some(i);
                self.fokus = mkw::Fokus::neu(1); // nur der Zurück-Knopf
                // Leseansicht gleitet herein (bei reduzierter Bewegung: sofort)
                self.lese_feder = mk::motion::Spring::new(if mk::bewegung_reduziert() { 1.0 } else { 0.0 });
                self.lese_feder.retarget(1.0);
                self.zustand_merken();
                Task::none()
            }
            Msg::Zurueck => {
                self.verlauf.push((self.kategorie, self.artikel));
                self.vorwaerts.clear();
                self.artikel = None;
                self.fokus = mkw::Fokus::neu(KATEGORIEN.len() + KATEGORIEN[self.kategorie].artikel.len());
                self.zustand_merken();
                Task::none()
            }
            Msg::VerlaufZurueck => {
                if let Some((k, a)) = self.verlauf.pop() {
                    self.vorwaerts.push((self.kategorie, self.artikel));
                    self.kategorie = k;
                    self.artikel = a;
                    self.fokus = mkw::Fokus::neu(if a.is_some() { 1 } else { KATEGORIEN.len() + KATEGORIEN[k].artikel.len() });
                }
                self.zustand_merken();
                Task::none()
            }
            Msg::VerlaufVor => {
                if let Some((k, a)) = self.vorwaerts.pop() {
                    self.verlauf.push((self.kategorie, self.artikel));
                    self.kategorie = k;
                    self.artikel = a;
                    self.fokus = mkw::Fokus::neu(if a.is_some() { 1 } else { KATEGORIEN.len() + KATEGORIEN[k].artikel.len() });
                }
                self.zustand_merken();
                Task::none()
            }
            Msg::Suche(s) => {
                self.suche = s;
                Task::none()
            }
            Msg::SucheLeeren => {
                self.suche.clear();
                Task::none()
            }
            Msg::Treffer(k, i) => {
                self.verlauf.push((self.kategorie, self.artikel));
                self.vorwaerts.clear();
                self.kategorie = k.min(KATEGORIEN.len() - 1);
                self.artikel = Some(i);
                self.suche.clear();
                self.fokus = mkw::Fokus::neu(1);
                self.lese_feder = mk::motion::Spring::new(if mk::bewegung_reduziert() { 1.0 } else { 0.0 });
                self.lese_feder.retarget(1.0);
                self.zustand_merken();
                Task::none()
            }
            Msg::Taste(t) => {
                // Root-Ebene zuerst; nur wenn sie die Taste nicht verbraucht,
                // steuert sie die App-eigene Kategorien-/Artikel-Navigation.
                if self.rahmen.taste(t) {
                    return Task::none();
                }
                match t {
                    mkw::Taste::Escape => {
                        // dismissSearch (macOS): Esc räumt zuerst die Suche
                        if !self.suche.is_empty() {
                            self.suche.clear();
                            return Task::none();
                        }
                        if self.artikel.is_some() {
                            return self.update(Msg::Zurueck);
                        }
                    }
                    mkw::Taste::Weiter => self.fokus.weiter(),
                    mkw::Taste::Zurueck => self.fokus.zurueck(),
                    mkw::Taste::Aktivieren => {
                        if let Some(i) = self.fokus.aktuell() {
                            if self.artikel.is_some() {
                                return self.update(Msg::Zurueck);
                            }
                            return self.update(if i < KATEGORIEN.len() {
                                Msg::Kategorie(i)
                            } else {
                                Msg::Artikel(i - KATEGORIEN.len())
                            });
                        }
                    }
                    // Strg+F (macOS keyboardShortcut): Suche fokussieren
                    mkw::Taste::Suchen => return mkw::suche_fokussieren(),
                    mkw::Taste::Einstellungen => {}
                    mkw::Taste::Rueckgaengig => {}
                    mkw::Taste::Aktualisieren => {}
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        let mut abos = vec![
            self.rahmen.abo().map(Msg::Rahmen),
            mkw::tick("hilfe", Duration::from_secs(2)).map(|_| Msg::Tick),
            mkw::tasten_abo(Msg::Taste),
        ];
        // Die Leseansicht gleitet mit eigener Feder herein.
        if !self.lese_feder.is_settled() {
            abos.push(mkw::tick("hilfe-lese", Duration::from_millis(16)).map(|_| Msg::LeseTick));
        }
        Subscription::batch(abos)
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let kat = &KATEGORIEN[self.kategorie];

        // MatrixUI SidebarFamily: Einträge liefern, Anatomie kommt mit.
        let symbole = [mkw::symbol::ROCKET_LAUNCH, mkw::symbol::TOUCH_APP, mkw::symbol::SHIELD, mkw::symbol::PALETTE, mkw::symbol::CODE];
        let punkte: Vec<mkw::SidebarPunkt> = KATEGORIEN
            .iter()
            .enumerate()
            .map(|(i, k)| mkw::SidebarPunkt {
                zeichen: symbole[i.min(symbole.len() - 1)],
                titel: k.name,
                anzahl: Some(k.artikel.len()),
            })
            .collect();

        // --- Inhalt: Suche, Kartenübersicht oder Leseansicht ---
        let detail: Element<'_, Msg> = if !self.suche.is_empty() {
            // Volltext-Treffer über ALLE Kategorien (macOS .searchable)
            let anfrage = self.suche.to_lowercase();
            let mut liste = column![].spacing(mk::spacing::M);
            let mut anzahl = 0usize;
            for (ki, k) in KATEGORIEN.iter().enumerate() {
                for (ai, a) in k.artikel.iter().enumerate() {
                    let passt = a.titel.to_lowercase().contains(&anfrage)
                        || a.teaser.to_lowercase().contains(&anfrage)
                        || a.inhalt.to_lowercase().contains(&anfrage);
                    if !passt {
                        continue;
                    }
                    anzahl += 1;
                    liste = liste.push(
                        iced::widget::button(
                            column![
                                mkw::txt(k.name, mk::typo::ETIKETT, p.primary),
                                mkw::txt(a.titel, mk::typo::UNTERTITEL, p.on_surface),
                                Space::new().height(mk::spacing::XS),
                                mkw::txt(a.teaser, mk::typo::FLIESS, p.on_surface_variant),
                            ]
                            .spacing(2)
                            .width(Length::Fill),
                        )
                        .width(Length::Fill)
                        .padding(mk::spacing::L)
                        .on_press(Msg::Treffer(ki, ai))
                        .style(move |_, status| {
                            let base = p.surface_container_high;
                            let bg = match status {
                                iced::widget::button::Status::Hovered => {
                                    p.on_surface.over(base, mk::state_layer::HOVER)
                                }
                                iced::widget::button::Status::Pressed => {
                                    p.on_surface.over(base, mk::state_layer::PRESSED)
                                }
                                _ => base,
                            };
                            // familien-ausnahme: Hilfe-Navigation: Karten/Zurück mit Fokusring — eigene Nav-Grammatik
                            iced::widget::button::Style {
                                background: Some(color(bg).into()),
                                border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                                ..Default::default()
                            }
                        }),
                    );
                }
            }
            if anzahl == 0 {
                liste = liste.push(mkw::txt(
                    format!("Keine Treffer für „{}“ — anders formulieren oder kürzer suchen.", self.suche),
                    mk::typo::FLIESS,
                    p.on_surface_variant,
                ));
            }
            column![
                mkw::txt(
                    if anzahl == 1 { String::from("1 Treffer") } else { format!("{anzahl} Treffer") },
                    mk::typo::TITEL,
                    p.on_surface,
                ),
                Space::new().height(mk::spacing::M),
                self.rahmen.scrollflaeche(liste.into(), Msg::Rahmen),
            ]
            .spacing(0)
            .into()
        } else { match self.artikel {
            None => {
                // Karten der Kategorie (eine Spalte — ruhig und lesbar)
                let mut karten = column![].spacing(mk::spacing::M);
                for (i, a) in kat.artikel.iter().enumerate() {
                    let karte_fokus = self.fokus.ist(KATEGORIEN.len() + i);
                    karten = karten.push(
                        iced::widget::button(
                            column![
                                mkw::txt(a.titel, mk::typo::UNTERTITEL, p.on_surface),
                                Space::new().height(mk::spacing::XS),
                                mkw::txt(a.teaser, mk::typo::FLIESS, p.on_surface_variant),
                            ]
                            .spacing(0)
                            .width(Length::Fill),
                        )
                        .width(Length::Fill)
                        .padding(mk::spacing::L)
                        .on_press(Msg::Artikel(i))
                        .style(move |_, status| {
                            let base = p.surface_container_high;
                            let bg = match status {
                                iced::widget::button::Status::Hovered => {
                                    p.on_surface.over(base, mk::state_layer::HOVER)
                                }
                                iced::widget::button::Status::Pressed => {
                                    p.on_surface.over(base, mk::state_layer::PRESSED)
                                }
                                _ => base,
                            };
                            // familien-ausnahme: Hilfe-Navigation: Karten/Zurück mit Fokusring — eigene Nav-Grammatik
                            iced::widget::button::Style {
                                background: Some(color(bg).into()),
                                border: mkw::fokus_ring(karte_fokus, mk::CORNER_RADIUS, p),
                                ..Default::default()
                            }
                        }),
                    );
                }
                column![
                    mkw::txt(kat.name, mk::typo::TITEL, p.on_surface),
                    Space::new().height(mk::spacing::M),
                    self.rahmen.scrollflaeche(karten.into(), Msg::Rahmen),
                ]
                .spacing(0)
                .into()
            }
            Some(i) => {
                let a = &kat.artikel[i.min(kat.artikel.len() - 1)];
                let lf = self.lese_feder.value.clamp(0.0, 1.2);
                let einzug = (24.0 * (1.0 - lf)).max(0.0);
                let zurueck_fokus = self.fokus.ist(0);
                let zurueck = iced::widget::button(
                    row![
                        mkw::symbol::<Msg>(mkw::symbol::ARROW_BACK, mk::font_size::MEDIUM, p.primary),
                        Space::new().width(mk::spacing::XS),
                        mkw::txt(kat.name, mk::typo::FLIESS, p.primary),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([mk::spacing::XS as u16, mk::spacing::S as u16])
                .on_press(Msg::Zurueck)
                .style(move |_, status| {
                    let base = p.surface_container;
                    let bg = match status {
                        iced::widget::button::Status::Hovered => {
                            Some(color(p.on_surface.over(base, mk::state_layer::HOVER)).into())
                        }
                        iced::widget::button::Status::Pressed => {
                            Some(color(p.on_surface.over(base, mk::state_layer::PRESSED)).into())
                        }
                        _ => None,
                    };
                    // familien-ausnahme: Hilfe-Navigation: Karten/Zurück mit Fokusring — eigene Nav-Grammatik
                    iced::widget::button::Style {
                        background: bg,
                        border: mkw::fokus_ring(zurueck_fokus, mk::CORNER_RADIUS, p),
                        ..Default::default()
                    }
                });
                column![
                    zurueck,
                    Space::new().height(Length::Fixed(mk::spacing::M + einzug)),
                    mkw::txt(a.titel, mk::typo::TITEL, p.on_surface),
                    Space::new().height(mk::spacing::M),
                    self.rahmen.scrollflaeche(
                        container(
                            mkw::txt(a.inhalt, mk::typo::FLIESS, p.on_surface),
                        )
                        .padding(iced::Padding {
                            top: 0.0,
                            right: mk::spacing::L + mk::spacing::XS,
                            bottom: mk::spacing::L,
                            left: 0.0,
                        })
                        .into(),
                        Msg::Rahmen,
                    ),
                ]
                .spacing(0)
                .into()
            }
        } };

        // macOS-Verlaufspille: ‹ › — gedimmt, wenn nichts im Verlauf liegt
        let pfeil = |zeichen: char, aktiv: bool, msg: Msg| {
            let farbe = if aktiv { p.on_surface } else { p.on_surface.over(p.surface_container, 0.35) };
            let mut b = iced::widget::button(mkw::symbol::<Msg>(zeichen, mk::font_size::LARGE, farbe))
                .padding([2, mk::spacing::S as u16])
                .style(move |_, status| {
                    let bg = match status {
                        iced::widget::button::Status::Hovered if aktiv => Some(
                            color(p.on_surface.over(p.surface_container, mk::state_layer::HOVER)).into(),
                        ),
                        _ => None,
                    };
                    // familien-ausnahme: Hilfe-Navigation: Karten/Zurück mit Fokusring — eigene Nav-Grammatik
                    iced::widget::button::Style {
                        background: bg,
                        border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
                        ..Default::default()
                    }
                });
            if aktiv {
                b = b.on_press(msg);
            }
            b
        };
        let verlaufspille = row![
            pfeil(mkw::symbol::ARROW_BACK, !self.verlauf.is_empty(), Msg::VerlaufZurueck),
            pfeil(mkw::symbol::CHEVRON_RIGHT, !self.vorwaerts.is_empty(), Msg::VerlaufVor),
        ]
        .spacing(2);
        let kopfzeile = row![
            verlaufspille,
            Space::new().width(Length::Fill),
            container(mkw::suchfeld(&self.suche, "Hilfe durchsuchen", Msg::Suche, Msg::SucheLeeren, p))
                .width(Length::Fixed(240.0)),
        ]
        .align_y(iced::Alignment::Center);
        let detail = column![kopfzeile, Space::new().height(mk::spacing::S), detail].spacing(0);

        let inhalt = mkw::ui::sidebar_family(
            punkte,
            self.kategorie,
            self.artikel.is_none().then(|| self.fokus.aktuell()).flatten(),
            Msg::Kategorie,
            detail.into(),
            p,
        );

        self.rahmen.fenster(
            "Matrix Hilfe",
            env!("CARGO_PKG_VERSION"),
            "Alles Wissenswerte zu MatrixKit — für Nutzer und Entwickler.",
            inhalt.into(),
            Msg::Rahmen,
        )
    }
}
