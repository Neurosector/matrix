//! Matrix Start — die Suchpalette-Palette (Super+Space öffnet sie über der
//! Bildschirmmitte). Getippt wird über ALLES gesucht:
//!   • installierte Apps (aus den .desktop-Dateien),
//!   • OFFENE Fenster — Enter fährt die Leinwand-Kamera hin (focus-window),
//!   • System-Aktionen (Sperren, Erscheinung umschalten, Einstellungen,
//!     Abmelden, Neustart, Ausschalten),
//!   • ein Rechner (Eingabe wie „12*8+4" → „= 100", Enter kopiert das Ergebnis).
//! ↑/↓ wählt, Enter löst aus, Esc schließt. Zweiter Aufruf schließt (Toggle).

use iced::widget::{column, container, image, mouse_area, row, Space};
use iced::{Color, Element, Font, Length, Subscription, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;

const BREITE: u32 = 620;
const HOEHE: u32 = 520;
const TREFFER: usize = 8;
const ICON: f32 = 32.0;

fn main() -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};

    // Toggle: lief die Palette schon, ist sie jetzt zu — wir auch.
    if !mkw::leiste_toggle() {
        return Ok(());
    }

    iced_layershell::application(
        App::new,
        || String::from("matrix-start"),
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
            size: Some((BREITE, HOEHE)),
            anchor: Anchor::empty(),
            layer: Layer::Overlay,
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            exclusive_zone: 0,
            ..Default::default()
        },
        default_font: Font::with_name("Inter Variable"),
        fonts: mkw::symbol_font_laden().into_iter().collect(),
        ..Default::default()
    })
    .run()
}

// ----------------------------------------------------------- App-Index

/// Eine startbare App aus einer .desktop-Datei.
#[derive(Debug, Clone)]
struct Eintrag {
    app_id: String,
    name: String,
    exec: String,
    terminal: bool,
}

fn index_bauen() -> Vec<Eintrag> {
    let home = std::env::var("HOME").unwrap_or_default();
    let quellen = [
        String::from("/usr/share/applications"),
        String::from("/var/lib/flatpak/exports/share/applications"),
        format!("{home}/.local/share/flatpak/exports/share/applications"),
        format!("{home}/.local/share/applications"),
    ];
    let mut nach_id: std::collections::HashMap<String, Eintrag> = std::collections::HashMap::new();
    for q in &quellen {
        let Ok(rd) = std::fs::read_dir(q) else { continue };
        for e in rd.flatten() {
            let pfad = e.path();
            if pfad.extension().and_then(|x| x.to_str()) != Some("desktop") {
                continue;
            }
            let Some(stamm) = pfad.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Some(eintrag) = desktop_lesen(&pfad, stamm) {
                nach_id.insert(stamm.to_string(), eintrag);
            } else {
                nach_id.remove(stamm);
            }
        }
    }
    // R48: ~/Programme — AppImage im Ordner = installiert, wie das Leitbild'
    // /Applications. Kein .desktop nötig: der Ordner IST das Register.
    let prog = format!("{home}/Programme");
    if let Ok(rd) = std::fs::read_dir(&prog) {
        for e in rd.flatten() {
            let pfad = e.path();
            let endung_ok = pfad
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("appimage"));
            if !endung_ok {
                continue;
            }
            let Some(stamm) = pfad.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let id = format!("programm-{}", stamm.to_lowercase());
            nach_id.entry(id.clone()).or_insert(Eintrag {
                app_id: id,
                name: stamm.to_string(),
                exec: format!("\"{}\"", pfad.display()),
                terminal: false,
            });
        }
    }
    let mut liste: Vec<Eintrag> = nach_id.into_values().collect();
    liste.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    liste
}

fn desktop_lesen(pfad: &std::path::Path, stamm: &str) -> Option<Eintrag> {
    let inhalt = std::fs::read_to_string(pfad).ok()?;
    let mut im_entry = false;
    let (mut name, mut name_de, mut exec, mut typ) = (None, None, None, None);
    let (mut versteckt, mut terminal) = (false, false);
    for zeile in inhalt.lines() {
        let z = zeile.trim();
        if z.starts_with('[') {
            im_entry = z == "[Desktop Entry]";
            continue;
        }
        if !im_entry {
            continue;
        }
        let Some((k, v)) = z.split_once('=') else { continue };
        match k.trim() {
            "Name" => name = Some(v.trim().to_string()),
            "Name[de]" => name_de = Some(v.trim().to_string()),
            "Exec" => exec = Some(v.trim().to_string()),
            "Type" => typ = Some(v.trim().to_string()),
            "NoDisplay" | "Hidden" if v.trim() == "true" => versteckt = true,
            "Terminal" if v.trim() == "true" => terminal = true,
            _ => {}
        }
    }
    if versteckt || typ.as_deref() != Some("Application") {
        return None;
    }
    Some(Eintrag {
        app_id: stamm.to_string(),
        name: name_de.or(name)?,
        exec: exec?,
        terminal,
    })
}

