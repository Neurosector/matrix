//! Leinwand & Hintergrund — als Panel der Matrix Einstellungen (Fusion R41).
//! War App #10 (matrix-leinwand); jetzt zwei Bereiche der EINEN
//! Einstellungen-App. Alle Regler wirken unverändert live.

use iced::widget::{column, container, Space};
use iced::{Element, Length, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;

/// Geist-Modi in Anzeige-Reihenfolge (Index 0 = Default „Beim Scrollen").
const GEIST_MODI: [&str; 3] = ["Beim Scrollen", "Immer auf Leerraum", "Aus"];

pub struct Panel {
    pub palette: mk::Palette,
    geist_modus: usize,
    blaettern: bool,
    klick_holt: bool,
    heimweg: bool,
    gespeichert: Option<String>,
    /// Hintergrund: Ziel-Modus fürs Setzen (0 = Hell, 1 = Dunkel).
    ziel_modus: usize,
    /// Galerie: (Pfad, Thumbnail 224px, aus dem Matrix-Systemordner?).
    galerie: Vec<(std::path::PathBuf, iced::widget::image::Handle, bool)>,
    galerie_laedt: bool,
    hell_pfad: Option<String>,
    dunkel_pfad: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    GeistModus(usize),
    Blaettern(bool),
    KlickHolt(bool),
    Heimweg(bool),
    ZielModus(usize),
    HintergrundSetzen(usize),
    GalerieGeladen(Vec<(std::path::PathBuf, u32, u32, Vec<u8>, bool)>),
}

const ZIELE: [&str; 2] = ["Hell", "Dunkel"];

fn session_json_pfad() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(home).join(".local/state/DankMaterialShell/session.json")
}

fn wallpaper_zuordnung() -> (Option<String>, Option<String>) {
    let Ok(raw) = std::fs::read_to_string(session_json_pfad()) else {
        return (None, None);
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return (None, None);
    };
    let hol = |k: &str| v.get(k).and_then(|x| x.as_str()).map(String::from).filter(|s| !s.is_empty());
    (hol("wallpaperPathLight"), hol("wallpaperPathDark"))
}

fn wallpaper_zuordnen(hell: bool, pfad: &str) {
    let p = session_json_pfad();
    let mut v: serde_json::Value = std::fs::read_to_string(&p)
        .ok()
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    v[if hell { "wallpaperPathLight" } else { "wallpaperPathDark" }] =
        serde_json::Value::String(pfad.to_string());
    if let Ok(neu) = serde_json::to_string_pretty(&v) {
        let _ = std::fs::write(&p, neu);
    }
}

/// Live setzen: die eigene Hintergrund-Kultur — matrix-hintergrund
/// (App #21) liest ~/.config/matrix/hintergrund-<modus>, malt neu und
/// erzeugt die Palette (matugen). Kein DMS mehr in der Kette.
fn wallpaper_live_setzen(pfad: &str) {
    if let Ok(heim) = std::env::var("HOME") {
        let hell = mk::Palette::load().map(|p| p.is_light).unwrap_or(false);
        let ziel = format!(
            "{heim}/.config/matrix/hintergrund-{}",
            if hell { "hell" } else { "dunkel" }
        );
        let _ = std::fs::write(ziel, pfad);
    }
}

/// Thumbnails im Hintergrund erzeugen (224px, Leitbild-Kachelgröße) —
/// die Originale (bis 24 MP) bleiben dem Renderer erspart.
fn thumbs_erzeugen() -> Vec<(std::path::PathBuf, u32, u32, Vec<u8>, bool)> {
    galerie_sammeln()
        .into_iter()
        .filter_map(|(pfad, system)| {
            let bild = image::open(&pfad).ok()?;
            // BildKachel-Familie (R44): EIN Thumb-Rezept für Hintergrund
            // und Galerie — 16:9, 448x252, gebackene Ecken.
            let (b, h, roh) = mkw::bild::kachel_thumb(bild);
            Some((pfad, b, h, roh, system))
        })
        .collect()
}


