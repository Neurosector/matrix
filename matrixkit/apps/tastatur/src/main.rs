//! Matrix Tastatur — App #27, die Bildschirmtastatur (R58).
//!
//! Die Tablet-Leitbild-Grammatik: Sie ERSCHEINT von selbst, wenn man ein
//! Eingabefeld berührt und keine physische Tastatur angeschlossen ist —
//! und verschwindet, wenn das Feld den Fokus verliert, eine Tastatur
//! angesteckt wird oder ✕ sie schließt.
//!
//! Drei Gesichter eines Binaries:
//! * **Wache** (ohne Argument, Autostart): lauscht auf dem Datagram-
//!   Sockel auf die `auf`-Herzschläge der MatrixKit-Eingabefelder
//!   (mkw::tastatur::funken), prüft alle 2 s die Tastatur-Lage in
//!   /proc/bus/input/devices und startet/beendet die Fläche.
//! * **`--flaeche`**: die Layershell-Pille (Layer::Top, nimmt NIE den
//!   Tastatur-Fokus) — QWERTZ-CH, ⇧, ?123, tippt über die Einspeisung.
//! * **`zeigen` / `verbergen` / `--tipptest`**: Handgriffe und Beweis.
//!
//! Getippt wird über `zwp_virtual_keyboard_v1` (einspeisung.rs) — die
//! Zeichen landen im FOKUSSIERTEN Fenster, darum funktioniert dieselbe
//! Tastatur in jeder App und im Greeter-Passwortfeld.

mod einspeisung;

use std::sync::mpsc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use iced::widget::{button, column, container, row};
use iced::{Color, Element, Font, Length, Subscription, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use matrixkit_widgets::color;

/// Grundtaste (1 Einheit) und Zeilenhöhe — Touch-Ziele, keine Deko.
const TASTE: f32 = 56.0;
const TASTE_H: f32 = 48.0;
const LUECKE: f32 = 6.0;
const RAND: f32 = 12.0;
/// Breiteste Zeile: 11 Einheiten (qwertzuiopü).
const PILLE_B: f32 = 11.0 * TASTE + 10.0 * LUECKE + 2.0 * RAND;
const PILLE_H: f32 = 4.0 * TASTE_H + 3.0 * LUECKE + 2.0 * RAND;

/// Exit-Code der Fläche für „✕ gedrückt" — die Wache sperrt dann bis
/// zum nächsten Fokus-Wechsel, statt sofort wieder aufzupoppen.
const MANUELL_ZU: i32 = 7;

/// Der Einspeisungs-Kanal — vor dem iced-Lauf gelegt (App::new hat keine
/// Argumente, also wohnt er im OnceLock).
static KANAL: OnceLock<Option<mpsc::Sender<einspeisung::Befehl>>> = OnceLock::new();

fn main() -> Result<(), iced_layershell::Error> {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--flaeche") => flaeche(),
        Some("--tipptest") => {
            let text: String = std::env::args().skip(2).collect::<Vec<_>>().join(" ");
            einspeisung::tipptest(&text);
            Ok(())
        }
        Some("zeigen") => {
            mkw::tastatur::senden("zeigen");
            Ok(())
        }
        Some("verbergen") => {
            mkw::tastatur::senden("zu");
            Ok(())
        }
        _ => {
            wache();
            Ok(())
        }
    }
}

// ------------------------------------------------------------------ Wache

/// Hängt eine PHYSISCHE Tastatur am Gerät? Kernel-Wahrheit statt Raten:
/// ein Block in /proc/bus/input/devices mit kbd-Handler, EV_KEY UND
/// EV_REP (nur echte Tastaturen wiederholen) — Video Bus und Power-
/// Knöpfe fallen durch, virtuelle Geräte sowieso.
fn physische_tastatur() -> bool {
    let Ok(inhalt) = std::fs::read_to_string("/proc/bus/input/devices") else {
        // Im Zweifel keine Tastatur AUFDRÄNGEN.
        return true;
    };
    inhalt.split("\n\n").any(|block| {
        let kbd = block
            .lines()
            .any(|z| z.starts_with("H:") && z.contains("kbd"));
        let virtuell = block
            .lines()
            .any(|z| z.starts_with("N:") && z.to_lowercase().contains("virtual"));
        let ev = block
            .lines()
            .find_map(|z| z.trim().strip_prefix("B: EV="))
            .and_then(|h| u64::from_str_radix(h.trim(), 16).ok())
            .unwrap_or(0);
        kbd && !virtuell && (ev & 0x2 != 0) && (ev & 0x10_0000 != 0)
    })
}

