//! Leiste & Dock — als Panel der Matrix Einstellungen (Fusion R41).
//! War App #24 (matrix-anpassen): Bar-Zonen, Dock-Pins und die zweite
//! Dock-Zeile bearbeiten, mit Wackel-Modus und Widget-Bibliothek.

use iced::widget::{button, column, container, row, Space};
use iced::{Alignment, Element, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::time::Duration;

/// Dock-/Katalog-Symbolmaße: an die Kachelgeometrie gebunden.
const DOCK_SYMBOL: f32 = 26.0;
const KATALOG_SYMBOL: f32 = 22.0;

// ----------------------------------------------------------------- Zonen

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Links,
    Mitte,
    Rechts,
}

impl Zone {
    const ALLE: [Zone; 3] = [Zone::Links, Zone::Mitte, Zone::Rechts];
    fn titel(self) -> &'static str {
        match self {
            Zone::Links => "Links",
            Zone::Mitte => "Mitte",
            Zone::Rechts => "Rechts",
        }
    }
}

// ---------------------------------------------------------------- Widgets

/// Die zehn Bar-Widgets — identisch zur `enum Widget` der Bar, damit die
/// Bibliothek exakt das anbietet, was die Leiste kennt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarWidget {
    Matrix,
    Fokus,
    Uhr,
    Puls,
    Glocke,
    Tray,
    Akku,
    Zentrale,
    Nutzer,
    Aufnahme,
}

impl BarWidget {
    const ALLE: [BarWidget; 10] = [
        BarWidget::Matrix,
        BarWidget::Fokus,
        BarWidget::Uhr,
        BarWidget::Puls,
        BarWidget::Glocke,
        BarWidget::Tray,
        BarWidget::Akku,
        BarWidget::Zentrale,
        BarWidget::Nutzer,
        BarWidget::Aufnahme,
    ];

    fn schluessel(self) -> &'static str {
        match self {
            BarWidget::Matrix => "matrix",
            BarWidget::Fokus => "fokus",
            BarWidget::Uhr => "uhr",
            BarWidget::Puls => "puls",
            BarWidget::Glocke => "glocke",
            BarWidget::Tray => "tray",
            BarWidget::Akku => "akku",
            BarWidget::Zentrale => "zentrale",
            BarWidget::Nutzer => "nutzer",
            BarWidget::Aufnahme => "aufnahme",
        }
    }

    fn aus(name: &str) -> Option<Self> {
        BarWidget::ALLE.into_iter().find(|w| w.schluessel() == name)
    }

    fn titel(self) -> &'static str {
        match self {
            BarWidget::Matrix => "Matrix-Menü",
            BarWidget::Fokus => "Fokus",
            BarWidget::Uhr => "Uhr",
            BarWidget::Puls => "Puls",
            BarWidget::Glocke => "Glocke",
            BarWidget::Tray => "Ablage",
            BarWidget::Akku => "Akku",
            BarWidget::Zentrale => "Zentrale",
            BarWidget::Nutzer => "Nutzer",
            BarWidget::Aufnahme => "Aufnahme",
        }
    }

    fn untertitel(self) -> &'static str {
        match self {
            BarWidget::Matrix => "Das Systemmenü ganz links",
            BarWidget::Fokus => "Titel der aktiven App",
            BarWidget::Uhr => "Uhrzeit und Datum",
            BarWidget::Puls => "CPU, RAM, Ton, Netz",
            BarWidget::Glocke => "Mitteilungen & Nicht stören",
            BarWidget::Tray => "System-Tray-Symbole",
            BarWidget::Akku => "Ladestand",
            BarWidget::Zentrale => "Kontrollzentrum",
            BarWidget::Nutzer => "Sitzungsmenü",
            BarWidget::Aufnahme => "Bildschirmfoto & -film",
        }
    }

    fn glyph(self) -> char {
        match self {
            BarWidget::Matrix => mkw::symbol::APPS,
            BarWidget::Fokus => mkw::symbol::TOUCH_APP,
            BarWidget::Uhr => mkw::symbol::SCHEDULE,
            BarWidget::Puls => mkw::symbol::MONITORING,
            BarWidget::Glocke => mkw::symbol::NOTIFICATIONS,
            BarWidget::Tray => mkw::symbol::WIDGETS,
            BarWidget::Akku => mkw::symbol::BATTERY_FULL,
            BarWidget::Zentrale => mkw::symbol::TUNE,
            BarWidget::Nutzer => mkw::symbol::PERSON,
            BarWidget::Aufnahme => mkw::symbol::IMAGE,
        }
    }
}