/// Bilder-Kandidaten: ~/Bilder(+Unterordner 1 Ebene) und ~/Downloads.
fn galerie_sammeln() -> Vec<(std::path::PathBuf, bool)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut kandidaten: Vec<(std::path::PathBuf, bool)> = Vec::new();
    let mut ordner = vec![
        String::from("/usr/share/backgrounds/matrix"),
        format!("{home}/Bilder"),
        format!("{home}/Pictures"),
        format!("{home}/Downloads"),
    ];
    for basis in [format!("{home}/Bilder"), format!("{home}/Pictures")] {
        if let Ok(unter) = std::fs::read_dir(&basis) {
            for u in unter.flatten() {
                if u.path().is_dir() && u.file_name() != "Screenshots" {
                    ordner.push(u.path().display().to_string());
                }
            }
        }
    }
    for o in ordner {
        if let Ok(eintraege) = std::fs::read_dir(&o) {
            for e in eintraege.flatten() {
                let pf = e.path();
                let endung = pf
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_lowercase())
                    .unwrap_or_default();
                if ["jpg", "jpeg", "png"].contains(&endung.as_str()) {
                    kandidaten.push((pf, o.starts_with("/usr/share")));
                }
            }
        }
    }
    kandidaten.sort();
    kandidaten.dedup();
    kandidaten.truncate(24);
    kandidaten
}

fn schalter_lesen(name: &str) -> bool {
    mk::einstellung::lesen(name).as_deref() != Some("aus")
}

fn schalter_schreiben(name: &str, an: bool) {
    mk::einstellung::schreiben(name, if an { "an" } else { "aus" });
}

impl Panel {
    pub fn new() -> (Self, Task<Msg>) {
        let geist_modus = match mk::einstellung::lesen("leinwand-geist").as_deref() {
            Some("immer") => 1,
            Some("aus") => 2,
            _ => 0,
        };
        let (hell_pfad, dunkel_pfad) = wallpaper_zuordnung();
        let ist_hell = mk::Palette::load().map(|p| p.is_light).unwrap_or(false);
        (
            Self {
                palette: mk::Palette::load().unwrap_or_default(),
                geist_modus,
                blaettern: schalter_lesen("leinwand-blaettern"),
                klick_holt: schalter_lesen("leinwand-klick-holt"),
                heimweg: schalter_lesen("leinwand-heimweg"),
                gespeichert: None,
                ziel_modus: if ist_hell { 0 } else { 1 },
                galerie: Vec::new(),
                galerie_laedt: true,
                hell_pfad,
                dunkel_pfad,
            },
            Task::perform(async { thumbs_erzeugen() }, Msg::GalerieGeladen),
        )
    }

    fn quittung(&mut self, text: &str) {
        self.gespeichert = Some(format!("{text} \u{2713} — wirkt sofort"));
    }

    /// Vom Host je Tick gerufen: Quittung verblasst, Palette folgt.
    pub fn tick(&mut self, p: mk::Palette) {
        self.palette = p;
        self.gespeichert = None;
    }

