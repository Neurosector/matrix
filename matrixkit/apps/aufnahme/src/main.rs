//! Matrix Aufnahme — App #29 (R69): Bildschirmfoto und Bildschirmfilm
//! in der Leitbild-Grammatik, empirisch vom Referenzsystem abgelesen.
//!
//! Das Aufnahme-Panel: EIN kleines Panel bündelt alle Aufnahme-
//! Arten (ganzer Schirm, Bereich, Fenster, Film). Nach jedem Foto
//! schwebt unten rechts das THUMBNAIL (~6 s, Leitbild-Verhalten): Klick
//! öffnet, ✕ verwirft die Vorschau — die Datei ist längst gesichert.
//! Dateinamen sprechen Leitbild-Deutsch: „Bildschirmfoto 2026-07-17 um
//! 00.31.12.png". Läuft ein Film, zeigt die BAR den roten Stopp-Punkt
//! (Leitbild- Menüleisten-Verhalten) — Klick beendet.
//!
//! Backend: die Foto-Wege gehen über die Leinwand-Fassade
//! (screenshot/-screen/-window, inkl. niris eingebauter Bereichswahl);
//! der Film über wf-recorder (screencopy — der Fork spricht es; das
//! Werkzeug kommt mit dem Image, Runde 69).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::{column, container, image, row};
use iced::{Color, Element, Font, Length, Subscription, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use matrixkit_widgets::color;

/// Schwebe-Dauer des Thumbnails — Leitbild zeigt es ~6 s.
const THUMB_SICHTBAR_S: u64 = 6;
const THUMB_B: f32 = 236.0;
const THUMB_H: f32 = 156.0;

/// XDG-Benutzerordner aus `user-dirs.dirs` — auf Matrix deutsch
/// (PICTURES → ~/Bilder), mit Fallback auf den deutschen Namen.
fn xdg_ordner(schluessel: &str, fallback: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    let konf = PathBuf::from(&home).join(".config/user-dirs.dirs");
    if let Ok(text) = std::fs::read_to_string(konf) {
        let praefix = format!("XDG_{schluessel}_DIR=\"");
        for zeile in text.lines() {
            if let Some(rest) = zeile.trim().strip_prefix(&praefix) {
                let wert = rest.trim_end_matches('"').replace("$HOME", &home);
                return PathBuf::from(wert);
            }
        }
    }
    PathBuf::from(home).join(fallback)
}

/// Fotos landen in „Bilder/Bildschirmfotos" — dort schaut die Galerie
/// von Matrix Dateien hin. MUSS zum screenshot-path des Compositors
/// passen (die Warteschleife zählt Dateien in diesem Ordner).
fn schirm_ordner() -> PathBuf {
    xdg_ordner("PICTURES", "Bilder").join("Bildschirmfotos")
}

/// Filme landen in „Videos/Bildschirmaufnahmen" — die zweite
/// Galerie-Wurzel von Matrix Dateien.
fn film_ordner() -> PathBuf {
    xdg_ordner("VIDEOS", "Videos").join("Bildschirmaufnahmen")
}

/// Leitbild-Deutsch: „Bildschirmfoto 2026-07-17 um 00.31.12.png".
fn zeitstempel_name(prefix: &str, endung: &str) -> String {
    let jetzt = std::process::Command::new("date")
        .arg("+%Y-%m-%d um %H.%M.%S")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    format!("{prefix} {jetzt}.{endung}")
}

fn film_status_datei() -> PathBuf {
    let lauf = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(lauf).join("matrix-aufnahme-film")
}

fn main() -> Result<(), iced_layershell::Error> {
    let arg = std::env::args().nth(1).unwrap_or_else(|| String::from("panel"));
    match arg.as_str() {
        "foto" => {
            aufnehmen("screenshot-screen");
            Ok(())
        }
        "bereich" => {
            // niris eingebaute Bereichswahl — der Nutzer zieht den Rahmen.
            aufnehmen("screenshot");
            Ok(())
        }
        "fenster" => {
            aufnehmen("screenshot-window");
            Ok(())
        }
        "film" => {
            film_start();
            Ok(())
        }
        "film-stopp" => {
            film_stopp();
            Ok(())
        }
        "thumb" => {
            let pfad = std::env::args().nth(2).unwrap_or_default();
            thumb_ui(PathBuf::from(pfad))
        }
        _ => panel_ui(),
    }
}

// ------------------------------------------------------------ Aufnahme

/// Foto machen: Aktion auslösen, auf die NEUE Datei warten (die
/// Speicher-Doktrin: zählen statt glauben), Leitbild-Namen geben,
/// Thumbnail zeigen. Bereichswahl darf sich Zeit lassen (60 s).
fn aufnehmen(aktion: &str) {
    let ordner = schirm_ordner();
    let _ = std::fs::create_dir_all(&ordner);
    let vorher: usize = std::fs::read_dir(&ordner).map(|d| d.count()).unwrap_or(0);
    if !mk::leinwand::aktion(&["msg", "action", aktion]) {
        eprintln!("[ma] Leinwand-Aktion {aktion} fehlgeschlagen");
        return;
    }
    let frist = Instant::now();
    let neueste = loop {
        std::thread::sleep(Duration::from_millis(400));
        let jetzt: usize = std::fs::read_dir(&ordner).map(|d| d.count()).unwrap_or(0);
        if jetzt > vorher {
            // jüngste Datei im Ordner
            let mut dateien: Vec<_> = std::fs::read_dir(&ordner)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            dateien.sort_by_key(|p| {
                std::fs::metadata(p)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH)
            });
            break dateien.pop();
        }
        // Bereichswahl kann dauern; Esc in der Wahl = nie eine Datei.
        if frist.elapsed() > Duration::from_secs(60) {
            break None;
        }
    };
    let Some(roh) = neueste else { return };
    let ziel = ordner.join(zeitstempel_name("Bildschirmfoto", "png"));
    let pfad = if std::fs::rename(&roh, &ziel).is_ok() { ziel } else { roh };
    // Vorschau schweben lassen — eigener Prozess, wir sind fertig.
    let _ = std::process::Command::new(std::env::current_exe().unwrap_or_default())
        .arg("thumb")
        .arg(&pfad)
        .spawn();
}

