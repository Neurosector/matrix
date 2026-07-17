//! Matrix Bar — App #12, Mitglied der MatrixKit-Leisten-Familie.
//!
//! Die Leiste am oberen Rand, seit 2.0 aus WIDGETS gebaut: drei Zonen
//! (links | mitte | rechts), belegt über die Einstellungs-Kultur —
//! `~/.config/matrix/bar-widgets`, z. B. `fokus | uhr | puls zentrale
//! nutzer`. Widgets: fokus (aktive App), uhr, puls (Systemwerte),
//! zentrale (öffnet/schließt das Kontrollzentrum), nutzer (Kontoname +
//! Sitzungsmenü: Sperren, Abmelden, Standby, Neustart, Herunterfahren,
//! Wiederherstellung — Endgültiges will einen zweiten Klick).
//!
//! Layer::Top (Bottom verhungert in der Leinwand-Session, Task #39).
//! v1 reserviert KEINEN Platz (exclusive_zone 0), solange die DMS-Bar
//! parallel existiert — `matrix-bar --reservieren` schaltet die Zone
//! scharf, wenn die Bar die alte ersetzt.

use iced::widget::{button, column, container, image, mouse_area, row, stack, Space};
use iced::{Color, Element, Font, Length, Subscription, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;

mod tray;
use matrixkit_widgets::tick;

const HOEHE: u32 = 44;
/// Einheitlicher Innenabstand der Pillen-Bar (Nutzer, R50c): Hover-
/// Flächen halten RUNDUM dieselbe Luft zur Barkante — links wie oben.
/// Er deckelt zugleich die Knopfhöhe (44 − 2·6 = 32) gegen die hohen
/// Zeilenboxen der Symbol-Schrift.
const EINZUG: f32 = 6.0;
/// Surface-Höhe bei offenem Sitzungsmenü (Bar + Panel + Luft).
const MENUE_HOEHE: u32 = 340;

fn main() -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};
    let reservieren = std::env::args().any(|a| a == "--reservieren");
    iced_layershell::application(
        App::new,
        || String::from("matrix-bar"),
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
            // Breite 0 = spannt zwischen den Ankern auf (volle Kante).
            size: Some((0, HOEHE + mkw::leiste::SCHATTEN_RAND as u32)),
            anchor: Anchor::Top | Anchor::Left | Anchor::Right,
            // R50: die Bar schwebt wie das Dock — 10 px Luft zur Kante.
            margin: (10, 0, 0, 0),
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

// ---------------------------------------------------------------- Widgets

/// Ein Bar-Widget — die Bausteine der drei Zonen.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Widget {
    Matrix,
    Tray,
    Fokus,
    Uhr,
    Puls,
    Glocke,
    Akku,
    Zentrale,
    Nutzer,
    /// R70c: öffnet das Aufnahme-Panel (Aufnahme-Panel).
    Aufnahme,
}

impl Widget {
    fn aus(name: &str) -> Option<Self> {
        match name {
            "matrix" => Some(Self::Matrix),
            "fokus" => Some(Self::Fokus),
            "uhr" => Some(Self::Uhr),
            "puls" => Some(Self::Puls),
            "glocke" => Some(Self::Glocke),
            "tray" => Some(Self::Tray),
            "akku" => Some(Self::Akku),
            "zentrale" => Some(Self::Zentrale),
            "nutzer" => Some(Self::Nutzer),
            "aufnahme" => Some(Self::Aufnahme),
            _ => None,
        }
    }
}

/// Zonen-Belegung aus `~/.config/matrix/bar-widgets` (Einstellungs-Kultur):
/// drei Zonen durch `|`, Widgets durch Leerzeichen. Fehlt die Datei,
/// gilt die Standard-Belegung.
fn widgets_lesen() -> (Vec<Widget>, Vec<Widget>, Vec<Widget>) {
    zonen_parse(
        &mk::einstellung::lesen("bar-widgets").unwrap_or_else(|| {
            String::from("matrix fokus | uhr | akku puls tray glocke zentrale nutzer")
        }),
    )
}

/// Reines Parsing (testbar): drei Zonen durch `|`, Unbekanntes fällt still raus.
fn zonen_parse(konf: &str) -> (Vec<Widget>, Vec<Widget>, Vec<Widget>) {
    let mut zonen = konf.splitn(3, '|').map(|z| {
        z.split_whitespace()
            .filter_map(Widget::aus)
            .collect::<Vec<_>>()
    });
    (
        zonen.next().unwrap_or_default(),
        zonen.next().unwrap_or_default(),
        zonen.next().unwrap_or_default(),
    )
}

