//! Verbindungen — WLAN und Bluetooth in den Matrix Einstellungen (R44).
//!
//! Leitbild-Grammatik (Systemeinstellungen → WLAN/Bluetooth): oben der
//! Master-Schalter, darunter das aktive Netz, dann „Andere Netzwerke"
//! mit Signal und Schloss; Bluetooth als Geräteliste mit Verbinden-
//! Knopf. Unterbau: nmcli und bluetoothctl — beides asynchron über
//! Task::perform, damit der Scan die Oberfläche nie anhält.

use iced::widget::{column, container, row, Space};
use iced::{Element, Length, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;

#[derive(Debug, Clone, Default)]
pub struct Netz {
    pub ssid: String,
    pub signal: u8,
    pub gesichert: bool,
    pub aktiv: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Geraet {
    pub mac: String,
    pub name: String,
    pub verbunden: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Status {
    pub wlan_an: bool,
    pub netze: Vec<Netz>,
    pub bt_an: bool,
    pub geraete: Vec<Geraet>,
}

pub struct Panel {
    pub palette: mk::Palette,
    status: Status,
    laedt: bool,
    /// SSID, für die gerade ein Passwort erfragt wird + Eingabe.
    passwort_fuer: Option<String>,
    passwort: String,
    meldung: Option<(bool, String)>,
}

#[derive(Debug, Clone)]
pub enum Msg {
    Neuladen,
    Geladen(Status),
    WlanSchalten(bool),
    BtSchalten(bool),
    Verbinden(String, bool),
    PasswortTipp(String),
    PasswortAbschicken,
    PasswortAbbrechen,
    BtVerbinden(String, bool),
    Ergebnis(bool, String),
}

fn befehl(programm: &str, args: &[&str]) -> Option<String> {
    mk::befehl::text_von(programm, args)
}

/// Ein nmcli-Terse-Feld: `\:` ist Teil des Werts, `:` trennt.
fn terse_felder(zeile: &str) -> Vec<String> {
    let mut felder = Vec::new();
    let mut aktuell = String::new();
    let mut escape = false;
    for c in zeile.chars() {
        if escape {
            aktuell.push(c);
            escape = false;
        } else if c == '\\' {
            escape = true;
        } else if c == ':' {
            felder.push(std::mem::take(&mut aktuell));
        } else {
            aktuell.push(c);
        }
    }
    felder.push(aktuell);
    felder
}

fn status_erheben() -> Status {
    let wlan_an = befehl("nmcli", &["radio", "wifi"])
        .map(|s| s.trim() == "enabled")
        .unwrap_or(false);
    let mut netze: Vec<Netz> = Vec::new();
    if wlan_an {
        if let Some(aus) = befehl(
            "nmcli",
            &["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "dev", "wifi", "list"],
        ) {
            for zeile in aus.lines() {
                let f = terse_felder(zeile);
                if f.len() < 4 || f[1].is_empty() {
                    continue;
                }
                // Dieselbe SSID funkt oft auf mehreren Bändern — die
                // stärkste gewinnt, die aktive sowieso.
                let netz = Netz {
                    aktiv: f[0] == "*",
                    ssid: f[1].clone(),
                    signal: f[2].parse().unwrap_or(0),
                    gesichert: !f[3].is_empty() && f[3] != "--",
                };
                if let Some(alt) = netze.iter_mut().find(|n| n.ssid == netz.ssid) {
                    if netz.aktiv || netz.signal > alt.signal {
                        let war_aktiv = alt.aktiv;
                        *alt = netz;
                        alt.aktiv = alt.aktiv || war_aktiv;
                    }
                } else {
                    netze.push(netz);
                }
            }
        }
        netze.sort_by(|a, b| b.aktiv.cmp(&a.aktiv).then(b.signal.cmp(&a.signal)));
        netze.truncate(12);
    }

    let bt_an = befehl("bluetoothctl", &["show"])
        .map(|s| s.lines().any(|l| l.trim() == "Powered: yes"))
        .unwrap_or(false);
    let mut geraete = Vec::new();
    if bt_an {
        if let Some(aus) = befehl("bluetoothctl", &["devices"]) {
            for zeile in aus.lines() {
                // "Device AA:BB:CC:DD:EE:FF Name mit Leerzeichen"
                let mut teile = zeile.splitn(3, ' ');
                if teile.next() != Some("Device") {
                    continue;
                }
                let (Some(mac), Some(name)) = (teile.next(), teile.next()) else {
                    continue;
                };
                let verbunden = befehl("bluetoothctl", &["info", mac])
                    .map(|i| i.lines().any(|l| l.trim() == "Connected: yes"))
                    .unwrap_or(false);
                geraete.push(Geraet {
                    mac: mac.to_string(),
                    name: name.to_string(),
                    verbunden,
                });
            }
        }
        geraete.sort_by(|a, b| b.verbunden.cmp(&a.verbunden).then(a.name.cmp(&b.name)));
    }

    Status { wlan_an, netze, bt_an, geraete }
}

impl Panel {
    pub fn new() -> (Self, Task<Msg>) {
        (
            Self {
                palette: mk::Palette::load().unwrap_or_default(),
                status: Status::default(),
                laedt: true,
                passwort_fuer: None,
                passwort: String::new(),
                meldung: None,
            },
            Task::perform(async { status_erheben() }, Msg::Geladen),
        )
    }

    /// Vom Host je Tick: Palette folgt. (Kein Auto-Rescan — WLAN-Scans
    /// kosten Strom; Neuladen sitzt als Knopf in der Ansicht.)
    pub fn tick(&mut self, p: mk::Palette) {
        self.palette = p;
    }

    pub fn fusstext(&self) -> String {
        if self.laedt {
            String::from("Netze werden gesucht \u{2026}")
        } else {
            String::from("WLAN über NetworkManager, Bluetooth über BlueZ — Passwörter tippst immer du selbst")
        }
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Neuladen => {
                self.laedt = true;
                self.meldung = None;
                return Task::perform(async { status_erheben() }, Msg::Geladen);
            }
            Msg::Geladen(s) => {
                self.status = s;
                self.laedt = false;
            }
            Msg::WlanSchalten(an) => {
                self.laedt = true;
                return Task::perform(
                    async move {
                        let _ = std::process::Command::new("nmcli")
                            .args(["radio", "wifi", if an { "on" } else { "off" }])
                            .status();
                        // Der Funk braucht einen Moment, bis Netze da sind.
                        std::thread::sleep(std::time::Duration::from_millis(if an { 2500 } else { 300 }));
                        status_erheben()
                    },
                    Msg::Geladen,
                );
            }
            Msg::BtSchalten(an) => {
                self.laedt = true;
                return Task::perform(
                    async move {
                        let _ = std::process::Command::new("bluetoothctl")
                            .args(["power", if an { "on" } else { "off" }])
                            .status();
                        status_erheben()
                    },
                    Msg::Geladen,
                );
            }
            Msg::Verbinden(ssid, gesichert) => {
                // Bekannte Profile verbindet nmcli ohne Passwort; nur
                // wenn das scheitert und das Netz gesichert ist, fragen
                // wir nach dem Schlüssel.
                self.meldung = None;
                self.laedt = true;
                return Task::perform(
                    async move {
                        let ok = std::process::Command::new("nmcli")
                            .args(["dev", "wifi", "connect", &ssid])
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or(false);
                        (ok, ssid, gesichert)
                    },
                    |(ok, ssid, gesichert)| {
                        if ok {
                            Msg::Ergebnis(true, format!("Mit \u{201e}{ssid}\u{201c} verbunden"))
                        } else if gesichert {
                            Msg::PasswortTipp(format!("\u{0}{ssid}"))
                        } else {
                            Msg::Ergebnis(false, format!("Verbindung mit \u{201e}{ssid}\u{201c} fehlgeschlagen"))
                        }
                    },
                );
            }
            Msg::PasswortTipp(t) => {
                // NUL-Präfix = interner Marker: Passwortfeld für SSID öffnen.
                if let Some(ssid) = t.strip_prefix('\u{0}') {
                    self.passwort_fuer = Some(ssid.to_string());
                    self.passwort.clear();
                    self.laedt = false;
                } else {
                    self.passwort = t;
                }
            }
            Msg::PasswortAbschicken => {
                let (Some(ssid), pw) = (self.passwort_fuer.take(), std::mem::take(&mut self.passwort))
                else {
                    return Task::none();
                };
                if pw.is_empty() {
                    return Task::none();
                }
                self.laedt = true;
                return Task::perform(
                    async move {
                        let ok = std::process::Command::new("nmcli")
                            .args(["dev", "wifi", "connect", &ssid, "password", &pw])
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or(false);
                        (ok, ssid)
                    },
                    |(ok, ssid)| {
                        Msg::Ergebnis(
                            ok,
                            if ok {
                                format!("Mit \u{201e}{ssid}\u{201c} verbunden")
                            } else {
                                format!("Passwort für \u{201e}{ssid}\u{201c} abgelehnt")
                            },
                        )
                    },
                );
            }
            Msg::PasswortAbbrechen => {
                self.passwort_fuer = None;
                self.passwort.clear();
            }
            Msg::BtVerbinden(mac, verbinden) => {
                self.laedt = true;
                return Task::perform(
                    async move {
                        let _ = std::process::Command::new("bluetoothctl")
                            .args([if verbinden { "connect" } else { "disconnect" }, &mac])
                            .output();
                        status_erheben()
                    },
                    Msg::Geladen,
                );
            }
            Msg::Ergebnis(ok, text) => {
                self.meldung = Some((ok, text));
                self.laedt = true;
                return Task::perform(async { status_erheben() }, Msg::Geladen);
            }
        }
        Task::none()
    }

    pub fn ansicht(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let mut spalte = column![].spacing(mk::spacing::L);

        if let Some((ok, text)) = &self.meldung {
            spalte = spalte.push(mkw::meldung(*ok, text.clone(), p));
        }

        // --- WLAN ---
        let mut wlan_zeilen: Vec<Element<'_, Msg>> = vec![mkw::zeile_schalter(
            "WLAN",
            None,
            Some(mkw::symbol::<Msg>(mkw::symbol::WIFI, mk::icon_size::MEDIUM, p.primary)),
            self.status.wlan_an,
            p,
            Some(Msg::WlanSchalten(!self.status.wlan_an)),
        )];
        if self.status.wlan_an {
            for netz in &self.status.netze {
                let staerke = mkw::symbol::WIFI;
                let mut rechts = row![].spacing(mk::spacing::S).align_y(iced::Alignment::Center);
                if netz.gesichert {
                    rechts = rechts.push(mkw::symbol::<Msg>(mkw::symbol::LOCK, mk::icon_size::XSMALL, p.on_surface_variant));
                }
                rechts = rechts.push(mkw::txt(
                    format!("{} %", netz.signal),
                    mk::typo::KLEIN,
                    p.on_surface_variant,
                ));
                if netz.aktiv {
                    rechts = rechts.push(mkw::txt("Verbunden", mk::typo::KLEIN, p.primary));
                } else {
                    rechts = rechts.push(mkw::knopf(
                        "Verbinden",
                        mkw::knopfart::Stil::Getoent,
                        mkw::knopfart::Rolle::Normal,
                        mkw::knopfart::Groesse::Klein,
                        p,
                        Some(Msg::Verbinden(netz.ssid.clone(), netz.gesichert)),
                    ));
                }
                wlan_zeilen.push(mkw::zeile(
                    &netz.ssid,
                    None,
                    Some(mkw::symbol::<Msg>(staerke, mk::icon_size::MEDIUM, if netz.aktiv { p.primary } else { p.on_surface_variant })),
                    Some(rechts.into()),
                    p,
                ));
                // Passwort-Zeile direkt unter dem betroffenen Netz.
                if self.passwort_fuer.as_deref() == Some(netz.ssid.as_str()) {
                    wlan_zeilen.push(
                        container(
                            row![
                                container(mkw::eingabefeld(
                                    "Passwort",
                                    &self.passwort,
                                    Msg::PasswortTipp,
                                    Some(Msg::PasswortAbschicken),
                                    true,
                                    p,
                                ))
                                .width(Length::Fill),
                                mkw::knopf(
                                    "Verbinden",
                                    mkw::knopfart::Stil::Prominent,
                                    mkw::knopfart::Rolle::Normal,
                                    mkw::knopfart::Groesse::Klein,
                                    p,
                                    Some(Msg::PasswortAbschicken),
                                ),
                                mkw::knopf(
                                    "Abbrechen",
                                    mkw::knopfart::Stil::Randlos,
                                    mkw::knopfart::Rolle::Normal,
                                    mkw::knopfart::Groesse::Klein,
                                    p,
                                    Some(Msg::PasswortAbbrechen),
                                ),
                            ]
                            .spacing(mk::spacing::S)
                            .align_y(iced::Alignment::Center),
                        )
                        .padding(iced::Padding {
                            left: mk::spacing::M,
                            right: mk::spacing::M,
                            top: 4.0,
                            bottom: mk::spacing::S,
                        })
                        .into(),
                    );
                }
            }
            if self.status.netze.is_empty() && !self.laedt {
                wlan_zeilen.push(mkw::zeile("Keine Netze gefunden", None, None, None, p));
            }
        }
        spalte = spalte.push(mkw::sektion("WLAN", wlan_zeilen, p));

        // --- Bluetooth ---
        let mut bt_zeilen: Vec<Element<'_, Msg>> = vec![mkw::zeile_schalter(
            "Bluetooth",
            None,
            Some(mkw::symbol::<Msg>(mkw::symbol::BLUETOOTH, mk::icon_size::MEDIUM, p.primary)),
            self.status.bt_an,
            p,
            Some(Msg::BtSchalten(!self.status.bt_an)),
        )];
        if self.status.bt_an {
            for g in &self.status.geraete {
                let (aktion, ziel) = if g.verbunden {
                    ("Trennen", false)
                } else {
                    ("Verbinden", true)
                };
                bt_zeilen.push(mkw::zeile(
                    &g.name,
                    Some(if g.verbunden { "Verbunden" } else { "Gekoppelt" }),
                    Some(mkw::symbol::<Msg>(
                        mkw::symbol::BLUETOOTH,
                        mk::icon_size::MEDIUM,
                        if g.verbunden { p.primary } else { p.on_surface_variant },
                    )),
                    Some(
                        mkw::knopf(
                            aktion,
                            mkw::knopfart::Stil::Getoent,
                            mkw::knopfart::Rolle::Normal,
                            mkw::knopfart::Groesse::Klein,
                            p,
                            Some(Msg::BtVerbinden(g.mac.clone(), ziel)),
                        )
                        .into(),
                    ),
                    p,
                ));
            }
            if self.status.geraete.is_empty() {
                bt_zeilen.push(mkw::zeile(
                    "Keine gekoppelten Geräte",
                    Some("Kopplung neuer Geräte folgt — bis dahin: bluetoothctl"),
                    None,
                    None,
                    p,
                ));
            }
        }
        spalte = spalte.push(mkw::sektion("BLUETOOTH", bt_zeilen, p));

        // --- Neu suchen ---
        spalte = spalte.push(
            container(mkw::knopf(
                if self.laedt { "Suche läuft \u{2026}" } else { "Neu suchen" },
                mkw::knopfart::Stil::Getoent,
                mkw::knopfart::Rolle::Normal,
                mkw::knopfart::Groesse::Klein,
                p,
                Some(Msg::Neuladen),
            ))
            .width(Length::Fill)
            .align_x(iced::Alignment::Center),
        );

        spalte = spalte.push(Space::new().height(mk::spacing::S));
        spalte.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terse_felder_mit_escape() {
        assert_eq!(terse_felder("*:Nick\\:Netz:76:WPA2"), vec!["*", "Nick:Netz", "76", "WPA2"]);
        assert_eq!(terse_felder(" ::29:WPA2"), vec![" ", "", "29", "WPA2"]);
    }
}
