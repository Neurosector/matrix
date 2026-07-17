//! Matrix Hintergrund — App #21, der Wallpaper-Dienst.
//!
//! Ersetzt die letzte große DMS-Rolle: malt das Hintergrundbild auf den
//! Background-Layer UND erzeugt beim Bildwechsel die System-Palette
//! (matugen mit unserem matrix-palette-Template) — die Farbkette gehört
//! damit vollständig Matrix. Hell/Dunkel folgt dem Sonnenstand-Slot.
//!
//! Bild-Quellen (erste gewinnt):
//!   ~/.config/matrix/hintergrund-hell|dunkel (eigene Kultur)
//!   session.json wallpaperPath (DMS-Erbe)
//!   /usr/share/backgrounds/matrix/matrix-standard.jpg (Falke)
//!
//! Surface-Wache: Beim Session-Start (spawn-at-startup, sleep 3) verliert
//! der Dienst das Rennen gegen die Output-Initialisierung — die Layer-
//! Surface bleibt aus oder hängt bei 0x0, der Hintergrund bleibt schwarz.
//! Der 2-s-Puls prüft darum die Surface-Lage: 0x0 → SizeChange-Nudge
//! (set_size+commit erzwingt frische Configure); gar keine Surface →
//! nach WACHE_LIMIT Fehlpulsen exec-Neustart, denn das Single-Window-
//! Pattern kann keine neue Surface anlegen (NewLayerShell gibt es nur
//! im multi-Makro, AllScreens verbietet application() per assert).

use iced::widget::{image, Space};
use iced::{Color, Element, Length, Subscription, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets::tick;

fn main() -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};
    eprintln!("[mh] start pid={}", std::process::id());
    if !matrixkit_widgets::leiste_toggle() {
        eprintln!("[mh] toggle: andere Instanz — Ende");
        return Ok(());
    }
    eprintln!("[mh] toggle ok, layershell startet");
    iced_layershell::application(
        App::new,
        || String::from("matrix-hintergrun"),
        App::update,
        App::view,
    )
    .subscription(App::subscription)
    .style(|_s, _t| iced::theme::Style {
        background_color: Color::BLACK,
        text_color: Color::WHITE,
    })
    .settings(Settings {
        layer_settings: LayerShellSettings {
            size: Some((0, 0)),
            anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
            layer: Layer::Background,
            keyboard_interactivity: KeyboardInteractivity::None,
            exclusive_zone: -1,
            ..Default::default()
        },
        ..Default::default()
    })
    .run()
    .inspect(|_| eprintln!("[mh] run() endete SAUBER (Surface zu?)"))
    .inspect_err(|e| eprintln!("[mh] run() Fehler: {e}"))
}

fn ist_hell() -> bool {
    std::fs::read_to_string(mk::session_path())
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("isLightMode").and_then(|b| b.as_bool()))
        .unwrap_or(false)
}

/// Der aktuell gewünschte Bildpfad (Quellen-Kaskade).
fn bild_pfad() -> Option<String> {
    let heim = std::env::var("HOME").ok()?;
    let eigene = format!(
        "{heim}/.config/matrix/hintergrund-{}",
        if ist_hell() { "hell" } else { "dunkel" }
    );
    if let Ok(p) = std::fs::read_to_string(&eigene) {
        let p = p.trim().to_string();
        if !p.is_empty() && std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    if let Ok(raw) = std::fs::read_to_string(mk::session_path()) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(p) = v.get("wallpaperPath").and_then(|s| s.as_str()) {
                if !p.is_empty() && std::path::Path::new(p).exists() {
                    return Some(p.to_string());
                }
            }
        }
    }
    let falke = "/usr/share/backgrounds/matrix/matrix-standard.jpg";
    std::path::Path::new(falke).exists().then(|| falke.to_string())
}

/// Palette aus dem Bild erzeugen — matugen mit unseren Templates
/// (App-Palette + niri-Rahmenfarben), danach die Firefox-Brücke.
/// `-m` folgt dem Modus, damit {{..default.hex}} im niri-Template
/// die richtige Seite trifft.
fn palette_erzeugen(pfad: &str) {
    let pfad = pfad.to_string();
    let modus = if ist_hell() { "light" } else { "dark" };
    std::thread::spawn(move || {
        let _ = std::process::Command::new("matugen")
            .args([
                "image", &pfad,
                "-t", "scheme-tonal-spot",
                "--source-color-index", "0",
                "-m", modus,
            ])
            .status();
        // Firefox folgt (Pywalfox liest die matugen-wal-Farben).
        let _ = std::process::Command::new("pywalfox").arg("update").status();
    });
}

