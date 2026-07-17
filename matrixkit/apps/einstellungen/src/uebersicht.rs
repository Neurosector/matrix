//! Übersicht — das Dashboard der Matrix Einstellungen (Bereich 0, Startseite).
//!
//! Empirisch dem Leitbild-Vorbild entnommen (Systemeinstellungen → Allgemein →
//! Info + Speicher): Hero mit Gerätename und stillem Untertitel, darunter
//! eine Info-Karte mit Label-links/Wert-rechts-Zeilen, dazu Live-Karten
//! (Prozessor, Arbeitsspeicher, Speicher mit Segmentbalken, Netz, Energie).
//! Die Live-Werte laufen über den Host-Tick (3 s) — kein eigenes Abo nötig.

use iced::widget::{column, container, row, Space};
use iced::{Element, Length, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::time::Instant;
use sysinfo::{Disks, System};

pub struct Panel {
    pub palette: mk::Palette,
    sys: System,
    netz: sysinfo::Networks,
    netz_stand: Instant,
    // Statische Identität (einmal bei Start erhoben)
    geraet: String,
    basis: String,
    kernel: String,
    rechnername: String,
    cpu_modell: String,
    cpu_kerne: usize,
    // Live-Werte
    cpu: f32,
    temperatur: Option<f32>,
    mem_used: u64,
    mem_total: u64,
    disk_used: u64,
    disk_total: u64,
    akku: Option<(u8, bool)>,
    ip: String,
    rx_rate: f64,
    tx_rate: f64,
}

#[derive(Debug, Clone)]
pub enum Msg {}

/// Erste Zeile einer Datei, getrimmt.
fn zeile_aus(pfad: &str) -> Option<String> {
    std::fs::read_to_string(pfad)
        .ok()
        .map(|s| s.lines().next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
}

/// PRETTY_NAME aus os-release — die Basis unter Matrix.
fn os_basis() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("PRETTY_NAME="))
                .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"').to_string())
        })
        .unwrap_or_else(|| String::from("Linux"))
}

/// Höchste plausible CPU-Temperatur aus den Thermal-Zonen.
fn temperatur_lesen() -> Option<f32> {
    let mut max: Option<f32> = None;
    if let Ok(eintraege) = std::fs::read_dir("/sys/class/thermal") {
        for e in eintraege.flatten() {
            let p = e.path();
            if !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("thermal_zone")) {
                continue;
            }
            if let Some(t) = zeile_aus(&format!("{}/temp", p.display()))
                .and_then(|s| s.parse::<f32>().ok())
            {
                let grad = t / 1000.0;
                if (10.0..=120.0).contains(&grad) && max.map_or(true, |m| grad > m) {
                    max = Some(grad);
                }
            }
        }
    }
    max
}

/// Akku: (%-Stand, lädt?) — erster BAT*-Eintrag.
fn akku_lesen() -> Option<(u8, bool)> {
    let eintraege = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for e in eintraege.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("BAT") {
            continue;
        }
        let basis = format!("/sys/class/power_supply/{name}");
        let stand = zeile_aus(&format!("{basis}/capacity"))?.parse::<u8>().ok()?;
        let laedt = zeile_aus(&format!("{basis}/status"))
            .is_some_and(|s| s == "Charging" || s == "Full");
        return Some((stand.min(100), laedt));
    }
    None
}

/// Erste globale IPv4-Adresse (ohne Schnittstellen-Zoo).
fn ip_lesen() -> String {
    mk::befehl::text_von("ip", &["-4", "-o", "addr", "show", "scope", "global"])
        .and_then(|aus| {
            aus.lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(3))
                .map(|a| a.split('/').next().unwrap_or(a).to_string())
        })
        .unwrap_or_else(|| String::from("—"))
}

// R65b: Zahlensprache wohnt in mk::format (Foundation-Extrakt).