fn exec_saeubern(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|w| !w.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ")
}

fn app_starten(e: &Eintrag) {
    let befehl = exec_saeubern(&e.exec);
    let _ = if e.terminal {
        std::process::Command::new("foot").args(["-e", "sh", "-c", &befehl]).spawn()
    } else {
        std::process::Command::new("sh").args(["-c", &befehl]).spawn()
    };
}

/// Wortanfang schlägt Substring, Namensanfang schlägt alles.
fn punkte(name: &str, suche: &str) -> Option<u32> {
    let n = name.to_lowercase();
    let s = suche.to_lowercase();
    if s.is_empty() {
        return Some(1);
    }
    if n.starts_with(&s) {
        return Some(100);
    }
    if n.split_whitespace().any(|w| w.starts_with(&s)) {
        return Some(50);
    }
    n.contains(&s).then_some(10)
}

// ----------------------------------------------------------- Offene Fenster

#[derive(Debug, Clone)]
struct FensterInfo {
    id: u64,
    name: String,  // App-freundlicher Name (aus dem Index) oder app_id
    titel: String,
}

/// Offene Fenster von niri holen (JSON). Leere Liste, wenn niri fehlt.
fn fenster_holen(apps: &[Eintrag]) -> Vec<FensterInfo> {
    let Some(aus) = mk::leinwand::roh(&["msg", "-j", "windows"]) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&aus.stdout) else {
        return Vec::new();
    };
    let Some(arr) = v.as_array() else { return Vec::new() };
    arr.iter()
        .filter_map(|w| {
            let id = w["id"].as_u64()?;
            let app_id = w["app_id"].as_str().unwrap_or("");
            let titel = w["title"].as_str().unwrap_or("").to_string();
            // Freundlicher Name: den App-Index nach der app_id fragen.
            let name = apps
                .iter()
                .find(|e| e.app_id == app_id)
                .map(|e| e.name.clone())
                .unwrap_or_else(|| {
                    if app_id.is_empty() { titel.clone() } else { app_id.to_string() }
                });
            Some(FensterInfo { id, name, titel })
        })
        .collect()
}

// ----------------------------------------------------------- Rechner

/// Winziger Ausdruck-Rechner (+ - * / x, Klammern, Dezimal). None, wenn die
/// Eingabe nicht wie eine vollständige Rechnung aussieht (dann kein Treffer).
fn rechnen(eingabe: &str) -> Option<String> {
    let s = eingabe.trim();
    // Muss eine Ziffer UND einen Operator enthalten — sonst ist es Text.
    if !s.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if !s.contains(['+', '-', '*', '/', 'x', '×', '·']) {
        return None;
    }
    let zeichen: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
    let mut p = Parser { z: &zeichen, i: 0 };
    let wert = p.ausdruck()?;
    if p.i != zeichen.len() {
        return None; // Rest übrig = kein sauberer Ausdruck
    }
    if !wert.is_finite() {
        return None;
    }
    // Hübsch formatieren: ganze Zahlen ohne Komma, sonst bis 6 Stellen.
    let g = if (wert.fract()).abs() < 1e-9 {
        format!("{}", wert.round() as i64)
    } else {
        let t = format!("{wert:.6}");
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    Some(g)
}

struct Parser<'a> {
    z: &'a [char],
    i: usize,
}
impl Parser<'_> {
    fn ausdruck(&mut self) -> Option<f64> {
        let mut a = self.term()?;
        while let Some(&c) = self.z.get(self.i) {
            if c == '+' || c == '-' {
                self.i += 1;
                let b = self.term()?;
                a = if c == '+' { a + b } else { a - b };
            } else {
                break;
            }
        }
        Some(a)
    }
    fn term(&mut self) -> Option<f64> {
        let mut a = self.faktor()?;
        while let Some(&c) = self.z.get(self.i) {
            if c == '*' || c == 'x' || c == '×' || c == '·' {
                self.i += 1;
                a *= self.faktor()?;
            } else if c == '/' {
                self.i += 1;
                let b = self.faktor()?;
                if b == 0.0 {
                    return None;
                }
                a /= b;
            } else {
                break;
            }
        }
        Some(a)
    }
    fn faktor(&mut self) -> Option<f64> {
        match self.z.get(self.i) {
            Some('(') => {
                self.i += 1;
                let v = self.ausdruck()?;
                if self.z.get(self.i) == Some(&')') {
                    self.i += 1;
                    Some(v)
                } else {
                    None
                }
            }
            Some('-') => {
                self.i += 1;
                Some(-self.faktor()?)
            }
            Some(c) if c.is_ascii_digit() || *c == '.' => {
                let start = self.i;
                while matches!(self.z.get(self.i), Some(d) if d.is_ascii_digit() || *d == '.') {
                    self.i += 1;
                }
                let s: String = self.z[start..self.i].iter().collect();
                s.parse::<f64>().ok()
            }
            _ => None,
        }
    }
}

