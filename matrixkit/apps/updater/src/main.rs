//! Matrix Updater — Softwareupdate im MatrixKit-Stil (App #9).
//!
//! Leitbild-Softwareupdate-Grammatik: „Matrix ist auf dem neuesten Stand" mit
//! Häkchen, oder Update-Karte mit „Was ist neu" und EINEM Knopf. Liest den
//! laufenden Stand aus rpm-ostree, vergleicht mit dem Registry-Digest
//! (anonym via skopeo — das Image ist öffentlich) und aktualisiert über
//! den schmalen Root-Helfer /usr/bin/matrix-update-helfer (sudoers,
//! Wächter-Präzedenzfall). Neustart via logind (aktive Sitzung darf das).

use iced::widget::{column, container, row, Space};
use iced::{Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::time::Duration;

const APP_ID: &str = "matrix-updater";
const REGISTRY_BILD: &str = "ghcr.io/neurosector/matrix:latest";
/// Der Bau-PC im Heimnetz: veröffentlicht Images unter ~/matrix-updates/.
/// Überschreibbar via ~/.config/matrix/update-pc (user@host).
const PC_STANDARD: &str = "benutzer@bau-rechner";
const PC_ARCHIV: &str = "/var/tmp/matrix-pc-update.tar";

fn pc_adresse() -> String {
    mk::einstellung::lesen("update-pc").unwrap_or_else(|| String::from(PC_STANDARD))
}

fn main() -> iced::Result {
    if !mk::fenster::einzelinstanz(APP_ID) {
        return Ok(());
    }
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| String::from("Matrix Updater"))
        .subscription(App::subscription)
        .window(mkw::fenster_settings(APP_ID, 460.0, 560.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

/// Der laufende Stand des Systems (aus rpm-ostree).
#[derive(Debug, Clone, Default)]
struct Stand {
    /// Volle Image-Referenz des Origins (Registry ODER Stick-Archiv).
    origin: String,
    /// Digest des gebooteten Images (sha256:…).
    digest: String,
    /// Zeitstempel des Deployments (Unix; 0 = unbekannt).
    seit_unix: i64,
}

#[derive(Debug, Clone, PartialEq)]
enum Lage {
    Pruefe,
    Aktuell,
    UpdateDa,
    /// Registry nicht lesbar: offline, privat, oder kein skopeo.
    Fehler(String),
}

struct App {
    rahmen: mkw::Rahmen,
    stand: Stand,
    registry_digest: Option<String>,
    /// true = die geprüfte Quelle ist der Heimnetz-PC (SSH), nicht ghcr.io.
    von_pc: bool,
    lage: Lage,
    /// „Was ist neu" — Commit-Titel aus dem öffentlichen Repo.
    neuigkeiten: Vec<String>,
    update_laeuft: bool,
    /// Some(true) = Update fertig, Neustart übernimmt es.
    update_fertig: Option<bool>,
    puls: f32,
}

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Tick,
    PulsTick,
    Taste(mkw::Taste),
    Pruefen,
    Stand(Stand, Result<String, String>, bool),
    Neuigkeiten(Vec<String>),
    Aktualisieren,
    UpdateFertig(bool),
    Neustart,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let app = Self {
            rahmen: mkw::Rahmen::neu(APP_ID, &[mk::rechte::Recht::Netzwerk]),
            stand: Stand::default(),
            registry_digest: None,
            von_pc: false,
            lage: Lage::Pruefe,
            neuigkeiten: Vec::new(),
            update_laeuft: false,
            update_fertig: None,
            puls: 0.0,
        };
        let netz = app.rahmen.rechte.erlaubt(mk::rechte::Recht::Netzwerk);
        (app, pruef_task(netz))
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(m) => return self.rahmen.update(m).map(Msg::Rahmen),
            Msg::Tick => {
                self.rahmen.palette_geaendert();
            }
            Msg::PulsTick => {
                self.puls = (self.puls + 1.0 / 90.0).fract();
            }
            Msg::Taste(t) => {
                if self.rahmen.taste(t) {
                    return Task::none();
                }
                // Strg+R (Leitbild refreshable): frisch prüfen.
                if t == mkw::Taste::Aktualisieren && !self.update_laeuft {
                    return self.update(Msg::Pruefen);
                }
            }
            Msg::Pruefen => {
                self.lage = Lage::Pruefe;
                self.update_fertig = None;
                let netz = self.rahmen.rechte.erlaubt(mk::rechte::Recht::Netzwerk);
                return pruef_task(netz);
            }
            Msg::Stand(stand, registry, von_pc) => {
                self.stand = stand;
                self.von_pc = von_pc;
                match registry {
                    Ok(digest) => {
                        self.lage = if !self.stand.digest.is_empty()
                            && self.stand.digest == digest
                        {
                            Lage::Aktuell
                        } else {
                            Lage::UpdateDa
                        };
                        self.registry_digest = Some(digest);
                    }
                    Err(grund) => self.lage = Lage::Fehler(grund),
                }
            }
            Msg::Neuigkeiten(n) => self.neuigkeiten = n,
            Msg::Aktualisieren => {
                if !self.update_laeuft {
                    self.update_laeuft = true;
                    let von_pc = self.von_pc;
                    return Task::perform(
                        async move {
                            if von_pc {
                                update_vom_pc()
                            } else {
                                update_ausfuehren()
                            }
                        },
                        Msg::UpdateFertig,
                    );
                }
            }
            Msg::UpdateFertig(ok) => {
                self.update_laeuft = false;
                self.update_fertig = Some(ok);
            }
            Msg::Neustart => {
                // logind erlaubt der aktiven Sitzung den Neustart ohne Root.
                let _ = std::process::Command::new("systemctl").arg("reboot").spawn();
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        let mut abos = vec![
            self.rahmen.abo().map(Msg::Rahmen),
            mkw::tick("updater", Duration::from_secs(3)).map(|_| Msg::Tick),
            mkw::tasten_abo(Msg::Taste),
        ];
        if self.lage == Lage::Pruefe || self.update_laeuft {
            abos.push(mkw::tick("updater-puls", Duration::from_millis(16)).map(|_| Msg::PulsTick));
        }
        Subscription::batch(abos)
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;

        // --- Status-Sektion (das Leitbild-Softwareupdate-Panel) ---
        let netz_erlaubt = self.rahmen.rechte.erlaubt(mk::rechte::Recht::Netzwerk);
        let (symbol, farbe, titel, beschreibung): (char, mk::Rgba, String, String) =
            if self.update_laeuft {
                (
                    mkw::symbol::MONITORING,
                    p.primary,
                    String::from("Update wird installiert …"),
                    String::from("Das kann einige Minuten dauern — Fenster darf offen bleiben."),
                )
            } else if self.update_fertig == Some(true) {
                (
                    mkw::symbol::CHECK,
                    p.primary,
                    String::from("Update installiert"),
                    String::from("Beim nächsten Start ist es aktiv."),
                )
            } else if self.update_fertig == Some(false) {
                (
                    mkw::symbol::WARNUNG,
                    p.error,
                    String::from("Update fehlgeschlagen"),
                    String::from("Erneut versuchen — oder Details im Systemprotokoll."),
                )
            } else {
                match &self.lage {
                    Lage::Pruefe => (
                        mkw::symbol::SEARCH,
                        p.on_surface_variant,
                        String::from("Suche nach Updates …"),
                        String::from("Vergleiche mit der Matrix-Quelle."),
                    ),
                    Lage::Aktuell => (
                        mkw::symbol::CHECK,
                        p.primary,
                        String::from("Matrix ist auf dem neuesten Stand"),
                        format!("Stand von {}", self.stand_seit_text()),
                    ),
                    Lage::UpdateDa => (
                        mkw::symbol::ARROW_DOWNWARD,
                        p.primary,
                        String::from("Ein Update ist verfügbar"),
                        String::from("Ein Klick holt es — der Neustart übernimmt es."),
                    ),
                    Lage::Fehler(grund) => (
                        mkw::symbol::WARNUNG,
                        p.on_surface_variant,
                        String::from("Quelle nicht erreichbar"),
                        grund.clone(),
                    ),
                }
            };

        // Dynamische Texte: direkte Row statt mkw::zeile (kein Box::leak).
        let mut status_text = column![mkw::txt(titel, mk::typo::KOPF, p.on_surface)].spacing(2);
        status_text = status_text.push(mkw::txt(beschreibung, mk::typo::KLEIN, p.on_surface_variant));
        let mut status_row = row![
            mkw::symbol::<Msg>(symbol, mk::icon_size::LARGE, farbe),
            container(status_text).width(Length::Fill),
        ]
        .spacing(mk::spacing::M)
        .align_y(iced::Alignment::Center);
        if let Some(k) = self.status_knopf(p, netz_erlaubt) {
            status_row = status_row.push(k);
        }
        let mut status_zeilen: Vec<Element<'_, Msg>> = vec![container(status_row)
            .padding(iced::Padding {
                top: mk::spacing::M,
                right: mk::spacing::M,
                bottom: mk::spacing::M,
                left: mk::spacing::M,
            })
            .into()];
        if self.update_laeuft || self.lage == Lage::Pruefe {
            status_zeilen.push(
                container(mkw::puls(self.puls, p.primary))
                    .padding(iced::Padding {
                        left: mk::spacing::M,
                        right: mk::spacing::M,
                        bottom: mk::spacing::S,
                        ..iced::Padding::ZERO
                    })
                    .into(),
            );
        }

        // --- Dieses System ---
        let digest_kurz = |d: &str| -> String {
            d.strip_prefix("sha256:").unwrap_or(d).chars().take(12).collect()
        };
        let stand_text = format!("{} · {}", digest_kurz(&self.stand.digest), self.stand_seit_text());
        let system_zeilen: Vec<Element<'_, Msg>> = vec![
            mkw::zeile("Quelle", Some(quelle_lesbar(&self.stand.origin)), None, None, p),
            container(
                column![
                    mkw::txt("Stand", mk::typo::FLIESS, p.on_surface),
                    mkw::txt(stand_text, mk::typo::KLEIN, p.on_surface_variant),
                ]
                .spacing(2),
            )
            .padding(iced::Padding {
                top: mk::spacing::S + 2.0,
                right: mk::spacing::M,
                bottom: mk::spacing::S + 2.0,
                left: mk::spacing::M,
            })
            .into(),
        ];

        // --- Was ist neu ---
        let mut inhalt = column![
            mkw::sektion("SOFTWAREUPDATE", status_zeilen, p),
            Space::new().height(mk::spacing::L),
            mkw::sektion("DIESES SYSTEM", system_zeilen, p),
        ]
        .spacing(0);
        if !self.neuigkeiten.is_empty() {
            let neu_zeilen: Vec<Element<'_, Msg>> = self
                .neuigkeiten
                .iter()
                .map(|t| {
                    mkw::zeile(
                        t.as_str(),
                        None,
                        Some(mkw::symbol::<Msg>(
                            mkw::symbol::CHEVRON_RIGHT,
                            mk::font_size::SMALL,
                            p.on_surface_variant,
                        )),
                        None,
                        p,
                    )
                })
                .collect();
            inhalt = inhalt
                .push(Space::new().height(mk::spacing::L))
                .push(mkw::sektion("WAS IST NEU", neu_zeilen, p));
        }

        let karte = container(
            column![
                self.rahmen.scrollflaeche(inhalt.into(), Msg::Rahmen),
                mkw::fusszeile(
                    String::from("Updates kommen als komplettes System-Image — Zurückrollen jederzeit im Startmenü"),
                    p
                ),
            ]
            .spacing(0),
        )
        .padding(mk::spacing::L)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(color(p.surface_container).into()),
            border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
            ..Default::default()
        });

        self.rahmen.fenster(
            "Matrix Updater",
            env!("CARGO_PKG_VERSION"),
            "Softwareupdate für das ganze Betriebssystem — prüfen, holen, neu starten. Ohne Netzwerk-Recht wird die Quelle nie kontaktiert.",
            karte.into(),
            Msg::Rahmen,
        )
    }

    /// „Stand von …": relativ, solange es frisch ist (Leitbild-Kultur);
    /// ab zwei Wochen das absolute Datum.
    fn stand_seit_text(&self) -> String {
        if self.stand.seit_unix == 0 {
            return String::from("unbekannt");
        }
        let jetzt = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(self.stand.seit_unix);
        if jetzt - self.stand.seit_unix < 14 * 86400 {
            mk::zeit::relativ(self.stand.seit_unix, jetzt)
        } else {
            zeit_lesbar(self.stand.seit_unix)
        }
    }

    /// Der EINE Knopf rechts in der Status-Zeile — je nach Lage.
    fn status_knopf(&self, p: mk::Palette, netz: bool) -> Option<Element<'_, Msg>> {
        use mkw::knopfart::*;
        if self.update_laeuft {
            return None;
        }
        if self.update_fertig == Some(true) {
            return Some(
                mkw::knopf("Neu starten", Stil::Prominent, Rolle::Normal, Groesse::Normal, p, Some(Msg::Neustart))
                    .into(),
            );
        }
        let e: Element<'_, Msg> = match &self.lage {
            Lage::UpdateDa => {
                mkw::knopf("Aktualisieren", Stil::Prominent, Rolle::Normal, Groesse::Normal, p, Some(Msg::Aktualisieren)).into()
            }
            Lage::Pruefe => return None,
            _ => mkw::knopf(
                "Erneut suchen",
                Stil::Getoent,
                Rolle::Normal,
                Groesse::Normal,
                p,
                netz.then_some(Msg::Pruefen),
            )
            .into(),
        };
        Some(e)
    }
}