/// Die Bibliothek fürs Dock: die anpinnbaren Matrix-Apps (app_id, Titel,
/// Glyph). Reihenfolge = Anzeige in der Galerie.
const DOCK_KATALOG: &[(&str, &str, char)] = &[
    ("matrix-einstellungen", "Einstellungen", mkw::symbol::TUNE),
    ("matrix-farben", "Farben", mkw::symbol::PALETTE),
    ("matrix-start", "Start", mkw::symbol::SEARCH),
    ("matrix-dateien", "Dateien", mkw::symbol::FOLDER),
    ("matrix-web", "Web", mkw::symbol::PUBLIC),
    ("matrix-codes", "Codes", mkw::symbol::KEY),
    ("matrix-schluessel", "Schlüssel", mkw::symbol::USB),
    ("matrix-updater", "Updater", mkw::symbol::RESTART),
    ("matrix-mitteilungen", "Mitteilungen", mkw::symbol::NOTIFICATIONS),
    ("matrix-klaenge", "Klänge", mkw::symbol::VOLUME_UP),
    ("matrix-sysmon", "Systemmonitor", mkw::symbol::MONITORING),
    ("matrix-icons-app", "Icon Composer", mkw::symbol::APPS),
    ("matrix-aufnahme", "Aufnahme", mkw::symbol::IMAGE),
    ("matrix-hilfe", "Hilfe", mkw::symbol::HELP),
];

fn dock_titel(app_id: &str) -> &str {
    DOCK_KATALOG
        .iter()
        .find(|(id, _, _)| *id == app_id)
        .map(|(_, t, _)| *t)
        .unwrap_or(app_id)
}

fn dock_glyph(app_id: &str) -> char {
    DOCK_KATALOG
        .iter()
        .find(|(id, _, _)| *id == app_id)
        .map(|(_, _, g)| *g)
        .unwrap_or(mkw::symbol::APPS)
}

// ---- reine Serialisierung (testbar) --------------------------------------

const BAR_STANDARD: &str = "matrix fokus | uhr | akku puls tray glocke zentrale nutzer";
const DOCK_STANDARD: &str = "matrix-einstellungen";
const ZEILE2_STANDARD: &str = "apps";

/// Die Widgets der zweiten Dock-Zeile: (schluessel, Titel, Glyph, Untertitel).
const ZEILE2_KATALOG: &[(&str, &str, char, &str)] = &[
    ("apps", "Apps", mkw::symbol::APPS, "Der App-Launcher-Knopf"),
    ("zentrale", "Zentrale", mkw::symbol::TUNE, "Kontrollzentrum"),
    ("uhr", "Uhr", mkw::symbol::SCHEDULE, "Uhrzeit"),
    ("zwischenablage", "Zwischenablage", mkw::symbol::CONTENT_COPY, "Kopien-Verlauf"),
    ("aufnahme", "Aufnahme", mkw::symbol::IMAGE, "Bildschirmfoto & -film"),
];

fn zeile2_eintrag(schluessel: &str) -> Option<&'static (&'static str, &'static str, char, &'static str)> {
    ZEILE2_KATALOG.iter().find(|(id, _, _, _)| *id == schluessel)
}

fn zonen_parse(konf: &str) -> (Vec<BarWidget>, Vec<BarWidget>, Vec<BarWidget>) {
    let mut zonen = konf.splitn(3, '|').map(|z| {
        z.split_whitespace()
            .filter_map(BarWidget::aus)
            .collect::<Vec<_>>()
    });
    (
        zonen.next().unwrap_or_default(),
        zonen.next().unwrap_or_default(),
        zonen.next().unwrap_or_default(),
    )
}

