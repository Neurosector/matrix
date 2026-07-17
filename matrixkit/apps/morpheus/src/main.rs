//! Morpheus — App #26 (R39, R74): der Matrix-Installer. Er ersetzt
//! den Standard-Installer: der geführte Assistent, der Matrix vom Stick auf eine
//! Platte bringt — inklusive Konto-Anlage für den ersten Benutzer.
//!
//! Matrix ist ein bootc-Image-OS; installieren heißt `bootc install
//! to-disk`. Dieser Assistent ist das würdevolle Gesicht davor — im
//! Geist des Leitbild-Installationsassistenten: eine Sache pro Schritt,
//! immer klar, wohin es geht, und eine unübersehbare rote Wand vor dem
//! Punkt ohne Wiederkehr.
//!
//! SICHERHEIT (das Wichtigste an einem Installer):
//! * Das BOOT-MEDIUM (der Stick, von dem wir laufen) wird erkannt und
//!   ist gesperrt — man kann sich nicht selbst überschreiben.
//! * Kein Datenträger ist vorausgewählt; die Wahl ist immer bewusst.
//! * Vor dem Löschen muss der Gerätename ABGETIPPT werden (wie „DELETE
//!   eingeben") — kein Vertippen, kein Reflex-Klick.
//! * Der eigentliche `bootc install` läuft über einen kleinen
//!   Root-Helfer (/usr/libexec/matrix-installiere) via pkexec; die App
//!   selbst fasst nie eine Platte an und tippt nie ein Passwort.
//! * `--demo` spielt den Ablauf ohne jede Plattenberührung durch.