impl Panel {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let cpu_modell = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| String::from("Prozessor"));
        let mut panel = Self {
            palette: mk::Palette::load().unwrap_or_default(),
            netz: sysinfo::Networks::new_with_refreshed_list(),
            netz_stand: Instant::now(),
            geraet: zeile_aus("/sys/class/dmi/id/product_name")
                .unwrap_or_else(|| String::from("Dieser Rechner")),
            basis: os_basis(),
            kernel: zeile_aus("/proc/sys/kernel/osrelease").unwrap_or_default(),
            rechnername: System::host_name().unwrap_or_else(|| String::from("matrix")),
            cpu_kerne: sys.cpus().len(),
            cpu_modell,
            cpu: 0.0,
            temperatur: None,
            mem_used: 0,
            mem_total: 0,
            disk_used: 0,
            disk_total: 0,
            akku: None,
            ip: ip_lesen(),
            rx_rate: 0.0,
            tx_rate: 0.0,
            sys,
        };
        panel.messen();
        panel
    }

    /// Vom Host je Tick (3 s): Palette folgt, Messwerte auffrischen.
    pub fn tick(&mut self, p: mk::Palette) {
        self.palette = p;
        self.messen();
    }

    fn messen(&mut self) {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.cpu = self.sys.global_cpu_usage();
        self.mem_used = self.sys.used_memory();
        self.mem_total = self.sys.total_memory();
        self.temperatur = temperatur_lesen();
        self.akku = akku_lesen();
        self.ip = ip_lesen();

        // Auf bootc/ostree ist "/" ein Overlay — den echten Datenträger über
        // die bekannten Mounts suchen, sonst den größten nehmen (wie sysmon).
        let disks = Disks::new_with_refreshed_list();
        let by_mount =
            |m: &str| disks.list().iter().find(|d| d.mount_point().to_str() == Some(m));
        if let Some(d) = by_mount("/sysroot")
            .or_else(|| by_mount("/var/home"))
            .or_else(|| by_mount("/"))
            .or_else(|| disks.list().iter().max_by_key(|d| d.total_space()))
        {
            self.disk_total = d.total_space();
            self.disk_used = d.total_space() - d.available_space();
        }

        // Netz-Raten über das Empfangs-/Sende-Delta seit dem letzten Tick.
        let dt = self.netz_stand.elapsed().as_secs_f64().max(0.5);
        self.netz.refresh();
        self.netz_stand = Instant::now();
        let (mut rx, mut tx) = (0u64, 0u64);
        for (name, daten) in self.netz.iter() {
            // Loopback zählt nicht — sonst „funkt" PipeWire ins Dashboard.
            if name == "lo" {
                continue;
            }
            rx += daten.received();
            tx += daten.transmitted();
        }
        self.rx_rate = rx as f64 / dt;
        self.tx_rate = tx as f64 / dt;
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {}
    }

    pub fn fusstext(&self) -> String {
        format!(
            "{} · Kernel {} · seit {}",
            self.basis,
            self.kernel,
            mk::format::dauer(System::uptime())
        )
    }

    pub fn ansicht(&self) -> Element<'_, Msg> {
        let p = self.palette;

        // --- Hero: wie das Leitbild-Info — Gerätename groß, stille Zeile darunter ---
        let hero = column![
            mkw::txt(String::from("Matrix"), mk::typo::GROSSTITEL, p.on_surface),
            Space::new().height(2),
            mkw::txt(self.geraet.clone(), mk::typo::FLIESS, p.on_surface_variant),
            Space::new().height(2),
            // Die Signatur des Hauses — wie Leitbild- „M1, 2020"-Zeile,
            // nur dass hier der Schöpfer steht.
            mkw::txt(
                String::from("Neurosector \u{00b7} System Anomaly"),
                mk::typo::ETIKETT,
                p.primary,
            ),
        ]
        .align_x(iced::Alignment::Center);
        let hero = container(hero).width(Length::Fill).align_x(iced::Alignment::Center);

        // --- Info-Karte (Leitbild: Label links grau, Wert rechts) ---
        let wertzeile = |titel: &'static str, wert: String| {
            mkw::zeile(
                titel,
                None,
                None,
                Some(mkw::txt(wert, mk::typo::FLIESS, p.on_surface_variant).into()),
                p,
            )
        };
        let info = mkw::sektion(
            "",
            vec![
                wertzeile("Gerät", self.geraet.clone()),
                wertzeile(
                    "Betriebssystem",
                    // Das Image setzt PRETTY_NAME selbst auf Matrix —
                    // dann nicht doppeln (Screenshot-Fund R51).
                    if self.basis.starts_with("Matrix") {
                        self.basis.clone()
                    } else {
                        format!("Matrix \u{00b7} {}", self.basis)
                    },
                ),
                wertzeile("Kernel", self.kernel.clone()),
                wertzeile("Rechnername", self.rechnername.clone()),
                wertzeile("Betriebszeit", mk::format::dauer(System::uptime())),
            ],
            p,
        );

        // --- Live-Karten ---
        let cpu_wert = format!("{:.0} %", self.cpu);
        let cpu_beschr = match self.temperatur {
            Some(t) => format!("{} · {} Kerne · {:.0} °C", self.cpu_modell, self.cpu_kerne, t),
            None => format!("{} · {} Kerne", self.cpu_modell, self.cpu_kerne),
        };
        let prozessor = self.messkarte(
            "PROZESSOR",
            mkw::symbol::MEMORY,
            cpu_wert,
            Some(cpu_beschr),
            self.cpu / 100.0,
            p,
        );

        let ram_anteil = if self.mem_total > 0 {
            self.mem_used as f32 / self.mem_total as f32
        } else {
            0.0
        };
        let arbeitsspeicher = self.messkarte(
            "ARBEITSSPEICHER",
            mkw::symbol::MONITORING,
            mk::format::bytes_speicher_paar(self.mem_used, self.mem_total),
            None,
            ram_anteil,
            p,
        );

        let disk_anteil = if self.disk_total > 0 {
            self.disk_used as f32 / self.disk_total as f32
        } else {
            0.0
        };
        let speicher = self.messkarte(
            "SPEICHER",
            mkw::symbol::STORAGE,
            // Platten DEZIMAL wie der Dateimanager-Referenz (R65b) — vorher binaer.
            format!("{} verwendet", mk::format::bytes_paar(self.disk_used, self.disk_total)),
            None,
            disk_anteil,
            p,
        );

        let netzwerk = mkw::sektion(
            "NETZWERK",
            vec![mkw::zeile(
                "IP-Adresse",
                Some(self.ip.as_str()),
                Some(symbolkreis(mkw::symbol::PUBLIC, p)),
                Some(
                    mkw::txt(
                        format!("↓ {}  ↑ {}", mk::format::rate(self.rx_rate), mk::format::rate(self.tx_rate)),
                        mk::typo::FLIESS,
                        p.on_surface_variant,
                    )
                    .into(),
                ),
                p,
            )],
            p,
        );

        let mut spalte = column![
            Space::new().height(mk::spacing::M),
            hero,
            Space::new().height(mk::spacing::L),
            info,
            Space::new().height(mk::spacing::L),
            row![prozessor, arbeitsspeicher].spacing(mk::spacing::L),
            Space::new().height(mk::spacing::L),
            speicher,
            Space::new().height(mk::spacing::L),
            netzwerk,
        ]
        .spacing(0);

        // Energie-Karte nur, wenn ein Akku existiert (PC hat keinen).
        if let Some((stand, laedt)) = self.akku {
            let symbol = if laedt {
                mkw::symbol::BATTERY_CHARGING
            } else {
                mkw::symbol::BATTERY_FULL
            };
            let zustand = if laedt { "Lädt / am Netz" } else { "Entlädt" };
            spalte = spalte
                .push(Space::new().height(mk::spacing::L))
                .push(self.messkarte(
                    "ENERGIE",
                    symbol,
                    format!("{stand} %"),
                    Some(zustand.to_string()),
                    stand as f32 / 100.0,
                    p,
                ));
        }

        spalte.into()
    }

    /// Eine Live-Karte: Symbol im Kreis, Wert rechts, Balken darunter.
    fn messkarte(
        &self,
        titel: &'static str,
        symbol: char,
        wert: String,
        beschreibung: Option<String>,
        anteil: f32,
        p: mk::Palette,
    ) -> Element<'_, Msg> {
        let mut inhalt = column![
            row![
                symbolkreis(symbol, p),
                Space::new().width(mk::spacing::S),
                mkw::txt(wert, mk::typo::FLIESS, p.on_surface),
                Space::new().width(Length::Fill),
            ]
            .align_y(iced::Alignment::Center),
        ]
        .spacing(mk::spacing::XS);
        if let Some(b) = beschreibung {
            inhalt = inhalt.push(mkw::txt(b, mk::typo::ETIKETT, p.on_surface_variant));
        }
        inhalt = inhalt.push(Space::new().height(2)).push(balken(anteil, p));
        let karte: Element<'_, Msg> =
            container(inhalt).padding(mk::spacing::M).width(Length::Fill).into();
        mkw::sektion(titel, vec![karte], p)
    }
}