fn zonen_string(l: &[BarWidget], m: &[BarWidget], r: &[BarWidget]) -> String {
    let teil = |z: &[BarWidget]| {
        z.iter()
            .map(|w| w.schluessel())
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!("{} | {} | {}", teil(l), teil(m), teil(r))
}

/// „Bewegung reduzieren" hält die Kacheln still.
fn wackeln_an() -> bool {
    mk::einstellung::lesen("bewegung-reduzieren").as_deref() != Some("an")
}

// ------------------------------------------------------------------- App

pub struct Panel {
    pub palette: mk::Palette,
    links: Vec<BarWidget>,
    mitte: Vec<BarWidget>,
    rechts: Vec<BarWidget>,
    pins: Vec<String>,
    /// Zweite Dock-Zeile (dock-widgets), nur bekannte Schlüssel.
    zeile2: Vec<String>,
    ziel: Zone,
    phase: f32,
    wackeln: bool,
}

#[derive(Debug, Clone)]
pub enum Msg {
    JiggleTick,
    ZielWahl(Zone),
    BarHinzu(BarWidget),
    BarWeg(Zone, usize),
    BarLinks(Zone, usize),
    BarRechts(Zone, usize),
    DockHinzu(String),
    DockWeg(usize),
    DockSchieben(usize, i32),
    Zeile2Hinzu(String),
    Zeile2Weg(usize),
    Zeile2Schieben(usize, i32),
    Standard,
}

impl Panel {
    pub fn new() -> Self {
        let (links, mitte, rechts) =
            zonen_parse(&mk::einstellung::lesen("bar-widgets").unwrap_or_else(|| BAR_STANDARD.into()));
        let pins = mk::einstellung::lesen("dock-pins")
            .unwrap_or_else(|| DOCK_STANDARD.into())
            .split_whitespace()
            .map(String::from)
            .collect();
        let zeile2 = mk::einstellung::lesen("dock-widgets")
            .unwrap_or_else(|| ZEILE2_STANDARD.into())
            .split_whitespace()
            .filter(|w| zeile2_eintrag(w).is_some())
            .map(String::from)
            .collect();
        Self {
            palette: mk::Palette::load().unwrap_or_default(),
            links,
            mitte,
            rechts,
            pins,
            zeile2,
            ziel: Zone::Rechts,
            phase: 0.0,
            wackeln: wackeln_an(),
        }
    }

    /// Vom Host je Tick: Palette folgt, Wackel-Kultur frisch lesen.
    pub fn tick(&mut self, p: mk::Palette) {
        self.palette = p;
        self.wackeln = wackeln_an();
    }

    fn zone_mut(&mut self, z: Zone) -> &mut Vec<BarWidget> {
        match z {
            Zone::Links => &mut self.links,
            Zone::Mitte => &mut self.mitte,
            Zone::Rechts => &mut self.rechts,
        }
    }

    fn zone_ref(&self, z: Zone) -> &[BarWidget] {
        match z {
            Zone::Links => &self.links,
            Zone::Mitte => &self.mitte,
            Zone::Rechts => &self.rechts,
        }
    }

    fn platziert(&self, w: BarWidget) -> bool {
        self.links.contains(&w) || self.mitte.contains(&w) || self.rechts.contains(&w)
    }

    /// Eine Kachel nach links: innerhalb der Zone tauschen; am linken Rand
    /// ans Ende der vorigen Zone wandern (die drei Zonen als ein Band).
    fn bar_links(&mut self, z: Zone, i: usize) {
        if i > 0 {
            self.zone_mut(z).swap(i - 1, i);
            return;
        }
        match z {
            Zone::Links => {}
            Zone::Mitte => {
                if !self.mitte.is_empty() {
                    let w = self.mitte.remove(0);
                    self.links.push(w);
                }
            }
            Zone::Rechts => {
                if !self.rechts.is_empty() {
                    let w = self.rechts.remove(0);
                    self.mitte.push(w);
                }
            }
        }
    }

    /// Eine Kachel nach rechts: innerhalb der Zone tauschen; am rechten Rand
    /// an den Anfang der nächsten Zone wandern.
    fn bar_rechts(&mut self, z: Zone, i: usize) {
        let len = self.zone_ref(z).len();
        if i + 1 < len {
            self.zone_mut(z).swap(i, i + 1);
            return;
        }
        match z {
            Zone::Rechts => {}
            Zone::Links => {
                if i < self.links.len() {
                    let w = self.links.remove(i);
                    self.mitte.insert(0, w);
                }
            }
            Zone::Mitte => {
                if i < self.mitte.len() {
                    let w = self.mitte.remove(i);
                    self.rechts.insert(0, w);
                }
            }
        }
    }

    fn sichern(&self) {
        mk::einstellung::schreiben(
            "bar-widgets",
            &zonen_string(&self.links, &self.mitte, &self.rechts),
        );
        mk::einstellung::schreiben("dock-pins", &self.pins.join(" "));
        mk::einstellung::schreiben("dock-widgets", &self.zeile2.join(" "));
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::JiggleTick => {
                // ~30 fps genügt fürs Wackeln — halbe Redraw-Last, damit die
                // Animation die (thermisch empfindliche) Surface nicht heizt.
                // Gleicher Winkel-Zuwachs pro Sekunde wie zuvor.
                self.phase += 0.32;
                if self.phase > std::f32::consts::TAU * 32.0 {
                    self.phase -= std::f32::consts::TAU * 32.0;
                }
            }
            Msg::ZielWahl(z) => self.ziel = z,
            Msg::BarHinzu(w) => {
                if !self.platziert(w) {
                    let z = self.ziel;
                    self.zone_mut(z).push(w);
                    self.sichern();
                }
            }
            Msg::BarWeg(z, i) => {
                if i < self.zone_ref(z).len() {
                    self.zone_mut(z).remove(i);
                    self.sichern();
                }
            }
            Msg::BarLinks(z, i) => {
                self.bar_links(z, i);
                self.sichern();
            }
            Msg::BarRechts(z, i) => {
                self.bar_rechts(z, i);
                self.sichern();
            }
            Msg::DockHinzu(id) => {
                if !self.pins.contains(&id) {
                    self.pins.push(id);
                    self.sichern();
                }
            }
            Msg::DockWeg(i) => {
                if i < self.pins.len() {
                    self.pins.remove(i);
                    self.sichern();
                }
            }
            Msg::DockSchieben(i, d) => {
                let j = i as i32 + d;
                if j >= 0 && (j as usize) < self.pins.len() {
                    self.pins.swap(i, j as usize);
                    self.sichern();
                }
            }
            Msg::Zeile2Hinzu(id) => {
                if !self.zeile2.contains(&id) {
                    self.zeile2.push(id);
                    self.sichern();
                }
            }
            Msg::Zeile2Weg(i) => {
                if i < self.zeile2.len() {
                    self.zeile2.remove(i);
                    self.sichern();
                }
            }
            Msg::Zeile2Schieben(i, d) => {
                let j = i as i32 + d;
                if j >= 0 && (j as usize) < self.zeile2.len() {
                    self.zeile2.swap(i, j as usize);
                    self.sichern();
                }
            }
            Msg::Standard => {
                let (l, m, r) = zonen_parse(BAR_STANDARD);
                self.links = l;
                self.mitte = m;
                self.rechts = r;
                self.pins = DOCK_STANDARD.split_whitespace().map(String::from).collect();
                self.zeile2 = ZEILE2_STANDARD.split_whitespace().map(String::from).collect();
                self.sichern();
            }
        }
        Task::none()
    }

    /// Nur der Wackel-Takt — alles andere abonniert der Host.
    pub fn abo(&self) -> Subscription<Msg> {
        if self.wackeln {
            mkw::tick("anpassen-jiggle", Duration::from_millis(33)).map(|_| Msg::JiggleTick)
        } else {
            Subscription::none()
        }
    }

    // ---- Bausteine der Ansicht -------------------------------------------

    /// Der Wackel-Versatz einer Kachel (organisch versetzt über den Startwinkel).
    fn versatz(&self, seed: f32) -> (f32, f32) {
        if !self.wackeln {
            return (0.0, 0.0);
        }
        let a = 1.8;
        (
            a * (self.phase + seed).sin(),
            a * (self.phase * 1.17 + seed * 1.7).cos(),
        )
    }

    /// Ein winziger Steuerknopf unter einer Kachel (‹ − ›).
    fn mini(&self, glyph: char, tonung: bool, on: Msg) -> Element<'_, Msg> {
        let p = self.palette;
        let farbe = if tonung { p.error } else { p.on_surface_variant };
        button(
            container(mkw::symbol::<Msg>(glyph, mk::icon_size::SMALL, farbe))
                .center_x(Length::Fixed(22.0)),
        )
        .padding(iced::Padding {
            top: 2.0,
            bottom: 2.0,
            ..iced::Padding::ZERO
        })
        .style(move |_, status| mkw::leiste::knopf_stil(p, status, mk::radius::KLEIN))
        .on_press(on)
        .into()
    }

    /// Eine platzierte, wackelnde Kachel mit Steuerzeile darunter.
    fn kachel<'a>(
        &'a self,
        glyph: char,
        name: &'a str,
        seed: f32,
        steuern: Element<'a, Msg>,
    ) -> Element<'a, Msg> {
        let p = self.palette;
        let (dx, dy) = self.versatz(seed);
        let icon = container(mkw::symbol::<Msg>(glyph, DOCK_SYMBOL, p.on_surface)).padding(iced::Padding {
            left: 2.0 + dx,
            right: 2.0 - dx,
            top: 2.0 + dy,
            bottom: 2.0 - dy,
        });
        container(
            column![
                icon,
                mkw::txt(name, mk::typo::KLEIN, p.on_surface_variant),
                steuern,
            ]
            .spacing(mk::spacing::XXS)
            .align_x(Alignment::Center),
        )
        .padding(mk::spacing::S)
        .width(Length::Fixed(96.0))
        .style(move |_| container::Style {
            background: Some(color(p.surface_container_high).into()),
            border: iced::Border {
                radius: mk::radius::NORMAL.into(),
                width: 1.0,
                color: color(p.outline.over(p.surface_container_high, 0.3)),
            },
            ..Default::default()
        })
        .into()
    }

    /// Eine Bibliotheks-Kachel (zum Hinzufügen). Grau, wenn schon platziert.
    fn galerie_kachel<'a>(
        &self,
        glyph: char,
        name: &'a str,
        unter: &'a str,
        aktiv: bool,
        on: Msg,
    ) -> Element<'a, Msg> {
        let p = self.palette;
        let vorder = if aktiv { p.on_surface } else { p.on_surface_variant };
        let inhalt = row![
            container(mkw::symbol::<Msg>(glyph, KATALOG_SYMBOL, vorder)).center_y(Length::Fixed(38.0)),
            column![
                mkw::txt(name, mk::typo::KLEIN, vorder),
                mkw::txt(unter, mk::typo::ETIKETT, p.on_surface_variant),
            ]
            .spacing(1),
            Space::new().width(Length::Fill),
            mkw::symbol::<Msg>(
                if aktiv { mkw::symbol::ADD } else { mkw::symbol::CHECK },
                18.0,
                if aktiv { p.primary } else { p.on_surface_variant },
            ),
        ]
        .spacing(mk::spacing::S)
        .align_y(Alignment::Center);
        let mut knopf = button(inhalt)
            .padding(mk::spacing::S)
            .width(Length::Fixed(272.0))
            .style(move |_, status| mkw::leiste::knopf_stil(p, status, mk::radius::NORMAL));
        if aktiv {
            knopf = knopf.on_press(on);
        }
        knopf.into()
    }

    /// Eine Bar-Zone als beschriftetes Tablett mit ihren wackelnden Kacheln.
    fn zone_tablett(&self, z: Zone) -> Element<'_, Msg> {
        let p = self.palette;
        let widgets = self.zone_ref(z);
        let mut reihe = row![].spacing(mk::spacing::S).align_y(Alignment::Center);
        if widgets.is_empty() {
            reihe = reihe.push(
                container(mkw::txt("leer", mk::typo::KLEIN, p.on_surface_variant))
                    .padding(mk::spacing::S),
            );
        }
        for (i, w) in widgets.iter().enumerate() {
            // ARROW_BACK zeigt nach links, CHEVRON_RIGHT nach rechts; − entfernt.
            let steuern = row![
                self.mini(mkw::symbol::ARROW_BACK, false, Msg::BarLinks(z, i)),
                self.mini(mkw::symbol::REMOVE, true, Msg::BarWeg(z, i)),
                self.mini(mkw::symbol::CHEVRON_RIGHT, false, Msg::BarRechts(z, i)),
            ]
            .spacing(2);
            let seed = i as f32 * 1.7 + z as u8 as f32;
            reihe = reihe.push(self.kachel(w.glyph(), w.titel(), seed, steuern.into()));
        }
        column![
            mkw::txt(z.titel(), mk::typo::ETIKETT, p.on_surface_variant),
            container(reihe)
                .width(Length::Fill)
                .padding(mk::spacing::XS)
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container.over(p.surface, 0.6)).into()),
                    border: iced::Border {
                        radius: mk::radius::NORMAL.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .spacing(mk::spacing::XXS)
        .into()
    }

    fn ziel_wahl(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let mut reihe = row![mkw::txt("Neues Widget nach:", mk::typo::KLEIN, p.on_surface_variant)]
            .spacing(mk::spacing::S)
            .align_y(Alignment::Center);
        for z in Zone::ALLE {
            let gewaehlt = self.ziel == z;
            reihe = reihe.push(
                button(mkw::txt(
                    z.titel(),
                    mk::typo::KLEIN,
                    if gewaehlt { p.on_primary } else { p.on_surface },
                ))
                .padding(iced::Padding {
                    left: mk::spacing::M,
                    right: mk::spacing::M,
                    top: mk::spacing::XS,
                    bottom: mk::spacing::XS,
                })
                .style(move |_, status| {
                    let mut s = mkw::leiste::knopf_stil(p, status, mk::radius::KLEIN);
                    if gewaehlt {
                        s.background = Some(color(p.primary).into());
                    }
                    s
                })
                .on_press(Msg::ZielWahl(z)),
            );
        }
        reihe.into()
    }

    fn bar_editor(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let mut spalte = column![
            mkw::txt(
                "Die drei Zonen der Top-Bar. Kacheln wackeln im Bearbeiten-Modus — ‹ › ordnen und schieben über Zonengrenzen, − entfernt.",
                mk::typo::KLEIN,
                p.on_surface_variant,
            ),
            self.zone_tablett(Zone::Links),
            self.zone_tablett(Zone::Mitte),
            self.zone_tablett(Zone::Rechts),
            self.ziel_wahl(),
        ]
        .spacing(mk::spacing::M);

        let frei: Vec<BarWidget> = BarWidget::ALLE
            .into_iter()
            .filter(|w| !self.platziert(*w))
            .collect();
        if frei.is_empty() {
            spalte = spalte.push(mkw::txt(
                "Alle Widgets sind in der Leiste.",
                mk::typo::KLEIN,
                p.on_surface_variant,
            ));
        } else {
            spalte = spalte.push(mkw::txt("BIBLIOTHEK", mk::typo::ETIKETT, p.on_surface_variant));
            let mut gal = column![].spacing(mk::spacing::S);
            let mut zeile = row![].spacing(mk::spacing::S);
            for (n, w) in frei.iter().enumerate() {
                zeile = zeile.push(self.galerie_kachel(
                    w.glyph(),
                    w.titel(),
                    w.untertitel(),
                    true,
                    Msg::BarHinzu(*w),
                ));
                if (n + 1) % 2 == 0 {
                    gal = gal.push(zeile);
                    zeile = row![].spacing(mk::spacing::S);
                }
            }
            gal = gal.push(zeile);
            spalte = spalte.push(gal);
        }
        spalte.into()
    }

    fn dock_editor(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let mut reihe = row![].spacing(mk::spacing::S).align_y(Alignment::Center);
        if self.pins.is_empty() {
            reihe = reihe.push(
                container(mkw::txt("keine Pins", mk::typo::KLEIN, p.on_surface_variant))
                    .padding(mk::spacing::S),
            );
        }
        for (i, id) in self.pins.iter().enumerate() {
            let steuern = row![
                self.mini(mkw::symbol::ARROW_BACK, false, Msg::DockSchieben(i, -1)),
                self.mini(mkw::symbol::REMOVE, true, Msg::DockWeg(i)),
                self.mini(mkw::symbol::CHEVRON_RIGHT, false, Msg::DockSchieben(i, 1)),
            ]
            .spacing(2);
            reihe = reihe.push(self.kachel(
                dock_glyph(id),
                dock_titel(id),
                i as f32 * 2.3 + 11.0,
                steuern.into(),
            ));
        }

        let frei: Vec<&(&str, &str, char)> = DOCK_KATALOG
            .iter()
            .filter(|entry| !self.pins.iter().any(|q| q == entry.0))
            .collect();

        let mut spalte = column![
            mkw::txt(
                "Angepinnte Apps im Dock. − löst den Pin, ‹ › ordnet.",
                mk::typo::KLEIN,
                p.on_surface_variant,
            ),
            container(reihe)
                .width(Length::Fill)
                .padding(mk::spacing::XS)
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container.over(p.surface, 0.6)).into()),
                    border: iced::Border {
                        radius: mk::radius::NORMAL.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .spacing(mk::spacing::M);

        if frei.is_empty() {
            spalte = spalte.push(mkw::txt(
                "Alle Apps sind angepinnt.",
                mk::typo::KLEIN,
                p.on_surface_variant,
            ));
        } else {
            spalte = spalte.push(mkw::txt("BIBLIOTHEK", mk::typo::ETIKETT, p.on_surface_variant));
            let mut gal = column![].spacing(mk::spacing::S);
            let mut zeile = row![].spacing(mk::spacing::S);
            for (n, (id, titel, glyph)) in frei.iter().enumerate() {
                zeile = zeile.push(self.galerie_kachel(
                    *glyph,
                    titel,
                    id,
                    true,
                    Msg::DockHinzu((*id).to_string()),
                ));
                if (n + 1) % 2 == 0 {
                    gal = gal.push(zeile);
                    zeile = row![].spacing(mk::spacing::S);
                }
            }
            gal = gal.push(zeile);
            spalte = spalte.push(gal);
        }
        spalte.into()
    }

    /// Editor der zweiten Dock-Zeile (dock-widgets): Apps-Knopf, Zentrale,
    /// Uhr, Zwischenablage — gleiche Grammatik wie die Bar-Zonen.
    fn zeile2_editor(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let mut reihe = row![].spacing(mk::spacing::S).align_y(Alignment::Center);
        if self.zeile2.is_empty() {
            reihe = reihe.push(
                container(mkw::txt("leer", mk::typo::KLEIN, p.on_surface_variant))
                    .padding(mk::spacing::S),
            );
        }
        for (i, id) in self.zeile2.iter().enumerate() {
            let Some((_, titel, glyph, _)) = zeile2_eintrag(id) else { continue };
            let steuern = row![
                self.mini(mkw::symbol::ARROW_BACK, false, Msg::Zeile2Schieben(i, -1)),
                self.mini(mkw::symbol::REMOVE, true, Msg::Zeile2Weg(i)),
                self.mini(mkw::symbol::CHEVRON_RIGHT, false, Msg::Zeile2Schieben(i, 1)),
            ]
            .spacing(2);
            reihe = reihe.push(self.kachel(*glyph, titel, i as f32 * 2.9 + 23.0, steuern.into()));
        }

        let frei: Vec<&(&str, &str, char, &str)> = ZEILE2_KATALOG
            .iter()
            .filter(|(id, _, _, _)| !self.zeile2.iter().any(|w| w == id))
            .collect();

        let mut spalte = column![
            mkw::txt(
                "Die Widget-Zeile unter den App-Icons — z. B. der Apps-Knopf oder die Zwischenablage.",
                mk::typo::KLEIN,
                p.on_surface_variant,
            ),
            container(reihe)
                .width(Length::Fill)
                .padding(mk::spacing::XS)
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container.over(p.surface, 0.6)).into()),
                    border: iced::Border {
                        radius: mk::radius::NORMAL.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .spacing(mk::spacing::M);

        if frei.is_empty() {
            spalte = spalte.push(mkw::txt(
                "Alle Widgets sind in der Zeile.",
                mk::typo::KLEIN,
                p.on_surface_variant,
            ));
        } else {
            spalte = spalte.push(mkw::txt("BIBLIOTHEK", mk::typo::ETIKETT, p.on_surface_variant));
            let mut gal = column![].spacing(mk::spacing::S);
            let mut zeile = row![].spacing(mk::spacing::S);
            for (n, (id, titel, glyph, unter)) in frei.iter().enumerate() {
                zeile = zeile.push(self.galerie_kachel(
                    *glyph,
                    titel,
                    unter,
                    true,
                    Msg::Zeile2Hinzu((*id).to_string()),
                ));
                if (n + 1) % 2 == 0 {
                    gal = gal.push(zeile);
                    zeile = row![].spacing(mk::spacing::S);
                }
            }
            gal = gal.push(zeile);
            spalte = spalte.push(gal);
        }
        spalte.into()
    }

    /// Der Panel-Inhalt — Scroll/Fusszeile/Karte liefert der Host.
    pub fn ansicht(&self) -> Element<'_, Msg> {
        let p = self.palette;
        column![
            mkw::sektion("TOP-BAR", vec![self.bar_editor()], p),
            mkw::sektion("DOCK", vec![self.dock_editor()], p),
            mkw::sektion("DOCK — ZWEITE ZEILE", vec![self.zeile2_editor()], p),
            container(mkw::knopf(
                "Standard wiederherstellen",
                mkw::knopfart::Stil::Getoent,
                mkw::knopfart::Rolle::Normal,
                mkw::knopfart::Groesse::Klein,
                p,
                Some(Msg::Standard),
            ))
            .width(Length::Fill)
            .center_x(Length::Fill),
        ]
        .spacing(mk::spacing::L)
        .into()
    }
}

