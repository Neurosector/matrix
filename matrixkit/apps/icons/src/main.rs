//! Matrix Icons — App #22, der Icon Composer.
//!
//! Unsere Fassung von Leitbild- Icon Composer: Icons werden aus EBENEN
//! komponiert (Squircle/RoundRect auf den lebenden Palette-Slots
//! a/b/c), die große Vorschau rendert mit der AKTUELLEN Palette über
//! die echte Icon-Pipeline (Kachel + Schattenlage), und „Sichern"
//! schreibt ein Rezept nach ~/.config/matrix/icons/<app-id>.json —
//! ab da wirkt die Komposition SYSTEMWEIT (Dock, Launcher,
//! Mitteilungen) und färbt mit jedem Hintergrundwechsel um.
//!
//! MatrixUI: Ebenen-Liste = SidebarFamily-Anatomie, Ebenen-Aktionen =
//! nav_knopf (HarnessFamily), Formular = Kit-Zeilen + Stepper.

use iced::widget::{column, container, image, row, Space};
use iced::{Element, Length, Subscription, Task};
use matrixkit_icons as icons;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use matrixkit_widgets::color;

const APP_ID: &str = "matrix-icons";

fn main() -> iced::Result {
    if !mk::fenster::einzelinstanz(APP_ID) {
        return Ok(());
    }
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| String::from("Matrix Icons"))
        .subscription(App::subscription)
        .window(mkw::fenster_settings(APP_ID, 860.0, 640.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(iced::Font::with_name("Inter Variable"))
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Feld {
    A,
    B,
    C,
    D,
    E,
}

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Taste(mkw::Taste),
    Tick,
    Ebene(usize),
    EbeneNeu,
    EbeneWeg,
    EbeneHoch,
    EbeneRunter,
    FormSquircle,
    FormRect,
    Slot(&'static str),
    DeckkraftPlus,
    DeckkraftMinus,
    MischungWechsel,
    GlanzWechsel,
    Plus(Feld),
    Minus(Feld),
    AppId(String),
    Sichern,
}

struct App {
    rahmen: mkw::Rahmen,
    rezept: icons::Rezept,
    gewaehlt: usize,
    app_id: String,
    vorschau: Option<image::Handle>,
    quittung: Option<String>,
}

fn start_rezept() -> icons::Rezept {
    // Der Photos-Gruß: drei transluzente Blätter, multiplikativ gemischt.
    let blatt = |cx: f32, cy: f32, slot: &str| icons::RezeptEbene {
        form: icons::RezeptForm::Squircle { cx, cy, r: 52.0, n: 2.0 },
        slot: String::from(slot),
        deckkraft: 0.72,
        mischung: String::from("multiplizieren"),
        glanz: true,
    };
    icons::Rezept {
        ebenen: vec![
            blatt(104.0, 108.0, "a"),
            blatt(152.0, 108.0, "b"),
            blatt(128.0, 152.0, "c"),
        ],
    }
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let mut app = App {
            rahmen: mkw::Rahmen::neu(APP_ID, &[]),
            rezept: start_rezept(),
            gewaehlt: 0,
            app_id: String::from("meine-app"),
            vorschau: None,
            quittung: None,
        };
        app.rendern();
        (app, Task::none())
    }

    fn rendern(&mut self) {
        self.vorschau = icons::rezept_png(&self.rezept, &self.rahmen.palette)
            .map(image::Handle::from_bytes);
    }

    fn ebene(&mut self) -> Option<&mut icons::RezeptEbene> {
        self.rezept.ebenen.get_mut(self.gewaehlt)
    }

    fn schieben(&mut self, feld: Feld, delta: f32) {
        if let Some(e) = self.ebene() {
            match &mut e.form {
                icons::RezeptForm::Squircle { cx, cy, r, n } => match feld {
                    Feld::A => *cx = (*cx + delta).clamp(0.0, 256.0),
                    Feld::B => *cy = (*cy + delta).clamp(0.0, 256.0),
                    Feld::C => *r = (*r + delta).clamp(4.0, 128.0),
                    Feld::D => *n = (*n + delta / 8.0).clamp(2.0, 6.0),
                    Feld::E => {}
                },
                icons::RezeptForm::RoundRect { x1, y1, x2, y2, r } => match feld {
                    Feld::A => *x1 = (*x1 + delta).clamp(0.0, 256.0),
                    Feld::B => *y1 = (*y1 + delta).clamp(0.0, 256.0),
                    Feld::C => *x2 = (*x2 + delta).clamp(0.0, 256.0),
                    Feld::D => *y2 = (*y2 + delta).clamp(0.0, 256.0),
                    Feld::E => *r = (*r + delta).clamp(0.0, 64.0),
                },
            }
        }
        self.rendern();
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(r) => return self.rahmen.update(r).map(Msg::Rahmen),
            Msg::Taste(t) => {
                let _ = self.rahmen.taste(t);
            }
            Msg::Tick => {
                if self.rahmen.palette_geaendert() {
                    self.rendern();
                }
            }
            Msg::Ebene(i) => self.gewaehlt = i.min(self.rezept.ebenen.len().saturating_sub(1)),
            Msg::EbeneNeu => {
                self.rezept.ebenen.push(icons::RezeptEbene {
                    form: icons::RezeptForm::Squircle { cx: 128.0, cy: 128.0, r: 40.0, n: 2.0 },
                    slot: String::from("c"),
                    deckkraft: 1.0,
                    mischung: String::from("normal"),
                    glanz: false,
                });
                self.gewaehlt = self.rezept.ebenen.len() - 1;
                self.rendern();
            }
            Msg::EbeneWeg => {
                if self.rezept.ebenen.len() > 1 {
                    self.rezept.ebenen.remove(self.gewaehlt);
                    self.gewaehlt = self.gewaehlt.min(self.rezept.ebenen.len() - 1);
                    self.rendern();
                }
            }
            Msg::EbeneHoch => {
                if self.gewaehlt > 0 {
                    self.rezept.ebenen.swap(self.gewaehlt, self.gewaehlt - 1);
                    self.gewaehlt -= 1;
                    self.rendern();
                }
            }
            Msg::EbeneRunter => {
                if self.gewaehlt + 1 < self.rezept.ebenen.len() {
                    self.rezept.ebenen.swap(self.gewaehlt, self.gewaehlt + 1);
                    self.gewaehlt += 1;
                    self.rendern();
                }
            }
            Msg::FormSquircle => {
                if let Some(e) = self.ebene() {
                    if !matches!(e.form, icons::RezeptForm::Squircle { .. }) {
                        e.form =
                            icons::RezeptForm::Squircle { cx: 128.0, cy: 128.0, r: 56.0, n: 2.0 };
                    }
                }
                self.rendern();
            }
            Msg::FormRect => {
                if let Some(e) = self.ebene() {
                    if !matches!(e.form, icons::RezeptForm::RoundRect { .. }) {
                        e.form = icons::RezeptForm::RoundRect {
                            x1: 88.0,
                            y1: 88.0,
                            x2: 168.0,
                            y2: 168.0,
                            r: 16.0,
                        };
                    }
                }
                self.rendern();
            }
            Msg::Slot(slot) => {
                if let Some(e) = self.ebene() {
                    e.slot = String::from(slot);
                }
                self.rendern();
            }
            Msg::DeckkraftPlus => {
                if let Some(e) = self.ebene() {
                    e.deckkraft = (e.deckkraft + 0.05).min(1.0);
                }
                self.rendern();
            }
            Msg::DeckkraftMinus => {
                if let Some(e) = self.ebene() {
                    e.deckkraft = (e.deckkraft - 0.05).max(0.15);
                }
                self.rendern();
            }
            Msg::MischungWechsel => {
                if let Some(e) = self.ebene() {
                    e.mischung = if e.mischung == "multiplizieren" {
                        String::from("normal")
                    } else {
                        String::from("multiplizieren")
                    };
                }
                self.rendern();
            }
            Msg::GlanzWechsel => {
                if let Some(e) = self.ebene() {
                    e.glanz = !e.glanz;
                }
                self.rendern();
            }
            Msg::Plus(f) => self.schieben(f, 4.0),
            Msg::Minus(f) => self.schieben(f, -4.0),
            Msg::AppId(s) => {
                self.app_id = s
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                    .collect::<String>()
                    .to_lowercase();
            }
            Msg::Sichern => {
                let name = if self.app_id.is_empty() { "meine-app" } else { &self.app_id };
                self.quittung = match icons::rezept_speichern(name, &self.rezept) {
                    Ok(()) => {
                        mk::feedback::erfolg();
                        Some(format!(
                            "Gesichert - wirkt systemweit als {name} (Dock und Launcher)"
                        ))
                    }
                    Err(e) => Some(format!("Sichern fehlgeschlagen: {e}")),
                };
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch([
            self.rahmen.abo().map(Msg::Rahmen),
            mkw::tick("icons", std::time::Duration::from_secs(2)).map(|_| Msg::Tick),
            mkw::tasten_abo(Msg::Taste),
        ])
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;

        // --- SidebarFamily: die Ebenen-Liste (unten = vorn) ---
        let punkte: Vec<mkw::SidebarPunkt> = self
            .rezept
            .ebenen
            .iter()
            .enumerate()
            .map(|(i, e)| mkw::SidebarPunkt {
                zeichen: match e.form {
                    icons::RezeptForm::Squircle { .. } => mkw::symbol::PALETTE,
                    icons::RezeptForm::RoundRect { .. } => mkw::symbol::CODE,
                },
                titel: match e.form {
                    icons::RezeptForm::Squircle { .. } => match i {
                        0 => "Squircle (hinten)",
                        _ => "Squircle",
                    },
                    icons::RezeptForm::RoundRect { .. } => "Rechteck",
                },
                anzahl: None,
            })
            .collect();

        // --- Detail: Vorschau + Ebenen-Aktionen + Eigenschaften ---
        let vorschau: Element<'_, Msg> = match &self.vorschau {
            Some(h) => image(h.clone()).width(220).height(220).into(),
            None => Space::new().into(),
        };
        let aktionen = row![
            mkw::ui::nav_knopf(mkw::symbol::ADD, true, Msg::EbeneNeu, p),
            mkw::ui::nav_knopf(mkw::symbol::REMOVE, self.rezept.ebenen.len() > 1, Msg::EbeneWeg, p),
            mkw::ui::nav_knopf(mkw::symbol::ARROW_UPWARD, self.gewaehlt > 0, Msg::EbeneHoch, p),
            mkw::ui::nav_knopf(
                mkw::symbol::ARROW_DOWNWARD,
                self.gewaehlt + 1 < self.rezept.ebenen.len(),
                Msg::EbeneRunter,
                p,
            ),
        ]
        .spacing(mk::spacing::XS);

        let (ist_squircle, slot) = self
            .rezept
            .ebenen
            .get(self.gewaehlt)
            .map(|e| (matches!(e.form, icons::RezeptForm::Squircle { .. }), e.slot.clone()))
            .unwrap_or((true, String::from("a")));

        let form_wahl = row![
            mkw::ui::familien_knopf(None, Some("Squircle"), ist_squircle, Msg::FormSquircle, p, mk::typo::FLIESS),
            mkw::ui::familien_knopf(None, Some("Rechteck"), !ist_squircle, Msg::FormRect, p, mk::typo::FLIESS),
        ]
        .spacing(mk::spacing::XS);

        // Slot-Kacheln: die lebenden Töne a/b/c als Farbfelder.
        let slot_kachel = |name: &'static str, farbe: mk::Rgba| {
            let aktiv = slot == name;
            mkw::lupe(
                iced::widget::button(Space::new().width(28).height(28))
                    .padding(0)
                    .on_press(Msg::Slot(name))
                    // familien-ausnahme: Farb-Swatch: der Knopf IST die Farbe
                    .style(move |_, _| iced::widget::button::Style {
                        background: Some(color(farbe).into()),
                        border: iced::Border {
                            color: if aktiv { color(p.on_surface) } else { color(farbe) },
                            width: if aktiv { 2.0 } else { 0.0 },
                            radius: mk::radius::KLEIN.into(),
                        },
                        ..Default::default()
                    }),
            )
        };
        let slots = row![
            slot_kachel("a", p.primary),
            slot_kachel("b", p.secondary),
            slot_kachel("c", p.tertiary),
        ]
        .spacing(mk::spacing::S);

        // Material (Icon-Composer-Kern): Deckkraft, Mischung, Glanz.
        let (deckkraft, multipliziert, glanz_an) = self
            .rezept
            .ebenen
            .get(self.gewaehlt)
            .map(|e| (e.deckkraft, e.mischung == "multiplizieren", e.glanz))
            .unwrap_or((1.0, false, false));
        let material = column![
            row![
                container(mkw::txt("Deckkraft", mk::typo::FLIESS, p.on_surface))
                    .width(Length::Fixed(110.0)),
                mkw::stepper(
                    format!("{:.0} %", deckkraft * 100.0),
                    Some(Msg::DeckkraftMinus),
                    Some(Msg::DeckkraftPlus),
                    p,
                ),
            ]
            .spacing(mk::spacing::S)
            .align_y(iced::Alignment::Center),
            row![
                mkw::ui::familien_knopf(None, Some("Normal"), !multipliziert, Msg::MischungWechsel, p, mk::typo::FLIESS),
                mkw::ui::familien_knopf(None, Some("Multiplizieren"), multipliziert, Msg::MischungWechsel, p, mk::typo::FLIESS),
                Space::new().width(mk::spacing::M),
                mkw::txt("Glanz", mk::typo::FLIESS, p.on_surface),
                mkw::schalter(glanz_an, p, Msg::GlanzWechsel),
            ]
            .spacing(mk::spacing::XS)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(mk::spacing::XS);

        // Parameter-Stepper je Form.
        let wert = |f: Feld| -> String {
            self.rezept
                .ebenen
                .get(self.gewaehlt)
                .map(|e| match (&e.form, f) {
                    (icons::RezeptForm::Squircle { cx, .. }, Feld::A) => format!("{cx:.0}"),
                    (icons::RezeptForm::Squircle { cy, .. }, Feld::B) => format!("{cy:.0}"),
                    (icons::RezeptForm::Squircle { r, .. }, Feld::C) => format!("{r:.0}"),
                    (icons::RezeptForm::Squircle { n, .. }, Feld::D) => format!("{n:.1}"),
                    (icons::RezeptForm::RoundRect { x1, .. }, Feld::A) => format!("{x1:.0}"),
                    (icons::RezeptForm::RoundRect { y1, .. }, Feld::B) => format!("{y1:.0}"),
                    (icons::RezeptForm::RoundRect { x2, .. }, Feld::C) => format!("{x2:.0}"),
                    (icons::RezeptForm::RoundRect { y2, .. }, Feld::D) => format!("{y2:.0}"),
                    (icons::RezeptForm::RoundRect { r, .. }, Feld::E) => format!("{r:.0}"),
                    _ => String::from("—"),
                })
                .unwrap_or_default()
        };
        let stepper_zeile = |label: &'static str, f: Feld| {
            row![
                container(mkw::txt(label, mk::typo::FLIESS, p.on_surface))
                    .width(Length::Fixed(110.0)),
                mkw::stepper(wert(f), Some(Msg::Minus(f)), Some(Msg::Plus(f)), p),
            ]
            .spacing(mk::spacing::S)
            .align_y(iced::Alignment::Center)
        };
        let mut parameter = column![].spacing(mk::spacing::XS);
        if ist_squircle {
            parameter = parameter
                .push(stepper_zeile("Mitte X", Feld::A))
                .push(stepper_zeile("Mitte Y", Feld::B))
                .push(stepper_zeile("Radius", Feld::C))
                .push(stepper_zeile("Eckigkeit", Feld::D));
        } else {
            parameter = parameter
                .push(stepper_zeile("Links", Feld::A))
                .push(stepper_zeile("Oben", Feld::B))
                .push(stepper_zeile("Rechts", Feld::C))
                .push(stepper_zeile("Unten", Feld::D))
                .push(stepper_zeile("Rundung", Feld::E));
        }

        let ziel = row![
            container(mkw::eingabefeld(
                "app-id (z. B. firefox)",
                &self.app_id,
                Msg::AppId,
                Some(Msg::Sichern),
                false,
                p,
            ))
            .width(Length::Fixed(220.0)),
            mkw::ui::kopf_text_knopf("Sichern", Msg::Sichern, p),
        ]
        .spacing(mk::spacing::S)
        .align_y(iced::Alignment::Center);

        let detail = column![
            container(vorschau).center_x(Length::Fill),
            container(aktionen).center_x(Length::Fill),
            mkw::trenner(p),
            form_wahl,
            slots,
            material,
            parameter,
            mkw::trenner(p),
            ziel,
        ]
        .spacing(mk::spacing::M);

        let fusstext = self.quittung.clone().unwrap_or_else(|| {
            String::from("Ebenen unten liegen vorn · Rezepte wirken sofort systemweit")
        });

        let inhalt = mkw::ui::sidebar_family(
            punkte,
            self.gewaehlt,
            None,
            Msg::Ebene,
            column![
                self.rahmen.scrollflaeche(detail.into(), Msg::Rahmen),
                mkw::fusszeile(fusstext, p),
            ]
            .spacing(0)
            .into(),
            p,
        );

        self.rahmen.fenster(
            "Matrix Icons",
            env!("CARGO_PKG_VERSION"),
            "Der Icon Composer — lebende Icons aus Ebenen, gespeichert als Rezepte, systemweit wirksam.",
            inhalt,
            Msg::Rahmen,
        )
    }
}
