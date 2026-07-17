//! Matrix Dock — App #11, Mitglied der MatrixKit-Leisten-Familie.
//!
//! Seit 2.0 hat die Pille ZWEI Zeilen:
//!
//! * **Zeile 1 — Shortcuts**: angepinnte Apps (`~/.config/matrix/
//!   dock-pins`, eine app_id je Wort) plus alles Laufende. Der Punkt
//!   sagt „läuft", Akzent sagt „hat den Fokus". Klick holt das Fenster
//!   (fokussierte App zykelt) — oder STARTET den Pin, wenn er ruht.
//! * **Zeile 2 — Widgets**: wie die Bar, nur im Dock (`~/.config/
//!   matrix/dock-widgets`, Standard `apps`): „Apps" öffnet den
//!   Launcher Matrix Start mittig, `zentrale` und `uhr` gibt es auch.
//!
//! Layer::Top mit Bedacht (Task #39); `--reservieren` hält die Kante
//! frei, seit die DMS-Leisten schlafen.

use std::collections::HashMap;

use iced::widget::{button, column, container, image, row, Space};
use iced::{Color, Element, Font, Length, Subscription, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use matrixkit_widgets::{color, tick};

/// Bildkante des App-Icons in der Pille.
const ICON: f32 = 44.0;
/// Knopffläche um das Icon (Innenluft für den Hover-Ton).
const KNOPF: f32 = 52.0;
/// Knopf-Rundung: großes schwebendes Element.
const KNOPF_RADIUS: f32 = mk::radius::GROSS;
const LUECKE: f32 = mk::spacing::XS;
const RAND: f32 = mk::spacing::S;
/// Zeile 1 (Knopf + Punkt) + Zeile 2 (Widgets) + Innenränder.
const HOEHE: u32 = 108;
/// Der Lauf-Punkt unter dem Icon.
const PUNKT: f32 = 4.0;
/// So lange bleibt das Dock nach einem Tastendruck im OSD-Gesicht (ms).
const OSD_STANDZEIT: u128 = 1500;
/// Zusatzhöhe, wenn das Kachel-Kontextmenü offen ist (wächst nach oben).
const MENUE_HOEHE: u32 = 128;

fn breite(kacheln: usize) -> u32 {
    let n = kacheln.max(3) as f32; // Platz für die Widget-Zeile
    (n * KNOPF + (n - 1.0) * LUECKE + 2.0 * RAND) as u32
}

/// Zeilen-Scroller (R49): eine Dock-Zeile als unsichtbarer Horizontal-
/// Scroller — gedeckelt auf die Pillen-Innenbreite. Passt der Inhalt,
/// zentriert ihn der Rufer; läuft er über, scrollt Rad/Trackpad quer,
/// und zwar nur in der Zeile unter der Maus.
fn zeilen_scroller<'a>(
    inhalt: Element<'a, Msg>,
    innen_breite: f32,
    id: &'static str,
) -> Element<'a, Msg> {
    let kern = container(
        iced::widget::scrollable(inhalt)
            .id(iced::advanced::widget::Id::new(id))
            .direction(iced::widget::scrollable::Direction::Horizontal(
                iced::widget::scrollable::Scrollbar::new()
                    .width(0)
                    .scroller_width(0)
                    .margin(0),
            ))
            .width(Length::Shrink),
    )
    .max_width(innen_breite);
    // R62 (Nutzer-Fund am PC): ein SENKRECHTES Mausrad soll die Quer-
    // zeile schieben — iced übersetzt das nur mit gehaltenem Shift
    // (scrollable.rs: „movement = if !is_shift_pressed…"), das Surface-
    // Trackpad lieferte echte X-Deltas und hat den Fall versteckt.
    // Der Fänger unten macht Rad-Y zur Quer-Geste; horizontale Deltas
    // konsumiert weiterhin der Scroller selbst.
    iced::widget::mouse_area(kern)
        .on_scroll(move |delta| {
            let y = match delta {
                iced::mouse::ScrollDelta::Lines { y, .. } => y * 48.0,
                iced::mouse::ScrollDelta::Pixels { y, .. } => y,
            };
            Msg::ZeilenRad(id, y)
        })
        .into()
}