/// Steht die Sonne über dem Horizont? NOAA-Kurzform: Tag des Jahres →
/// Deklination + Zeitgleichung → Auf-/Untergangs-Minute (UTC) → mit
/// der lokalen UTC-Abweichung verglichen. Genau genug fürs Umschalten.
fn sonne_oben() -> Option<bool> {
    let ort = mk::einstellung::lesen("ort").unwrap_or_else(|| String::from("47.38,8.54"));
    let mut teile = ort.split(',');
    let lat: f64 = teile.next()?.trim().parse().ok()?;
    let lon: f64 = teile.next()?.trim().parse().ok()?;

    // Tag des Jahres + aktuelle UTC-Minuten + lokaler Offset via date.
    let tag: f64 = mk::befehl::erste_zeile("date", &["-u", "+%j"])?.parse().ok()?;
    let utc_min: f64 = {
        let hm = mk::befehl::erste_zeile("date", &["-u", "+%H %M"])?;
        let mut t = hm.split_whitespace();
        let h: f64 = t.next()?.parse().ok()?;
        let m: f64 = t.next()?.parse().ok()?;
        h * 60.0 + m
    };

    let rad = std::f64::consts::PI / 180.0;
    let gamma = 2.0 * std::f64::consts::PI / 365.0 * (tag - 1.0);
    // Zeitgleichung (Minuten) und Sonnendeklination (rad) — NOAA-Reihen.
    let eqtime = 229.18
        * (0.000075 + 0.001868 * gamma.cos()
            - 0.032077 * gamma.sin()
            - 0.014615 * (2.0 * gamma).cos()
            - 0.040849 * (2.0 * gamma).sin());
    let decl = 0.006918 - 0.399912 * gamma.cos() + 0.070257 * gamma.sin()
        - 0.006758 * (2.0 * gamma).cos()
        + 0.000907 * (2.0 * gamma).sin()
        - 0.002697 * (3.0 * gamma).cos()
        + 0.00148 * (3.0 * gamma).sin();

    // Stundenwinkel für Sonnenauf-/-untergang (Zenit 90.833°).
    let cos_ha = ((90.833 * rad).cos() - (lat * rad).sin() * decl.sin())
        / ((lat * rad).cos() * decl.cos());
    if !(-1.0..=1.0).contains(&cos_ha) {
        // Polartag/-nacht: Vorzeichen entscheidet.
        return Some(cos_ha < -1.0);
    }
    let ha = cos_ha.acos() / rad; // Grad
    let aufgang = 720.0 - 4.0 * (lon + ha) - eqtime; // Minuten UTC
    let untergang = 720.0 - 4.0 * (lon - ha) - eqtime;
    Some(utc_min >= aufgang && utc_min <= untergang)
}

/// Nach so vielen Pulsen ohne gesunde Surface (~10 s) wird neu gestartet.
const WACHE_LIMIT: u8 = 5;

/// Selbst-Neustart per exec: gleiche PID — leiste_toggle sieht sich nicht
/// selbst als „andere Instanz" —, frische Wayland-Verbindung, frische
/// Layer-Surface. Auflösung wie leiste::app_starten: Dev-Slot vor System.
fn selbst_neustart() -> ! {
    use std::os::unix::process::CommandExt;
    let lokal = std::env::var("HOME")
        .map(|h| format!("{h}/.local/bin/matrix-hintergrund"))
        .unwrap_or_default();
    let programm = if std::path::Path::new(&lokal).exists() {
        lokal
    } else {
        String::from("matrix-hintergrund")
    };
    let fehler = std::process::Command::new(programm).exec();
    eprintln!("[mh] Wache: exec-Neustart fehlgeschlagen: {fehler}");
    std::process::exit(1);
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Msg {
    Puls,
    /// Antwort der Surface-Wache: Größe der jüngsten Surface,
    /// None wenn gar kein Fenster (mehr) existiert.
    SurfaceLage(Option<iced::Size>),
}

struct App {
    pfad: Option<String>,
    bild: Option<image::Handle>,
    /// Pulse in Folge ohne gesunde Surface (Wache).
    fehlpulse: u8,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let pfad = bild_pfad();
        let bild = pfad.as_deref().map(image::Handle::from_path);
        (App { pfad, bild, fehlpulse: 0 }, Task::none())
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Puls => {
                // ECHTER Sonnenstand (Runde 23, NSAppearance-Auto-Extrakt):
                // NOAA-Näherung aus Ort (~/.config/matrix/ort = "lat,lon",
                // Default Schweiz) — hell zwischen Aufgang und Untergang.
                if mk::einstellung::lesen("hintergrund-modus").as_deref() == Some("auto") {
                    if let Some(soll_hell) = sonne_oben() {
                        if soll_hell != ist_hell() {
                            mk::hell_umschalten();
                        }
                    }
                }
                let neu = bild_pfad();
                if neu != self.pfad {
                    self.pfad = neu;
                    self.bild = self.pfad.as_deref().map(image::Handle::from_path);
                    if let Some(p) = &self.pfad {
                        // Bildwechsel = Palettenwechsel: die Farbkette lebt.
                        palette_erzeugen(p);
                    }
                }
                // Surface-Wache: Lage abfragen. Läuft auch ohne Fenster —
                // latest() antwortet dann mit None.
                iced::window::latest().then(|id| match id {
                    Some(id) => iced::window::size(id).map(|g| Msg::SurfaceLage(Some(g))),
                    None => Task::done(Msg::SurfaceLage(None)),
                })
            }
            Msg::SurfaceLage(groesse) => {
                if groesse.is_some_and(|g| g.width >= 1.0 && g.height >= 1.0) {
                    self.fehlpulse = 0;
                    return Task::none();
                }
                self.fehlpulse += 1;
                eprintln!(
                    "[mh] Wache: Surface {:?} — Fehlpuls {}/{}",
                    groesse, self.fehlpulse, WACHE_LIMIT
                );
                if self.fehlpulse >= WACHE_LIMIT {
                    eprintln!("[mh] Wache: Surface bleibt aus — exec-Neustart");
                    selbst_neustart();
                }
                if groesse.is_some() {
                    // 0x0 konfiguriert: Nudge — die Runtime setzt die
                    // Größe neu und committet, der Compositor antwortet
                    // mit einer frischen Configure.
                    return Task::done(Msg::SizeChange((0, 0)));
                }
                Task::none()
            }
            // vom to_layer_message-Makro ergänzte Varianten
            _ => Task::none(),
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        tick("hintergrund", std::time::Duration::from_secs(2)).map(|_| Msg::Puls)
    }

    fn view(&self) -> Element<'_, Msg> {
        match &self.bild {
            Some(h) => image(h.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Cover)
                .into(),
            None => Space::new().into(),
        }
    }
}
