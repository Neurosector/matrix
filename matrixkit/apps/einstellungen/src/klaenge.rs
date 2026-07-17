//! Ton — als Panel der Matrix Einstellungen (Fusion R41b, R72 erweitert).
//! Leitbild- „Ton"-Grammatik in einem Bereich: Toneffekte (die Klänge,
//! war App #4), Ausgabe (Gerätewahl + Lautstärke + Lautsprecher-Test)
//! und Eingabe (Gerätewahl + Pegelanzeige) — alles über wpctl/PipeWire.
//! Schalter sind bindend für die Hooks (klaenge.conf).

use iced::widget::{column, container, mouse_area, row, Space};
use iced::{Element, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Ereignis-Katalog — Reihenfolge = Anzeige.
const EREIGNISSE: &[(&str, &str, &str)] = &[
    ("01-anmeldung", "Anmelden", "Drei tiefe Pulse steigen ruhig auf"),
    ("02-abmeldung", "Abmelden", "Dieselben Stufen abwärts"),
    ("03-benachrichtigung", "Benachrichtigung", "Zwei Pulse aufwärts — gute Nachricht"),
    ("04-benachrichtigung-leise", "Leiser Hinweis", "Ein einzelner ruhiger Puls"),
    ("05-lautstaerke", "Lautstärke", "Kurzer gedämpfter Tupfer"),
    ("06-geraet-verbunden", "Gerät verbunden", "Tiefe Quinte aufwärts"),
    ("07-geraet-getrennt", "Gerät getrennt", "Dieselbe Quinte, rückwärts"),
    ("08-arbeitsflaeche", "Arbeitsfläche", "Dunkler Hauch mit Grundpuls"),
    ("09-screenshot", "Bildschirmfoto", "Gedämpfter Auslöser"),
    ("10-papierkorb", "Papierkorb", "Dunkles Rascheln, tiefer Punkt"),
    ("11-fertig", "Fertig", "Das positive Spiegelbild des Fehlers"),
    ("12-fehler", "Fehler", "Zwei gedämpfte tiefe Pulse — die Referenz"),
    ("13-schluessel-erkannt", "Schlüssel erkannt", "Der Wächter — bewusst auffällig; am Login-Screen immer aktiv"),
    ("14-waechter-ruf", "Wächter-Ruf", "Der Ruf der Wiederherstellung — am Login-Screen immer aktiv"),
];

/// Klangordner: Dev-Stand im Home gewinnt, sonst die Image-Version.
/// Fehlt beides, gilt die Image-Version als gegeben (das matrix-klaenge-
/// Binary rendert bei Bedarf selbst).
fn klangordner() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    let lokal = PathBuf::from(&home).join(".local/share/matrix/klaenge");
    if lokal.join("12-fehler.wav").exists() {
        return lokal;
    }
    let system = PathBuf::from("/usr/share/matrix/klaenge");
    if system.join("12-fehler.wav").exists() {
        return system;
    }
    lokal
}

/// Die Klang-Einstellungen: fehlender Eintrag = an (Opt-out wie Rechte).
struct Einstellungen {
    aus: Vec<String>,
}

impl Einstellungen {
    fn pfad() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        PathBuf::from(home).join(".config/matrix/klaenge.conf")
    }

    fn laden() -> Self {
        let mut aus = Vec::new();
        if let Ok(inhalt) = std::fs::read_to_string(Self::pfad()) {
            for zeile in inhalt.lines() {
                if let Some((k, w)) = zeile.split_once('=') {
                    if w.trim() == "aus" {
                        aus.push(k.trim().to_string());
                    }
                }
            }
        }
        Self { aus }
    }

    fn an(&self, schluessel: &str) -> bool {
        !self.aus.iter().any(|a| a == schluessel)
    }

    fn setzen(&mut self, schluessel: &str, an: bool) {
        self.aus.retain(|a| a != schluessel);
        if !an {
            self.aus.push(schluessel.to_string());
        }
        let pfad = Self::pfad();
        if let Some(dir) = pfad.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut inhalt =
            String::from("# Matrix-Systemklänge — verwaltet von der App „Matrix Klänge\u{201c}\n");
        for a in &self.aus {
            inhalt.push_str(&format!("{a}=aus\n"));
        }
        let tmp = pfad.with_extension("conf.neu");
        if std::fs::write(&tmp, inhalt).is_ok() {
            let _ = std::fs::rename(&tmp, &pfad);
        }
    }
}