use iced::widget::{button, column, container, row, Space};
use iced::{Alignment, Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::time::Duration;

/// Platten-Zeilen-Symbol: die Token-Skala, kein freies Maß (R63).
const PLATTEN_SYMBOL: f32 = mk::icon_size::NORMAL;

const APP_ID: &str = "matrix-morpheus";
const HELFER: &str = "/usr/libexec/morpheus-installiere";
const LOG: &str = "/tmp/morpheus-install.log";
const FERTIG: &str = "/tmp/morpheus-install.fertig";
const FEHLER: &str = "/tmp/morpheus-install.fehler";
/// Konto-Übergabe an den Root-Helfer (3 Zeilen: Rechner/Nutzer/Hash).
const KONTO_DATEI: &str = "/tmp/morpheus-konto";

fn demo() -> bool {
    std::env::args().any(|a| a == "--demo")
}

fn main() -> iced::Result {
    if !mk::fenster::einzelinstanz(APP_ID) {
        return Ok(());
    }
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| String::from("Morpheus"))
        .subscription(App::subscription)
        .window(mkw::fenster_settings(APP_ID, 720.0, 560.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .font(mkw::mono_font_laden())
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

// ------------------------------------------------------------ Datenträger

#[derive(Debug, Clone)]
struct Disk {
    dev: String,      // /dev/sda
    kurz: String,     // sda
    groesse: u64,     // Bytes
    modell: String,
    entfernbar: bool, // USB-Stick etc.
    live: bool,       // von HIER laufen wir gerade — gesperrt
}

impl Disk {
    fn groesse_text(&self) -> String {
        // R65b: die EINE Zahlensprache des Kits.
        mk::format::bytes(self.groesse)
    }
}

/// Basis-Platte eines Geräts. Idempotent auf ganzen Platten.
/// * sd/vd/hd/xvd: die Partition hängt die Ziffer direkt an (sda2 → sda);
///   die ganze Platte endet auf einen Buchstaben (sda → sda).
/// * nvme/mmcblk/loop/md: die Ziffer gehört zur PLATTE (nvme0n1, mmcblk0);
///   Partitionen trennt ein `p` (nvme0n1p2 → nvme0n1, mmcblk0 → mmcblk0).
fn basis(part: &str) -> String {
    let p = part.trim_start_matches("/dev/");
    let p_marker = ["nvme", "mmcblk", "loop", "md"].iter().any(|k| p.starts_with(k));
    if p_marker {
        if let Some(pos) = p.rfind('p') {
            let vor_ok = p[..pos].chars().last().is_some_and(|c| c.is_ascii_digit());
            let nach = &p[pos + 1..];
            if vor_ok && !nach.is_empty() && nach.chars().all(|c| c.is_ascii_digit()) {
                return p[..pos].to_string();
            }
        }
        return p.to_string(); // ganze nvme/mmc-Platte
    }
    p.trim_end_matches(|c: char| c.is_ascii_digit()).to_string()
}

/// Welche Basis-Platten sind gerade in Benutzung — und dürfen NIE Ziel
/// sein? Live-Medium (`/run/initramfs/live`) UND, auf einem installierten
/// System, die echte Systemplatte: bei bootc/ostree ist `/` nur composefs,
/// das WIRKLICHE Blockgerät hängt unter `/sysroot` und `/boot`. Beide zu
/// prüfen schließt die Lücke, die System-Platte versehentlich anzubieten.
fn live_platten() -> Vec<String> {
    let mut aus = Vec::new();
    for ziel in ["/", "/sysroot", "/boot", "/run/initramfs/live", "/run/media"] {
        if let Some(src) = mk::befehl::erste_zeile("findmnt", &["-n", "-o", "SOURCE", ziel]) {
            let src = src.split_whitespace().next().unwrap_or("").to_string();
            if src.starts_with("/dev/") {
                aus.push(basis(&src));
            }
        }
    }
    aus
}

fn disks_lesen() -> Vec<Disk> {
    let live = live_platten();
    let Some(text) = mk::befehl::text_von("lsblk", &["-dnb", "-o", "NAME,SIZE,TYPE,RM,MODEL"]) else {
        return Vec::new();
    };
    let mut aus = Vec::new();
    for zeile in text.lines() {
        let mut f = zeile.split_whitespace();
        let (Some(name), Some(size), Some(typ), Some(rm)) =
            (f.next(), f.next(), f.next(), f.next())
        else {
            continue;
        };
        if typ != "disk" {
            continue;
        }
        // RAM-Disks, Loops, optische Laufwerke: nie ein Installationsziel
        // (zram0 ist groesser als 1 GB und rutschte durch — PC-Fund).
        if ["zram", "loop", "sr", "md", "fd", "ram"].iter().any(|k| name.starts_with(k)) {
            continue;
        }
        // Der Rest ist das Modell (kann Leerzeichen haben).
        let modell = f.collect::<Vec<_>>().join(" ");
        let groesse: u64 = size.parse().unwrap_or(0);
        // Winzige Pseudo-Geräte (zram, loop, <1 GB) raus.
        if groesse < 1_000_000_000 {
            continue;
        }
        aus.push(Disk {
            dev: format!("/dev/{name}"),
            kurz: name.to_string(),
            groesse,
            modell: if modell.is_empty() { String::from("Datenträger") } else { modell },
            entfernbar: rm == "1",
            live: live.iter().any(|l| l == name),
        });
    }
    aus
}

// ------------------------------------------------------------------- App

/// Was Morpheus erschafft: eine Installation auf diesem Gerät — oder
/// einen Stick, der bootet und Morpheus selbst startet (R76).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Modus {
    Geraet,
    Medium,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Schritt {
    Willkommen,
    Ziel,
    Konto,
    Bestaetigen,
    Laeuft,
    Fertig,
    Fehler,
}

/// Der erste Benutzer der neuen Matrix — Morpheus legt ihn nach dem
/// bootc-Install im Deployment an (den Standard-Installers Kern-Aufgabe).
#[derive(Debug, Clone, Default)]
struct Konto {
    rechner: String,
    nutzer: String,
    pw: String,
    pw2: String,
}

impl Konto {
    fn rechner_ok(&self) -> bool {
        let r = self.rechner.trim();
        !r.is_empty()
            && r.len() <= 63
            && r.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !r.starts_with('-')
    }
    fn nutzer_ok(&self) -> bool {
        let n = self.nutzer.trim();
        let mut z = n.chars();
        matches!(z.next(), Some(c) if c.is_ascii_lowercase())
            && n.len() <= 32
            && z.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }
    fn pw_ok(&self) -> bool {
        self.pw.chars().count() >= 4 && self.pw == self.pw2
    }
    fn ok(&self) -> bool {
        self.rechner_ok() && self.nutzer_ok() && self.pw_ok()
    }
}

struct App {
    rahmen: mkw::Rahmen,
    schritt: Schritt,
    modus: Modus,
    disks: Vec<Disk>,
    wahl: Option<usize>,
    konto: Konto,
    tipp: String,
    log: String,
    demo: bool,
}

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Tick,
    Taste(mkw::Taste),
    NeuLesen,
    ModusWeiter(Modus),
    Schliessen,
    DiskWahl(usize),
    Weiter,
    Zurueck,
    TippInput(String),
    KontoRechner(String),
    KontoNutzer(String),
    KontoPw(String),
    KontoPw2(String),
    Installieren,
    Neustarten,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        // Dev-Haken für Screenshots: MORPHEUS_SCHRITT=konto startet dort.
        let start = match std::env::var("MORPHEUS_SCHRITT").as_deref() {
            Ok("konto") => Schritt::Konto,
            _ => Schritt::Willkommen,
        };
        let app = Self {
            rahmen: mkw::Rahmen::neu(APP_ID, &[]),
            schritt: start,
            modus: Modus::Geraet,
            disks: disks_lesen(),
            wahl: None,
            konto: Konto {
                rechner: String::from("matrix"),
                ..Konto::default()
            },
            tipp: String::new(),
            log: String::new(),
            demo: demo(),
        };
        (app, Task::none())
    }

    /// Die gewählte Platte, falls sie noch gültig (und nicht Live) ist.
    fn ziel(&self) -> Option<&Disk> {
        self.wahl
            .and_then(|i| self.disks.get(i))
            .filter(|d| !d.live)
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(m) => return self.rahmen.update(m).map(Msg::Rahmen),
            Msg::Tick => {
                self.rahmen.palette_geaendert();
                if self.schritt == Schritt::Laeuft {
                    self.log = std::fs::read_to_string(LOG).unwrap_or_default();
                    if std::path::Path::new(FERTIG).exists() {
                        self.schritt = Schritt::Fertig;
                        mk::feedback::erfolg();
                    } else if std::path::Path::new(FEHLER).exists() {
                        self.schritt = Schritt::Fehler;
                        mk::feedback::fehler();
                    }
                }
            }
            Msg::Taste(t) => {
                if self.rahmen.taste(t) {
                    return Task::none();
                }
            }
            Msg::ModusWeiter(m) => {
                self.modus = m;
                self.disks = disks_lesen();
                self.schritt = Schritt::Ziel;
            }
            Msg::Schliessen => {
                std::process::exit(0);
            }
            Msg::NeuLesen => {
                self.disks = disks_lesen();
                if self.ziel().is_none() {
                    self.wahl = None;
                }
            }
            Msg::DiskWahl(i) => {
                if self.disks.get(i).is_some_and(|d| !d.live) {
                    self.wahl = Some(i);
                }
            }
            Msg::Weiter => {
                self.schritt = match self.schritt {
                    Schritt::Willkommen => {
                        self.disks = disks_lesen();
                        Schritt::Ziel
                    }
                    Schritt::Ziel if self.ziel().is_some() => {
                        if self.modus == Modus::Medium {
                            self.tipp.clear();
                            Schritt::Bestaetigen
                        } else {
                            Schritt::Konto
                        }
                    }
                    Schritt::Konto if self.konto.ok() => {
                        self.tipp.clear();
                        Schritt::Bestaetigen
                    }
                    s => s,
                };
            }
            Msg::Zurueck => {
                self.schritt = match self.schritt {
                    Schritt::Ziel => Schritt::Willkommen,
                    Schritt::Konto => Schritt::Ziel,
                    Schritt::Bestaetigen if self.modus == Modus::Medium => Schritt::Ziel,
                    Schritt::Bestaetigen => Schritt::Konto,
                    s => s,
                };
            }
            Msg::TippInput(s) => self.tipp = s,
            Msg::KontoRechner(s) => self.konto.rechner = s,
            Msg::KontoNutzer(s) => self.konto.nutzer = s,
            Msg::KontoPw(s) => self.konto.pw = s,
            Msg::KontoPw2(s) => self.konto.pw2 = s,
            Msg::Installieren => {
                // Nur wenn Ziel gültig UND der Gerätename exakt getippt ist.
                if let Some(d) = self.ziel() {
                    if self.tipp.trim() == d.kurz {
                        let dev = d.dev.clone();
                        let demo = self.demo;
                        let konto = self.konto.clone();
                        let medium = self.modus == Modus::Medium;
                        self.log.clear();
                        self.schritt = Schritt::Laeuft;
                        std::thread::spawn(move || installieren(&dev, demo, &konto, medium));
                    }
                }
            }
            Msg::Neustarten => {
                mk::befehl::still("systemctl", &["reboot"]);
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        let takt = if self.schritt == Schritt::Laeuft { 400 } else { 1500 };
        Subscription::batch([
            self.rahmen.abo().map(Msg::Rahmen),
            mkw::tick("installer", Duration::from_millis(takt)).map(|_| Msg::Tick),
            mkw::tasten_abo(Msg::Taste),
        ])
    }

    // ---------------------------------------------------------- Ansichten

    /// Protokoll-Ausschnitt in der Konsolen-Familie (R65: mkw::konsole).
    fn konsole(&self, zeilen: usize, breite: f32) -> Element<'_, Msg> {
        let text: String = self
            .log
            .lines()
            .rev()
            .take(zeilen)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        mkw::konsole(
            if text.is_empty() { String::from("\u{2026}") } else { text },
            breite,
            self.rahmen.palette,
        )
    }

    /// Der Assistenten-Kompass: vier Punkte, der aktuelle ist die
    /// Primary-Kapsel — die stille Weganzeige der Leitbild-Assistenten.
    fn schritt_punkte(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let medium = self.modus == Modus::Medium;
        let gesamt = if medium { 4 } else { 5 };
        let aktiv = match self.schritt {
            Schritt::Willkommen => 0usize,
            Schritt::Ziel => 1,
            Schritt::Konto => 2,
            Schritt::Bestaetigen => {
                if medium { 2 } else { 3 }
            }
            Schritt::Laeuft | Schritt::Fertig | Schritt::Fehler => gesamt - 1,
        };
        let mut punkte = row![].spacing(mk::spacing::XS).align_y(Alignment::Center);
        for i in 0..gesamt {
            let (b, farbe) = if i == aktiv {
                (18.0, p.primary)
            } else {
                (6.0, p.outline.mit_alpha(0.5))
            };
            punkte = punkte.push(
                container(Space::new().width(Length::Fixed(b)).height(Length::Fixed(6.0)))
                    .style(move |_| container::Style {
                        background: Some(color(farbe).into()),
                        border: iced::Border {
                            radius: mk::radius::kapsel(6.0).into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            );
        }
        container(punkte).center_x(Length::Fill).into()
    }

    fn kopf<'a>(&self, glyph: char, titel: &'a str, unter: &'a str) -> Element<'a, Msg> {
        let p = self.rahmen.palette;
        column![
            mkw::symbol::<Msg>(glyph, mk::icon_size::HERO, p.primary),
            Space::new().height(mk::spacing::S),
            mkw::txt(titel, mk::typo::TITEL, p.on_surface),
            Space::new().height(mk::spacing::XXS),
            mkw::txt(unter, mk::typo::FLIESS, p.on_surface_variant),
        ]
        .align_x(Alignment::Center)
        .into()
    }

    fn disk_zeile(&self, i: usize) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let d = &self.disks[i];
        let gewaehlt = self.wahl == Some(i);
        let (rand, ring) = if d.live {
            (p.outline, p.on_surface_variant)
        } else if gewaehlt {
            (p.primary, p.primary)
        } else {
            (p.outline.over(p.surface_container_high, 0.4), p.on_surface_variant)
        };
        let glyph = if d.entfernbar { mkw::symbol::USB } else { mkw::symbol::STORAGE };
        let mut zeile = row![
            mkw::symbol::<Msg>(glyph, PLATTEN_SYMBOL, if d.live { p.on_surface_variant } else { p.on_surface }),
            column![
                mkw::txt(&d.modell, mk::typo::KOPF, if d.live { p.on_surface_variant } else { p.on_surface }),
                mkw::txt(
                    format!("{} · {} · {}", d.dev, d.groesse_text(), if d.entfernbar { "Wechselmedium" } else { "Festplatte" }),
                    mk::typo::KLEIN,
                    p.on_surface_variant,
                ),
            ]
            .spacing(1),
            Space::new().width(Length::Fill),
        ]
        .spacing(mk::spacing::M)
        .align_y(Alignment::Center);

        if d.live {
            // Status-Kapsel-Familie (R65).
            zeile = zeile.push(mkw::status_kapsel("Boot-Medium \u{2014} gesperrt", p));
        } else {
            zeile = zeile.push(mkw::symbol::<Msg>(
                if gewaehlt { mkw::symbol::CHECK } else { mkw::symbol::ADD },
                mk::icon_size::MEDIUM,
                ring,
            ));
        }

        if d.live {
            container(zeile)
                .width(Length::Fill)
                .padding(mk::spacing::M)
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container.over(p.surface, 0.5)).into()),
                    border: iced::Border { radius: mk::radius::NORMAL.into(), width: 1.0, color: color(rand) },
                    ..Default::default()
                })
                .into()
        } else {
            // R63c (PC-Screenshot): KEINE Lupe auf fensterbreiten Karten —
            // die 1,07-fache Skalierung sprengt die Liste. Die Hover-
            // Sprache einer Karte ist der Farbton (state_layer), wie in
            // den Formular-Zeilen; die Karte wandert dafuer in den Knopf.
            // familien-ausnahme: Datenträger-Karte — Karte + Hover-Ton in EINEM Knopf-Stil
            button(container(zeile).width(Length::Fill).padding(mk::spacing::M))
                .padding(0)
                .width(Length::Fill)
                .style(move |_, status| {
                    let base = if gewaehlt {
                        p.primary.over(p.surface_container_high, 0.14)
                    } else {
                        p.surface_container.over(p.surface, 0.5)
                    };
                    let bg = match status {
                        iced::widget::button::Status::Hovered => {
                            p.on_surface.over(base, mk::state_layer::HOVER)
                        }
                        iced::widget::button::Status::Pressed => {
                            p.on_surface.over(base, mk::state_layer::PRESSED)
                        }
                        _ => base,
                    };
                    // familien-ausnahme: Datenträger-Karte — Karte + Hover-Ton in EINEM Knopf-Stil (R63c)
                    iced::widget::button::Style {
                        background: Some(color(bg).into()),
                        text_color: color(p.on_surface),
                        border: iced::Border {
                            radius: mk::radius::NORMAL.into(),
                            width: 1.0,
                            color: color(rand),
                        },
                        ..Default::default()
                    }
                })
                .on_press(Msg::DiskWahl(i))
                .into()
        }
    }

    fn inhalt(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        match self.schritt {
            Schritt::Willkommen => {
                // Zwei Wege (R76): dieses Gerät — oder ein Stick, der
                // bootet und selbst in Morpheus startet.
                let weg = |zeichen: char, titel: &'static str, unter: &'static str, m: Modus| {
                    button(
                        container(
                            row![
                                mkw::symbol::<Msg>(zeichen, mk::icon_size::LARGE, p.primary),
                                column![
                                    mkw::txt(titel, mk::typo::KOPF, p.on_surface),
                                    mkw::txt(unter, mk::typo::KLEIN, p.on_surface_variant),
                                ]
                                .spacing(1),
                            ]
                            .spacing(mk::spacing::M)
                            .align_y(Alignment::Center),
                        )
                        .width(Length::Fill)
                        .padding(mk::spacing::M),
                    )
                    .padding(0)
                    .width(Length::Fixed(440.0))
                    .style(move |_, status| {
                        let base = p.surface_container.over(p.surface, 0.5);
                        let bg = match status {
                            iced::widget::button::Status::Hovered => {
                                p.on_surface.over(base, mk::state_layer::HOVER)
                            }
                            iced::widget::button::Status::Pressed => {
                                p.on_surface.over(base, mk::state_layer::PRESSED)
                            }
                            _ => base,
                        };
                        // familien-ausnahme: Weg-Karte des Installers — Karte + Hover-Ton in EINEM Knopf-Stil (wie die Datenträger-Karte)
                        iced::widget::button::Style {
                            background: Some(color(bg).into()),
                            text_color: color(p.on_surface),
                            border: iced::Border {
                                radius: mk::radius::NORMAL.into(),
                                width: 1.0,
                                color: color(p.outline.over(p.surface_container_high, 0.4)),
                            },
                            ..Default::default()
                        }
                    })
                    .on_press(Msg::ModusWeiter(m))
                };
                column![
                    Space::new().height(Length::Fill),
                    self.kopf(
                        mkw::symbol::ROCKET_LAUNCH,
                        "Morpheus",
                        "Der Matrix-Installer — bringt Matrix auf einen Datenträger.",
                    ),
                    Space::new().height(mk::spacing::L),
                    weg(
                        mkw::symbol::STORAGE,
                        "Matrix auf diesem Gerät installieren",
                        "Geführte Installation mit Konto-Anlage auf eine Platte",
                        Modus::Geraet,
                    ),
                    Space::new().height(mk::spacing::S),
                    weg(
                        mkw::symbol::USB,
                        "Installations-Stick erstellen",
                        "Ein USB-Stick, der bootet und direkt Morpheus startet",
                        Modus::Medium,
                    ),
                    Space::new().height(mk::spacing::M),
                    container(mkw::txt(
                        if self.demo {
                            "Vorführmodus — es wird keine Platte berührt."
                        } else {
                            "Das Boot-Medium, von dem du gerade läufst, bleibt gesperrt."
                        },
                        mk::typo::KLEIN,
                        p.on_surface_variant,
                    ))
                    .max_width(440),
                    Space::new().height(Length::Fill),
                ]
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .into()
            }

            Schritt::Ziel => {
                let mut liste = column![
                    mkw::txt(
                        if self.modus == Modus::Medium {
                            "USB-Stick für das Installations-Medium wählen"
                        } else {
                            "Ziel-Datenträger wählen"
                        },
                        mk::typo::UNTERTITEL,
                        p.on_surface,
                    ),
                    mkw::txt(
                        "Alle Daten auf dem gewählten Datenträger werden gelöscht.",
                        mk::typo::KLEIN,
                        p.on_surface_variant,
                    ),
                    Space::new().height(mk::spacing::S),
                ]
                .spacing(mk::spacing::XS);
                if self.disks.is_empty() {
                    liste = liste.push(mkw::txt(
                        "Kein Datenträger gefunden.",
                        mk::typo::FLIESS,
                        p.on_surface_variant,
                    ));
                }
                for i in 0..self.disks.len() {
                    liste = liste.push(self.disk_zeile(i));
                }
                self.rahmen.scrollflaeche(liste.into(), Msg::Rahmen)
            }

            Schritt::Konto => {
                let k = &self.konto;
                let hinweis = |ok: bool, text: &'static str| -> Element<'_, Msg> {
                    mkw::txt(
                        text,
                        mk::typo::KLEIN,
                        if ok { p.on_surface_variant } else { p.error },
                    )
                    .into()
                };
                let feld = |platzhalter: &'static str,
                            wert: &str,
                            auf: fn(String) -> Msg,
                            geheim: bool| {
                    container(mkw::eingabefeld(platzhalter, wert, auf, None, geheim, p))
                        .max_width(360)
                };
                column![
                    Space::new().height(mk::spacing::M),
                    self.kopf(
                        mkw::symbol::PERSON,
                        "Dein Konto",
                        "Der erste Benutzer der neuen Matrix — mit Administrator-Rechten.",
                    ),
                    Space::new().height(mk::spacing::L),
                    feld("Rechnername", &k.rechner, Msg::KontoRechner, false),
                    hinweis(
                        k.rechner.is_empty() || k.rechner_ok(),
                        "Buchstaben, Ziffern und Bindestrich",
                    ),
                    Space::new().height(mk::spacing::S),
                    feld("Benutzername", &k.nutzer, Msg::KontoNutzer, false),
                    hinweis(
                        k.nutzer.is_empty() || k.nutzer_ok(),
                        "Kleinbuchstabe am Anfang, dann a\u{2013}z, 0\u{2013}9, Bindestrich",
                    ),
                    Space::new().height(mk::spacing::S),
                    feld("Passwort", &k.pw, Msg::KontoPw, true),
                    feld("Passwort wiederholen", &k.pw2, Msg::KontoPw2, true),
                    hinweis(
                        k.pw2.is_empty() || k.pw_ok(),
                        "Mindestens 4 Zeichen, beide Eingaben gleich",
                    ),
                ]
                .spacing(mk::spacing::XS)
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .into()
            }

            Schritt::Bestaetigen => {
                let d = self.ziel();
                let (dev, kurz, modell, groesse) = match d {
                    Some(d) => (d.dev.clone(), d.kurz.clone(), d.modell.clone(), d.groesse_text()),
                    None => (String::new(), String::new(), String::new(), String::new()),
                };
                column![
                    Space::new().height(mk::spacing::M),
                    mkw::karte_farbig::<Msg>(
                        column![
                            mkw::symbol::<Msg>(mkw::symbol::WARNUNG, mk::icon_size::XLARGE, p.error),
                            Space::new().height(mk::spacing::S),
                            mkw::txt("Alle Daten werden gelöscht", mk::typo::UNTERTITEL, p.error),
                            Space::new().height(mk::spacing::XXS),
                            mkw::txt(
                                format!("{modell} · {dev} · {groesse}"),
                                mk::typo::FLIESS,
                                p.on_surface,
                            ),
                            mkw::txt(
                                if self.modus == Modus::Medium {
                                    String::from("Der Stick bootet danach direkt in Morpheus.")
                                } else {
                                    format!(
                                        "Konto: {} auf \u{201e}{}\u{201c}",
                                        self.konto.nutzer.trim(),
                                        self.konto.rechner.trim()
                                    )
                                },
                                mk::typo::KLEIN,
                                p.on_surface_variant,
                            ),
                            mkw::txt(
                                "Diese Aktion kann nicht rückgängig gemacht werden. Es gibt kein Zurück.",
                                mk::typo::KLEIN,
                                p.on_surface_variant,
                            ),
                        ]
                        .align_x(Alignment::Center)
                        .spacing(0)
                        .width(Length::Fill)
                        .into(),
                        p.error,
                        p,
                    ),
                    Space::new().height(mk::spacing::L),
                    row![
                        mkw::txt(
                            "Tippe zum Bestätigen den Gerätenamen ein:",
                            mk::typo::KLEIN,
                            p.on_surface_variant,
                        ),
                        mkw::code_chip(kurz.clone(), p),
                    ]
                    .spacing(mk::spacing::XS)
                    .align_y(Alignment::Center),
                    Space::new().height(mk::spacing::XS),
                    container(mkw::eingabefeld(&kurz, &self.tipp, Msg::TippInput, None, false, p))
                        .max_width(240),
                ]
                .align_x(Alignment::Center)
                .width(Length::Fill)
                .into()
            }

            Schritt::Laeuft => column![
                Space::new().height(Length::Fill),
                self.kopf(
                    mkw::symbol::STORAGE,
                    "Matrix wird installiert …",
                    "Bitte das Gerät nicht ausschalten.",
                ),
                Space::new().height(mk::spacing::M),
                self.konsole(6, 520.0),
                Space::new().height(Length::Fill),
            ]
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .into(),

            Schritt::Fertig => column![
                Space::new().height(Length::Fill),
                if self.modus == Modus::Medium {
                    self.kopf(
                        mkw::symbol::CHECK,
                        "Der Stick ist bereit",
                        "Boote einen Rechner davon — Morpheus startet automatisch.",
                    )
                } else {
                    self.kopf(
                        mkw::symbol::CHECK,
                        "Willkommen in der Matrix",
                        "Entferne das Boot-Medium und starte neu — dein Konto wartet am Login.",
                    )
                },
                Space::new().height(Length::Fill),
            ]
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .into(),

            Schritt::Fehler => column![
                Space::new().height(Length::Fill),
                self.kopf(
                    mkw::symbol::WARNUNG,
                    "Installation fehlgeschlagen",
                    "Es wurde nichts installiert. Details im Protokoll unten.",
                ),
                Space::new().height(mk::spacing::M),
                self.konsole(8, 560.0),
                Space::new().height(Length::Fill),
            ]
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .into(),
        }
    }

    fn fussleiste(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let knopf = |label: &'static str, stil, rolle, on: Option<Msg>| {
            mkw::knopf(label, stil, rolle, mkw::knopfart::Groesse::Normal, p, on)
        };
        use mkw::knopfart::{Rolle, Stil};
        let reihe = match self.schritt {
            Schritt::Willkommen => row![Space::new().width(Length::Fill)],
            Schritt::Ziel => row![
                knopf("Zurück", Stil::Randlos, Rolle::Normal, Some(Msg::Zurueck)),
                mkw::ui::werkzeug_knopf(mkw::symbol::RESTART, Some(Msg::NeuLesen), p),
                Space::new().width(Length::Fill),
                knopf(
                    "Weiter",
                    Stil::Prominent,
                    Rolle::Normal,
                    self.ziel().is_some().then_some(Msg::Weiter),
                ),
            ],
            Schritt::Konto => row![
                knopf("Zurück", Stil::Randlos, Rolle::Normal, Some(Msg::Zurueck)),
                Space::new().width(Length::Fill),
                knopf(
                    "Weiter",
                    Stil::Prominent,
                    Rolle::Normal,
                    self.konto.ok().then_some(Msg::Weiter),
                ),
            ],
            Schritt::Bestaetigen => {
                let bereit = self.ziel().is_some_and(|d| self.tipp.trim() == d.kurz);
                row![
                    knopf("Zurück", Stil::Randlos, Rolle::Normal, Some(Msg::Zurueck)),
                    Space::new().width(Length::Fill),
                    knopf(
                        if self.modus == Modus::Medium {
                            "Löschen und Stick erstellen"
                        } else {
                            "Löschen und installieren"
                        },
                        Stil::Prominent,
                        Rolle::Destruktiv,
                        bereit.then_some(Msg::Installieren),
                    ),
                ]
            }
            Schritt::Laeuft => row![Space::new().width(Length::Fill)],
            Schritt::Fertig if self.modus == Modus::Medium => row![
                Space::new().width(Length::Fill),
                knopf("Schließen", Stil::Prominent, Rolle::Normal, Some(Msg::Schliessen)),
            ],
            Schritt::Fertig => row![
                Space::new().width(Length::Fill),
                knopf("Jetzt neu starten", Stil::Prominent, Rolle::Normal, Some(Msg::Neustarten)),
            ],
            Schritt::Fehler => row![
                knopf("Zurück zur Auswahl", Stil::Getoent, Rolle::Normal, Some(Msg::Zurueck)),
                Space::new().width(Length::Fill),
            ],
        };
        reihe.spacing(mk::spacing::S).align_y(Alignment::Center).into()
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let karte = container(
            column![
                self.schritt_punkte(),
                container(self.inhalt()).width(Length::Fill).height(Length::Fill),
                self.fussleiste(),
            ]
            .spacing(mk::spacing::M),
        )
        .padding(mk::spacing::L)
        .width(Length::Fill)
        .height(Length::Fill);

        let _ = p;
        self.rahmen.huelle("Morpheus", karte.into(), None, Msg::Rahmen)
    }
}