// ----------------------------------------------------------- Treffer

/// Was ein Treffer beim Auslösen tut.
#[derive(Debug, Clone)]
enum Ziel {
    App(usize),
    Fenster(u64),
    Kommando { cmd: String, terminal: bool },
    Erscheinung,
    Abmelden,
    Kopieren(String),
}

#[derive(Debug, Clone)]
enum Ikone {
    App(String),
    Symbol(char),
}

#[derive(Debug, Clone)]
struct Treffer {
    name: String,
    kategorie: &'static str,
    ikone: Ikone,
    punkte: u32,
    ziel: Ziel,
}

/// Die festen System-Aktionen (Name, Symbol, Ziel).
fn aktionen() -> Vec<(&'static str, char, Ziel)> {
    vec![
        ("Sperren", mkw::symbol::LOCK, Ziel::Kommando { cmd: "loginctl lock-session".into(), terminal: false }),
        ("Erscheinung umschalten", mkw::symbol::DARK_MODE, Ziel::Erscheinung),
        ("Einstellungen", mkw::symbol::TUNE, Ziel::Kommando { cmd: "matrix-einstellungen".into(), terminal: false }),
        ("Abmelden", mkw::symbol::LOGOUT, Ziel::Abmelden),
        ("Neustarten", mkw::symbol::RESTART, Ziel::Kommando { cmd: "systemctl reboot".into(), terminal: false }),
        ("Ausschalten", mkw::symbol::POWER, Ziel::Kommando { cmd: "systemctl poweroff".into(), terminal: false }),
    ]
}

fn ausloesen(z: &Ziel, apps: &[Eintrag]) {
    match z {
        Ziel::App(i) => {
            if let Some(e) = apps.get(*i) {
                app_starten(e);
            }
        }
        Ziel::Fenster(id) => {
            // Fokus faehrt (im Leinwand-Modus) auch die Kamera hin.
            mk::leinwand::fenster_fokussieren(*id);
        }
        Ziel::Kommando { cmd, terminal } => {
            let _ = if *terminal {
                std::process::Command::new("foot").args(["-e", "sh", "-c", cmd]).spawn()
            } else {
                std::process::Command::new("sh").args(["-c", cmd]).spawn()
            };
        }
        Ziel::Erscheinung => mk::hell_umschalten(),
        Ziel::Abmelden => mk::leinwand::abmelden(false),
        Ziel::Kopieren(s) => {
            use std::io::Write;
            if let Ok(mut kind) = std::process::Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                if let Some(mut ein) = kind.stdin.take() {
                    let _ = ein.write_all(s.as_bytes());
                }
                let _ = kind.wait();
            }
        }
    }
    std::process::exit(0);
}

// ----------------------------------------------------------------- App

#[to_layer_message]
#[derive(Debug, Clone)]
enum Msg {
    Suche(String),
    Leeren,
    Taste(mkw::Taste),
    Klick(usize),
}