fn kind_beenden(kind: &mut Option<std::process::Child>) {
    if let Some(mut k) = kind.take() {
        let _ = k.kill();
        let _ = k.wait();
    }
}

/// Der Daemon: Herzschläge sammeln, Tastatur-Lage prüfen, Fläche führen.
/// Stille > 4 s heißt Blur (der Herzschlag kommt vom Cursor-Blinken der
/// fokussierten Felder, alle ~700 ms).
fn wache() {
    use std::os::unix::net::UnixDatagram;
    let sockel = mkw::tastatur::sockel();
    let _ = std::fs::remove_file(&sockel);
    let sock = UnixDatagram::bind(&sockel).expect("matrix-tastatur: Sockel belegt?");
    let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));

    let mut kind: Option<std::process::Child> = None;
    let mut letzte_auf = Instant::now() - Duration::from_secs(60);
    // Nach ✕ oder `verbergen`: erst ein Blur (Stille) rüstet wieder scharf.
    let mut gesperrt = false;
    // Nach `zeigen`: sichtbar trotz physischer Tastatur (Handgriff/Test).
    let mut erzwungen = false;
    let mut phys = physische_tastatur();
    let mut letzter_check = Instant::now();
    let mut puffer = [0u8; 64];

    loop {
        if let Ok(n) = sock.recv(&mut puffer) {
            match String::from_utf8_lossy(&puffer[..n]).trim() {
                "auf" => letzte_auf = Instant::now(),
                "zeigen" => {
                    erzwungen = true;
                    gesperrt = false;
                }
                "zu" | "verbergen" => {
                    erzwungen = false;
                    gesperrt = true;
                    kind_beenden(&mut kind);
                }
                _ => {}
            }
        }
        if letzter_check.elapsed() >= Duration::from_secs(2) {
            phys = physische_tastatur();
            letzter_check = Instant::now();
        }
        if let Some(k) = kind.as_mut() {
            if let Ok(Some(status)) = k.try_wait() {
                if status.code() == Some(MANUELL_ZU) {
                    gesperrt = true;
                    erzwungen = false;
                }
                kind = None;
            }
        }
        let stille = letzte_auf.elapsed() > Duration::from_secs(4);
        if stille {
            gesperrt = false; // Blur entsperrt — der nächste Fokus ruft neu
        }
        let soll = erzwungen || (!phys && !gesperrt && !stille);
        if soll && kind.is_none() {
            if let Ok(selbst) = std::env::current_exe() {
                kind = std::process::Command::new(selbst).arg("--flaeche").spawn().ok();
            }
        } else if !soll {
            kind_beenden(&mut kind);
        }
    }
}

// ------------------------------------------------------------ Belegung

#[derive(Debug, Clone)]
enum Aktion {
    Text(char),
    Shift,
    Loeschen,
    Enter,
    Leer,
    Seite,
    Schliessen,
}

struct TasteDef {
    anzeige: String,
    einheiten: f32,
    aktion: Aktion,
    akzent: bool,
}

fn zeichen_taste(z: char) -> TasteDef {
    TasteDef { anzeige: z.to_string(), einheiten: 1.0, aktion: Aktion::Text(z), akzent: false }
}

fn gross(z: char) -> char {
    z.to_uppercase().next().unwrap_or(z)
}

