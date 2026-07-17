//! Matrix Zentrale — App #13, das Kontrollzentrum der Leisten-Familie.
//!
//! Ein schwebendes Panel oben rechts unter der Bar: Klang (Lautstärke,
//! Stumm, Mikrofon), Funk (WLAN, Bluetooth), Bildschirm (Helligkeit,
//! falls der Monitor DDC spricht), System (Bewegung reduzieren,
//! Erscheinungsbild falls DMS lebt). Derselbe Aufruf öffnet und
//! schließt (mkw::leiste_toggle) — später klickt die Bar hierher.
//!
//! Alle Stellhebel sind Kommando-Brücken (wpctl, nmcli, bluetoothctl,
//! ddcutil, dms ipc) — Zeilen ohne Backend erscheinen einfach nicht.

use iced::widget::{button, column, container, row, Space};
use iced::{Color, Element, Font, Length, Subscription, Task};
use iced_layershell::to_layer_message;
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use matrixkit_widgets::tick;

const BREITE: u32 = 340;
/// Höhe einer mkw-Zeile (Titelzeile + Innenluft), fürs Panel-Maß.
const ZEILE: f32 = 50.0;
const KOPF: f32 = 44.0;
const SEKTION_TITEL: f32 = 28.0;

fn main() -> Result<(), iced_layershell::Error> {
    use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
    use iced_layershell::settings::{LayerShellSettings, Settings};

    // Toggle: lief die Zentrale schon, ist sie jetzt zu — wir auch.
    if !mkw::leiste_toggle() {
        return Ok(());
    }

    iced_layershell::application(
        App::new,
        || String::from("matrix-zentrale"),
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
            size: Some((BREITE + 2 * mkw::leiste::SCHATTEN_RAND as u32, 240)), // wächst gleich auf das echte Maß
            anchor: Anchor::Top | Anchor::Right,
            // Die Bar reserviert ihre Zone (36) — der Compositor schiebt
            // uns schon darunter; hier nur noch die Atempause.
            margin: (8, 10, 0, 0),
            layer: Layer::Top,
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

/// „Volume: 0.45 [MUTED]" → (0.45, true)
fn klang_lesen(ziel: &str) -> Option<(f32, bool)> {
    let z = mk::befehl::erste_zeile("wpctl", &["get-volume", ziel])?;
    let wert: f32 = z.split_whitespace().nth(1)?.parse().ok()?;
    Some((wert, z.contains("MUTED")))
}

fn wlan_lesen() -> Option<bool> {
    mk::befehl::erste_zeile("nmcli", &["radio", "wifi"]).map(|z| z == "enabled")
}

fn bt_lesen() -> Option<bool> {
    let out = std::process::Command::new("bluetoothctl")
        .arg("show")
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    if !s.contains("Controller") {
        return None;
    }
    Some(s.lines().any(|l| l.trim() == "Powered: yes"))
}

/// Woher die Helligkeit kommt: Laptop-Backlight oder Monitor-DDC.
#[derive(Debug, Clone, Copy, PartialEq)]
enum HelligkeitsQuelle {
    Backlight,
    Ddc,
}

/// Erst das Laptop-Backlight (Surface, schnell), dann Monitor-DDC
/// (Desktop, träge — deshalb läuft das Ganze asynchron).
fn helligkeit_lesen() -> Option<(f32, HelligkeitsQuelle)> {
    // brightnessctl -m: "intel_backlight,backlight,48000,50%,96000"
    if let Some(z) = mk::befehl::erste_zeile("brightnessctl", &["-m"]) {
        if let Some(p) = z.split(',').nth(3) {
            if let Ok(wert) = p.trim_end_matches('%').parse::<f32>() {
                return Some((wert, HelligkeitsQuelle::Backlight));
            }
        }
    }
    // ddcutil --brief: "VCP 10 C 70 100"
    let z = mk::befehl::erste_zeile("ddcutil", &["getvcp", "10", "--brief"])?;
    let wert = z.split_whitespace().nth(3)?.parse().ok()?;
    Some((wert, HelligkeitsQuelle::Ddc))
}

fn helligkeit_setzen(wert: f32, quelle: HelligkeitsQuelle) {
    match quelle {
        HelligkeitsQuelle::Backlight => {
            let _ = mk::befehl::still("brightnessctl", &["set", &format!("{wert:.0}%")]);
        }
        HelligkeitsQuelle::Ddc => {
            let _ = mk::befehl::still("ddcutil", &["setvcp", "10", &format!("{wert:.0}")]);
        }
    }
}

/// Lebt DMS? Nur dann gibt es den Erscheinungsbild-Schalter.
fn dms_lebt() -> bool {
    // Der Quickshell-Prozess heisst "qs" (comm), nicht "quickshell".
    std::process::Command::new("pgrep")
        .args(["-x", "qs"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Msg {
    Tick,
    Lautstaerke(f32),
    TonStumm,
    MikroStumm,
    Wlan(bool),
    Bluetooth(bool),
    Helligkeit(f32),
    HelligkeitSetzen,
    HelligkeitDa(Option<(f32, HelligkeitsQuelle)>),
    Bewegung(bool),
    Transparenz(bool),
    Kontrast(bool),
    Erscheinung,
    Schliessen,
}

struct App {
    palette: mk::Palette,
    watcher: mk::PaletteWatcher,
    lautstaerke: Option<(f32, bool)>,
    mikro_stumm: Option<bool>,
    wlan: Option<bool>,
    bluetooth: Option<bool>,
    /// 0–100 samt Quelle; None = kein Backend (oder noch am Laden).
    helligkeit: Option<(f32, HelligkeitsQuelle)>,
    bewegung_reduziert: bool,
    transparenz_reduziert: bool,
    kontrast_hoch: bool,
    dms: bool,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let app = App {
            palette: mk::Palette::load().unwrap_or_default(),
            watcher: mk::PaletteWatcher::new(),
            lautstaerke: klang_lesen("@DEFAULT_AUDIO_SINK@"),
            mikro_stumm: klang_lesen("@DEFAULT_AUDIO_SOURCE@").map(|(_, m)| m),
            wlan: wlan_lesen(),
            bluetooth: bt_lesen(),
            helligkeit: None,
            bewegung_reduziert: mk::bewegung_reduziert(),
            transparenz_reduziert: mk::transparenz_reduziert(),
            kontrast_hoch: mk::kontrast_hoch(),
            dms: dms_lebt(),
        };
        let start_hoehe = app.hoehe();
        (
            app,
            Task::batch([
                Task::done(Msg::SizeChange((BREITE + 2 * mkw::leiste::SCHATTEN_RAND as u32, start_hoehe + mkw::leiste::SCHATTEN_RAND as u32))),
                // DDC braucht Sekunden — das Panel wartet nicht darauf.
                Task::perform(async { helligkeit_lesen() }, Msg::HelligkeitDa),
            ]),
        )
    }

    /// Panel-Maß aus den tatsächlich sichtbaren Zeilen.
    fn hoehe(&self) -> u32 {
        let mut zeilen = 0.0;
        let mut sektionen = 0.0;
        // Klang: Lautstärke + Ton aus (falls Senke) + Mikro (falls Quelle)
        if self.lautstaerke.is_some() || self.mikro_stumm.is_some() {
            sektionen += 1.0;
            if self.lautstaerke.is_some() {
                zeilen += 2.0;
            }
            if self.mikro_stumm.is_some() {
                zeilen += 1.0;
            }
        }
        if self.wlan.is_some() || self.bluetooth.is_some() {
            sektionen += 1.0;
            zeilen += self.wlan.is_some() as u8 as f32 + self.bluetooth.is_some() as u8 as f32;
        }
        if self.helligkeit.is_some() {
            sektionen += 1.0;
            zeilen += 1.0;
        }
        // Bedienungshilfen: die Trias, immer.
        sektionen += 1.0;
        zeilen += 3.0;
        // System: Erscheinung nur mit DMS
        if self.dms {
            sektionen += 1.0;
            zeilen += 1.0;
        }

        (KOPF
            + zeilen * ZEILE
            + sektionen * (SEKTION_TITEL + mk::spacing::M)
            + 2.0 * mk::spacing::M) as u32
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Tick => {
                if self.watcher.changed() {
                    if let Some(neu) = mk::Palette::load() {
                        self.palette = neu;
                    }
                }
                self.lautstaerke = klang_lesen("@DEFAULT_AUDIO_SINK@");
                self.mikro_stumm = klang_lesen("@DEFAULT_AUDIO_SOURCE@").map(|(_, m)| m);
                self.wlan = wlan_lesen();
                self.bluetooth = bt_lesen();
                self.bewegung_reduziert = mk::bewegung_reduziert();
                self.transparenz_reduziert = mk::transparenz_reduziert();
                self.kontrast_hoch = mk::kontrast_hoch();
            }
            Msg::Lautstaerke(v) => {
                let _ = mk::befehl::still("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{v:.2}")]);
                if let Some((_, stumm)) = self.lautstaerke {
                    self.lautstaerke = Some((v, stumm));
                }
            }
            Msg::TonStumm => {
                let _ = mk::befehl::still("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]);
                if let Some((v, stumm)) = self.lautstaerke {
                    self.lautstaerke = Some((v, !stumm));
                }
            }
            Msg::MikroStumm => {
                let _ = mk::befehl::still("wpctl", &["set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"]);
                self.mikro_stumm = self.mikro_stumm.map(|m| !m);
            }
            Msg::Wlan(an) => {
                let _ = mk::befehl::still("nmcli", &["radio", "wifi", if an { "on" } else { "off" }]);
                self.wlan = Some(an);
            }
            Msg::Bluetooth(an) => {
                let _ = mk::befehl::still("bluetoothctl", &["power", if an { "on" } else { "off" }]);
                self.bluetooth = Some(an);
            }
            Msg::Helligkeit(v) => {
                // Anzeige folgt sofort, gesetzt wird beim Loslassen.
                if let Some((_, q)) = self.helligkeit {
                    self.helligkeit = Some((v, q));
                }
            }
            Msg::HelligkeitSetzen => {
                if let Some((v, q)) = self.helligkeit {
                    // DDC ist träge — setzen läuft im Hintergrund, das
                    // Echo (None) ändert nichts am Zustand.
                    return Task::perform(
                        async move {
                            helligkeit_setzen(v, q);
                            None
                        },
                        Msg::HelligkeitDa,
                    );
                }
            }
            Msg::HelligkeitDa(w) => {
                if w.is_some() {
                    self.helligkeit = w;
                    return Task::done(Msg::SizeChange((BREITE + 2 * mkw::leiste::SCHATTEN_RAND as u32, self.hoehe() + mkw::leiste::SCHATTEN_RAND as u32)));
                }
            }
            Msg::Bewegung(reduziert) => {
                mk::einstellung::schreiben("bewegung", if reduziert { "reduziert" } else { "voll" });
                self.bewegung_reduziert = reduziert;
            }
            Msg::Transparenz(reduziert) => {
                mk::einstellung::schreiben(
                    "transparenz",
                    if reduziert { "reduziert" } else { "voll" },
                );
                self.transparenz_reduziert = reduziert;
            }
            Msg::Kontrast(hoch) => {
                mk::einstellung::schreiben("kontrast", if hoch { "hoch" } else { "normal" });
                if let Some(neu) = mk::Palette::load() {
                    self.palette = neu; // sofort mit eigener Medizin färben
                }
                self.kontrast_hoch = hoch;
            }
            Msg::Erscheinung => {
                mk::hell_umschalten(); // eigene Farbkette, kein DMS
            }
            Msg::Schliessen => std::process::exit(0),
            // vom to_layer_message-Makro ergänzte Varianten
            _ => {}
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        tick("zentrale", std::time::Duration::from_secs(2)).map(|_| Msg::Tick)
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let mut inhalt = column![].spacing(mk::spacing::M);

        // Kopf: Titel + Schließen.
        let zu = button(mkw::txt("✕", mk::typo::KOPF, p.on_surface_variant))
            .padding([2, mk::spacing::S as u16])
            .style(move |_, status| mkw::leiste::knopf_stil(p, status, mk::radius::KLEIN))
            .on_press(Msg::Schliessen);
        inhalt = inhalt.push(
            row![
                mkw::txt("Kontrollzentrum", mk::typo::KOPF, p.on_surface),
                Space::new().width(Length::Fill),
                zu,
            ]
            .align_y(iced::Alignment::Center),
        );

        // KLANG
        let mut klang: Vec<Element<'_, Msg>> = Vec::new();
        if let Some((v, stumm)) = self.lautstaerke {
            klang.push(mkw::zeile(
                "Lautstärke",
                None,
                Some(mkw::symbol(mkw::symbol::VOLUME_UP, mk::icon_size::SMALL + 4.0, p.on_surface_variant)),
                Some(
                    mkw::regler(0.0..=1.0, v, 0.01, p, Msg::Lautstaerke)
                        .width(Length::Fixed(150.0))
                        .into(),
                ),
                p,
            ));
            klang.push(mkw::zeile_schalter(
                "Ton aus",
                None,
                None,
                stumm,
                p,
                Some(Msg::TonStumm),
            ));
        }
        if let Some(m) = self.mikro_stumm {
            klang.push(mkw::zeile_schalter(
                "Mikrofon aus",
                None,
                None,
                m,
                p,
                Some(Msg::MikroStumm),
            ));
        }
        if !klang.is_empty() {
            inhalt = inhalt.push(mkw::sektion("KLANG", klang, p));
        }

        // FUNK
        let mut funk: Vec<Element<'_, Msg>> = Vec::new();
        if let Some(an) = self.wlan {
            funk.push(mkw::zeile_schalter("WLAN", None, Some(mkw::symbol(mkw::symbol::WIFI, mk::icon_size::SMALL + 4.0, p.on_surface_variant)), an, p, Some(Msg::Wlan(!an))));
        }
        if let Some(an) = self.bluetooth {
            funk.push(mkw::zeile_schalter(
                "Bluetooth",
                None,
                Some(mkw::symbol(mkw::symbol::BLUETOOTH, mk::icon_size::SMALL + 4.0, p.on_surface_variant)),
                an,
                p,
                Some(Msg::Bluetooth(!an)),
            ));
        }
        if !funk.is_empty() {
            inhalt = inhalt.push(mkw::sektion("FUNK", funk, p));
        }

        // BILDSCHIRM (nur wenn der Monitor DDC spricht)
        if let Some((h, _)) = self.helligkeit {
            inhalt = inhalt.push(mkw::sektion(
                "BILDSCHIRM",
                vec![mkw::zeile(
                    "Helligkeit",
                    None,
                    Some(mkw::symbol(mkw::symbol::BRIGHTNESS, mk::icon_size::SMALL + 4.0, p.on_surface_variant)),
                    Some(
                        mkw::regler(0.0..=100.0, h, 1.0, p, Msg::Helligkeit)
                            .on_release(Msg::HelligkeitSetzen)
                            .width(Length::Fixed(150.0))
                            .into(),
                    ),
                    p,
                )],
                p,
            ));
        }

        // BEDIENUNGSHILFEN — die Leitbild-Trias (Motion/Transparency/Contrast).
        inhalt = inhalt.push(mkw::sektion(
            "BEDIENUNGSHILFEN",
            vec![
                mkw::zeile_schalter(
                    "Bewegung reduzieren",
                    Some("Federn und Übergänge stillhalten"),
                    None,
                    self.bewegung_reduziert,
                    p,
                    Some(Msg::Bewegung(!self.bewegung_reduziert)),
                ),
                mkw::zeile_schalter(
                    "Transparenz reduzieren",
                    Some("Leisten und Schleier deckend"),
                    None,
                    self.transparenz_reduziert,
                    p,
                    Some(Msg::Transparenz(!self.transparenz_reduziert)),
                ),
                mkw::zeile_schalter(
                    "Erhöhter Kontrast",
                    Some("Kräftigere Text- und Konturfarben"),
                    None,
                    self.kontrast_hoch,
                    p,
                    Some(Msg::Kontrast(!self.kontrast_hoch)),
                ),
            ],
            p,
        ));

        // SYSTEM
        let mut system: Vec<Element<'_, Msg>> = Vec::new();
        if self.dms {
            system.push(mkw::zeile_schalter(
                "Helles Erscheinungsbild",
                None,
                Some(mkw::symbol(mkw::symbol::DARK_MODE, mk::icon_size::SMALL + 4.0, p.on_surface_variant)),
                p.is_light,
                p,
                Some(Msg::Erscheinung),
            ));
        }
        if !system.is_empty() {
            inhalt = inhalt.push(mkw::sektion("SYSTEM", system, p));
        }

        let karte = container(inhalt.padding(mk::spacing::M))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| mkw::leiste::pille(p, mk::radius::KLEIN, mk::spacing::S));
        // Schatten-Atemraum: die Surface ist um SCHATTEN_RAND größer.
        container(karte)
            .padding(iced::Padding {
                top: 0.0,
                left: mkw::leiste::SCHATTEN_RAND,
                right: mkw::leiste::SCHATTEN_RAND,
                bottom: mkw::leiste::SCHATTEN_RAND,
            })
            .into()
    }
}