    pub fn fusstext(&self) -> Option<String> {
        self.gespeichert.clone()
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::GeistModus(i) => {
                self.geist_modus = i.min(2);
                mk::einstellung::schreiben(
                    "leinwand-geist",
                    ["scrollen", "immer", "aus"][self.geist_modus],
                );
                self.quittung("Fenster-Geist");
            }
            Msg::Blaettern(an) => {
                self.blaettern = an;
                schalter_schreiben("leinwand-blaettern", an);
                self.quittung("Rad-Blättern");
            }
            Msg::KlickHolt(an) => {
                self.klick_holt = an;
                schalter_schreiben("leinwand-klick-holt", an);
                self.quittung("Klick holt Fenster");
            }
            Msg::Heimweg(an) => {
                self.heimweg = an;
                schalter_schreiben("leinwand-heimweg", an);
                self.quittung("Heimweg");
            }
            Msg::ZielModus(i) => self.ziel_modus = i.min(1),
            Msg::GalerieGeladen(thumbs) => {
                self.galerie = thumbs
                    .into_iter()
                    .map(|(pfad, b, h, rgba, system)| {
                        (pfad, iced::widget::image::Handle::from_rgba(b, h, rgba), system)
                    })
                    .collect();
                self.galerie_laedt = false;
            }
            Msg::HintergrundSetzen(i) => {
                if let Some((pfad, _, _)) = self.galerie.get(i) {
                    let pfad_s = pfad.display().to_string();
                    let hell = self.ziel_modus == 0;
                    // ERST live setzen (DMS schreibt die Session danach aus
                    // seinem Speicher neu), DANN unsere Zuordnung daruber —
                    // sonst verliert der Patch das Rennen gegen DMS.
                    if hell == self.palette.is_light {
                        wallpaper_live_setzen(&pfad_s);
                    }
                    wallpaper_zuordnen(hell, &pfad_s);
                    if hell {
                        self.hell_pfad = Some(pfad_s);
                    } else {
                        self.dunkel_pfad = Some(pfad_s);
                    }
                    self.quittung(if hell { "Hintergrund (Hell)" } else { "Hintergrund (Dunkel)" });
                }
            }
        }
        Task::none()
    }

    /// Bereich „Leinwand": Fenster-Geist + Navigation.
    pub fn leinwand_ansicht(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let in_leinwand = mkw::session_ist_leinwand();

        let mut inhalt = column![].spacing(0);

        // Ehrlicher Kontext-Hinweis, wenn die Leinwand gar nicht läuft.
        if !in_leinwand {
            inhalt = inhalt
                .push(mkw::sektion(
                    "",
                    vec![mkw::zeile(
                        "Du bist im klassischen Desktop",
                        Some("Diese Einstellungen wirken in der Sitzung „Matrix Leinwand\u{201c} — beim Anmelden wählbar."),
                        Some(mkw::symbol::<Msg>(mkw::symbol::INFO, mk::icon_size::LARGE, p.on_surface_variant)),
                        None,
                        p,
                    )],
                    p,
                ))
                .push(Space::new().height(mk::spacing::L));
        }

        // Der Fenster-Geist
        inhalt = inhalt.push(mkw::sektion(
            "DER FENSTER-GEIST",
            vec![
                mkw::zeile(
                    "Anzeigen",
                    Some("Die Icon-Karte der offenen Fenster neben der Maus."),
                    None,
                    Some(mkw::segmente(&GEIST_MODI[..], self.geist_modus, Msg::GeistModus, p)),
                    p,
                ),
            ],
            p,
        ));
        inhalt = inhalt.push(Space::new().height(mk::spacing::L));

        // Navigation
        inhalt = inhalt.push(mkw::sektion(
            "NAVIGATION",
            vec![
                mkw::zeile_schalter(
                    "Rad-Blättern",
                    Some("Mausrad auf dem Leerraum wandert durch die Fenster."),
                    None,
                    self.blaettern,
                    p,
                    Some(Msg::Blaettern(!self.blaettern)),
                ),
                mkw::zeile_schalter(
                    "Klick holt Fenster",
                    Some("Angeschnittene Fenster fahren beim Anklicken ganz herein."),
                    None,
                    self.klick_holt,
                    p,
                    Some(Msg::KlickHolt(!self.klick_holt)),
                ),
                mkw::zeile_schalter(
                    "Heimweg",
                    Some("Kurzer Klick auf den Leerraum kehrt zu den Fenstern zurück."),
                    None,
                    self.heimweg,
                    p,
                    Some(Msg::Heimweg(!self.heimweg)),
                ),
            ],
            p,
        ));

        inhalt.into()
    }

    /// Bereich „Hintergrund": Vorschau, Ziel-Modus, Galerie-Kacheln.
    pub fn hintergrund_ansicht(&self) -> Element<'_, Msg> {
        let p = self.palette;
        // Hintergrund — Leitbild-Grammatik (NSWorkspace desktopImage):
        // grosse Schreibtisch-Vorschau oben, kategorisierte Kacheln darunter.
        let name_von = |p: &Option<String>| -> String {
            p.as_deref()
                .and_then(|s| std::path::Path::new(s).file_stem().and_then(|f| f.to_str()))
                .unwrap_or("\u{2014}")
                .chars()
                .take(28)
                .collect()
        };
        let aktiv_hell = self.palette.is_light;
        let aktiv_pfad = if aktiv_hell { &self.hell_pfad } else { &self.dunkel_pfad };
        let mut hintergrund_zeilen: Vec<Element<'_, Msg>> = Vec::new();

        // Schreibtisch-Vorschau: das aktive Bild als Mini-Monitor.
        if let Some(pf) = aktiv_pfad.as_deref() {
            let vorschau = self
                .galerie
                .iter()
                .find(|(q, _, _)| q.display().to_string() == pf)
                .map(|(_, h, _)| h.clone())
                .unwrap_or_else(|| iced::widget::image::Handle::from_path(pf));
            hintergrund_zeilen.push(
                container(
                    column![
                        container(
                            iced::widget::image(vorschau)
                                .width(Length::Fixed(300.0))
                                .height(Length::Fixed(169.0))
                                .content_fit(iced::ContentFit::Fill),
                        )
                        .style(move |_| container::Style {
                            border: iced::Border {
                                color: color(p.outline.over(p.surface_container_high, 0.5)),
                                width: 1.0,
                                radius: mk::radius::GROSS.into(),
                            },
                            ..Default::default()
                        }),
                        mkw::txt(
                            format!(
                                "{} \u{b7} {}",
                                name_von(aktiv_pfad),
                                if aktiv_hell { "Hell aktiv" } else { "Dunkel aktiv" }
                            ),
                            mk::typo::KLEIN,
                            p.on_surface_variant,
                        ),
                    ]
                    .spacing(mk::spacing::XS)
                    .align_x(iced::Alignment::Center),
                )
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .padding(iced::Padding { top: mk::spacing::M, bottom: mk::spacing::S, ..iced::Padding::ZERO })
                .into(),
            );
        }
        hintergrund_zeilen.push(mkw::zeile(
            "Bild setzen für",
            Some("Der Sonnenstand wechselt automatisch zwischen beiden."),
            None,
            Some(mkw::segmente(&ZIELE[..], self.ziel_modus, Msg::ZielModus, p)),
            p,
        ));
        if self.galerie_laedt {
            hintergrund_zeilen.push(mkw::zeile(
                "Bilder werden geladen \u{2026}",
                None,
                None,
                None,
                p,
            ));
        }

        // BildKachel-Familie (mkw::ui) — Auswahl-Badge wie das Leitbild.
        let kachel = |i: usize,
                      pfad: &std::path::PathBuf,
                      handle: &iced::widget::image::Handle|
         -> Element<'_, Msg> {
            let pfad_s = pfad.display().to_string();
            let ist_hell = self.hell_pfad.as_deref() == Some(pfad_s.as_str());
            let ist_dunkel = self.dunkel_pfad.as_deref() == Some(pfad_s.as_str());
            let markiert = ist_hell || ist_dunkel;
            let badge = if ist_hell && ist_dunkel {
                Some(String::from("\u{2713} H+D"))
            } else if ist_hell {
                Some(String::from("\u{2713} Hell"))
            } else if ist_dunkel {
                Some(String::from("\u{2713} Dunkel"))
            } else {
                None
            };
            let name: String = pfad
                .file_stem()
                .and_then(|f| f.to_str())
                .unwrap_or("?")
                .to_string();
            iced::widget::button(mkw::ui::bild_kachel(
                Some(handle.clone()),
                mkw::symbol::IMAGE,
                name,
                markiert,
                badge,
                p,
            ))
            .padding(0)
            .style(|_, _| iced::widget::button::Style::default())
            .on_press(Msg::HintergrundSetzen(i))
            .into()
        };

        // Zwei Sektionen wie das Leitbild: System-Bilder und eigene.
        for (titel_gruppe, system) in [("Matrix", true), ("Deine Bilder", false)] {
            let indizes: Vec<usize> = self
                .galerie
                .iter()
                .enumerate()
                .filter(|(_, (_, _, s))| *s == system)
                .map(|(i, _)| i)
                .collect();
            if indizes.is_empty() {
                continue;
            }
            hintergrund_zeilen.push(
                container(mkw::txt(titel_gruppe, mk::typo::ETIKETT, p.on_surface_variant))
                    .padding(iced::Padding { left: mk::spacing::M, top: mk::spacing::S, ..iced::Padding::ZERO })
                    .into(),
            );
            for reihe_idx in indizes.chunks(3) {
                let mut reihe = iced::widget::Row::new().spacing(mk::spacing::S);
                for &i in reihe_idx {
                    let (pfad, handle, _) = &self.galerie[i];
                    reihe = reihe.push(kachel(i, pfad, handle));
                }
                hintergrund_zeilen.push(
                    container(reihe)
                        .padding(iced::Padding { left: mk::spacing::M, right: mk::spacing::M, bottom: mk::spacing::S, ..iced::Padding::ZERO })
                        .into(),
                );
            }
        }
        if self.galerie.is_empty() && !self.galerie_laedt {
            hintergrund_zeilen.push(mkw::zeile(
                "Keine Bilder gefunden",
                Some("Lege Bilder in ~/Bilder oder ~/Downloads (JPG/PNG)."),
                None,
                None,
                p,
            ));
        }
        // Credit-Pflicht des Standard-Hintergrunds (Pixabay-Kultur).
        hintergrund_zeilen.push(mkw::zeile(
            "Standard-Hintergrund",
            Some("„Falcon\u{201c} von wfranz auf Pixabay (5350832) — danke!"),
            Some(mkw::symbol::<Msg>(mkw::symbol::PALETTE, mk::font_size::SMALL, p.on_surface_variant)),
            None,
            p,
        ));
        let hintergrund_sektion = mkw::sektion("HINTERGRUND", hintergrund_zeilen, p);
        column![hintergrund_sektion].into()
    }
}