/// Die Belegung einer Seite — QWERTZ-CH mit Umlauten, Seite 1 = Ziffern
/// und Zeichen samt Akzent-Buchstaben (é è à ç).
fn belegung(seite: usize, shift: bool) -> Vec<Vec<TasteDef>> {
    let zeile = |s: &str| -> Vec<TasteDef> {
        s.chars()
            .map(|c| zeichen_taste(if shift { gross(c) } else { c }))
            .collect()
    };
    let loeschen = TasteDef {
        anzeige: "\u{232b}".into(),
        einheiten: 1.5,
        aktion: Aktion::Loeschen,
        akzent: false,
    };
    let unten = |wechsel: &str| -> Vec<TasteDef> {
        vec![
            TasteDef { anzeige: wechsel.into(), einheiten: 1.5, aktion: Aktion::Seite, akzent: false },
            zeichen_taste(','),
            TasteDef { anzeige: String::new(), einheiten: 5.0, aktion: Aktion::Leer, akzent: false },
            zeichen_taste('.'),
            TasteDef { anzeige: "\u{23ce}".into(), einheiten: 2.0, aktion: Aktion::Enter, akzent: true },
            TasteDef { anzeige: "\u{2715}".into(), einheiten: 1.0, aktion: Aktion::Schliessen, akzent: false },
        ]
    };
    match seite {
        0 => vec![
            zeile("qwertzuiopü"),
            zeile("asdfghjklöä"),
            {
                let mut z = vec![TasteDef {
                    anzeige: "\u{21e7}".into(),
                    einheiten: 1.5,
                    aktion: Aktion::Shift,
                    akzent: shift,
                }];
                z.extend(zeile("yxcvbnm"));
                z.push(loeschen);
                z
            },
            unten("?123"),
        ],
        _ => vec![
            zeile("1234567890"),
            zeile("-/:;()&@\"#"),
            {
                let mut z = zeile("=+*%_\u{00e9}\u{00e8}\u{00e0}\u{00e7}");
                z.push(loeschen);
                z
            },
            unten("ABC"),
        ],
    }
}

/// Alle Zeichen, die diese Tastatur je tippen kann — für die Keymap.
fn alle_zeichen() -> Vec<char> {
    let mut alle = vec![' '];
    for seite in 0..2 {
        for shift in [false, true] {
            for zeile in belegung(seite, shift) {
                for t in zeile {
                    if let Aktion::Text(z) = t.aktion {
                        alle.push(z);
                    }
                }
            }
        }
    }
    alle
}

// ------------------------------------------------------------- Fläche