struct App {
    palette: mk::Palette,
    apps: Vec<Eintrag>,
    fenster: Vec<FensterInfo>,
    suche: String,
    treffer: Vec<Treffer>,
    gewaehlt: usize,
    icons: std::collections::HashMap<String, Option<image::Handle>>,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let apps = index_bauen();
        let fenster = fenster_holen(&apps);
        let mut app = App {
            palette: mk::Palette::load().unwrap_or_default(),
            apps,
            fenster,
            suche: String::new(),
            treffer: Vec::new(),
            gewaehlt: 0,
            icons: std::collections::HashMap::new(),
        };
        app.filtern();
        (app, mkw::suche_fokussieren())
    }

    fn filtern(&mut self) {
        let s = self.suche.trim();
        let leer = s.is_empty();
        let mut kand: Vec<Treffer> = Vec::new();

        // 1) Rechner — nur wenn die Eingabe wie Mathe aussieht.
        if let Some(erg) = rechnen(s) {
            kand.push(Treffer {
                name: format!("= {erg}"),
                kategorie: "Rechnen",
                ikone: Ikone::Symbol(mkw::symbol::CODE),
                punkte: 100_000,
                ziel: Ziel::Kopieren(erg),
            });
        }

        // 2) Offene Fenster (bei leerer Suche zuerst — schnelle Übersicht).
        for f in &self.fenster {
            let p_name = punkte(&f.name, s);
            let p_titel = punkte(&f.titel, s);
            let Some(p) = p_name.into_iter().chain(p_titel).max() else { continue };
            let anzeige = if f.titel.is_empty() || f.titel == f.name {
                f.name.clone()
            } else {
                format!("{} — {}", f.name, f.titel)
            };
            // Fenster leicht bevorzugen (springen ist der häufigste Wunsch).
            let bonus = if leer { 4 } else { 15 };
            kand.push(Treffer {
                name: anzeige,
                kategorie: "Fenster",
                ikone: Ikone::App(
                    self.apps.iter().find(|e| e.name == f.name).map(|e| e.app_id.clone())
                        .unwrap_or_default(),
                ),
                punkte: p + bonus,
                ziel: Ziel::Fenster(f.id),
            });
        }

        // 3) Apps.
        for (i, e) in self.apps.iter().enumerate() {
            if let Some(p) = punkte(&e.name, s) {
                kand.push(Treffer {
                    name: e.name.clone(),
                    kategorie: "App",
                    ikone: Ikone::App(e.app_id.clone()),
                    punkte: p,
                    ziel: Ziel::App(i),
                });
            }
        }

        // 4) System-Aktionen — nur bei Eingabe (nicht die leere Palette zumüllen).
        if !leer {
            for (name, sym, ziel) in aktionen() {
                if let Some(p) = punkte(name, s) {
                    kand.push(Treffer {
                        name: name.to_string(),
                        kategorie: "Aktion",
                        ikone: Ikone::Symbol(sym),
                        punkte: p + 5,
                        ziel,
                    });
                }
            }
        }

        kand.sort_by(|a, b| b.punkte.cmp(&a.punkte).then(a.name.cmp(&b.name)));
        kand.truncate(TREFFER);
        self.treffer = kand;
        self.gewaehlt = 0;

        // Icons der sichtbaren App-/Fenster-Treffer backen.
        let p = self.palette;
        for t in &self.treffer {
            if let Ikone::App(id) = &t.ikone {
                if id.is_empty() {
                    continue;
                }
                self.icons.entry(id.clone()).or_insert_with(|| {
                    matrixkit_icons::render_png(id, &p).map(image::Handle::from_bytes)
                });
            }
        }
    }

    fn ausloesen_gewaehlt(&self) {
        if let Some(t) = self.treffer.get(self.gewaehlt) {
            ausloesen(&t.ziel, &self.apps);
        }
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Suche(s) => {
                self.suche = s;
                self.filtern();
            }
            Msg::Leeren => {
                self.suche.clear();
                self.filtern();
            }
            Msg::Klick(i) => {
                if let Some(t) = self.treffer.get(i) {
                    ausloesen(&t.ziel, &self.apps);
                }
            }
            Msg::Taste(t) => match t {
                mkw::Taste::Escape => std::process::exit(0),
                mkw::Taste::Weiter => {
                    if !self.treffer.is_empty() {
                        self.gewaehlt = (self.gewaehlt + 1) % self.treffer.len();
                    }
                }
                mkw::Taste::Zurueck => {
                    if !self.treffer.is_empty() {
                        self.gewaehlt =
                            (self.gewaehlt + self.treffer.len() - 1) % self.treffer.len();
                    }
                }
                mkw::Taste::Aktivieren => self.ausloesen_gewaehlt(),
                _ => {}
            },
            _ => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        mkw::tasten_abo(Msg::Taste)
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.palette;

        let mut inhalt = column![].spacing(mk::spacing::S);
        inhalt = inhalt.push(
            container(mkw::suchfeld(
                &self.suche,
                "Suchen, springen oder rechnen …",
                Msg::Suche,
                Msg::Leeren,
                p,
            ))
            .padding(mk::spacing::S),
        );

        for (reihe, t) in self.treffer.iter().enumerate() {
            let bild: Element<'_, Msg> = match &t.ikone {
                Ikone::App(id) => match self.icons.get(id).cloned().flatten() {
                    Some(h) => image(h).width(ICON).height(ICON).into(),
                    None => platzhalter_icon(&t.name, p),
                },
                Ikone::Symbol(sym) => container(mkw::symbol::<Msg>(*sym, ICON * 0.62, p.on_surface_variant))
                    .center_x(Length::Fixed(ICON))
                    .center_y(Length::Fixed(ICON))
                    .into(),
            };

            let aktiv = reihe == self.gewaehlt;
            let zeile = mouse_area(
                container(
                    row![
                        bild,
                        mkw::txt(&t.name, mk::typo::FLIESS, p.on_surface),
                        Space::new().width(Length::Fill),
                        mkw::txt(t.kategorie, mk::typo::KLEIN, p.on_surface_variant),
                    ]
                    .spacing(mk::spacing::M)
                    .align_y(iced::Alignment::Center),
                )
                .padding([mk::spacing::XS as u16, mk::spacing::S as u16])
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: aktiv.then(|| {
                        color(p.on_surface.over(p.surface_container, mkw::leiste::HOVER)).into()
                    }),
                    border: iced::border::rounded(mk::radius::NORMAL),
                    ..Default::default()
                }),
            )
            .on_press(Msg::Klick(reihe));
            inhalt = inhalt.push(zeile);
        }

        if self.treffer.is_empty() {
            inhalt = inhalt.push(
                container(mkw::txt("Nichts gefunden", mk::typo::HINWEIS, p.on_surface_variant))
                    .center_x(Length::Fill)
                    .padding(mk::spacing::L),
            );
        }

        container(
            container(inhalt.padding(mk::spacing::S))
                .width(Length::Fixed(BREITE as f32 - 2.0 * mk::spacing::M))
                .style(move |_| mkw::leiste::pille(p, mk::radius::NORMAL, mk::spacing::S)),
        )
        .center_x(Length::Fill)
        .into()
    }
}