fn film_start() {
    if film_status_datei().exists() {
        return; // läuft schon
    }
    let ordner = film_ordner();
    let _ = std::fs::create_dir_all(&ordner);
    let ziel = ordner.join(zeitstempel_name("Bildschirmaufnahme", "mp4"));
    let Ok(kind) = std::process::Command::new("wf-recorder")
        .arg("-f")
        .arg(&ziel)
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        eprintln!("[ma] wf-recorder fehlt — Film kommt mit dem nächsten Image");
        return;
    };
    // Status: PID + Pfad + Startzeit — die Bar liest ihn für den Stopp-Punkt.
    let start = std::process::Command::new("date")
        .arg("+%s")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let _ = std::fs::write(
        film_status_datei(),
        format!("{}\n{}\n{start}\n", kind.id(), ziel.display()),
    );
}

fn film_stopp() {
    let Ok(inhalt) = std::fs::read_to_string(film_status_datei()) else { return };
    let mut zeilen = inhalt.lines();
    let pid = zeilen.next().unwrap_or_default().to_string();
    let pfad = PathBuf::from(zeilen.next().unwrap_or_default());
    // SIGINT lässt wf-recorder die Datei sauber abschließen.
    let _ = std::process::Command::new("kill")
        .args(["-INT", &pid])
        .status();
    let _ = std::fs::remove_file(film_status_datei());
    std::thread::sleep(Duration::from_millis(600));
    if pfad.exists() {
        let _ = std::process::Command::new(std::env::current_exe().unwrap_or_default())
            .arg("thumb")
            .arg(&pfad)
            .spawn();
    }
}

// ------------------------------------------------------------ Das Panel