fn main() -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};
    let reservieren = std::env::args().any(|a| a == "--reservieren");
    iced_layershell::application(
        App::new,
        || String::from("matrix-dock"),
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
            size: Some((breite(1), HOEHE + 2 * mkw::leiste::SCHATTEN_RAND as u32)),
            anchor: Anchor::Bottom,
            // Schattenraum unten (Nutzer-Fund 15.7.): die Pille bleibt
            // 10 px über der Kante, aber die Surface reicht 14 px
            // darüber hinaus — der Blur fadet, statt an der Surface-
            // Unterkante hart zu brechen (die "Rechteck-Flügel").
            margin: (0, 0, 10 - mkw::leiste::SCHATTEN_RAND as i32, 0),
            layer: Layer::Top,
            keyboard_interactivity: KeyboardInteractivity::None,
            exclusive_zone: if reservieren { HOEHE as i32 + 10 } else { 0 },
            ..Default::default()
        },
        default_font: Font::with_name("Inter Variable"),
        fonts: mkw::symbol_font_laden().into_iter().collect(),
        ..Default::default()
    })
    .run()
}

// ------------------------------------------------------- Zeile 1: Kacheln

/// Eine Dock-Kachel: Pin und/oder laufende App.
#[derive(Debug, Clone)]
struct Kachel {
    app_id: String,
    fenster: Vec<mkw::leinwand::OffenesFenster>,
    gepinnt: bool,
    fokus: bool,
    /// NSDockTile.badgeLabel-Extrakt: kleine Notiz überm Icon.
    abzeichen: Option<String>,
}

impl Kachel {
    fn laeuft(&self) -> bool {
        !self.fenster.is_empty()
    }

    /// Klick-Ziel: fokussierte App zykelt, sonst das älteste Fenster.
    fn ziel(&self) -> Option<u64> {
        if self.fokus && self.fenster.len() > 1 {
            let an = self.fenster.iter().position(|f| f.fokus)?;
            return Some(self.fenster[(an + 1) % self.fenster.len()].id);
        }
        self.fenster.first().map(|f| f.id)
    }
}

/// Pins aus der Einstellungs-Datei (eine app_id je Wort).
fn pins_lesen() -> Vec<String> {
    mk::einstellung::lesen("dock-pins")
        .unwrap_or_else(|| String::from("matrix-einstellungen"))
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Pins zuerst (in Pin-Reihenfolge), dann Laufendes nach Dienstalter.
fn kacheln_lesen() -> Vec<Kachel> {
    let pins = pins_lesen();
    let mut nach_app: HashMap<String, Vec<mkw::leinwand::OffenesFenster>> = HashMap::new();
    for f in mkw::leinwand::fenster() {
        nach_app.entry(f.app_id.clone()).or_default().push(f);
    }
    let mut kacheln: Vec<Kachel> = Vec::new();
    for pin in &pins {
        let mut fenster = nach_app.remove(pin).unwrap_or_default();
        fenster.sort_by_key(|f| f.id);
        let fokus = fenster.iter().any(|f| f.fokus);
        let abzeichen = mk::abzeichen::lesen(pin);
        kacheln.push(Kachel { app_id: pin.clone(), fenster, gepinnt: true, fokus, abzeichen });
    }
    let mut laufende: Vec<Kachel> = nach_app
        .into_iter()
        .map(|(app_id, mut fenster)| {
            fenster.sort_by_key(|f| f.id);
            let fokus = fenster.iter().any(|f| f.fokus);
            let abzeichen = mk::abzeichen::lesen(&app_id);
            Kachel { app_id, fenster, gepinnt: false, fokus, abzeichen }
        })
        .collect();
    laufende.sort_by_key(|k| k.fenster[0].id);
    kacheln.extend(laufende);
    kacheln
}

/// Anfangsbuchstabe für Apps ohne Matrix-Icon ("org.mozilla.firefox" → F).
fn initial(app_id: &str) -> String {
    app_id
        .rsplit('.')
        .next()
        .and_then(|s| s.chars().next())
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| String::from("?"))
}

// ------------------------------------------------------- Zeile 2: Widgets

#[derive(Debug, Clone, Copy, PartialEq)]
enum Widget {
    Apps,
    Zentrale,
    Uhr,
    Zwischenablage,
    /// R69: öffnet das Aufnahme-Panel (Aufnahme-Panel).
    Aufnahme,
}

impl Widget {
    fn aus(name: &str) -> Option<Self> {
        match name {
            "apps" => Some(Self::Apps),
            "zentrale" => Some(Self::Zentrale),
            "uhr" => Some(Self::Uhr),
            "zwischenablage" => Some(Self::Zwischenablage),
            "aufnahme" => Some(Self::Aufnahme),
            _ => None,
        }
    }
}

fn widgets_lesen() -> Vec<Widget> {
    mk::einstellung::lesen("dock-widgets")
        .unwrap_or_else(|| String::from("apps"))
        .split_whitespace()
        .filter_map(Widget::aus)
        .collect()
}

