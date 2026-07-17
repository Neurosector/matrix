//! Developer Access — als Panel der Matrix Einstellungen (Fusion R41b).
//! War App #23 (matrix-entwicklerzugang): SSH-Zugang, Netz, Update-Quelle,
//! polkit-Freigabe, mobile Bau-Umgebung. Privilegiertes läuft über pkexec.

use iced::widget::{column, container, row, Space};
use iced::{Element, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;

const PC_STANDARD: &str = "benutzer@bau-rechner";

/// Momentaufnahme des Zugangs — einmal (async) erhoben, nicht blockierend.
#[derive(Debug, Clone, Default)]
pub struct Status {
    rechner: String,
    heim_ip: Option<String>,
    hotspot_ip: Option<String>,
    ssh_server: bool,
    firewall_ssh: Option<bool>,
    schluessel_kurz: Option<String>,
    schluessel_voll: Option<String>,
    polkit_frei: bool,
    bau_umgebung: bool,
    quellen: bool,
}

impl Status {
    fn netz_text(&self) -> String {
        match (&self.heim_ip, &self.hotspot_ip) {
            (Some(ip), _) => format!("Heimnetz · {ip}"),
            (None, Some(ip)) => format!("Handy-Hotspot · {ip}"),
            (None, None) => String::from("Offline"),
        }
    }
}

pub struct Panel {
    pub palette: mk::Palette,
    status: Status,
    laedt: bool,
    update_pc: String,
    partner: String,
    neuer_schluessel: String,
    /// Ergebnis-Banner: (Erfolg?, Text).
    meldung: Option<(bool, String)>,
    /// Erreichbarkeit der Gegenstelle nach „Testen".
    pc_erreichbar: Option<bool>,
    busy: bool,
    /// abgeleitete Anzeigetexte (leben so lange wie App — zeile_wert borgt &str).
    netz_text: String,
    fp_text: String,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Neuladen,
    Geladen(Status),
    UpdatePcInput(String),
    PartnerInput(String),
    Speichern,
    SchluesselInput(String),
    SchluesselErlauben,
    SchluesselKopieren,
    SshServer(bool),
    FreigabeInstallieren,
    GegenstelleTesten,
    GegenstelleGetestet(bool),
    Fertig(bool, String, bool),
}

impl Panel {
    pub fn new() -> (Self, Task<Msg>) {
        let app = Self {
            palette: mk::Palette::load().unwrap_or_default(),
            status: Status::default(),
            laedt: true,
            update_pc: mk::einstellung::lesen("update-pc")
                .unwrap_or_else(|| String::from(PC_STANDARD)),
            partner: mk::einstellung::lesen("dev-partner").unwrap_or_default(),
            neuer_schluessel: String::new(),
            meldung: None,
            pc_erreichbar: None,
            busy: false,
            netz_text: String::from("…"),
            fp_text: String::from("…"),
        };
        (app, Task::perform(async { status_erheben() }, Msg::Geladen))
    }

    /// Vom Host je Tick: Palette folgt (Meldung bleibt stehen).
    pub fn tick(&mut self, p: mk::Palette) {
        self.palette = p;
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Neuladen => {
                self.laedt = true;
                self.pc_erreichbar = None;
                return Task::perform(async { status_erheben() }, Msg::Geladen);
            }
            Msg::Geladen(s) => {
                self.netz_text = s.netz_text();
                self.fp_text = s
                    .schluessel_kurz
                    .clone()
                    .unwrap_or_else(|| String::from("kein Schlüssel"));
                self.status = s;
                self.laedt = false;
            }
            Msg::UpdatePcInput(v) => self.update_pc = v,
            Msg::PartnerInput(v) => self.partner = v,
            Msg::Speichern => {
                mk::einstellung::schreiben("update-pc", self.update_pc.trim());
                mk::einstellung::schreiben("dev-partner", self.partner.trim());
                self.meldung = Some((true, String::from("Gegenstellen gespeichert.")));
            }
            Msg::SchluesselInput(v) => self.neuer_schluessel = v,
            Msg::SchluesselErlauben => {
                let key = self.neuer_schluessel.trim().to_string();
                if !key.starts_with("ssh-") {
                    self.meldung =
                        Some((false, String::from("Das sieht nicht nach einem SSH-Schlüssel aus.")));
                    return Task::none();
                }
                self.neuer_schluessel.clear();
                return Task::perform(
                    async move { schluessel_erlauben(&key) },
                    |ok| {
                        Msg::Fertig(
                            ok,
                            if ok {
                                String::from("Schlüssel erlaubt — dieses Gerät ist jetzt erreichbar.")
                            } else {
                                String::from("Konnte den Schlüssel nicht hinterlegen.")
                            },
                            true,
                        )
                    },
                );
            }
            Msg::SchluesselKopieren => {
                if let Some(k) = self.status.schluessel_voll.clone() {
                    return Task::perform(async move { in_zwischenablage(&k) }, |ok| {
                        Msg::Fertig(
                            ok,
                            if ok {
                                String::from("Schlüssel in die Zwischenablage kopiert.")
                            } else {
                                String::from("wl-copy fehlt — Schlüssel steht oben zum Abtippen.")
                            },
                            false,
                        )
                    });
                }
            }
            Msg::SshServer(an) => {
                self.busy = true;
                return Task::perform(async move { ssh_server_setzen(an) }, move |ok| {
                    Msg::Fertig(
                        ok,
                        if ok {
                            String::from("SSH-Server umgeschaltet.")
                        } else {
                            String::from("Abgebrochen oder fehlgeschlagen.")
                        },
                        true,
                    )
                });
            }
            Msg::FreigabeInstallieren => {
                self.busy = true;
                return Task::perform(async { freigabe_installieren() }, |ok| {
                    Msg::Fertig(
                        ok,
                        if ok {
                            String::from("Update-Freigabe aktiv — Updates nun ohne Passwort.")
                        } else {
                            String::from("Freigabe nicht gesetzt (abgebrochen?).")
                        },
                        true,
                    )
                });
            }
            Msg::GegenstelleTesten => {
                let ziel = self.update_pc.trim().to_string();
                self.pc_erreichbar = None;
                return Task::perform(async move { gegenstelle_testen(&ziel) }, Msg::GegenstelleGetestet);
            }
            Msg::GegenstelleGetestet(ok) => self.pc_erreichbar = Some(ok),
            Msg::Fertig(ok, text, neuladen) => {
                self.busy = false;
                self.meldung = Some((ok, text));
                if neuladen {
                    return Task::perform(async { status_erheben() }, Msg::Geladen);
                }
            }
        }
        Task::none()
    }

    pub fn ansicht(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let mut inhalt = column![].spacing(mk::spacing::L);

        // --- Banner: Meldung ---
        if let Some((ok, text)) = &self.meldung {
            inhalt = inhalt.push(mkw::meldung(*ok, text.clone(), p));
        }

        // --- Dieser Rechner ---
        inhalt = inhalt.push(mkw::sektion(
            "DIESER RECHNER",
            vec![
                mkw::zeile_wert("Rechner", None, &self.status.rechner, p),
                mkw::zeile_wert("Netzwerk", Some("Heim = ich baue & rolle aus · Hotspot = mobiler Dev-Zugang"), &self.netz_text, p),
                mkw::zeile_schalter(
                    "SSH-Server",
                    Some("Fernzugang für Entwicklung & Updates"),
                    None,
                    self.status.ssh_server,
                    p,
                    (!self.busy).then_some(Msg::SshServer(!self.status.ssh_server)),
                ),
                mkw::zeile_wert(
                    "Firewall (SSH)",
                    None,
                    match self.status.firewall_ssh {
                        Some(true) => "offen",
                        Some(false) => "blockiert",
                        None => "—",
                    },
                    p,
                ),
            ],
            p,
        ));

        // --- Mein Schlüssel ---
        inhalt = inhalt.push(mkw::sektion(
            "MEIN SCHLÜSSEL",
            vec![
                mkw::zeile_wert("Fingerabdruck", Some("Damit meldet sich dieses Gerät bei anderen an"), &self.fp_text, p),
                mkw::zeile_knopf(
                    "Öffentlichen Schlüssel kopieren",
                    Some("Auf der Gegenstelle unter Zugang gewähren einfügen"),
                    "Kopieren",
                    p,
                    Msg::SchluesselKopieren,
                ),
            ],
            p,
        ));

        // --- Zugang gewähren ---
        inhalt = inhalt.push(mkw::sektion(
            "ZUGANG GEWÄHREN",
            vec![
                container(
                    column![
                        mkw::txt(
                            "Öffentlichen Schlüssel eines anderen Geräts einfügen, um ihm Zugang zu diesem Rechner zu geben:",
                            mk::typo::KLEIN,
                            p.on_surface_variant,
                        ),
                        Space::new().height(mk::spacing::XS),
                        mkw::textfeld(
                            "",
                            &self.neuer_schluessel,
                            "ssh-ed25519 AAAA… name@gerät",
                            Msg::SchluesselInput,
                            Some(Msg::SchluesselErlauben),
                            None,
                            false,
                            p,
                        ),
                        Space::new().height(mk::spacing::XS),
                        mkw::knopf("Zugang erlauben", mkw::knopfart::Stil::Prominent, mkw::knopfart::Rolle::Normal, mkw::knopfart::Groesse::Normal, p, (!self.neuer_schluessel.trim().is_empty()).then_some(Msg::SchluesselErlauben)),
                    ]
                    .spacing(0),
                )
                .padding(mk::spacing::M)
                .into(),
            ],
            p,
        ));

        // --- Gegenstellen ---
        let test_text = match self.pc_erreichbar {
            Some(true) => "erreichbar ✓",
            Some(false) => "nicht erreichbar",
            None => "Testen",
        };
        inhalt = inhalt.push(mkw::sektion(
            "GEGENSTELLEN",
            vec![
                container(
                    column![
                        mkw::txt("Bau-PC / Update-Quelle", mk::typo::KLEIN, p.on_surface_variant),
                        Space::new().height(mk::spacing::XXS),
                        mkw::textfeld(
                            "",
                            &self.update_pc,
                            "benutzer@bau-rechner",
                            Msg::UpdatePcInput,
                            Some(Msg::Speichern),
                            None,
                            false,
                            p,
                        ),
                        Space::new().height(mk::spacing::XS),
                        mkw::txt("Partner-Gerät (optional)", mk::typo::KLEIN, p.on_surface_variant),
                        Space::new().height(mk::spacing::XXS),
                        mkw::textfeld(
                            "",
                            &self.partner,
                            "benutzer@partner-geraet",
                            Msg::PartnerInput,
                            Some(Msg::Speichern),
                            None,
                            false,
                            p,
                        ),
                        Space::new().height(mk::spacing::S),
                        row![
                            mkw::knopf("Speichern", mkw::knopfart::Stil::Getoent, mkw::knopfart::Rolle::Normal, mkw::knopfart::Groesse::Normal, p, Some(Msg::Speichern)),
                            Space::new().width(mk::spacing::S),
                            mkw::knopf(test_text, mkw::knopfart::Stil::Getoent, mkw::knopfart::Rolle::Normal, mkw::knopfart::Groesse::Normal, p, (!self.update_pc.trim().is_empty()).then_some(Msg::GegenstelleTesten)),
                        ],
                    ]
                    .spacing(0),
                )
                .padding(mk::spacing::M)
                .into(),
            ],
            p,
        ));

        // --- Update-Freigabe & Werkzeuge ---
        inhalt = inhalt.push(mkw::sektion(
            "UPDATE & WERKZEUGE",
            vec![
                if self.status.polkit_frei {
                    mkw::zeile_wert(
                        "Update-Freigabe",
                        Some("Updates laufen ohne Passwort"),
                        "aktiv ✓",
                        p,
                    )
                } else {
                    mkw::zeile_knopf(
                        "Update-Freigabe",
                        Some("Einmalig einrichten (fragt nach dem Passwort)"),
                        "Einrichten",
                        p,
                        Msg::FreigabeInstallieren,
                    )
                },
                mkw::zeile_wert(
                    "Bau-Umgebung",
                    Some("Container zum Kompilieren unterwegs"),
                    if self.status.bau_umgebung { "bereit ✓" } else { "nicht eingerichtet" },
                    p,
                ),
                mkw::zeile_wert(
                    "Quelltext",
                    None,
                    if self.status.quellen { "vorhanden ✓" } else { "fehlt" },
                    p,
                ),
            ],
            p,
        ));

        inhalt.into()
    }
}