/// Symbol in einem stillen primary-container-Kreis — die Karten-Signatur.
fn symbolkreis<'a, M: 'a>(zeichen: char, p: mk::Palette) -> Element<'a, M> {
    container(mkw::symbol(zeichen, mk::icon_size::NORMAL, p.primary))
        .padding(6)
        .style(move |_| container::Style {
            background: Some(color(p.primary_container).into()),
            border: iced::Border { radius: mk::radius::kapsel(36.0).into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

/// Pillen-Balken wie im Sysmon: Track + primary-Füllung.
fn balken<'a, M: 'a>(anteil: f32, p: mk::Palette) -> Element<'a, M> {
    let h = 6.0;
    let voll = ((anteil.clamp(0.0, 1.0) * 1000.0).round() as u16).clamp(1, 999);
    let rest = 1000 - voll;
    let fuellung = if anteil >= 0.9 { p.error } else { p.primary };
    let track = p.on_surface.over(p.surface_container_high, 0.10);
    container(row![
        container(Space::new().width(Length::Fill).height(Length::Fixed(h)))
            .width(Length::FillPortion(voll))
            .style(move |_| container::Style {
                background: Some(color(fuellung).into()),
                border: iced::Border { radius: (h / 2.0).into(), ..Default::default() },
                ..Default::default()
            }),
        Space::new().width(Length::FillPortion(rest)).height(Length::Fixed(h)),
    ])
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(color(track).into()),
        border: iced::Border { radius: (h / 2.0).into(), ..Default::default() },
        ..Default::default()
    })
    .into()
}





#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn betriebszeit_stufen() {
        assert_eq!(mk::format::dauer(300), "5 Min.");
        assert_eq!(mk::format::dauer(3_900), "1 Std. 5 Min.");
        assert_eq!(mk::format::dauer(90_000), "1 T. 1 Std.");
    }

    #[test]
    fn ram_binaer_platte_dezimal() {
        assert_eq!(mk::format::bytes_speicher(8 * 1_073_741_824), "8.0 GB");
        assert_eq!(mk::format::bytes(512_000_000_000), "512 GB");
    }

    #[test]
    fn rate_einheiten() {
        assert_eq!(mk::format::rate(2_100_000.0), "2.1 MB/s");
    }
}