// ------------------------------------------------- Geräte (R72, wpctl)

/// Ein PipeWire-Endpunkt aus `wpctl status`.
#[derive(Debug, Clone)]
struct Geraet {
    id: u32,
    name: String,
    standard: bool,
}

fn wpctl(args: &[&str]) -> Option<String> {
    let aus = std::process::Command::new("wpctl").args(args).output().ok()?;
    aus.status
        .success()
        .then(|| String::from_utf8_lossy(&aus.stdout).into_owned())
}

/// `wpctl status` lesen: (Sinks, Sources). Zeilenform in den Abschnitten:
/// ` │  *   64. Name des Geräts [vol: 0.70]` — Stern = Standard.
fn geraete_lesen() -> (Vec<Geraet>, Vec<Geraet>) {
    let Some(text) = wpctl(&["status"]) else {
        return (Vec::new(), Vec::new());
    };
    let mut sinks = Vec::new();
    let mut quellen = Vec::new();
    let mut abschnitt = "";
    for zeile in text.lines() {
        if zeile.contains("Sinks:") {
            abschnitt = "sinks";
            continue;
        } else if zeile.contains("Sources:") {
            abschnitt = "quellen";
            continue;
        } else if zeile.trim() == "Video" {
            // Der Video-Abschnitt hat eigene "Sources:" (Kameras) —
            // ab hier ist nichts mehr Ton.
            break;
        } else if zeile.contains("Filters:") || zeile.contains("Streams:") {
            abschnitt = "";
            continue;
        }
        if abschnitt.is_empty() {
            continue;
        }
        // Baumzeichen und Sternchen abstreifen, "ID. Name [vol: …]" lesen.
        let kern = zeile.trim_start_matches([' ', '\u{2502}', '\u{251c}', '\u{2514}', '\u{2500}']);
        let standard = kern.starts_with('*');
        let kern = kern.trim_start_matches('*').trim_start();
        let Some((id_teil, rest)) = kern.split_once('.') else { continue };
        let Ok(id) = id_teil.trim().parse::<u32>() else { continue };
        let name = rest.split("[vol:").next().unwrap_or(rest).trim().to_string();
        if name.is_empty() {
            continue;
        }
        let eintrag = Geraet { id, name, standard };
        if abschnitt == "sinks" {
            sinks.push(eintrag);
        } else {
            quellen.push(eintrag);
        }
    }
    (sinks, quellen)
}

/// "Volume: 0.70" → 0.70.
fn volumen_lesen(ziel: &str) -> f32 {
    wpctl(&["get-volume", ziel])
        .and_then(|s| {
            s.split_whitespace()
                .nth(1)
                .and_then(|v| v.parse::<f32>().ok())
        })
        .unwrap_or(0.0)
}

/// Test-Ton erzeugen (0,5 s Sinus 440 Hz) — nur auf dem gewünschten
/// Kanal (0 = links, 1 = rechts), als WAV ins Runtime-Verzeichnis.
fn test_ton(kanal: usize) -> Option<PathBuf> {
    let basis = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| String::from("/tmp"));
    let pfad = PathBuf::from(basis).join(format!("matrix-ton-test-{kanal}.wav"));
    let rate: u32 = 44100;
    let n = rate / 2;
    let daten_len = n * 4; // 2 Kanäle × s16
    let mut wav = Vec::with_capacity(44 + daten_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + daten_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&2u16.to_le_bytes()); // Stereo
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 4).to_le_bytes());
    wav.extend_from_slice(&4u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&daten_len.to_le_bytes());
    for i in 0..n {
        // Sanfte Hüllkurve gegen Knackser an den Enden.
        let huelle = (i.min(n - i) as f32 / 2000.0).min(1.0);
        let s = (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.4 * huelle;
        let wert = (s * i16::MAX as f32) as i16;
        let paar = [if kanal == 0 { wert } else { 0 }, if kanal == 1 { wert } else { 0 }];
        for w in paar {
            wav.extend_from_slice(&w.to_le_bytes());
        }
    }
    std::fs::write(&pfad, wav).ok()?;
    Some(pfad)
}