// ---------------------------------------------------------------- Sonden

fn sh(cmd: &str) -> Option<String> {
    let aus = std::process::Command::new("sh").args(["-c", cmd]).output().ok()?;
    aus.status
        .success()
        .then(|| String::from_utf8_lossy(&aus.stdout).trim().to_string())
}

fn status_erheben() -> Status {
    let heim = std::env::var("HOME").unwrap_or_default();
    let rechner = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| String::from("?"));

    // IPs klassifizieren.
    let addr = sh("ip -brief addr 2>/dev/null").unwrap_or_default();
    let mut heim_ip = None;
    let mut hotspot_ip = None;
    for wort in addr.split_whitespace() {
        if let Some(ip) = wort.strip_suffix("/24").or_else(|| wort.split('/').next()) {
            if ip.starts_with("192.168.") {
                heim_ip.get_or_insert_with(|| ip.to_string());
            } else if ip.starts_with("172.20.10.") {
                hotspot_ip.get_or_insert_with(|| ip.to_string());
            }
        }
    }

    let ssh_server = sh("systemctl is-active sshd 2>/dev/null")
        .map(|s| s == "active")
        .unwrap_or(false);

    let firewall_ssh = sh("firewall-cmd --query-service=ssh 2>/dev/null")
        .map(|s| s == "yes")
        .or_else(|| sh("command -v firewall-cmd").map(|_| false));

    let pub_pfad = format!("{heim}/.ssh/id_ed25519.pub");
    let schluessel_voll = std::fs::read_to_string(&pub_pfad)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let schluessel_kurz = sh(&format!("ssh-keygen -lf {pub_pfad} 2>/dev/null"))
        .and_then(|s| s.split_whitespace().nth(1).map(String::from));

    let polkit_frei =
        std::path::Path::new("/etc/polkit-1/rules.d/90-matrix-update.rules").exists();

    let bau_umgebung = sh("distrobox list 2>/dev/null | grep -qi matrixdev && echo ja")
        .as_deref()
        == Some("ja");
    let quellen = std::path::Path::new(&format!("{heim}/matrixkit-dev/matrixkit/Cargo.toml")).exists();

    Status {
        rechner,
        heim_ip,
        hotspot_ip,
        ssh_server,
        firewall_ssh,
        schluessel_kurz,
        schluessel_voll,
        polkit_frei,
        bau_umgebung,
        quellen,
    }
}