/// Prüf-Auftrag: Stand + Registry-Digest + Neuigkeiten parallel.
fn pruef_task(netz_erlaubt: bool) -> Task<Msg> {
    let stand_task = Task::perform(
        async move {
            let stand = stand_lesen();
            if !netz_erlaubt {
                let e = Err(String::from(
                    "Netzwerk-Berechtigung ist aus (App-Name → Berechtigungen)",
                ));
                return (stand, e, false);
            }
            // Quelle 1: der Bau-PC im Heimnetz (frischer als jede Registry).
            match pc_digest() {
                Ok(d) => (stand, Ok(d), true),
                // Quelle 2: die öffentliche Registry.
                Err(_) => (stand, registry_digest(), false),
            }
        },
        |(s, r, pc)| Msg::Stand(s, r, pc),
    );
    if netz_erlaubt {
        Task::batch([
            stand_task,
            Task::perform(async { neuigkeiten_holen() }, Msg::Neuigkeiten),
        ])
    } else {
        stand_task
    }
}

/// Gebooteten Stand aus rpm-ostree lesen (geht ohne Root).
fn stand_lesen() -> Stand {
    let aus = std::process::Command::new("rpm-ostree")
        .args(["status", "--json"])
        .output();
    let Ok(aus) = aus else { return Stand::default() };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&aus.stdout) else {
        return Stand::default();
    };
    let leer = Vec::new();
    let deployments = json["deployments"].as_array().unwrap_or(&leer);
    let booted = deployments
        .iter()
        .find(|d| d["booted"].as_bool() == Some(true));
    let Some(b) = booted else { return Stand::default() };
    let origin = b["container-image-reference"]
        .as_str()
        .or_else(|| b["origin"].as_str())
        .unwrap_or("?")
        .to_string();
    let digest = b["container-image-reference-digest"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let seit_unix = b["timestamp"].as_i64().unwrap_or(0);
    Stand { origin, digest, seit_unix }
}