// ------------------------------------------------- Installation ausführen

/// Startet den Root-Helfer (echt) bzw. spielt den Ablauf vor (Demo).
/// Schreibt LOG laufend und berührt am Ende FERTIG oder FEHLER.
fn installieren(dev: &str, demo: bool, konto: &Konto, medium: bool) {
    let _ = std::fs::remove_file(FERTIG);
    let _ = std::fs::remove_file(FEHLER);

    if demo {
        for zeile in [
            "Vorführmodus: es wird keine Platte berührt.",
            &format!("Ziel: {dev}"),
            "Partitionen anlegen …",
            "Dateisystem schreiben …",
            "Abbild entpacken …",
            "Bootloader einrichten …",
            &(if medium {
                String::from("Installations-Medium markieren …")
            } else {
                format!("Konto {} auf \u{201e}{}\u{201c} anlegen …", konto.nutzer, konto.rechner)
            }),
            "Fertig.",
        ] {
            let vorher = std::fs::read_to_string(LOG).unwrap_or_default();
            let _ = std::fs::write(LOG, format!("{vorher}{zeile}\n"));
            std::thread::sleep(Duration::from_millis(700));
        }
        let _ = std::fs::write(FERTIG, "1");
        return;
    }

    // Konto-Übergabe: Hash statt Klartext (sha512crypt via openssl,
    // Passwort über stdin — nie in einer Prozessliste), Datei nur für
    // Besitzer lesbar; der Helfer löscht sie nach Gebrauch.
    if medium {
        // Medium: kein Konto — der Helfer markiert den Stick als
        // Installations-Medium (bootet direkt in Morpheus).
        let status = std::process::Command::new("pkexec")
            .args([HELFER, dev, "--medium"])
            .status();
        match status {
            Ok(s) if s.success() => {
                let _ = std::fs::write(FERTIG, "1");
            }
            _ => {
                let vorher = std::fs::read_to_string(LOG).unwrap_or_default();
                let _ = std::fs::write(LOG, format!("{vorher}\nErstellung abgebrochen.\n"));
                let _ = std::fs::write(FEHLER, "1");
            }
        }
        return;
    }

    let hash = (|| -> Option<String> {
        use std::io::Write;
        let mut kind = std::process::Command::new("openssl")
            .args(["passwd", "-6", "-stdin"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .ok()?;
        kind.stdin.take()?.write_all(konto.pw.as_bytes()).ok()?;
        let aus = kind.wait_with_output().ok()?;
        let h = String::from_utf8_lossy(&aus.stdout).trim().to_string();
        h.starts_with("$6$").then_some(h)
    })();
    match hash {
        Some(h) => {
            let inhalt = format!("{}\n{}\n{}\n", konto.rechner.trim(), konto.nutzer.trim(), h);
            let _ = std::fs::write(KONTO_DATEI, inhalt);
            let _ = std::process::Command::new("chmod").args(["600", KONTO_DATEI]).status();
        }
        None => {
            let vorher = std::fs::read_to_string(LOG).unwrap_or_default();
            let _ = std::fs::write(
                LOG,
                format!("{vorher}\nPasswort-Hash fehlgeschlagen (openssl fehlt?).\n"),
            );
            let _ = std::fs::write(FEHLER, "1");
            return;
        }
    }

    // Echt: der Helfer prüft das Gerät selbst noch einmal und ruft bootc.
    // pkexec fragt grafisch nach Berechtigung; Passwörter tippt der Mensch.
    let status = std::process::Command::new("pkexec")
        .args([HELFER, dev])
        .status();
    let _ = std::fs::remove_file(KONTO_DATEI);
    match status {
        Ok(s) if s.success() => {
            let _ = std::fs::write(FERTIG, "1");
        }
        _ => {
            let vorher = std::fs::read_to_string(LOG).unwrap_or_default();
            let _ = std::fs::write(LOG, format!("{vorher}\nInstallation abgebrochen.\n"));
            let _ = std::fs::write(FEHLER, "1");
        }
    }
}

// ------------------------------------------------------------------ Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basis_platte_stripped_partitionen() {
        assert_eq!(basis("/dev/sda2"), "sda");
        assert_eq!(basis("/dev/sda"), "sda");
        assert_eq!(basis("/dev/nvme0n1p3"), "nvme0n1");
        assert_eq!(basis("/dev/mmcblk0p1"), "mmcblk0");
        assert_eq!(basis("nvme0n1"), "nvme0n1");
    }

    #[test]
    fn groesse_menschlich() {
        let d = |g| Disk {
            dev: "/dev/x".into(),
            kurz: "x".into(),
            groesse: g,
            modell: "M".into(),
            entfernbar: false,
            live: false,
        };
        assert_eq!(d(512_000_000_000).groesse_text(), "512 GB");
        assert_eq!(d(1_000_000_000_000).groesse_text(), "1 TB");
    }
}