// ---------------------------------------------------------------- Aktionen

/// Fremden Schlüssel in authorized_keys — kein Root nötig.
fn schluessel_erlauben(key: &str) -> bool {
    let heim = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return false,
    };
    let dir = format!("{heim}/.ssh");
    if std::fs::create_dir_all(&dir).is_err() {
        return false;
    }
    let pfad = format!("{dir}/authorized_keys");
    let vorhanden = std::fs::read_to_string(&pfad).unwrap_or_default();
    if vorhanden.lines().any(|z| z.trim() == key.trim()) {
        return true; // schon da
    }
    use std::io::Write;
    let ok = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&pfad)
        .and_then(|mut f| writeln!(f, "{key}"))
        .is_ok();
    if ok {
        let _ = std::process::Command::new("chmod").args(["600", &pfad]).status();
    }
    ok
}

fn in_zwischenablage(text: &str) -> bool {
    use std::io::Write;
    let Ok(mut kind) = std::process::Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn()
    else {
        return false;
    };
    if let Some(mut ein) = kind.stdin.take() {
        let _ = ein.write_all(text.as_bytes());
    }
    kind.wait().map(|s| s.success()).unwrap_or(false)
}

/// SSH-Server an/aus + Firewall — braucht Root → pkexec (grafische Abfrage).
fn ssh_server_setzen(an: bool) -> bool {
    let cmd = if an {
        "systemctl enable --now sshd; firewall-cmd --add-service=ssh --permanent 2>/dev/null; firewall-cmd --reload 2>/dev/null; true"
    } else {
        "systemctl disable --now sshd; true"
    };
    std::process::Command::new("pkexec")
        .args(["sh", "-c", cmd])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// polkit-Regel: rpm-ostree für den aktuellen Nutzer ohne Passwort.
fn freigabe_installieren() -> bool {
    let nutzer = std::env::var("USER").unwrap_or_else(|_| String::from("benutzer"));
    let regel = format!(
        "polkit.addRule(function(action, subject) {{ \
if (action.id.indexOf(\"org.projectatomic.rpmostree1\") === 0 && \
subject.user === \"{nutzer}\") {{ return polkit.Result.YES; }} }});"
    );
    let cmd = format!(
        "mkdir -p /etc/polkit-1/rules.d && cat > /etc/polkit-1/rules.d/90-matrix-update.rules <<'RULE'\n{regel}\nRULE\nsystemctl restart polkit; true"
    );
    std::process::Command::new("pkexec")
        .args(["sh", "-c", &cmd])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn gegenstelle_testen(ziel: &str) -> bool {
    std::process::Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=5",
            ziel,
            "true",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
