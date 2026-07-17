//! Matrix Einstellungen — die System-Einstellungen im MatrixKit-Stil.
//!
//! Achte MatrixKit-App und Keimzelle von Roadmap-Etappe 3 (die
//! DankMaterialShell-Einstellungen durch Eigenes ersetzen). Zeigt zugleich
//! die in Runde 6 aus dem Leitbild System Settings entzogenen Controls in echtem
//! Einsatz: Schieber, segmentierte Auswahl, Stepper, aufklappbare Gruppe.
//! Jede Einstellung schreibt in eine Datei unter ~/.config/matrix/, die das
//! übrige System lesen kann — die App ist nur die Oberfläche darüber.

use iced::widget::{column, container, Space};
use iced::{Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::time::Duration;

mod dev;
mod klaenge;
mod uebersicht;
mod verbindungen;
mod leinwand;
mod leisten;

const APP_ID: &str = "matrix-einstellungen";

/// Bereiche der fusionierten Einstellungen (R41/42): Übersicht (Dashboard,
/// Startseite), Allgemein, Leinwand, Hintergrund, Leiste & Dock, Klänge,
/// Entwickler.
const BEREICHE: usize = 8;

/// Titel je Bereich — die Werkzeugleiste erzählt, wo man steht.
const BEREICH_TITEL: [&str; BEREICHE] = [
    "Übersicht", "Allgemein", "Verbindungen", "Leinwand", "Hintergrund",
    "Leiste & Dock", "Ton", "Entwickler",
];

/// Schlagworte je Bereich für die Sidebar-Suche (Leitbild-Grammatik:
/// die Suche findet Bereiche auch über ihre Inhalte).
const SCHLAGWORTE: [&str; BEREICHE] = [
    "dashboard monitor system info prozessor cpu ram arbeitsspeicher speicher akku batterie netzwerk ip kernel temperatur",
    "erscheinung hell dunkel modus name rechnername uhr zeit format endung dateiendung erweitert",
    "wlan wifi netz netzwerk internet funk bluetooth geraete kopfhoerer maus tastatur verbinden hotspot",
    "geist modi navigation maus scrollen klebe wisch touch",
    "wallpaper bild bilder hintergrundbild hell dunkel galerie",
    "bar dock topbar widgets anpassen wackeln pins zeile zwischenablage",
    "klaenge toene ton sound audio gong hoerprobe stumm master anmeldung fehler ausgabe eingabe mikrofon lautsprecher geraet pegel test lautstaerke hdmi kopfhoerer",
    "dev developer ssh netz zugang schluessel update partner freigabe",
];

/// Bereichs-Wunsch von außen (Bar/Dock/Kontextmenü): Datei im Runtime-
/// Verzeichnis, wird gelesen und verbraucht.
fn bereich_wunsch() -> Option<usize> {
    let basis = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| String::from("/tmp"));
    let pfad = std::path::PathBuf::from(basis).join("matrix-einstellungen-bereich");
    let wunsch = std::fs::read_to_string(&pfad).ok()?;
    let _ = std::fs::remove_file(&pfad);
    match wunsch.trim() {
        "uebersicht" | "dashboard" => Some(0),
        "verbindungen" | "wlan" | "bluetooth" | "netz" => Some(2),
        "leinwand" => Some(3),
        "hintergrund" => Some(4),
        "leisten" | "anpassen" => Some(5),
        "klaenge" | "klänge" | "ton" => Some(6),
        "dev" | "entwickler" | "entwicklerzugang" => Some(7),
        "allgemein" => Some(1),
        _ => None,
    }
}