pub struct Panel {
    pub palette: mk::Palette,
    einstellungen: Einstellungen,
    ordner: PathBuf,
    /// Zuletzt angespielter Klang — Rückmeldung in der Fußzeile.
    spielt: Option<&'static str>,
    /// Index der laufenden Hörprobe + Sprung-Feder des ▶ (Leitbild-Bounce).
    spielt_index: Option<usize>,
    sprung: mk::motion::Spring,
    // --- R72: Ausgabe & Eingabe ---
    sinks: Vec<Geraet>,
    quellen: Vec<Geraet>,
    aus_vol: f32,
    ein_vol: f32,
    /// Geräteliste nicht bei jedem Host-Tick neu erfragen.
    geprueft: Option<Instant>,
    /// Mikrofon-Pegel (0–100), gefüttert vom pw-record-Faden.
    pegel: Arc<AtomicU32>,
    pegel_aktiv: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    /// Eigener 60-fps-Tick für den ▶-Sprung (unabhängig von der Root-Feder).
    SprungTick,
    Schalten(usize, bool),
    MasterSchalten(bool),
    Probe(usize),
    // --- R72: Ausgabe & Eingabe ---
    AusgabeWahl(u32),
    EingabeWahl(u32),
    AusgabeVol(f32),
    EingabeVol(f32),
    /// Lautsprecher-Test: 0 = links, 1 = rechts.
    Test(usize),
    MikroTest(bool),
    /// Nur Neuzeichnen — der Pegel lebt im Atomic.
    PegelTick,
}