// ------------------------------------------------------------- Sitzung

/// Die Einträge des Sitzungsmenüs. Endgültiges fragt per Zweitklick nach.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Aktion {
    Sperren,
    Abmelden,
    Standby,
    Neustart,
    Herunterfahren,
    Wiederherstellung,
}

impl Aktion {
    const ALLE: [Aktion; 6] = [
        Aktion::Sperren,
        Aktion::Abmelden,
        Aktion::Standby,
        Aktion::Neustart,
        Aktion::Herunterfahren,
        Aktion::Wiederherstellung,
    ];

    fn zeichen(self) -> char {
        match self {
            Aktion::Sperren => mkw::symbol::LOCK,
            Aktion::Abmelden => mkw::symbol::LOGOUT,
            Aktion::Standby => mkw::symbol::DARK_MODE,
            Aktion::Neustart => mkw::symbol::RESTART,
            Aktion::Herunterfahren => mkw::symbol::POWER,
            Aktion::Wiederherstellung => mkw::symbol::SHIELD,
        }
    }

    fn titel(self) -> &'static str {
        match self {
            Aktion::Sperren => "Sperren",
            Aktion::Abmelden => "Abmelden …",
            Aktion::Standby => "Standby",
            Aktion::Neustart => "Neustart",
            Aktion::Herunterfahren => "Herunterfahren",
            Aktion::Wiederherstellung => "Wiederherstellung",
        }
    }

    /// Endgültige Aktionen wollen einen zweiten, roten Klick.
    fn endgueltig(self) -> bool {
        matches!(self, Aktion::Neustart | Aktion::Herunterfahren)
    }

    fn frage(self) -> &'static str {
        match self {
            Aktion::Neustart => "Wirklich neu starten?",
            Aktion::Herunterfahren => "Wirklich ausschalten?",
            _ => "",
        }
    }

    fn ausfuehren(self) {
        match self {
            Aktion::Sperren => {
                // Eigener Sperrschirm zuerst; Fallback DMS, dann logind.
                mkw::leiste::app_starten("matrix-sperre");
            }
            // niri zeigt seinen eigenen Bestätigungs-Dialog — deshalb „…".
            // Der Abmelde-Klang (02) läuft SYNCHRON zu Ende, bevor die
            // Session fällt — sonst würde poweroff ihn abschneiden (R30).
            Aktion::Abmelden => {
                mk::feedback::jetzt("abmeldung", "02-abmeldung.wav");
                mk::leinwand::abmelden(true);
            }
            Aktion::Standby => {
                mk::befehl::still("systemctl", &["suspend"]);
            }
            Aktion::Neustart => {
                mk::feedback::jetzt("abmeldung", "02-abmeldung.wav");
                mk::befehl::still("systemctl", &["reboot"]);
            }
            Aktion::Herunterfahren => {
                mk::feedback::jetzt("abmeldung", "02-abmeldung.wav");
                mk::befehl::still("systemctl", &["poweroff"]);
            }
            Aktion::Wiederherstellung => mkw::leiste::app_starten("matrix-wiederherstellung"),
        }
    }
}

/// Anzeigename des Kontos: GECOS aus /etc/passwd, sonst der Loginname.
fn nutzer_lesen() -> String {
    let login = std::env::var("USER").unwrap_or_else(|_| String::from("Konto"));
    if let Ok(passwd) = std::fs::read_to_string("/etc/passwd") {
        for l in passwd.lines() {
            if l.starts_with(&format!("{login}:")) {
                if let Some(gecos) = l.split(':').nth(4) {
                    let name = gecos.split(',').next().unwrap_or("").trim();
                    if !name.is_empty() {
                        return gross(name);
                    }
                }
            }
        }
    }
    gross(&login)
}

/// Erster Buchstabe groß — Kontonamen stehen aufrecht in der Bar.
fn gross(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_else(|| s.to_string())
}

/// Fokussiertes Fenster als „App — Titel" (gekürzt), über die Kit-Brücke.
fn fokus_lesen() -> Option<String> {
    let f = mkw::leinwand::fokus()?;
    let kurz = f
        .app_id
        .rsplit('.')
        .next()
        .unwrap_or(&f.app_id)
        .split('-')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|erst| erst.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut s = if f.titel.is_empty() || f.titel == kurz {
        kurz
    } else {
        format!("{kurz} — {}", f.titel)
    };
    if s.chars().count() > 64 {
        s = s.chars().take(63).collect::<String>() + "…";
    }
    Some(s)
}