fn main() -> iced::Result {
    if !mk::fenster::einzelinstanz(APP_ID) {
        return Ok(());
    }
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| String::from("Matrix Einstellungen"))
        .subscription(App::subscription)
        .window(mkw::fenster_settings("matrix-einstellungen", 940.0, 680.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

/// Icon-Stil-Auswahl — deckt sich mit ~/.config/matrix/icon-stil.
const ICON_STILE: [&str; 2] = ["Standard", "Getönt"];

/// Erscheinung (Leitbild Systemeinstellungen > Erscheinungsbild): Automatisch
/// folgt dem echten Sonnenstand (theme-sonne-Timer), Hell/Dunkel schalten
/// fest um und pausieren den Rhythmus.
const ERSCHEINUNGEN: [&str; 3] = ["Automatisch (Sonne)", "Hell", "Dunkel"];

/// Desktop-Modus (Nutzer-Leinwand-Konzept, 7.7.2026): Klassisch = Ablage-
/// Minimieren; Leinwand = Fenster bleiben an Ort, die −-Ampel legt den
/// Privatschleier über den Inhalt. Freies 2D-Panning kommt mit dem
/// Compositor-Ausbau (Roadmap).
const DESKTOP_MODI: [&str; 2] = ["Klassisch", "Unendliche Leinwand"];

/// Eckenradius-Spanne: MINI (4) bis über GROSS hinaus (24) — die Skala
/// aus mk::radius, in 2er-Schritten.
const RADIUS_MIN: u8 = mk::radius::MINI as u8;
const RADIUS_MAX: u8 = 24;

/// Steckbrief des Systems — einmal beim Start gelesen (Leitbild „Über diesen Referenzsystem").
struct Steckbrief {
    os: String,
    kernel: String,
    rechner: String,
    speicher: String,
}

impl Steckbrief {
    fn lesen() -> Self {
        let os = std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("PRETTY_NAME="))
                    .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
            })
            .unwrap_or_else(|| String::from("Matrix"));
        let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let rechner = std::fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let speicher = std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|s| {
                s.lines().find(|l| l.starts_with("MemTotal:")).and_then(|l| {
                    l.split_whitespace().nth(1).and_then(|kb| kb.parse::<u64>().ok())
                })
            })
            .map(|kb| mk::format::bytes_speicher(kb * 1024))
            .unwrap_or_default();
        Self { os, kernel, rechner, speicher }
    }
}

struct App {
    rahmen: mkw::Rahmen,
    /// 0 Allgemein · 1 Leinwand · 2 Hintergrund · 3 Leiste & Dock.
    bereich: usize,
    /// Besuchte Bereiche — die ◀ ▶-Chevrons der Werkzeugleisten-Familie.
    verlauf: Vec<usize>,
    verlauf_pos: usize,
    suche: String,
    uebersicht: uebersicht::Panel,
    verbindungen: verbindungen::Panel,
    leinwand: leinwand::Panel,
    leisten: leisten::Panel,
    klaenge: klaenge::Panel,
    dev: dev::Panel,
    steckbrief: Steckbrief,
    /// Erscheinungsbild
    erscheinung: usize,
    desktop_modus: usize,
    icon_stil: usize,
    bewegung_reduziert: bool,
    /// Anzeige
    skalierung: f32,
    eckenradius: u8,
    /// Index in mk::typo::STUFEN (dynamicTypeSize-Extrakt).
    textstufe: usize,
    /// Aufklappbare „Erweitert"-Gruppe
    erweitert_offen: bool,
    zeige_debug_overlay: bool,
    /// kurze Rückmeldung in der Fußzeile
    gespeichert: Option<String>,
    /// Rückgängig-Stapel (Leitbild UndoManager, Runde 13): Strg+Z stellt
    /// die letzte Regler-Änderung wieder her.
    undo: mk::rueckgaengig::Stapel<Aenderung>,
}