fn zeit_lesbar(unix: i64) -> String {
    // Ohne chrono: date übernimmt die Lokalisierung.
    std::process::Command::new("date")
        .args(["-d", &format!("@{unix}"), "+%-d. %B %Y, %H:%M"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("@{unix}"))
}

/// Digest des auf dem Bau-PC veröffentlichten Images (~/matrix-updates/
/// digest.txt, geschrieben von matrix-bau-und-veroeffentlichen.sh).
/// Erreichbar nur im Heimnetz — sonst Err und Rückfall auf die Registry.
fn pc_digest() -> Result<String, String> {
    let aus = std::process::Command::new("ssh")
        .args([
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=4",
            &pc_adresse(),
            "head -1 ~/matrix-updates/digest.txt",
        ])
        .output()
        .map_err(|_| String::from("ssh fehlt"))?;
    if aus.status.success() {
        let d = String::from_utf8_lossy(&aus.stdout).trim().to_string();
        if d.starts_with("sha256:") {
            return Ok(d);
        }
    }
    Err(String::from("PC nicht erreichbar"))
}

/// Update vom PC: Archiv per scp holen, dann rebase (polkit erlaubt
/// rpm-ostree für den Nutzer — kein Root-Helfer nötig).
fn update_vom_pc() -> bool {
    let geholt = std::process::Command::new("scp")
        .args([
            "-o", "BatchMode=yes",
            &format!("{}:matrix-updates/matrix.tar", pc_adresse()),
            PC_ARCHIV,
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !geholt {
        return false;
    }
    std::process::Command::new("rpm-ostree")
        .args([
            "rebase",
            &format!("ostree-unverified-image:oci-archive:{PC_ARCHIV}"),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Digest des neuesten Images — anonym (das Matrix-Image ist öffentlich).
fn registry_digest() -> Result<String, String> {
    let aus = std::process::Command::new("skopeo")
        .args([
            "inspect",
            "--format",
            "{{.Digest}}",
            &format!("docker://{REGISTRY_BILD}"),
        ])
        .output()
        .map_err(|_| String::from("skopeo fehlt auf diesem System"))?;
    if aus.status.success() {
        Ok(String::from_utf8_lossy(&aus.stdout).trim().to_string())
    } else {
        let fehler = String::from_utf8_lossy(&aus.stderr);
        if fehler.contains("unauthorized") || fehler.contains("authentication") {
            Err(String::from("Quelle ist privat — auf GitHub „Public“ schalten"))
        } else {
            Err(String::from("Offline oder Quelle nicht erreichbar"))
        }
    }
}

/// „Was ist neu": Commit-Titel des öffentlichen Repos (best effort).
fn neuigkeiten_holen() -> Vec<String> {
    let aus = std::process::Command::new("curl")
        .args([
            "-sf",
            "-m",
            "8",
            "https://api.github.com/repos/Neurosector/matrix/commits?per_page=8",
        ])
        .output();
    let Ok(aus) = aus else { return Vec::new() };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&aus.stdout) else {
        return Vec::new();
    };
    json.as_array()
        .map(|commits| {
            commits
                .iter()
                .filter_map(|c| c["commit"]["message"].as_str())
                .filter_map(|m| m.lines().next())
                .map(|z| z.chars().take(72).collect())
                .collect()
        })
        .unwrap_or_default()
}

/// Update über den schmalen Root-Helfer (sudoers NOPASSWD, nur dieser).
fn update_ausfuehren() -> bool {
    std::process::Command::new("sudo")
        .args(["-n", "/usr/bin/matrix-update-helfer"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Origin menschenlesbar: Registry, Stick-Archiv oder Unbekanntes.
fn quelle_lesbar(origin: &str) -> &'static str {
    if origin.contains("matrix-pc-update") || origin.contains("matrix-neu.tar") {
        "Vom Bau-PC — Updates kommen aus dem Heimnetz"
    } else if origin.contains("ghcr.io/neurosector/matrix") {
        "Matrix-Quelle (ghcr.io) — Updates kommen automatisch hierher"
    } else if origin.contains("oci-archive") {
        "USB-Stick-Installation — das erste Update stellt auf die Matrix-Quelle um"
    } else {
        "Eigene Quelle"
    }
}