impl Panel {
    pub fn new() -> Self {
        let (sinks, quellen) = geraete_lesen();
        Self {
            palette: mk::Palette::load().unwrap_or_default(),
            einstellungen: Einstellungen::laden(),
                ordner: klangordner(),
                spielt: None,
                spielt_index: None,
            sprung: mk::motion::Spring::new(1.0),
            sinks,
            quellen,
            aus_vol: volumen_lesen("@DEFAULT_AUDIO_SINK@"),
            ein_vol: volumen_lesen("@DEFAULT_AUDIO_SOURCE@"),
            geprueft: Some(Instant::now()),
            pegel: Arc::new(AtomicU32::new(0)),
            pegel_aktiv: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Vom Host je Tick: Palette folgt, Fusstext verblasst; die
    /// Geräteliste wird höchstens alle 3 s neu erfragt (Hotplug).
    pub fn tick(&mut self, p: mk::Palette) {
        self.palette = p;
        self.spielt = None;
        let frisch = self
            .geprueft
            .map(|t| t.elapsed() < Duration::from_secs(3))
            .unwrap_or(false);
        if !frisch {
            self.geprueft = Some(Instant::now());
            let (sinks, quellen) = geraete_lesen();
            self.sinks = sinks;
            self.quellen = quellen;
            self.aus_vol = volumen_lesen("@DEFAULT_AUDIO_SINK@");
            self.ein_vol = volumen_lesen("@DEFAULT_AUDIO_SOURCE@");
        }
    }

    pub fn fusstext(&self) -> String {
        match self.spielt {
            Some(name) => format!("\u{25b6} {name} \u{2026}"),
            None => String::from("\u{25b6} spielt eine Hörprobe"),
        }
    }

    fn abspielen(&mut self, i: usize) {
        let (datei, name, _) = EREIGNISSE[i];
        let pfad = self.ordner.join(format!("{datei}.wav"));
        if let Ok(mut kind) = std::process::Command::new("pw-play").arg(&pfad).spawn() {
            // Kind im Hintergrund einsammeln — sonst bleibt ein Zombie
            std::thread::spawn(move || {
                let _ = kind.wait();
            });
            self.spielt = Some(name);
            self.spielt_index = Some(i);
            // Der ▶ federt: kommt gross an, schwingt auf Normalgroesse ein
            self.sprung = mk::motion::Spring::new(1.45);
            self.sprung.retarget(1.0);
        }
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::SprungTick => {
                self.sprung.tick(1.0 / 60.0);
                Task::none()
            }
            Msg::Schalten(i, an) => {
                self.einstellungen.setzen(EREIGNISSE[i].0, an);
                Task::none()
            }
            Msg::MasterSchalten(an) => {
                self.einstellungen.setzen("alle", an);
                Task::none()
            }
            Msg::Probe(i) => {
                self.abspielen(i);
                Task::none()
            }
            Msg::AusgabeWahl(id) => {
                wpctl(&["set-default", &id.to_string()]);
                for s in &mut self.sinks {
                    s.standard = s.id == id;
                }
                self.aus_vol = volumen_lesen("@DEFAULT_AUDIO_SINK@");
                Task::none()
            }
            Msg::EingabeWahl(id) => {
                wpctl(&["set-default", &id.to_string()]);
                for q in &mut self.quellen {
                    q.standard = q.id == id;
                }
                self.ein_vol = volumen_lesen("@DEFAULT_AUDIO_SOURCE@");
                Task::none()
            }
            Msg::AusgabeVol(v) => {
                self.aus_vol = v;
                wpctl(&["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{v:.2}")]);
                Task::none()
            }
            Msg::EingabeVol(v) => {
                self.ein_vol = v;
                wpctl(&["set-volume", "@DEFAULT_AUDIO_SOURCE@", &format!("{v:.2}")]);
                Task::none()
            }
            Msg::Test(kanal) => {
                if let Some(pfad) = test_ton(kanal) {
                    if let Ok(mut kind) = std::process::Command::new("pw-play").arg(&pfad).spawn() {
                        std::thread::spawn(move || {
                            let _ = kind.wait();
                        });
                    }
                }
                Task::none()
            }
            Msg::MikroTest(an) => {
                self.pegel_aktiv.store(an, Ordering::SeqCst);
                if an {
                    // Der Pegel-Faden: pw-record liefert rohe s16-Proben von
                    // der Standard-Quelle; die Spitze je 100 ms wird 0–100.
                    let pegel = Arc::clone(&self.pegel);
                    let aktiv = Arc::clone(&self.pegel_aktiv);
                    std::thread::spawn(move || {
                        use std::io::Read;
                        let Ok(mut kind) = std::process::Command::new("pw-record")
                            .args(["--rate", "8000", "--channels", "1", "--format", "s16", "-"])
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::null())
                            .spawn()
                        else {
                            aktiv.store(false, Ordering::SeqCst);
                            return;
                        };
                        let mut rohr = kind.stdout.take().unwrap();
                        let mut puffer = [0u8; 1600];
                        while aktiv.load(Ordering::SeqCst) {
                            if rohr.read_exact(&mut puffer).is_err() {
                                break;
                            }
                            let mut spitze = 0i32;
                            for paar in puffer.chunks_exact(2) {
                                let wert = i16::from_le_bytes([paar[0], paar[1]]) as i32;
                                spitze = spitze.max(wert.abs());
                            }
                            pegel.store((spitze * 100 / i16::MAX as i32) as u32, Ordering::SeqCst);
                        }
                        let _ = kind.kill();
                        let _ = kind.wait();
                        pegel.store(0, Ordering::SeqCst);
                    });
                }
                Task::none()
            }
            Msg::PegelTick => Task::none(),
        }
    }

    /// Die ▶-Sprungfeder und — während des Mikrofon-Tests — der
    /// Pegel-Puls; alles andere abonniert der Host.
    pub fn abo(&self) -> Subscription<Msg> {
        let mut abos = Vec::new();
        if !self.sprung.is_settled() {
            abos.push(mkw::tick("klaenge-sprung", Duration::from_millis(16)).map(|_| Msg::SprungTick));
        }
        if self.pegel_aktiv.load(Ordering::SeqCst) {
            abos.push(mkw::tick("ton-pegel", Duration::from_millis(50)).map(|_| Msg::PegelTick));
        }
        Subscription::batch(abos)
    }

    pub fn ansicht(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let master_an = self.einstellungen.an("alle");

        // Master-Sektion — die Leitbild-Formular-Grammatik: Karte + Zeile mit
        // Beschreibung unter dem Titel, Schalter rechts.
        let master = mkw::sektion(
            "",
            vec![mkw::zeile_schalter(
                "Systemklänge",
                Some(if master_an {
                    "Aktiv — Ereignisse klingen nach den Schaltern unten"
                } else {
                    "Stummgeschaltet — kein Ereignis klingt"
                }),
                Some(mkw::symbol(
                    if master_an { mkw::symbol::VOLUME_UP } else { mkw::symbol::VOLUME_OFF },
                    mk::font_size::XLARGE,
                    if master_an { p.primary } else { p.on_surface_variant },
                )),
                master_an,
                p,
                Some(Msg::MasterSchalten(!master_an)),
            )],
            p,
        );

        // Ereignis-Sektion: ▶-Hörprobe führend, Beschreibung unter dem Titel
        let mut zeilen: Vec<Element<'_, Msg>> = Vec::new();
        for (i, (schluessel, name, beschreibung)) in EREIGNISSE.iter().enumerate() {
            let an = self.einstellungen.an(schluessel);
            let sprung_groesse = if self.spielt_index == Some(i) {
                mk::font_size::LARGE * self.sprung.value.clamp(0.6, 1.6)
            } else {
                mk::font_size::LARGE
            };
            let probe = iced::widget::button(mkw::symbol::<Msg>(
                mkw::symbol::PLAY_ARROW,
                sprung_groesse,
                if master_an && an { p.primary } else { p.on_surface_variant },
            ))
            .padding(6)
            .on_press(Msg::Probe(i))
            .style(move |_, status| {
                let base = p.surface_container_high;
                let bg = match status {
                    iced::widget::button::Status::Hovered => {
                        Some(color(p.on_surface.over(base, mk::state_layer::HOVER)).into())
                    }
                    iced::widget::button::Status::Pressed => {
                        Some(color(p.on_surface.over(base, mk::state_layer::PRESSED)).into())
                    }
                    _ => None,
                };
                // familien-ausnahme: Ereignis-Zeile mit ▶-Feder — eigener Zeilenkörper
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border { radius: mk::radius::GROSS.into(), ..Default::default() },
                    ..Default::default()
                }
            });
            let z = mkw::zeile_schalter(
                name,
                Some(beschreibung),
                Some(probe.into()),
                an,
                p,
                Some(Msg::Schalten(i, !an)),
            );
            zeilen.push(
                container(z)
                    .style(move |_| container::Style {
                        border: iced::Border::default(),
                        ..Default::default()
                    })
                    .into(),
            );
        }
        let ereignisse = mkw::sektion("EREIGNISSE", zeilen, p);

        // --- R72: AUSGABE — Leitbild- „Ton"-Reiter als Sektionen ---
        let mut aus_zeilen: Vec<Element<'_, Msg>> = Vec::new();
        for g in &self.sinks {
            aus_zeilen.push(geraetezeile(g, p, Msg::AusgabeWahl(g.id)));
        }
        if self.sinks.is_empty() {
            aus_zeilen.push(mkw::zeile_wert("Keine Ausgabegeräte", None, "\u{2014}", p));
        }
        aus_zeilen.push(mkw::zeile(
            "Lautstärke",
            None,
            Some(mkw::symbol(mkw::symbol::VOLUME_UP, mk::font_size::LARGE, p.on_surface_variant)),
            Some(
                mkw::regler(0.0..=1.0, self.aus_vol, 0.01, p, Msg::AusgabeVol)
                    .width(Length::Fixed(220.0))
                    .into(),
            ),
            p,
        ));
        aus_zeilen.push(mkw::zeile(
            "Lautsprecher testen",
            Some("Spielt einen Ton auf dem gewählten Gerät"),
            None,
            Some(
                row![
                    mkw::knopf("Links", mkw::knopfart::Stil::Getoent, mkw::knopfart::Rolle::Normal, mkw::knopfart::Groesse::Klein, p, Some(Msg::Test(0))),
                    Space::new().width(mk::spacing::S),
                    mkw::knopf("Rechts", mkw::knopfart::Stil::Getoent, mkw::knopfart::Rolle::Normal, mkw::knopfart::Groesse::Klein, p, Some(Msg::Test(1))),
                ]
                .into(),
            ),
            p,
        ));
        let ausgabe = mkw::sektion("AUSGABE", aus_zeilen, p);

        // --- R72: EINGABE ---
        let mut ein_zeilen: Vec<Element<'_, Msg>> = Vec::new();
        for g in &self.quellen {
            ein_zeilen.push(geraetezeile(g, p, Msg::EingabeWahl(g.id)));
        }
        if self.quellen.is_empty() {
            ein_zeilen.push(mkw::zeile_wert("Keine Eingabegeräte", None, "\u{2014}", p));
        }
        ein_zeilen.push(mkw::zeile(
            "Eingangslautstärke",
            None,
            Some(mkw::symbol(mkw::symbol::GRAPHIC_EQ, mk::font_size::LARGE, p.on_surface_variant)),
            Some(
                mkw::regler(0.0..=1.0, self.ein_vol, 0.01, p, Msg::EingabeVol)
                    .width(Length::Fixed(220.0))
                    .into(),
            ),
            p,
        ));
        let mikro_an = self.pegel_aktiv.load(Ordering::SeqCst);
        let pegel_balken: Element<'_, Msg> = if mikro_an {
            let wert = self.pegel.load(Ordering::SeqCst).min(100);
            let breite = 220.0;
            container(
                row![
                    container(Space::new().width(Length::Fixed(breite * wert as f32 / 100.0)).height(Length::Fixed(8.0)))
                        .style(move |_| container::Style {
                            background: Some(color(p.primary).into()),
                            border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
                            ..Default::default()
                        }),
                ],
            )
            .width(Length::Fixed(breite))
            .style(move |_| container::Style {
                background: Some(color(p.on_surface.over(p.surface_container_high, 0.10)).into()),
                border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
                ..Default::default()
            })
            .into()
        } else {
            Space::new().width(Length::Fixed(220.0)).into()
        };
        ein_zeilen.push(mkw::zeile_schalter(
            "Mikrofon testen",
            Some(if mikro_an { "Sprich — der Pegel folgt deiner Stimme" } else { "Zeigt den Eingangspegel live" }),
            Some(pegel_balken),
            mikro_an,
            p,
            Some(Msg::MikroTest(!mikro_an)),
        ));
        let eingabe = mkw::sektion("EINGABE", ein_zeilen, p);

        column![
            master,
            Space::new().height(mk::spacing::L),
            ausgabe,
            Space::new().height(mk::spacing::L),
            eingabe,
            Space::new().height(mk::spacing::L),
            ereignisse,
        ]
        .spacing(0)
        .into()
    }
}

/// Eine Geräte-Zeile: ✓ führt beim Standardgerät, die ganze Zeile ist
/// klickbar (Leitbild-Ausgabeliste).
fn geraetezeile<'a>(g: &'a Geraet, p: mk::Palette, on: Msg) -> Element<'a, Msg> {
    let fuehrend = if g.standard {
        mkw::symbol(mkw::symbol::CHECK, mk::font_size::LARGE, p.primary)
    } else {
        mkw::symbol(' ', mk::font_size::LARGE, p.on_surface_variant)
    };
    mouse_area(mkw::zeile(
        g.name.as_str(),
        None,
        Some(fuehrend),
        None,
        p,
    ))
    .on_press(on)
    .interaction(iced::mouse::Interaction::Pointer)
    .into()
}