/// Lautstärke der Standard-Senke: „45 %" oder „Stumm".
fn lautstaerke_lesen() -> Option<String> {
    let z = mk::befehl::erste_zeile("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])?;
    if z.contains("MUTED") {
        return Some(String::from("Stumm"));
    }
    let wert: f32 = z.split_whitespace().nth(1)?.parse().ok()?;
    Some(format!("{:.0} %", wert * 100.0))
}

/// Akkustand (0–100, lädt?) — None auf Geräten ohne Akku (PC).
fn akku_lesen() -> Option<(u8, bool)> {
    let rd = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for e in rd.flatten() {
        let pfad = e.path();
        let Ok(cap) = std::fs::read_to_string(pfad.join("capacity")) else {
            continue;
        };
        let Ok(prozent) = cap.trim().parse::<u8>() else {
            continue;
        };
        let laedt = std::fs::read_to_string(pfad.join("status"))
            .map(|s| s.trim() == "Charging")
            .unwrap_or(false);
        return Some((prozent.min(100), laedt));
    }
    None
}

/// Lebt DMS? Die Glocke zeigt sich nur, wenn ihr Verlauf existiert.
fn dms_lebt() -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", "qs"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Name der aktiven Netzverbindung (ohne Loopback).
fn netz_lesen() -> Option<String> {
    let out = std::process::Command::new("nmcli")
        .args(["-t", "-f", "NAME,TYPE", "connection", "show", "--active"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| !l.contains("loopback"))
        .and_then(|l| l.split(':').next())
        .map(|s| s.to_string())
}

// ----------------------------------------------------------------- App

#[to_layer_message]
#[derive(Debug, Clone)]
enum Msg {
    FadeTick,
    Uhr,
    Puls,
    Fokus,
    ZentraleToggle,
    /// R69: laufende Bildschirmaufnahme beenden.
    FilmStopp,
    /// R70c: das Aufnahme-Widget öffnet das Panel.
    AufnahmePanel,
    GlockeKlick,
    TrayKlick(String, String),
    TrayRechts(String, String),
    MenueAuf,
    MenueZu,
    /// Das Matrix-Menü (Apfel-Menü-Extrakt, Runde 28) ganz links.
    MatrixAuf,
    /// Eine App aus dem Matrix-Menü starten (schließt das Menü).
    App(&'static str),
    Aktion(Aktion),
}

struct App {
    palette: mk::Palette,
    watcher: mk::PaletteWatcher,
    zonen: (Vec<Widget>, Vec<Widget>, Vec<Widget>),
    sys: sysinfo::System,
    uhr: String,
    datum: String,
    fokus: Option<String>,
    cpu: f32,
    ram: String,
    lautstaerke: Option<String>,
    netz: Option<String>,
    nutzer: String,
    puls_zaehler: u8,
    akku: Option<(u8, bool)>,
    dms: bool,
    menue_offen: bool,
    /// Matrix-Menü (links) offen — teilt sich Wachstum & Schließen mit dem
    /// Sitzungsmenü, aber nur eines ist je offen.
    matrix_offen: bool,
    tray_items: std::sync::Arc<std::sync::Mutex<Vec<tray::TrayItem>>>,
    tray_befehl: std::sync::mpsc::Sender<tray::TrayBefehl>,
    /// Endgültige Aktion, die auf ihren zweiten (roten) Klick wartet.
    bestaetigung: Option<Aktion>,
    /// R69: Start-Epoche einer laufenden Bildschirmaufnahme — die Bar
    /// zeigt dann den roten Stopp-Punkt (Leitbild- Menüleisten-Verhalten).
    film_start: Option<u64>,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        // Statusleisten-Leitbild-Extrakt: die Bar ist der Tray-Watcher (Runde 24).
        let tray_items_init = tray::starten();
        let mut app = App {
            palette: mk::Palette::load().unwrap_or_default(),
            watcher: mk::PaletteWatcher::new(),
            zonen: widgets_lesen(),
            sys: sysinfo::System::new(),
            uhr: String::new(),
            datum: String::new(),
            fokus: None,
            cpu: 0.0,
            ram: String::new(),
            lautstaerke: None,
            netz: None,
            nutzer: nutzer_lesen(),
            puls_zaehler: 0,
            akku: akku_lesen(),
            dms: dms_lebt(),
            menue_offen: false,
            matrix_offen: false,
            tray_items: tray_items_init.0,
            tray_befehl: tray_items_init.1,
            bestaetigung: None,
            film_start: None,
        };
        app.uhr_lesen();
        app.puls_lesen();
        (app, Task::none())
    }

    fn uhr_lesen(&mut self) {
        // Leitbild-Menueleisten-Uhr, empirisch (R33+R35): ShowDayOfWeek=1,
        // ShowDate=0, das LeitbildICUForce12HourTime=1 -> "So. 1:27 PM". Deutsche
        // Locales lassen %p oft leer — dann 24-h-Fallback.
        let am_pm = mk::befehl::erste_zeile("date", &["+%p"])
            .map(|p| !p.trim().is_empty())
            .unwrap_or(false);
        let format = if am_pm { "+%a. %-I:%M %p" } else { "+%a. %H:%M" };
        if let Some(z) = mk::befehl::erste_zeile("date", &[format]) {
            self.uhr = z;
            self.datum = String::new();
        }
    }

    fn puls_lesen(&mut self) {
        if self.watcher.changed() {
            if let Some(neu) = mk::Palette::load() {
                self.palette = neu;
            }
        }
        self.zonen = widgets_lesen(); // Belegung lebt — Datei ändern reicht
        self.fokus = fokus_lesen();
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.cpu = self.sys.global_cpu_usage();
        self.ram = mk::format::bytes_speicher(self.sys.used_memory());
        self.lautstaerke = lautstaerke_lesen();
        // Netz/Akku/DMS ändern sich träge — jeden 5. Puls (10 s) reicht.
        self.puls_zaehler = self.puls_zaehler.wrapping_add(1);
        if self.puls_zaehler % 5 == 0 || self.netz.is_none() {
            self.netz = netz_lesen();
            {
                // Lade-Chime (Leitbild-Extrakt R33, PowerChime): beim ANSTECKEN
                // klingt es — nur beim Anstecken, nie beim Abziehen.
                let neu = akku_lesen();
                if let (Some((_, alt_laedt)), Some((_, neu_laedt))) = (self.akku, neu) {
                    if !alt_laedt && neu_laedt {
                        std::thread::spawn(|| {
                            mk::feedback::jetzt("geraete", "06-geraet-verbunden.wav");
                        });
                    }
                }
                self.akku = neu;
            }
            self.dms = dms_lebt();
        }
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::FadeTick => {
                // NUR die Palette nachziehen — keine Prozess-Spawns,
                // keine Netz/Akku-Checks (die CPU-Lektion vom 8.7.).
                if self.watcher.changed() {
                    if let Some(p) = mk::Palette::load() {
                        self.palette = p;
                    }
                }
            }
            Msg::Uhr => self.uhr_lesen(),
            Msg::Fokus => self.fokus = fokus_lesen(),
            Msg::Puls => {
                self.puls_lesen();
                // R69: Aufnahme-Status von matrix-aufnahme lesen.
                let status = std::env::var("XDG_RUNTIME_DIR")
                    .map(|d| format!("{d}/matrix-aufnahme-film"))
                    .ok()
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .and_then(|s| s.lines().nth(2).and_then(|z| z.parse::<u64>().ok()));
                self.film_start = status;
            }
            Msg::FilmStopp => {
                mkw::leiste::app_starten_mit("matrix-aufnahme", &["film-stopp"]);
                self.film_start = None;
            }
            Msg::AufnahmePanel => {
                mkw::leiste::app_starten_mit("matrix-aufnahme", &["panel"]);
            }
            Msg::ZentraleToggle => {
                // Die Zentrale togglet sich selbst (mkw::leiste_toggle).
                mkw::leiste::app_starten("matrix-zentrale");
            }
            Msg::TrayKlick(d, p) => {
                let _ = self.tray_befehl.send(tray::TrayBefehl::Aktivieren(d, p));
            }
            Msg::TrayRechts(d, p) => {
                let _ = self.tray_befehl.send(tray::TrayBefehl::Sekundaer(d, p));
            }
            Msg::GlockeKlick => {
                // Toggle-Signal an den Mitteilungs-Daemon (Verlauf öffnen).
                let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
                let _ = std::fs::write(format!("{dir}/matrix-mitteilungen-zeige"), "1");
            }
            Msg::MenueAuf => {
                self.menue_offen = true;
                self.matrix_offen = false;
                self.bestaetigung = None;
                return Task::done(Msg::SizeChange((0, MENUE_HOEHE + mkw::leiste::SCHATTEN_RAND as u32)));
            }
            Msg::MatrixAuf => {
                self.matrix_offen = true;
                self.menue_offen = false;
                self.bestaetigung = None;
                return Task::done(Msg::SizeChange((0, MENUE_HOEHE + mkw::leiste::SCHATTEN_RAND as u32)));
            }
            Msg::MenueZu => {
                self.menue_offen = false;
                self.matrix_offen = false;
                self.bestaetigung = None;
                return Task::done(Msg::SizeChange((0, HOEHE + mkw::leiste::SCHATTEN_RAND as u32)));
            }
            Msg::App(id) => {
                // "name bereich" → Wunsch-Datei schreiben, dann starten (R41).
                let mut teile = id.splitn(2, ' ');
                let name = teile.next().unwrap_or(id);
                if let Some(bereich) = teile.next() {
                    let basis = std::env::var("XDG_RUNTIME_DIR")
                        .unwrap_or_else(|_| String::from("/tmp"));
                    let _ = std::fs::write(
                        format!("{basis}/{name}-bereich"),
                        bereich,
                    );
                }
                mkw::leiste::app_starten(name);
                self.menue_offen = false;
                self.matrix_offen = false;
                self.bestaetigung = None;
                return Task::done(Msg::SizeChange((0, HOEHE + mkw::leiste::SCHATTEN_RAND as u32)));
            }
            Msg::Aktion(a) => {
                if a.endgueltig() && self.bestaetigung != Some(a) {
                    self.bestaetigung = Some(a); // erst der rote Zweitklick zählt
                } else {
                    a.ausfuehren();
                    self.menue_offen = false;
                    self.matrix_offen = false;
                    self.bestaetigung = None;
                    return Task::done(Msg::SizeChange((0, HOEHE + mkw::leiste::SCHATTEN_RAND as u32)));
                }
            }
            // vom to_layer_message-Makro ergänzte Varianten
            _ => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        let mut alle = vec![
            // TimelineView(.everyMinute)-Extrakt: exakt zur Minutengrenze.
            mkw::tick_zur_minute().map(|_| Msg::Uhr),
            tick("puls", std::time::Duration::from_secs(2)).map(|_| Msg::Puls),
            mkw::palette_fade_abo().map(|_| Msg::FadeTick),
            // Fokus-Wechsel kommen SOFORT über den Ereignis-Strom.
            mkw::leinwand_strom().map(|_| Msg::Fokus),
        ];
        // Selbsttest-Haken: MATRIX_BAR_TEST_MENUE=1 öffnet das Menü nach
        // 2 s — prüft SizeChange/Stretch ohne Maus (Ferndiagnose).
        if !self.menue_offen && std::env::var("MATRIX_BAR_TEST_MENUE").is_ok() {
            alle.push(tick("menuetest", std::time::Duration::from_secs(2)).map(|_| Msg::MenueAuf));
        }
        Subscription::batch(alle)
    }

    /// Klickbares Bar-Element im Knopf-Ton der Leisten-Familie.
    /// Bar-Widgets tragen die SidebarFamily-Optik (MatrixUI, 8.7.).
    fn familien<'a>(&'a self, zeichen: Option<char>, titel: Option<&'a str>, aktiv: bool, msg: Msg) -> Element<'a, Msg> {
        // R60: EIN Bar-Textmaß — HINWEIS, wie Uhr/Fokus/Puls (vorher
        // stach der Nutzer-Knopf mit FLIESS um 1 px heraus).
        mkw::ui::familien_knopf(zeichen, titel, aktiv, msg, self.palette, mk::typo::HINWEIS)
    }

    /// None = Widget hat auf diesem Gerät gerade nichts zu sagen.
    fn widget_bauen(&self, w: Widget) -> Option<Element<'_, Msg>> {
        let p = self.palette;
        Some(match w {
            // Apfel-Menü-Extrakt (Runde 28): das Systemmenü ganz links —
            // immer da, immer gleich, von hier ist alles erreichbar.
            // Menü-Wandern (R37, 1984er-Grammatik): ist das ANDERE Menü
            // offen, wechselt schon das Überfahren — ohne Klick.
            Widget::Matrix => {
                let knopf = self.familien(
                    Some(mkw::symbol::APPS),
                    None,
                    self.matrix_offen,
                    if self.matrix_offen { Msg::MenueZu } else { Msg::MatrixAuf },
                );
                if self.menue_offen {
                    mouse_area(knopf).on_enter(Msg::MatrixAuf).into()
                } else {
                    knopf
                }
            }
            Widget::Fokus => mkw::txt(
                self.fokus
                    .clone()
                    .unwrap_or_else(|| String::from("Matrix")),
                mk::typo::HINWEIS,
                p.on_surface,
            )
            .into(),
            Widget::Uhr => {
                let mut z = row![mkw::txt(&self.uhr, mk::typo::HINWEIS, p.on_surface)];
                if !self.datum.is_empty() {
                    z = z.push(mkw::txt(
                        format!("   {}", self.datum),
                        mk::typo::HINWEIS,
                        p.on_surface_variant,
                    ));
                }
                z.into()
            }
            Widget::Puls => {
                let mut puls: Vec<String> =
                    vec![format!("CPU {:.0} %", self.cpu), self.ram.clone()];
                if let Some(v) = &self.lautstaerke {
                    puls.push(format!("Ton {v}"));
                }
                if let Some(n) = &self.netz {
                    puls.push(n.clone());
                }
                mkw::txt(puls.join("  ·  "), mk::typo::HINWEIS, p.on_surface_variant).into()
            }
            Widget::Tray => {
                let items = self.tray_items.lock().ok()?.clone();
                if items.is_empty() {
                    return None;
                }
                let mut zeile = row![].spacing(mk::spacing::XXS);
                for it in items {
                    let bild: Element<'_, Msg> = match &it.icon {
                        Some((w, h, rgba)) => iced::widget::image(
                            iced::widget::image::Handle::from_rgba(*w, *h, rgba.clone()),
                        )
                        .width(20)
                        .height(20)
                        .into(),
                        None => mkw::txt(
                            it.titel.chars().next().unwrap_or('•').to_string(),
                            mk::typo::HINWEIS,
                            p.on_surface,
                        )
                        .into(),
                    };
                    zeile = zeile.push(
                        iced::widget::mouse_area(mkw::lupe(
                            button(bild)
                                .padding(4)
                                .style(move |_, status| {
                                    mkw::leiste::knopf_stil(p, status, mk::radius::KLEIN)
                                })
                                .on_press(Msg::TrayKlick(it.dienst.clone(), it.pfad.clone())),
                        ))
                        .on_right_press(Msg::TrayRechts(it.dienst.clone(), it.pfad.clone())),
                    );
                }
                zeile.into()
            }
            Widget::Glocke => {
                // „Nicht stören" (mk-Einstellung) zeigt eine stumme Glocke.
                let dnd = mk::einstellung::lesen("nicht-stoeren").as_deref() == Some("an");
                let (zeichen, farbe) = if dnd {
                    (mkw::symbol::VOLUME_OFF, p.on_surface_variant)
                } else {
                    (mkw::symbol::NOTIFICATIONS, p.on_surface)
                };
                let _ = farbe;
                self.familien(Some(zeichen), None, false, Msg::GlockeKlick)
            }
            Widget::Akku => {
                let (prozent, laedt) = self.akku?;
                let zeichen = if laedt {
                    mkw::symbol::BATTERY_CHARGING
                } else {
                    mkw::symbol::BATTERY_FULL
                };
                mkw::etikett(
                    zeichen,
                    format!("{prozent} %"),
                    mk::typo::HINWEIS,
                    p.on_surface_variant,
                )
            }
            Widget::Zentrale => {
                self.familien(Some(mkw::symbol::TUNE), None, false, Msg::ZentraleToggle)
            }
            Widget::Aufnahme => {
                self.familien(Some(mkw::symbol::IMAGE), None, false, Msg::AufnahmePanel)
            }
            Widget::Nutzer => {
                let knopf = self.familien(
                    None,
                    Some(self.nutzer.as_str()),
                    self.menue_offen, // Menü offen = Aktiv-Pille wie in der Sidebar
                    if self.menue_offen { Msg::MenueZu } else { Msg::MenueAuf },
                );
                // Menü-Wandern: vom offenen Matrix-Menü herüberfahren genügt.
                if self.matrix_offen {
                    mouse_area(knopf).on_enter(Msg::MenueAuf).into()
                } else {
                    knopf
                }
            }
        })
    }

    fn zone_bauen(&self, widgets: &[Widget]) -> Element<'_, Msg> {
        let mut z = row![]
            .spacing(mk::spacing::M)
            .align_y(iced::Alignment::Center);
        for w in widgets {
            if let Some(el) = self.widget_bauen(*w) {
                z = z.push(el);
            }
        }
        z.into()
    }

    /// Das Sitzungsmenü — eine Pille unter dem Nutzer-Widget.
    fn menue(&self) -> Element<'_, Msg> {
        let p = self.palette;
        // Kopf: der lebende Avatar grüßt namentlich.
        let kopf: Element<'_, Msg> = match matrixkit_icons::avatar_png(&p)
            .map(image::Handle::from_bytes)
        {
            Some(h) => row![
                image(h).width(28).height(28),
                mkw::txt(&self.nutzer, mk::typo::KOPF, p.on_surface),
            ]
            .spacing(mk::spacing::S)
            .align_y(iced::Alignment::Center)
            .into(),
            None => mkw::txt(&self.nutzer, mk::typo::KOPF, p.on_surface).into(),
        };
        // MatrixUI MenuFamily: das Sitzungsmenü in der EINEN Menü-Sprache.
        let mut eintraege: Vec<mkw::ui::MenuEintrag<Msg>> = Vec::new();
        for a in Aktion::ALLE {
            if matches!(a, Aktion::Standby | Aktion::Wiederherstellung) {
                eintraege.push(mkw::ui::MenuEintrag::Trenner);
            }
            let (titel, farbe) = if self.bestaetigung == Some(a) {
                (a.frage().to_string(), Some(p.error))
            } else {
                (a.titel().to_string(), None)
            };
            eintraege.push(mkw::ui::MenuEintrag::Punkt {
                zeichen: Some(a.zeichen()),
                titel,
                farbe,
                msg: Msg::Aktion(a),
            });
        }
        mkw::ui::menu_family(Some(kopf), eintraege, p)
    }

    /// Das Matrix-Menü — der Apfel-Menü-Extrakt (Runde 28, live am Referenzsystem
    /// abgelesen): Gruppen durch Trenner, „…" = fragt nach / öffnet etwas,
    /// ohne „…" = passiert sofort, Live-Badge am Update-Eintrag, und der
    /// Abmelden-Eintrag trägt den ECHTEN Namen.
    fn matrix_menue(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let mut e: Vec<mkw::ui::MenuEintrag<Msg>> = vec![
            mkw::ui::MenuEintrag::Punkt {
                zeichen: Some(mkw::symbol::INFO),
                titel: String::from("Über diese Matrix"),
                farbe: None,
                msg: Msg::App("matrix-hilfe"),
            },
            mkw::ui::MenuEintrag::Trenner,
        ];
        // Systemeinstellungen + Softwareupdate — das Badge lebt (Apfel-
        // Grammatik: das Menü selbst trägt Status).
        e.push(mkw::ui::MenuEintrag::Punkt {
            zeichen: Some(mkw::symbol::TUNE),
            titel: String::from("Systemeinstellungen …"),
            farbe: None,
            msg: Msg::App("matrix-einstellungen"),
        });
        match mk::abzeichen::lesen("matrix-updater") {
            Some(n) => e.push(mkw::ui::MenuEintrag::PunktMitBadge {
                zeichen: Some(mkw::symbol::RESTART),
                titel: String::from("Softwareupdate …"),
                badge: if n == "1" { String::from("1 Update") } else { format!("{n} Updates") },
                farbe: None,
                msg: Msg::App("matrix-updater"),
            }),
            None => e.push(mkw::ui::MenuEintrag::Punkt {
                zeichen: Some(mkw::symbol::RESTART),
                titel: String::from("Softwareupdate …"),
                farbe: None,
                msg: Msg::App("matrix-updater"),
            }),
        }
        e.push(mkw::ui::MenuEintrag::Trenner);
        e.push(mkw::ui::MenuEintrag::Punkt {
            zeichen: Some(mkw::symbol::WIDGETS),
            titel: String::from("Leiste & Dock anpassen …"),
            farbe: None,
            // Fusion R41: öffnet die Einstellungen im Bereich Leiste & Dock.
            msg: Msg::App("matrix-einstellungen leisten"),
        });
        e.push(mkw::ui::MenuEintrag::Trenner);
        // Energie-Aktionen (Ruhezustand/Neustart/Ausschalten) leben NUR im
        // Sitzungsmenü rechts — keine Dopplung (Nutzer, 12.7.).
        e.push(mkw::ui::MenuEintrag::Punkt {
            zeichen: Some(mkw::symbol::LOCK),
            titel: String::from("Bildschirm sperren"),
            farbe: None,
            msg: Msg::Aktion(Aktion::Sperren),
        });
        e.push(mkw::ui::MenuEintrag::Punkt {
            zeichen: Some(mkw::symbol::LOGOUT),
            titel: format!("{} abmelden …", self.nutzer),
            farbe: None,
            msg: Msg::Aktion(Aktion::Abmelden),
        });
        mkw::ui::menu_family(None, e, p)
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.palette;

        // R69: der rote Stopp-Punkt hängt FEST rechts, sobald ein Film
        // läuft — unabhängig von der Widget-Konfiguration (wie das Leitbild).
        let mut rechts = row![].spacing(mk::spacing::M).align_y(iced::Alignment::Center);
        if let Some(start) = self.film_start {
            let sek = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().saturating_sub(start))
                .unwrap_or(0);
            rechts = rechts.push(mkw::lupe(
                button(
                    row![
                        mkw::txt("\u{25cf}", mk::typo::HINWEIS, p.error),
                        mkw::txt(
                            format!("{}:{:02}", sek / 60, sek % 60),
                            mk::typo::HINWEIS,
                            p.on_surface,
                        ),
                    ]
                    .spacing(mk::spacing::XS)
                    .align_y(iced::Alignment::Center),
                )
                .padding([4, mk::spacing::S as u16])
                .on_press(Msg::FilmStopp)
                .style(move |_, status| mkw::leiste::knopf_stil(p, status, mk::radius::KLEIN)),
            ));
        }
        rechts = rechts.push(self.zone_bauen(&self.zonen.2));
        let seiten = row![
            self.zone_bauen(&self.zonen.0),
            Space::new().width(Length::Fill),
            rechts,
        ]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .height(Length::Fill);

        let ebene = stack![
            seiten,
            container(self.zone_bauen(&self.zonen.1))
                .center_x(Length::Fill)
                .center_y(Length::Fill),
        ];

        // R50c (Nutzer, Screenshot-vermessen): EIN Einzug rundum.
        // Vorher klebten Hover-Pillen oben/unten an der Kante (0 px)
        // bei 16 px links — jetzt überall EINZUG Luft, Knopfhöhe damit
        // gedeckelt, und die Bar-Rundung ist exakt konzentrisch:
        // CORNER_RADIUS + EINZUG. Seitlich hält die Surface
        // SCHATTEN_RAND Atemraum (Pillen-Lektion vom 8.7.).
        let bar = container(mkw::leiste::schatten_schichten(
            container(ebene)
                .padding(EINZUG)
                .width(Length::Fill)
                .height(Length::Fixed(HOEHE as f32))
                .style(move |_| mkw::leiste::pille(p, mk::CORNER_RADIUS, EINZUG))
                .into(),
            mk::CORNER_RADIUS + EINZUG,
        ))
        // Seitenabstand = Kantenabstand oben (10) — Nutzer-Wunsch nach
        // gleichem Rand ringsum. Der Schatten bekommt damit seitlich nur
        // 10 px Atem statt SCHATTEN_RAND; der Blur-Kern (Offset 4) passt,
        // nur der äußerste Gauss-Saum wird beschnitten — unsichtbar.
        .padding(iced::Padding {
            left: 10.0,
            right: 10.0,
            ..iced::Padding::ZERO
        });

        if !self.menue_offen && !self.matrix_offen {
            return bar.into();
        }

        // Offenes Menü: Bar oben, Pille darunter — Sitzungsmenü rechts,
        // Matrix-Menü links (unter seinem Anker, wie das Apfel-Menü).
        // Klick auf die freie Fläche schließt (deshalb wächst die Surface).
        let (pille, seite) = if self.matrix_offen {
            (self.matrix_menue(), iced::alignment::Horizontal::Left)
        } else {
            (self.menue(), iced::alignment::Horizontal::Right)
        };
        let unten = mouse_area(
            container(pille)
                .align_x(seite)
                .padding([mk::spacing::S as u16, mk::spacing::M as u16])
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Msg::MenueZu);

        column![bar, unten].into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zonen_parse_standard() {
        let (l, m, r) = zonen_parse("matrix fokus | uhr | akku puls tray glocke zentrale nutzer");
        assert_eq!(l, vec![Widget::Matrix, Widget::Fokus]);
        assert_eq!(m, vec![Widget::Uhr]);
        assert_eq!(
            r,
            vec![
                Widget::Akku,
                Widget::Puls,
                Widget::Tray,
                Widget::Glocke,
                Widget::Zentrale,
                Widget::Nutzer
            ]
        );
    }

    #[test]
    fn zonen_parse_robust() {
        // Unbekanntes fällt raus, fehlende Zonen bleiben leer, kein Panik.
        let (l, m, r) = zonen_parse("quatsch fokus");
        assert_eq!(l, vec![Widget::Fokus]);
        assert!(m.is_empty() && r.is_empty());
        let (l, _, _) = zonen_parse("");
        assert!(l.is_empty());
    }

    #[test]
    fn gross_kapitalisiert() {
        assert_eq!(gross("nicolas"), "Nutzer");
        assert_eq!(gross(""), "");
    }
}