// ------------------------------------------------ Zwischenablage-Verlauf

/// Verlaufs-Datei (JSON-Array, neueste zuerst) — flüchtig im Runtime-
/// Verzeichnis; matrix-kontext liest sie für das Verlaufs-Menü.
fn zw_pfad() -> std::path::PathBuf {
    let basis = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| String::from("/tmp"));
    std::path::PathBuf::from(basis).join("matrix-zwischenablage.json")
}

fn zw_lesen() -> Vec<String> {
    std::fs::read_to_string(zw_pfad())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Die aktuelle Kopie in den Verlauf einpflegen (dedupliziert, max 8,
/// Riesen-Kopien bleiben draußen). Gibt den neuesten Eintrag zurück.
fn zw_pflegen() -> Option<String> {
    let aus = std::process::Command::new("wl-paste")
        .args(["--no-newline", "--type", "text"])
        .output()
        .ok()?;
    let text = String::from_utf8(aus.stdout).ok()?;
    let mut verlauf = zw_lesen();
    if aus.status.success() && !text.trim().is_empty() && text.chars().count() <= 10_000 {
        verlauf.retain(|e| e != &text);
        verlauf.insert(0, text);
        verlauf.truncate(8);
        if let Ok(json) = serde_json::to_string(&verlauf) {
            let _ = std::fs::write(zw_pfad(), json);
        }
    }
    verlauf.into_iter().next()
}

// ----------------------------------------------------------------- App

#[to_layer_message]
#[derive(Debug, Clone)]
enum Msg {
    FadeTick,
    Tick,
    OsdPuls,
    /// Nur neu zeichnen — treibt das Start-Hüpfen (60 ms, nur solange nötig).
    HuepfTick,
    Klick(usize),
    Kontext(usize),
    MenueZu,
    PinWechsel(usize),
    Beenden(usize),
    Anpassen,
    AppsOeffnen,
    ZentraleToggle,
    /// R69: das Aufnahme-Panel über dem Dock öffnen.
    AufnahmeOeffnen,
    /// Zwischenablage-Verlauf öffnen (externes Menü, Dock wächst nie).
    ZwischenablageMenue,
    /// R62: Rad-Y über einer Dockzeile → Quer-Scroll (nur diese Zeile).
    ZeilenRad(&'static str, f32),
}

struct App {
    palette: mk::Palette,
    watcher: mk::PaletteWatcher,
    kacheln: Vec<Kachel>,
    widgets: Vec<Widget>,
    uhr: String,
    /// Neueste Kopie für die Widget-Vorschau (Zeile 2).
    zw_vorschau: Option<String>,
    /// Live gerenderte Matrix-Icons je app_id; None = fremde App (Initial).
    icons: HashMap<String, Option<image::Handle>>,
    /// Das Dynamic-Dock-Gesicht: letzter OSD-Stand + Tastendruck-Zeit.
    osd: Option<(mk::osd::Stand, std::time::SystemTime)>,
    hart: bool,
    /// launchanim-Extrakt (R38): startende Pins hüpfen, bis ihr Fenster da
    /// ist (app_id → Klick-Zeit). Referenzsystem: das Icon hüpft beim App-Start.
    huepfer: std::collections::HashMap<String, std::time::Instant>,
    /// Offenes Kachel-Kontextmenü (Rechtsklick): Kachel-Index.
    menue: Option<usize>,
    /// Menü wartet auf die gewachsene Surface (Anti-Glitch, 8.7.):
    /// erst wachsen, im nächsten Puls zeigen — sonst quetscht sich das
    /// Menü-Layout einen Frame lang in die alte Fläche.
    menue_wartet: Option<usize>,
}


impl App {
    fn new() -> (Self, Task<Msg>) {
        let mut app = App {
            palette: mk::Palette::load().unwrap_or_default(),
            watcher: mk::PaletteWatcher::new(),
            kacheln: Vec::new(),
            widgets: widgets_lesen(),
            uhr: String::new(),
            zw_vorschau: None,
            icons: HashMap::new(),
            osd: None,
            hart: mk::bewegung_reduziert(),
            huepfer: std::collections::HashMap::new(),
            menue: None,
            menue_wartet: None,
        };
        let task = app.aktualisieren();
        (app, task)
    }

    /// Kacheln + Palette auffrischen; bei neuer Spaltenzahl wächst die Pille.
    fn aktualisieren(&mut self) -> Task<Msg> {
        if self.watcher.changed() {
            if let Some(neu) = mk::Palette::load() {
                self.palette = neu;
            }
            // Icons nur am FADE-ENDE neu backen (Flanke), nie pro Frame.
            if !mk::uebergang::aktiv() {
                self.icons.clear();
            }
        }
        let vorher = self.kacheln.len();
        self.kacheln = kacheln_lesen();
        {
            let kacheln = &self.kacheln;
            self.huepfer.retain(|id, seit| {
                seit.elapsed().as_secs() < 8
                    && !kacheln.iter().any(|k| &k.app_id == id && k.laeuft())
            });
        }
        self.hart = mk::bewegung_reduziert();
        self.widgets = widgets_lesen();
        if self.widgets.contains(&Widget::Uhr) {
            if let Ok(out) = std::process::Command::new("date").arg("+%H:%M").output() {
                self.uhr = String::from_utf8_lossy(&out.stdout).trim().to_string();
            }
        }
        // Zwischenablage nur beobachten, wenn das Widget aktiv ist —
        // sonst kein wl-paste-Aufruf pro Tick.
        if self.widgets.contains(&Widget::Zwischenablage) {
            self.zw_vorschau = zw_pflegen();
        }
        for k in &self.kacheln {
            let p = self.palette;
            self.icons.entry(k.app_id.clone()).or_insert_with(|| {
                matrixkit_icons::render_png(&k.app_id, &p).map(image::Handle::from_bytes)
            });
        }
        if self.kacheln.len() != vorher {
            return self.mass();
        }
        Task::none()
    }

    /// Surface-Maß: Grundhöhe + Menü-Raum + Schatten-Atemraum
    /// (oben und seitlich; unten sitzt die Kante am Schirmrand).
    fn mass(&self) -> Task<Msg> {
        let rand = mkw::leiste::SCHATTEN_RAND as u32;
        let hoehe = HOEHE
            + if self.menue.is_some() || self.menue_wartet.is_some() {
                MENUE_HOEHE
            } else {
                0
            };
        Task::done(Msg::SizeChange((
            breite(self.kacheln.len()) + 2 * rand,
            hoehe + 2 * rand,
        )))
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::FadeTick => {
                if self.watcher.changed() {
                    if let Some(neu) = mk::Palette::load() {
                        self.palette = neu;
                    }
                    if !mk::uebergang::aktiv() {
                        self.icons.clear();
                    }
                }
                return Task::none();
            }
            Msg::HuepfTick => Task::none(),
            Msg::Tick => self.aktualisieren(),
            Msg::OsdPuls => {
                if let Some(i) = self.menue_wartet.take() {
                    self.menue = Some(i);
                }
                let war = self.osd_aktiv();
                self.osd = mk::osd::lesen();
                let _ = war; // Zustandswechsel rendert der nächste view
                Task::none()
            }
            Msg::Kontext(i) => {
                // Externes Menü (matrix-kontext dock ...): das Dock wächst
                // nie — der Resize-Glitch ist strukturell unmöglich.
                if let Some(k) = self.kacheln.get(i) {
                    let breite_gesamt = breite(self.kacheln.len()) as f32
                        + 2.0 * mkw::leiste::SCHATTEN_RAND;
                    let kachel_x = mkw::leiste::SCHATTEN_RAND
                        + RAND
                        + i as f32 * (KNOPF + LUECKE)
                        + KNOPF / 2.0;
                    let offset_x = kachel_x - breite_gesamt / 2.0;
                    let unten = (HOEHE + 12) as f32;
                    let ids = if k.fenster.is_empty() {
                        String::from("-")
                    } else {
                        k.fenster
                            .iter()
                            .map(|f| f.id.to_string())
                            .collect::<Vec<_>>()
                            .join(",")
                    };
                    let arg = format!(
                        "PATH=$HOME/.local/bin:$PATH exec matrix-kontext dock {} {} {} {} {}",
                        offset_x as i32,
                        unten as i32,
                        k.app_id,
                        u8::from(k.gepinnt),
                        ids
                    );
                    let _ = std::process::Command::new("sh").args(["-c", &arg]).spawn().map(
                        |mut kind| {
                            std::thread::spawn(move || {
                                let _ = kind.wait();
                            })
                        },
                    );
                }
                return Task::none();
            }
            Msg::MenueZu => {
                self.menue = None;
                self.menue_wartet = None;
                return self.mass();
            }
            Msg::PinWechsel(i) => {
                if let Some(k) = self.kacheln.get(i) {
                    let mut pins = pins_lesen();
                    if k.gepinnt {
                        pins.retain(|p| p != &k.app_id);
                    } else {
                        pins.push(k.app_id.clone());
                    }
                    mk::einstellung::schreiben("dock-pins", &pins.join(" "));
                }
                self.menue = None;
                let _ = self.aktualisieren();
                return self.mass();
            }
            Msg::Beenden(i) => {
                if let Some(k) = self.kacheln.get(i) {
                    for f in &k.fenster {
                        mk::leinwand::fenster_schliessen(f.id);
                    }
                }
                self.menue = None;
                return self.mass();
            }
            Msg::Klick(i) => {
                if let Some(k) = self.kacheln.get(i) {
                    if let Some(id) = k.ziel() {
                        mkw::leinwand::fokussieren(id);
                    } else if k.gepinnt {
                        // Ruhender Pin: die App zum Leben erwecken — und
                        // hüpfen, bis das Fenster da ist (launchanim, R38).
                        mkw::leiste::app_starten(&k.app_id);
                        self.huepfer
                            .insert(k.app_id.clone(), std::time::Instant::now());
                    }
                }
                self.aktualisieren()
            }
            Msg::Anpassen => {
                self.menue = None;
                // Fusion R41: Einstellungen, Bereich Leiste & Dock.
                let basis = std::env::var("XDG_RUNTIME_DIR")
                    .unwrap_or_else(|_| String::from("/tmp"));
                let _ = std::fs::write(
                    format!("{basis}/matrix-einstellungen-bereich"),
                    "leisten",
                );
                mkw::leiste::app_starten("matrix-einstellungen");
                return self.mass();
            }
            Msg::ZwischenablageMenue => {
                // Externes Menü wie beim Kachel-Kontext: das Dock wächst nie.
                // Mittig über dem Dock (offset 0), Unterkante über der Pille.
                let unten = (HOEHE + 12) as i32;
                let arg = format!(
                    "PATH=$HOME/.local/bin:$PATH exec matrix-kontext clips 0 {unten}"
                );
                let _ = std::process::Command::new("sh").args(["-c", &arg]).spawn().map(
                    |mut kind| {
                        std::thread::spawn(move || {
                            let _ = kind.wait();
                        })
                    },
                );
                Task::none()
            }
            Msg::AppsOeffnen => {
                // Matrix Start togglet sich selbst (mkw::leiste_toggle).
                mkw::leiste::app_starten("matrix-start");
                Task::none()
            }
            Msg::AufnahmeOeffnen => {
                mkw::leiste::app_starten("matrix-aufnahme");
                Task::none()
            }
            Msg::ZentraleToggle => {
                mkw::leiste::app_starten("matrix-zentrale");
                Task::none()
            }
            Msg::ZeilenRad(id, y) => {
                return iced::advanced::widget::operate(
                    iced::advanced::widget::operation::scrollable::scroll_by(
                        iced::advanced::widget::Id::new(id),
                        iced::advanced::widget::operation::scrollable::AbsoluteOffset {
                            x: -y,
                            y: 0.0,
                        },
                    ),
                );
            }
            // vom to_layer_message-Makro ergänzte Varianten (Layer-Steuerung)
            _ => Task::none(),
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        let mut alle = vec![
            // Sofort-Reaktion: der Ereignis-Strom ersetzt das 1-s-Pollen;
            // ein 5-s-Poll bleibt als Fallback (Uhr-Widget, Palette).
            mkw::leinwand_strom().map(|_| Msg::Tick),
            mkw::tick_zur_minute().map(|_| Msg::Tick),
            tick("dock", std::time::Duration::from_secs(5)).map(|_| Msg::Tick),
            mkw::palette_fade_abo().map(|_| Msg::FadeTick),
            // OSD-Kanal: nur ein stat() — schnell genug für Tastendruck-Echo.
            tick("dock-osd", std::time::Duration::from_millis(150)).map(|_| Msg::OsdPuls),
        ];
        if !self.huepfer.is_empty() && !self.hart {
            alle.push(
                tick("dock-huepf", std::time::Duration::from_millis(60)).map(|_| Msg::HuepfTick),
            );
        }
        Subscription::batch(alle)
    }

    /// Ist das Dock gerade das OSD?
    fn osd_aktiv(&self) -> bool {
        self.osd
            .map(|(_, seit)| {
                seit.elapsed().map(|d| d.as_millis() < OSD_STANDZEIT).unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// Das Dynamic-Dock-Gesicht (Nutzer-OSD-Entwurf): die Pille bleibt,
    /// Zeile 1 zeigt die STUFE als Balken, Zeile 2 den Klartext.
    fn osd_gesicht(&self, stand: mk::osd::Stand) -> Element<'_, Msg> {
        let p = self.palette;
        let (zeichen, voll) = mkw::osd_anzeige::zeichen(stand);
        let symbol_farbe = if voll { p.on_surface } else { p.on_surface_variant };
        let anteil = stand.prozent / 100.0;

        // Zeile 1: Symbol + Stufen-Balken (Kit-Baustein) in Kachel-Höhe.
        let spur_breite = (breite(self.kacheln.len().max(3)) as f32
            - 2.0 * RAND
            - mk::icon_size::LARGE
            - mk::spacing::M)
            .max(80.0);
        let balken = mkw::osd_anzeige::stufen_balken(p, anteil, voll, spur_breite);
        let stufe = container(
            row![
                mkw::symbol(zeichen, mk::icon_size::LARGE, symbol_farbe),
                balken,
            ]
            .spacing(mk::spacing::M)
            .align_y(iced::Alignment::Center),
        )
        .height(Length::Fixed(KNOPF + PUNKT + mk::spacing::XXS))
        .center_y(Length::Fill);

        // Zeile 2: was geändert wurde, in Worten.
        let wort = mkw::txt(stand.text(), mk::typo::HINWEIS, p.on_surface_variant);

        container(
            container(
                container(
                    column![stufe, container(wort).center_x(Length::Fill)]
                        .spacing(mk::spacing::XXS)
                        .align_x(iced::Alignment::Center),
                )
                .center_x(Length::Fill)
                .center_y(Length::Fill),
            )
            .padding(RAND)
            // EXAKT die Dock-Silhouette (Nutzer, 8.7.): das OSD-Gesicht
            // ist maßgleich mit der Ruhe-Pille — gleiche Breite, gleiche
            // Höhe, gleiche Lage. Nur der Inhalt morpht.
            .width(Length::Fixed(breite(self.kacheln.len()) as f32))
            .height(Length::Fixed(HOEHE as f32))
            .style(move |_| mkw::leiste::pille(p, KNOPF_RADIUS, RAND)),
        )
        .center_x(Length::Fill)
        .align_bottom(Length::Fill)
        .padding(iced::Padding {
            top: mkw::leiste::SCHATTEN_RAND,
            left: mkw::leiste::SCHATTEN_RAND,
            right: mkw::leiste::SCHATTEN_RAND,
            bottom: mkw::leiste::SCHATTEN_RAND,
        })
        .into()
    }

    /// Kleiner Widget-Knopf für Zeile 2 (Symbol + Wort).
    fn zeilen_knopf<'a>(&self, inhalt: Element<'a, Msg>, msg: Msg) -> Element<'a, Msg> {
        let p = self.palette;
        mkw::lupe(
            button(inhalt)
                .padding([2, mk::spacing::S as u16])
                .style(move |_, status| mkw::leiste::knopf_stil(p, status, mk::radius::KLEIN))
                .on_press(msg),
        )
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.palette;

        // ---- Das OSD-Gesicht: Zeile 1 = Stufe, Zeile 2 = was geändert wurde.
        if self.osd_aktiv() {
            if let Some((stand, _)) = self.osd {
                return self.osd_gesicht(stand);
            }
        }

        // ---- Zeile 1: die Kacheln
        let mut leiste = row![].spacing(LUECKE).align_y(iced::Alignment::Center);
        for (i, k) in self.kacheln.iter().enumerate() {
            let kante = ICON;
            let bild: Element<'_, Msg> = match self.icons.get(&k.app_id).cloned().flatten() {
                Some(h) => image(h).width(kante).height(kante).into(),
                None => container(
                    mkw::txt(initial(&k.app_id), mk::typo::TITEL, p.on_primary_container),
                )
                .center_x(Length::Fixed(kante))
                .center_y(Length::Fixed(kante))
                .style(move |_| container::Style {
                    background: Some(color(p.primary_container).into()),
                    border: iced::border::rounded(mk::radius::innen(
                        KNOPF_RADIUS,
                        (KNOPF - ICON) / 2.0,
                    )),
                    ..Default::default()
                })
                .into(),
            };

            // launchanim (R38): |sin| = wiederholte Hüpfer. Ruhelage ist
            // MITTIG (4 px Boden = (52−44)/2) — sonst sitzt das Icon zu
            // tief und der Hover-Grund wirkt verrutscht (Nutzer-Fund).
            // Bleiben 4 px Spielraum nach oben für den Hub.
            let hub = if self.hart {
                0.0
            } else {
                self.huepfer.get(&k.app_id).map_or(0.0, |seit| {
                    let t = seit.elapsed().as_secs_f32();
                    (t * std::f32::consts::PI / 0.55).sin().abs() * 4.0
                })
            };
            let knopf = button(
                container(bild)
                    .center_x(Length::Fill)
                    .align_y(iced::alignment::Vertical::Bottom)
                    .padding(iced::Padding {
                        bottom: (KNOPF - ICON) / 2.0 + hub,
                        ..iced::Padding::ZERO
                    }),
            )
            .width(Length::Fixed(KNOPF))
            .height(Length::Fixed(KNOPF))
            .padding(0)
            .style(move |_theme, status| mkw::leiste::knopf_stil(p, status, KNOPF_RADIUS))
            .on_press(Msg::Klick(i));
            let knopf = iced::widget::mouse_area(knopf)
                .on_right_press(Msg::Kontext(i));
            // Abzeichen: rote Kapsel oben rechts (badgeLabel-Grammatik).
            let knopf: Element<'_, Msg> = match &k.abzeichen {
                Some(text) => iced::widget::stack![
                    knopf,
                    container(
                        container(mkw::txt(text.clone(), mk::typo::ETIKETT, p.on_primary))
                            .padding([0, 5])
                            .style(move |_| container::Style {
                                background: Some(color(p.error).into()),
                                border: iced::border::rounded(mk::radius::kapsel(16.0)),
                                ..Default::default()
                            }),
                    )
                    .align_x(iced::alignment::Horizontal::Right)
                    .width(Length::Fill),
                ]
                .into(),
                None => knopf.into(),
            };
            // Kit-Lupe: die Kachel wächst federnd unterm Zeiger.
            let knopf = mkw::lupe_stark(knopf, 1.22);

            // Punkt: läuft (Akzent bei Fokus) — ruhende Pins bleiben still.
            let punkt_farbe = if !k.laeuft() {
                Color::TRANSPARENT
            } else if k.fokus {
                color(p.primary)
            } else {
                color(p.on_surface_variant.mit_alpha(0.55))
            };
            let punkt = container(Space::new())
                .width(Length::Fixed(PUNKT))
                .height(Length::Fixed(PUNKT))
                .style(move |_| container::Style {
                    background: Some(punkt_farbe.into()),
                    border: iced::border::rounded(mk::radius::kapsel(PUNKT)),
                    ..Default::default()
                });

            leiste = leiste.push(
                column![
                    knopf,
                    container(punkt).center_x(Length::Fixed(KNOPF)),
                ]
                .spacing(mk::spacing::XXS)
                .align_x(iced::Alignment::Center),
            );
        }

        // ---- Zeile 2: die Widgets (wie die Bar, nur im Dock)
        let mut unten = row![]
            .spacing(mk::spacing::M)
            .align_y(iced::Alignment::Center);
        for w in &self.widgets {
            let el = match w {
                Widget::Apps => self.zeilen_knopf(
                    row![
                        mkw::symbol(mkw::symbol::APPS, mk::font_size::MEDIUM, p.on_surface),
                        mkw::txt("Apps", mk::typo::HINWEIS, p.on_surface),
                    ]
                    .spacing(mk::spacing::XXS)
                    .align_y(iced::Alignment::Center)
                    .into(),
                    Msg::AppsOeffnen,
                ),
                Widget::Zentrale => self.zeilen_knopf(
                    mkw::symbol(mkw::symbol::TUNE, mk::font_size::MEDIUM, p.on_surface),
                    Msg::ZentraleToggle,
                ),
                Widget::Aufnahme => self.zeilen_knopf(
                    mkw::symbol(mkw::symbol::IMAGE, mk::font_size::MEDIUM, p.on_surface),
                    Msg::AufnahmeOeffnen,
                ),
                Widget::Uhr => {
                    mkw::txt(&self.uhr, mk::typo::HINWEIS, p.on_surface_variant).into()
                }
                Widget::Zwischenablage => {
                    // Vorschau der neuesten Kopie, einzeilig und kurz;
                    // Klick öffnet den Verlauf (matrix-kontext clips).
                    let vorschau = self.zw_vorschau.as_deref().map(|t| {
                        let mut z: String =
                            t.split_whitespace().collect::<Vec<_>>().join(" ");
                        if z.chars().count() > 18 {
                            z = z.chars().take(17).collect::<String>() + "…";
                        }
                        z
                    });
                    self.zeilen_knopf(
                        row![
                            mkw::symbol(
                                mkw::symbol::CONTENT_COPY,
                                mk::font_size::MEDIUM,
                                p.on_surface,
                            ),
                            mkw::txt(
                                vorschau.unwrap_or_else(|| String::from("Zwischenablage")),
                                mk::typo::HINWEIS,
                                p.on_surface_variant,
                            ),
                        ]
                        .spacing(mk::spacing::XXS)
                        .align_y(iced::Alignment::Center)
                        .into(),
                        Msg::ZwischenablageMenue,
                    )
                }
            };
            unten = unten.push(el);
        }

        // Zeilen-Scroller (R49, Nutzer-Fund): Läuft eine Zeile über die
        // Pillenbreite hinaus (z. B. lange Zwischenablage-Vorschau in
        // Zeile 2), glitchte sie bisher aus dem Dock. Jetzt ist JEDE
        // Zeile ihr eigener horizontaler Scroller — unsichtbarer Balken,
        // Rad/Trackpad wirken quer, und weil das Ereignis an die Zeile
        // unter der Maus geht, scrollt genau die. Passt der Inhalt,
        // bleibt er zentriert wie bisher.
        let innen_breite = breite(self.kacheln.len()) as f32 - 2.0 * RAND;

        let pille = container(mkw::leiste::schatten_schichten(
            container(
                column![
                    container(zeilen_scroller(leiste.into(), innen_breite, "dock-zeile-1")).center_x(Length::Fill),
                    container(zeilen_scroller(unten.into(), innen_breite, "dock-zeile-2")).center_x(Length::Fill),
                ]
                .spacing(mk::spacing::XXS)
                .align_x(iced::Alignment::Center),
            )
            .padding(RAND)
            .style(move |_| mkw::leiste::pille(p, KNOPF_RADIUS, RAND))
            .into(),
            KNOPF_RADIUS + RAND,
        ))
        .center_x(Length::Fill);

        // Kachel-Kontextmenü (Rechtsklick): kleine Pille ÜBER dem Icon —
        // Anpinnen/Loslösen und Beenden; Klick daneben schließt.
        let Some(mi) = self.menue.filter(|&i| i < self.kacheln.len()) else {
            return container(pille)
                .center_x(Length::Fill)
                .align_bottom(Length::Fill)
                .padding(iced::Padding {
                    top: mkw::leiste::SCHATTEN_RAND,
                    left: mkw::leiste::SCHATTEN_RAND,
                    right: mkw::leiste::SCHATTEN_RAND,
                    bottom: mkw::leiste::SCHATTEN_RAND,
                })
                .into();
        };
        let k = &self.kacheln[mi];
        let pin_text = if k.gepinnt { "Loslösen" } else { "Anpinnen" };
        // MatrixUI MenuFamily: der Dock-Kontext in der EINEN Menü-Sprache.
        let mut eintraege: Vec<mkw::ui::MenuEintrag<Msg>> = vec![mkw::ui::MenuEintrag::Punkt {
            zeichen: None,
            titel: String::from(pin_text),
            farbe: None,
            msg: Msg::PinWechsel(mi),
        }];
        if k.laeuft() {
            eintraege.push(mkw::ui::MenuEintrag::Punkt {
                zeichen: None,
                titel: String::from("Beenden"),
                farbe: None,
                msg: Msg::Beenden(mi),
            });
        }
        // Immer erreichbar: das Dock (und die Bar) anpassen — wie „Dock
        // anpassen …" im Leitbild-Dock-Kontextmenü.
        eintraege.push(mkw::ui::MenuEintrag::Punkt {
            zeichen: Some(mkw::symbol::TUNE),
            titel: String::from("Anpassen …"),
            farbe: None,
            msg: Msg::Anpassen,
        });
        let menue_pille = mkw::ui::menu_family(None, eintraege, p);
        // Über der geklickten Kachel ausrichten.
        let dock_breite = breite(self.kacheln.len()) as f32;
        let kachel_x = RAND + mi as f32 * (KNOPF + LUECKE) + KNOPF / 2.0;
        let einzug = (kachel_x - mkw::ui::MENU_BREITE / 2.0)
            .clamp(0.0, (dock_breite - mkw::ui::MENU_BREITE).max(0.0));
        let menue_zeile = iced::widget::mouse_area(
            container(
                row![Space::new().width(Length::Fixed(einzug)), menue_pille]
            )
            .width(Length::Fill)
            .height(Length::Fixed(MENUE_HOEHE as f32))
            // Direkt über der Dock-Pille kleben, nicht am oberen Rand.
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(iced::Padding { bottom: mk::spacing::XS, ..iced::Padding::ZERO }),
        )
        .on_press(Msg::MenueZu);

        container(column![menue_zeile, pille])
            .padding(iced::Padding {
                top: 0.0,
                left: mkw::leiste::SCHATTEN_RAND,
                right: mkw::leiste::SCHATTEN_RAND,
                bottom: mkw::leiste::SCHATTEN_RAND,
            })
            .into()
    }
}