// ------------------------------------------------------------------ Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsen_und_serialisieren_ist_rund() {
        let (l, m, r) = zonen_parse(BAR_STANDARD);
        assert_eq!(l, vec![BarWidget::Matrix, BarWidget::Fokus]);
        assert_eq!(m, vec![BarWidget::Uhr]);
        assert_eq!(r.len(), 6);
        // Rundreise: string -> parse -> string identisch (normalisiert).
        let s = zonen_string(&l, &m, &r);
        let (l2, m2, r2) = zonen_parse(&s);
        assert_eq!((l, m, r), (l2, m2, r2));
    }

    #[test]
    fn unbekanntes_faellt_still_raus() {
        let (l, _m, _r) = zonen_parse("fokus quatsch | | ");
        assert_eq!(l, vec![BarWidget::Fokus]);
    }

    #[test]
    fn schieben_ueber_zonengrenze() {
        let mut app = probe();
        app.links = vec![BarWidget::Fokus, BarWidget::Uhr];
        app.mitte = vec![];
        app.rechts = vec![];
        // letzte Kachel in Links nach rechts -> an den Anfang von Mitte
        app.bar_rechts(Zone::Links, 1);
        assert_eq!(app.links, vec![BarWidget::Fokus]);
        assert_eq!(app.mitte, vec![BarWidget::Uhr]);
        // wieder zurück: erste Kachel in Mitte nach links -> ans Ende von Links
        app.bar_links(Zone::Mitte, 0);
        assert_eq!(app.links, vec![BarWidget::Fokus, BarWidget::Uhr]);
        assert!(app.mitte.is_empty());
    }

    #[test]
    fn platziert_erkennt_dubletten() {
        let mut app = probe();
        app.links = vec![BarWidget::Glocke];
        assert!(app.platziert(BarWidget::Glocke));
        assert!(!app.platziert(BarWidget::Akku));
    }

    fn probe() -> Panel {
        Panel {
            palette: mk::Palette::load().unwrap_or_default(),
            links: vec![],
            mitte: vec![],
            rechts: vec![],
            pins: vec![],
            zeile2: vec![],
            ziel: Zone::Rechts,
            phase: 0.0,
            wackeln: false,
        }
    }
}