fn platzhalter_icon<'a>(name: &str, p: mk::Palette) -> Element<'a, Msg> {
    container(mkw::txt(
        name.chars().next().unwrap_or('?').to_uppercase().to_string(),
        mk::typo::KOPF,
        p.on_primary_container,
    ))
    .center_x(Length::Fixed(ICON))
    .center_y(Length::Fixed(ICON))
    .style(move |_| container::Style {
        background: Some(color(p.primary_container).into()),
        border: iced::border::rounded(mk::radius::KLEIN),
        ..Default::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoring_ordnet_richtig() {
        assert!(punkte("Firefox", "fir").unwrap() > punkte("Fcitx Firefox-Helfer", "fir").unwrap());
        assert_eq!(punkte("Bazaar", "xyz"), None);
        assert_eq!(punkte("Egal", ""), Some(1));
    }

    #[test]
    fn exec_codes_fallen_raus() {
        assert_eq!(exec_saeubern("firefox %u"), "firefox");
        assert_eq!(exec_saeubern("app --flag %U %f"), "app --flag");
    }

    #[test]
    fn rechner_rechnet_und_lehnt_text_ab() {
        assert_eq!(rechnen("12*8+4").as_deref(), Some("100"));
        assert_eq!(rechnen("(2+3)*4").as_deref(), Some("20"));
        assert_eq!(rechnen("10/4").as_deref(), Some("2.5"));
        assert_eq!(rechnen("7x6").as_deref(), Some("42"));
        assert_eq!(rechnen("firefox"), None); // Text, keine Ziffer
        assert_eq!(rechnen("42"), None); // Zahl ohne Operator = kein Treffer
        assert_eq!(rechnen("5/0"), None); // durch null
    }
}