/// Das Aufnahme-Panel: schwebt über dem Dock, eine Aktion pro Punkt.
fn panel_ui() -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};
    let rand = mkw::leiste::SCHATTEN_RAND as u32;
    iced_layershell::application(
        Panel::new,
        || String::from("matrix-aufnahme"),
        Panel::update,
        Panel::view,
    )
    .style(|_s, _t| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .settings(Settings {
        layer_settings: LayerShellSettings {
            size: Some((mkw::ui::MENU_BREITE as u32 + 2 * rand, 300 + 2 * rand)),
            anchor: Anchor::Bottom,
            margin: (0, 0, 140 - mkw::leiste::SCHATTEN_RAND as i32, 0),
            layer: Layer::Overlay,
            keyboard_interactivity: KeyboardInteractivity::None,
            // Wie die Dock-Menüs (matrix-kontext): Zonen ignorieren — der
            // Abstand zählt ab Schirmkante, nicht ab Dock-Oberkante.
            exclusive_zone: -1,
            ..Default::default()
        },
        default_font: Font::with_name("Inter Variable"),
        fonts: mkw::symbol_font_laden().into_iter().collect(),
        ..Default::default()
    })
    .run()
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum PanelMsg {
    Aktion(&'static str),
    Zu,
}

struct Panel {
    palette: mk::Palette,
    film_laeuft: bool,
}

impl Panel {
    fn new() -> (Self, Task<PanelMsg>) {
        (
            Panel {
                palette: mk::Palette::load().unwrap_or_default(),
                film_laeuft: film_status_datei().exists(),
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: PanelMsg) -> Task<PanelMsg> {
        match msg {
            PanelMsg::Aktion(modus) => {
                let _ = std::process::Command::new(
                    std::env::current_exe().unwrap_or_default(),
                )
                .arg(modus)
                .spawn();
                std::process::exit(0);
            }
            PanelMsg::Zu => std::process::exit(0),
            _ => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, PanelMsg> {
        use mkw::ui::MenuEintrag as E;
        let p = self.palette;
        let mut eintraege = vec![
            E::Punkt {
                zeichen: Some(mkw::symbol::IMAGE),
                titel: String::from("Ganzen Bildschirm fotografieren"),
                farbe: None,
                msg: PanelMsg::Aktion("foto"),
            },
            E::Punkt {
                zeichen: Some(mkw::symbol::SEARCH),
                titel: String::from("Bereich auswählen …"),
                farbe: None,
                msg: PanelMsg::Aktion("bereich"),
            },
            E::Punkt {
                zeichen: Some(mkw::symbol::APPS),
                titel: String::from("Fenster fotografieren"),
                farbe: None,
                msg: PanelMsg::Aktion("fenster"),
            },
            E::Trenner,
        ];
        if self.film_laeuft {
            eintraege.push(E::Punkt {
                zeichen: Some(mkw::symbol::CLOSE),
                titel: String::from("Film beenden"),
                farbe: Some(p.error),
                msg: PanelMsg::Aktion("film-stopp"),
            });
        } else {
            eintraege.push(E::Punkt {
                zeichen: Some(mkw::symbol::PLAY_ARROW),
                titel: String::from("Bildschirm filmen"),
                farbe: None,
                msg: PanelMsg::Aktion("film"),
            });
        }
        eintraege.push(E::Trenner);
        eintraege.push(E::Punkt {
            zeichen: None,
            titel: String::from("Abbrechen"),
            farbe: None,
            msg: PanelMsg::Zu,
        });
        let karte = mkw::ui::menu_family(
            Some(mkw::txt("Aufnahme", mk::typo::KOPF, p.on_surface).into()),
            eintraege,
            p,
        );
        container(karte)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(mkw::leiste::SCHATTEN_RAND)
            .into()
    }
}

// -------------------------------------------------------- Das Thumbnail

/// Das schwebende Vorschaubild (Leitbild): unten rechts, ~6 s, Klick
/// öffnet die Datei, ✕ verwirft nur die Vorschau.
fn thumb_ui(pfad: PathBuf) -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};
    let rand = mkw::leiste::SCHATTEN_RAND as u32;
    THUMB_PFAD.set(pfad).ok();
    iced_layershell::application(
        Thumb::new,
        || String::from("matrix-aufnahme"),
        Thumb::update,
        Thumb::view,
    )
    .subscription(Thumb::subscription)
    .style(|_s, _t| iced::theme::Style {
        background_color: Color::TRANSPARENT,
        text_color: Color::WHITE,
    })
    .settings(Settings {
        layer_settings: LayerShellSettings {
            size: Some((THUMB_B as u32 + 2 * rand, THUMB_H as u32 + 2 * rand)),
            anchor: Anchor::Bottom | Anchor::Right,
            margin: (0, 16 - mkw::leiste::SCHATTEN_RAND as i32, 140 - mkw::leiste::SCHATTEN_RAND as i32, 0),
            layer: Layer::Overlay,
            keyboard_interactivity: KeyboardInteractivity::None,
            exclusive_zone: 0,
            ..Default::default()
        },
        default_font: Font::with_name("Inter Variable"),
        fonts: mkw::symbol_font_laden().into_iter().collect(),
        ..Default::default()
    })
    .run()
}

static THUMB_PFAD: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[to_layer_message]
#[derive(Debug, Clone)]
enum ThumbMsg {
    Tick,
    Oeffnen,
    Weg,
}

struct Thumb {
    palette: mk::Palette,
    seit: Instant,
    bild: Option<image::Handle>,
}

impl Thumb {
    fn new() -> (Self, Task<ThumbMsg>) {
        let pfad = THUMB_PFAD.get().cloned().unwrap_or_default();
        let ist_foto = pfad
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"));
        (
            Thumb {
                palette: mk::Palette::load().unwrap_or_default(),
                seit: Instant::now(),
                bild: ist_foto.then(|| image::Handle::from_path(&pfad)),
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: ThumbMsg) -> Task<ThumbMsg> {
        match msg {
            ThumbMsg::Tick => {
                if self.seit.elapsed().as_secs() >= THUMB_SICHTBAR_S {
                    std::process::exit(0);
                }
                Task::none()
            }
            ThumbMsg::Oeffnen => {
                if let Some(p) = THUMB_PFAD.get() {
                    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
                }
                std::process::exit(0);
            }
            ThumbMsg::Weg => std::process::exit(0),
            _ => Task::none(),
        }
    }

    fn subscription(&self) -> Subscription<ThumbMsg> {
        mkw::tick("aufnahme-thumb", Duration::from_millis(500)).map(|_| ThumbMsg::Tick)
    }

    fn view(&self) -> Element<'_, ThumbMsg> {
        let p = self.palette;
        let vorschau: Element<'_, ThumbMsg> = match &self.bild {
            Some(h) => image(h.clone())
                .width(Length::Fixed(THUMB_B - 24.0))
                .height(Length::Fixed(THUMB_H - 48.0))
                .into(),
            None => container(mkw::symbol::<ThumbMsg>(
                mkw::symbol::PLAY_ARROW,
                mk::icon_size::HERO,
                p.primary,
            ))
            .center_x(Length::Fixed(THUMB_B - 24.0))
            .center_y(Length::Fixed(THUMB_H - 48.0))
            .into(),
        };
        let kopf = row![
            mkw::txt("Aufnahme gesichert", mk::typo::KLEIN, p.text_stufe(2)),
            iced::widget::Space::new().width(Length::Fill),
            mkw::ui::kopf_text_knopf("✕", ThumbMsg::Weg, p),
        ]
        .align_y(iced::Alignment::Center);
        let karte = container(column![kopf, vorschau].spacing(mk::spacing::XS))
            .padding(mk::spacing::M)
            .width(Length::Fixed(THUMB_B))
            .style(move |_| mkw::leiste::pille(p, mk::radius::KLEIN, mk::spacing::M));
        let klickbar = iced::widget::mouse_area(mkw::leiste::schatten_schichten(
            karte.into(),
            mk::radius::KLEIN + mk::spacing::M,
        ))
        .on_press(ThumbMsg::Oeffnen);
        container(klickbar)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(mkw::leiste::SCHATTEN_RAND)
            .into()
    }
}