fn flaeche() -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};
    let rand = mkw::leiste::SCHATTEN_RAND as u32;
    iced_layershell::application(
        App::new,
        || String::from("matrix-tastatur"),
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
            size: Some((PILLE_B as u32 + 2 * rand, PILLE_H as u32 + 2 * rand)),
            anchor: Anchor::Bottom,
            // Schattenraum wie beim Dock: die Pille schwebt 10 px über
            // der Kante, die Surface reicht für den Blur darüber hinaus.
            margin: (0, 0, 10 - mkw::leiste::SCHATTEN_RAND as i32, 0),
            layer: Layer::Top,
            // NIE den Tastatur-Fokus nehmen — sonst tippte man ins Nichts.
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

#[to_layer_message]
#[derive(Debug, Clone)]
enum Msg {
    FadeTick,
    Puls,
    Druck(char),
    Shift,
    Loeschen,
    Enter,
    Leer,
    Seite,
    Schliessen,
}

struct App {
    palette: mk::Palette,
    watcher: mk::PaletteWatcher,
    shift: bool,
    seite: usize,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let _ = KANAL.set(einspeisung::starten(alle_zeichen()));
        (
            App {
                palette: mk::Palette::load().unwrap_or_default(),
                watcher: mk::PaletteWatcher::new(),
                shift: false,
                seite: 0,
            },
            Task::none(),
        )
    }

    fn tippen(befehl: einspeisung::Befehl) {
        if let Some(Some(kanal)) = KANAL.get() {
            let _ = kanal.send(befehl);
        }
        // Tippen hält die Wache wach — auch wenn eine App das Cursor-
        // Blinken (den Herzschlag) gerade aussetzt.
        mkw::tastatur::senden("auf");
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::FadeTick | Msg::Puls => {
                if self.watcher.changed() {
                    if let Some(neu) = mk::Palette::load() {
                        self.palette = neu;
                    }
                }
                Task::none()
            }
            Msg::Druck(z) => {
                Self::tippen(einspeisung::Befehl::Zeichen(z));
                self.shift = false;
                Task::none()
            }
            Msg::Leer => {
                Self::tippen(einspeisung::Befehl::Zeichen(' '));
                Task::none()
            }
            Msg::Loeschen => {
                Self::tippen(einspeisung::Befehl::Name("BackSpace"));
                Task::none()
            }
            Msg::Enter => {
                Self::tippen(einspeisung::Befehl::Name("Return"));
                Task::none()
            }
            Msg::Shift => {
                self.shift = !self.shift;
                Task::none()
            }
            Msg::Seite => {
                self.seite = 1 - self.seite;
                Task::none()
            }
            Msg::Schliessen => std::process::exit(MANUELL_ZU),
            // vom to_layer_message-Makro ergänzte Varianten
            _ => Task::none(),
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch([
            mkw::palette_fade_abo().map(|_| Msg::FadeTick),
            mkw::tick("tastatur", Duration::from_secs(5)).map(|_| Msg::Puls),
        ])
    }

    fn taste_element(&self, def: &TasteDef) -> Element<'_, Msg> {
        let p = self.palette;
        let breite = def.einheiten * TASTE + (def.einheiten - 1.0) * LUECKE;
        let msg = match def.aktion {
            Aktion::Text(z) => Msg::Druck(z),
            Aktion::Shift => Msg::Shift,
            Aktion::Loeschen => Msg::Loeschen,
            Aktion::Enter => Msg::Enter,
            Aktion::Leer => Msg::Leer,
            Aktion::Seite => Msg::Seite,
            Aktion::Schliessen => Msg::Schliessen,
        };
        let akzent = def.akzent;
        let schrift = if akzent { p.on_primary } else { p.on_surface };
        let inhalt = container(mkw::txt(def.anzeige.clone(), mk::typo::FLIESS, schrift))
            .center_x(Length::Fill)
            .center_y(Length::Fill);
        let knopf = button(inhalt)
            .width(Length::Fixed(breite))
            .height(Length::Fixed(TASTE_H))
            .padding(0)
            .on_press(msg.clone())
            .style(move |_, status| {
                let grund = if akzent {
                    p.primary
                } else {
                    p.on_surface.over(p.surface_container, 0.08)
                };
                let bg = match status {
                    iced::widget::button::Status::Hovered => {
                        p.on_surface.over(grund, mk::state_layer::HOVER)
                    }
                    iced::widget::button::Status::Pressed => {
                        p.on_surface.over(grund, mk::state_layer::PRESSED)
                    }
                    _ => grund,
                };
                // familien-ausnahme: Tastatur-Tasten — eigene Kachel-Fläche wie Dock-Kacheln
                iced::widget::button::Style {
                    background: Some(color(bg).into()),
                    text_color: color(schrift),
                    border: iced::Border {
                        radius: mk::radius::KLEIN.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            });
        // R59 (UIKeyboard-Extrakt): die Taste tippt beim BERÜHREN —
        // der Knopf feuert nur noch für Maus/Trackpad beim Loslassen.
        mkw::sofort_taste(knopf, msg)
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let mut zeilen = column![].spacing(LUECKE);
        for zeile in belegung(self.seite, self.shift) {
            let mut r = row![].spacing(LUECKE);
            for def in &zeile {
                r = r.push(self.taste_element(def));
            }
            zeilen = zeilen.push(container(r).center_x(Length::Fill));
        }
        let pille = container(zeilen)
            .padding(RAND)
            .width(Length::Fixed(PILLE_B))
            .style(move |_| mkw::leiste::pille(p, mk::radius::KLEIN, RAND));
        let schwebend = mkw::leiste::schatten_schichten(pille.into(), mk::radius::KLEIN + RAND);
        container(schwebend)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(iced::Padding {
                bottom: mkw::leiste::SCHATTEN_RAND,
                ..iced::Padding::ZERO
            })
            .into()
    }
}