/// Was rückgängig gemacht werden kann — der alte Wert VOR der Änderung.
#[derive(Debug, Clone)]
enum Aenderung {
    Skalierung(f32),
    Eckenradius(u8),
}

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Tick,
    Bereich(usize),
    Zurueck,
    Vor,
    Suche(String),
    SucheLeeren,
    Uebersicht(uebersicht::Msg),
    Verbindungen(verbindungen::Msg),
    Leinwand(leinwand::Msg),
    Leisten(leisten::Msg),
    Klaenge(klaenge::Msg),
    Dev(dev::Msg),
    Erscheinung(&'static str),
    DesktopModus(&'static str),
    IconStil(usize),
    BewegungUmschalten(bool),
    Skalierung(f32),
    RadiusMinus,
    RadiusPlus,
    TextKleiner,
    TextGroesser,
    ErweitertUmschalten,
    DebugUmschalten(bool),
    Taste(mkw::Taste),
}

/// Einstellungs-Kultur: alles läuft über mk::einstellung (AppStorage-Analog).
fn conf_schreiben(name: &str, wert: &str) {
    mk::einstellung::schreiben(name, wert);
}

fn conf_lesen(name: &str) -> Option<String> {
    mk::einstellung::lesen(name)
}

/// Kommando still ausführen (Theme-Umschaltung, Timer) — best effort.
fn still(cmd: &str, args: &[&str]) {
    let _ = std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Rendert die App-Icons neu (liest icon-stil selbst) — dadurch wirkt der
/// Icon-Stil-Schalter sofort sichtbar im Dock/Launcher.
fn icons_neu_rendern() {
    let home = std::env::var("HOME").unwrap_or_default();
    let kandidaten = [
        format!("{home}/.local/bin/matrixkit-icons"),
        String::from("/usr/local/bin/matrixkit-icons"),
        String::from("/usr/bin/matrixkit-icons"),
    ];
    for k in kandidaten {
        if std::path::Path::new(&k).exists() {
            let _ = std::process::Command::new(&k)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            return;
        }
    }
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        // Bestehende Einstellungen vom Datenträger übernehmen.
        let icon_stil = match conf_lesen("icon-stil").as_deref() {
            Some("getoent") => 1,
            _ => 0,
        };
        let erscheinung = match conf_lesen("erscheinung").as_deref() {
            Some("hell") => 1,
            Some("dunkel") => 2,
            _ => 0,
        };
        let bewegung_reduziert = conf_lesen("bewegung").as_deref() == Some("reduziert");
        // Der Desktop-Modus-Picker zeigt die REALITÄT der Session.
        let desktop_modus_real = if mkw::session_ist_leinwand() { 1 } else { 0 };
        conf_schreiben("desktop-modus", if desktop_modus_real == 1 { "leinwand" } else { "klassisch" });
        let skalierung = conf_lesen("skalierung")
            .and_then(|s| s.parse::<f32>().ok())
            .unwrap_or(1.0)
            .clamp(0.8, 1.5);
        let textstufe = mk::einstellung::lesen("textgroesse")
            .and_then(|w| mk::typo::STUFEN.iter().position(|s| *s == w))
            .unwrap_or(1);
        let eckenradius = conf_lesen("eckenradius")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(mk::radius::NORMAL as u8)
            .clamp(RADIUS_MIN, RADIUS_MAX);
        let start_bereich = std::env::args()
            .nth(1)
            .and_then(|a| match a.as_str() {
                "uebersicht" | "dashboard" => Some(0),
                "verbindungen" | "wlan" | "bluetooth" => Some(2),
                "leinwand" => Some(3),
                "hintergrund" => Some(4),
                "leisten" | "anpassen" => Some(5),
                "klaenge" => Some(6),
                "dev" | "entwickler" => Some(7),
                _ => None,
            })
            .or_else(bereich_wunsch)
            .unwrap_or(0);
        let (leinwand_panel, leinwand_task) = leinwand::Panel::new();
        let (dev_panel, dev_task) = dev::Panel::new();
        let (verbindungen_panel, verbindungen_task) = verbindungen::Panel::new();
        (
            Self {
                rahmen: mkw::Rahmen::neu(APP_ID, &[]),
                bereich: start_bereich.min(BEREICHE - 1),
                uebersicht: uebersicht::Panel::new(),
                verbindungen: verbindungen_panel,
                leinwand: leinwand_panel,
                leisten: leisten::Panel::new(),
                klaenge: klaenge::Panel::new(),
                dev: dev_panel,
                steckbrief: Steckbrief::lesen(),
                erscheinung,
                desktop_modus: desktop_modus_real,
                icon_stil,
                bewegung_reduziert,
                skalierung,
                eckenradius,
                textstufe,
                erweitert_offen: false,
                zeige_debug_overlay: false,
                gespeichert: None,
                verlauf: vec![start_bereich.min(BEREICHE - 1)],
                verlauf_pos: 0,
                suche: String::new(),
                undo: mk::rueckgaengig::Stapel::neu(),
            },
            Task::batch([
                leinwand_task.map(Msg::Leinwand),
                dev_task.map(Msg::Dev),
                verbindungen_task.map(Msg::Verbindungen),
            ]),
        )
    }

    /// Bereichswechsel mit Verlauf (wie Dateien-Ordnerwechsel): alles
    /// hinter der aktuellen Position verfällt, das Ziel wird angehängt.
    fn gehe_zu(&mut self, b: usize) {
        let b = b.min(BEREICHE - 1);
        if b == self.bereich {
            return;
        }
        self.verlauf.truncate(self.verlauf_pos + 1);
        self.verlauf.push(b);
        self.verlauf_pos = self.verlauf.len() - 1;
        self.bereich = b;
        self.suche.clear();
    }

    fn quittung(&mut self, text: &str) {
        self.gespeichert = Some(format!("{text} gespeichert \u{2713}"));
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(m) => return self.rahmen.update(m).map(Msg::Rahmen),
            Msg::Bereich(b) => self.gehe_zu(b),
            Msg::Zurueck => {
                if self.verlauf_pos > 0 {
                    self.verlauf_pos -= 1;
                    self.bereich = self.verlauf[self.verlauf_pos];
                }
            }
            Msg::Vor => {
                if self.verlauf_pos + 1 < self.verlauf.len() {
                    self.verlauf_pos += 1;
                    self.bereich = self.verlauf[self.verlauf_pos];
                }
            }
            Msg::Suche(s) => self.suche = s,
            Msg::SucheLeeren => self.suche.clear(),
            Msg::Uebersicht(m) => return self.uebersicht.update(m).map(Msg::Uebersicht),
            Msg::Verbindungen(m) => return self.verbindungen.update(m).map(Msg::Verbindungen),
            Msg::Leinwand(m) => return self.leinwand.update(m).map(Msg::Leinwand),
            Msg::Leisten(m) => return self.leisten.update(m).map(Msg::Leisten),
            Msg::Klaenge(m) => return self.klaenge.update(m).map(Msg::Klaenge),
            Msg::Dev(m) => return self.dev.update(m).map(Msg::Dev),
            Msg::Tick => {
                self.rahmen.palette_geaendert();
                self.gespeichert = None;
                let p = self.rahmen.palette;
                self.uebersicht.tick(p);
                self.verbindungen.tick(p);
                self.leinwand.tick(p);
                self.leisten.tick(p);
                self.klaenge.tick(p);
                self.dev.tick(p);
                // Bar/Dock/Kontext wünschen einen Bereich? (Wunsch-Datei)
                if let Some(b) = bereich_wunsch() {
                    self.gehe_zu(b);
                }
            }
            Msg::Erscheinung(wahl) => {
                self.erscheinung = ERSCHEINUNGEN.iter().position(|e| *e == wahl).unwrap_or(0);
                match self.erscheinung {
                    1 => {
                        conf_schreiben("erscheinung", "hell");
                        still("dms", &["ipc", "call", "theme", "light"]);
                        // fester Modus: Sonnen-Rhythmus pausieren
                        still("systemctl", &["--user", "stop", "theme-sonne.timer", "theme-sonne-hell.timer", "theme-sonne-dunkel.timer"]);
                        self.gespeichert = Some(String::from("Erscheinung: Hell \u{2713} — Sonnen-Rhythmus pausiert"));
                    }
                    2 => {
                        conf_schreiben("erscheinung", "dunkel");
                        still("dms", &["ipc", "call", "theme", "dark"]);
                        still("systemctl", &["--user", "stop", "theme-sonne.timer", "theme-sonne-hell.timer", "theme-sonne-dunkel.timer"]);
                        self.gespeichert = Some(String::from("Erscheinung: Dunkel \u{2713} — Sonnen-Rhythmus pausiert"));
                    }
                    _ => {
                        conf_schreiben("erscheinung", "automatisch");
                        // Rhythmus wieder aktivieren + sofort den korrekten
                        // Modus für JETZT anwenden (theme-sonne rechnet selbst)
                        still("systemctl", &["--user", "start", "theme-sonne.timer"]);
                        still("systemctl", &["--user", "start", "theme-sonne.service"]);
                        self.gespeichert = Some(String::from("Erscheinung: Automatisch \u{2713} — folgt der Sonne"));
                    }
                }
            }
            Msg::DesktopModus(wahl) => {
                self.desktop_modus = DESKTOP_MODI.iter().position(|m| *m == wahl).unwrap_or(0);
                conf_schreiben("desktop-modus", if self.desktop_modus == 1 { "leinwand" } else { "klassisch" });
                // Ehrliche Quittung: der Desktop-Modus IST die Session —
                // gewechselt wird beim Anmelden, nicht per Schalter.
                let real_leinwand = mkw::session_ist_leinwand();
                self.gespeichert = Some(if (self.desktop_modus == 1) == real_leinwand {
                    if real_leinwand {
                        String::from("Unendliche Leinwand ist aktiv \u{2713}")
                    } else {
                        String::from("Klassischer Desktop ist aktiv \u{2713}")
                    }
                } else if self.desktop_modus == 1 {
                    String::from("Vorgemerkt — beim n\u{e4}chsten Anmelden im Login \u{201e}Matrix Leinwand\u{201c} w\u{e4}hlen")
                } else {
                    String::from("Vorgemerkt — beim n\u{e4}chsten Anmelden im Login \u{201e}Niri\u{201c} w\u{e4}hlen")
                });
            }
            Msg::IconStil(i) => {
                self.icon_stil = i;
                conf_schreiben("icon-stil", if i == 1 { "getoent" } else { "standard" });
                icons_neu_rendern();
                self.gespeichert =
                    Some(String::from("Icon-Stil gespeichert \u{2713} — Icons werden neu gezeichnet"));
            }
            Msg::BewegungUmschalten(an) => {
                self.bewegung_reduziert = an;
                conf_schreiben("bewegung", if an { "reduziert" } else { "voll" });
                self.quittung("Bewegung");
            }
            Msg::Skalierung(v) => {
                let neu = (v * 20.0).round() / 20.0;
                if (neu - self.skalierung).abs() > f32::EPSILON {
                    self.undo.merken("Anzeigegröße", Aenderung::Skalierung(self.skalierung));
                }
                self.skalierung = neu; // 0.05er-Raster
                conf_schreiben("skalierung", &format!("{:.2}", self.skalierung));
                self.quittung("Anzeigegröße");
            }
            Msg::RadiusMinus => {
                self.undo.merken("Eckenradius", Aenderung::Eckenradius(self.eckenradius));
                self.eckenradius = self.eckenradius.saturating_sub(2).max(RADIUS_MIN);
                conf_schreiben("eckenradius", &self.eckenradius.to_string());
                self.quittung("Eckenradius");
            }
            Msg::RadiusPlus => {
                self.undo.merken("Eckenradius", Aenderung::Eckenradius(self.eckenradius));
                self.eckenradius = (self.eckenradius + 2).min(RADIUS_MAX);
                conf_schreiben("eckenradius", &self.eckenradius.to_string());
                self.quittung("Eckenradius");
            }
            Msg::TextKleiner => {
                self.textstufe = self.textstufe.saturating_sub(1);
                mk::einstellung::schreiben("textgroesse", mk::typo::STUFEN[self.textstufe]);
                self.quittung("Textgröße");
            }
            Msg::TextGroesser => {
                self.textstufe = (self.textstufe + 1).min(mk::typo::STUFEN.len() - 1);
                mk::einstellung::schreiben("textgroesse", mk::typo::STUFEN[self.textstufe]);
                self.quittung("Textgröße");
            }
            Msg::Taste(t) => {
                if self.rahmen.taste(t) {
                    return Task::none();
                }
                match t {
                    // Strg+Z (Leitbild UndoManager): letzte Änderung zurück.
                    mkw::Taste::Rueckgaengig => match self.undo.zurueck() {
                        Some((name, Aenderung::Skalierung(v))) => {
                            self.skalierung = v;
                            conf_schreiben("skalierung", &format!("{v:.2}"));
                            self.gespeichert =
                                Some(format!("{name} wiederhergestellt \u{21a9}"));
                        }
                        Some((name, Aenderung::Eckenradius(v))) => {
                            self.eckenradius = v;
                            conf_schreiben("eckenradius", &v.to_string());
                            self.gespeichert =
                                Some(format!("{name} wiederhergestellt \u{21a9}"));
                        }
                        None => {}
                    },
                    // Strg+R (Leitbild-Runde 14): der aktive Bereich lädt neu —
                    // Dev und Verbindungen erheben ihren Status asynchron.
                    mkw::Taste::Aktualisieren => {
                        return match self.bereich {
                            2 => self
                                .verbindungen
                                .update(verbindungen::Msg::Neuladen)
                                .map(Msg::Verbindungen),
                            7 => self.dev.update(dev::Msg::Neuladen).map(Msg::Dev),
                            _ => Task::none(),
                        };
                    }
                    _ => {}
                }
            }
            Msg::ErweitertUmschalten => self.erweitert_offen = !self.erweitert_offen,
            Msg::DebugUmschalten(an) => {
                self.zeige_debug_overlay = an;
                conf_schreiben("debug-overlay", if an { "1" } else { "0" });
                self.quittung("Debug-Overlay");
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch([
            self.rahmen.abo().map(Msg::Rahmen),
            mkw::tick("einstellungen", Duration::from_secs(3)).map(|_| Msg::Tick),
            mkw::tasten_abo(Msg::Taste),
            self.leisten.abo().map(Msg::Leisten),
            self.klaenge.abo().map(Msg::Klaenge),
        ])
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;

        // ── Erscheinungsbild ──────────────────────────────────────────
        let erscheinung = mkw::sektion(
            "Erscheinungsbild",
            vec![
                mkw::zeile_menue(
                    "Erscheinung",
                    Some("Hell, Dunkel — oder automatisch mit dem Sonnenstand."),
                    &ERSCHEINUNGEN,
                    Some(ERSCHEINUNGEN[self.erscheinung.min(2)]),
                    Msg::Erscheinung,
                    p,
                ),
                mkw::zeile(
                    "Icon-Stil",
                    Some("Farbige oder auf die Palette getönte App-Symbole."),
                    None,
                    Some(container(mkw::segmente(&ICON_STILE, self.icon_stil, Msg::IconStil, p))
                        .width(Length::Fixed(180.0))
                        .into()),
                    p,
                ),
                mkw::zeile_schalter(
                    "Bewegung reduzieren",
                    Some("Federn und Übergänge auf das Nötigste beschränken."),
                    None,
                    self.bewegung_reduziert,
                    p,
                    Some(Msg::BewegungUmschalten(!self.bewegung_reduziert)),
                ),
            ],
            p,
        );

        // ── Anzeige ───────────────────────────────────────────────────
        let anzeige = mkw::sektion(
            "Anzeige",
            vec![
                mkw::zeile(
                    "Anzeigegröße",
                    Some("Schrift und Bedienelemente — Vormerkung für den Shell-Ausbau."),
                    None,
                    Some(
                        iced::widget::row![
                            container(mkw::schieber(0.8..=1.5, self.skalierung, 0.05, Msg::Skalierung, p))
                                .width(Length::Fixed(150.0)),
                            mkw::txt(
                                format!("{:.0} %", self.skalierung * 100.0),
                                mk::typo::KLEIN,
                                p.on_surface_variant,
                            )
                            .width(Length::Fixed(44.0))
                            .align_x(iced::alignment::Horizontal::Right),
                        ]
                        .spacing(mk::spacing::S)
                        .align_y(iced::Alignment::Center)
                        .into(),
                    ),
                    p,
                ),
                mkw::zeile(
                    "Eckenradius",
                    Some("Rundung der Fenster und Karten — Vormerkung für den Shell-Ausbau."),
                    None,
                    Some(mkw::stepper(
                        format!("{} px", self.eckenradius),
                        (self.eckenradius > RADIUS_MIN).then_some(Msg::RadiusMinus),
                        (self.eckenradius < RADIUS_MAX).then_some(Msg::RadiusPlus),
                        p,
                    )),
                    p,
                ),
                mkw::zeile(
                    "Textgröße",
                    Some("Skaliert alle Matrix-Texte und -Symbole — sofort, überall."),
                    None,
                    Some(mkw::stepper(
                        match mk::typo::STUFEN[self.textstufe] {
                            "klein" => String::from("Klein"),
                            "gross" => String::from("Groß"),
                            "sehr-gross" => String::from("Sehr groß"),
                            _ => String::from("Normal"),
                        },
                        (self.textstufe > 0).then_some(Msg::TextKleiner),
                        (self.textstufe + 1 < mk::typo::STUFEN.len()).then_some(Msg::TextGroesser),
                        p,
                    )),
                    p,
                ),
            ],
            p,
        );

        // ── Erweitert (aufklappbar) ───────────────────────────────────
        let erweitert_inhalt = column![mkw::zeile_schalter(
            "Debug-Overlay anzeigen",
            Some("Blendet Bildrate und Ebenen-Grenzen ein."),
            None,
            self.zeige_debug_overlay,
            p,
            Some(Msg::DebugUmschalten(!self.zeige_debug_overlay)),
        )]
        .into();
        let erweitert = container(mkw::aufklappen(
            "Erweitert",
            self.erweitert_offen,
            Msg::ErweitertUmschalten,
            erweitert_inhalt,
            p,
        ))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(color(p.surface_container_high).into()),
            border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
            shadow: mkw::elevation::karte(),
            ..Default::default()
        });

        // ── Schreibtisch: Nutzer-Leinwand-Konzept ───────────────────
        let schreibtisch = mkw::sektion(
            "Schreibtisch",
            vec![mkw::zeile_menue(
                "Desktop-Modus",
                Some("Leinwand: Fenster bleiben an Ort — die \u{2212}-Ampel legt den Privatschleier über den Inhalt statt abzulegen."),
                &DESKTOP_MODI,
                Some(DESKTOP_MODI[self.desktop_modus.min(1)]),
                Msg::DesktopModus,
                p,
            )],
            p,
        );

        // ── Über dieses System (Leitbild „Über diesen Referenzsystem") ─────────────
        let ueber = mkw::sektion(
            "Über dieses System",
            vec![
                mkw::zeile_wert("Betriebssystem", None, &self.steckbrief.os, p),
                mkw::zeile_wert("Rechner", None, &self.steckbrief.rechner, p),
                mkw::zeile_wert("Kernel", None, &self.steckbrief.kernel, p),
                mkw::zeile_wert("Arbeitsspeicher", None, &self.steckbrief.speicher, p),
            ],
            p,
        );

        let liste = column![erscheinung, anzeige, schreibtisch, ueber, erweitert]
            .spacing(mk::spacing::L);

        // Fusion R41: die SidebarFamily-Anatomie — sieben Bereiche, EINE App.
        let alle_punkte = vec![
            mkw::SidebarPunkt { zeichen: mkw::symbol::MONITORING, titel: "Übersicht", anzahl: None },
            mkw::SidebarPunkt { zeichen: mkw::symbol::TUNE, titel: "Allgemein", anzahl: None },
            mkw::SidebarPunkt { zeichen: mkw::symbol::WIFI, titel: "Verbindungen", anzahl: None },
            mkw::SidebarPunkt { zeichen: mkw::symbol::TOUCH_APP, titel: "Leinwand", anzahl: None },
            mkw::SidebarPunkt { zeichen: mkw::symbol::PALETTE, titel: "Hintergrund", anzahl: None },
            mkw::SidebarPunkt { zeichen: mkw::symbol::WIDGETS, titel: "Leiste & Dock", anzahl: None },
            mkw::SidebarPunkt { zeichen: mkw::symbol::VOLUME_UP, titel: "Ton", anzahl: None },
            mkw::SidebarPunkt { zeichen: mkw::symbol::CODE, titel: "Entwickler", anzahl: None },
        ];
        // Suche (Werkzeugleisten-Familie): filtert die Sidebar wie die
        // Leitbild-Systemeinstellungen — Titel UND Schlagworte je Bereich.
        let s_klein = self.suche.to_lowercase();
        let (punkte, abbildung): (Vec<_>, Vec<usize>) = if s_klein.is_empty() {
            (alle_punkte, (0..BEREICHE).collect())
        } else {
            let mut pk = Vec::new();
            let mut ab = Vec::new();
            for (i, punkt) in alle_punkte.into_iter().enumerate() {
                let treffer = punkt.titel.to_lowercase().contains(&s_klein)
                    || SCHLAGWORTE[i].contains(&s_klein);
                if treffer {
                    pk.push(punkt);
                    ab.push(i);
                }
            }
            (pk, ab)
        };
        let aktiv_sichtbar = abbildung
            .iter()
            .position(|&b| b == self.bereich)
            .unwrap_or(usize::MAX);
        let wahl_abbildung = abbildung.clone();

        let aktiv_inhalt: Element<'_, Msg> = match self.bereich {
            0 => self.uebersicht.ansicht().map(Msg::Uebersicht),
            2 => self.verbindungen.ansicht().map(Msg::Verbindungen),
            3 => self.leinwand.leinwand_ansicht().map(Msg::Leinwand),
            4 => self.leinwand.hintergrund_ansicht().map(Msg::Leinwand),
            5 => self.leisten.ansicht().map(Msg::Leisten),
            6 => self.klaenge.ansicht().map(Msg::Klaenge),
            7 => self.dev.ansicht().map(Msg::Dev),
            _ => liste.into(),
        };
        let fusstext = match self.bereich {
            0 => self.uebersicht.fusstext(),
            2 => self.verbindungen.fusstext(),
            3 | 4 => self.leinwand.fusstext().unwrap_or_else(|| {
                String::from("Änderungen wirken sofort in der Leinwand — gespeichert in ~/.config/matrix/")
            }),
            5 => String::from("Änderungen greifen sofort — Bar und Dock lesen ihre Belegung laufend neu."),
            6 => self.klaenge.fusstext(),
            7 => String::from("Zugang & Netz für die Matrix-Entwicklung — Passwörter tippst immer du selbst"),
            _ => self
                .gespeichert
                .clone()
                .unwrap_or_else(|| String::from("Jede Änderung landet sofort in ~/.config/matrix/")),
        };

        // Werkzeugleisten-Familie (aus Matrix Dateien): ◀ ▶ Titel … Suche.
        let leiste = mkw::ui::werkzeugleiste(
            String::from(BEREICH_TITEL[self.bereich.min(BEREICHE - 1)]),
            (self.verlauf_pos > 0).then_some(Msg::Zurueck),
            (self.verlauf_pos + 1 < self.verlauf.len()).then_some(Msg::Vor),
            Vec::new(),
            Vec::new(),
            &self.suche,
            Msg::Suche,
            Msg::SucheLeeren,
            p,
        );
        let karte = mkw::ui::sidebar_family(
            punkte,
            aktiv_sichtbar,
            None,
            move |i| Msg::Bereich(wahl_abbildung[i]),
            column![
                leiste,
                Space::new().height(mk::spacing::S),
                self.rahmen.scrollflaeche(aktiv_inhalt, Msg::Rahmen),
                mkw::fusszeile(fusstext, p),
            ]
            .spacing(0)
            .into(),
            p,
        );

        self.rahmen.fenster(
            "Matrix Einstellungen",
            env!("CARGO_PKG_VERSION"),
            "Die System-Einstellungen im MatrixKit-Stil — Allgemein, Leinwand, Hintergrund, Leiste & Dock. Jede Änderung sofort auf dem Datenträger.",
            karte.into(),
            Msg::Rahmen,
        )
    }
}
