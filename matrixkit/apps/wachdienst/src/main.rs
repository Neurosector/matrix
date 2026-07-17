//! Matrix Wachdienst — das Dienst-Leitbild-KeepAlive-Extrakt (Runde 24,
//! launch.h Z. 52: LAUNCH_JOBKEY_KEEPALIVE). Winziger Daemon ohne UI:
//! prüft alle 5 s die Kern-Dienste der Shell und startet Gefallene
//! nach — mit Anlauf-Sperre (15 s je Dienst), damit ein crashender
//! Dienst nicht flattert. Selbst gestartet vom Autostart; stirbt der
//! Wachdienst, bleibt alles wie zuvor (kein Single Point of Failure,
//! nur ein Netz darunter).

use matrixkit_theme as mk;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// (comm ≤15 Zeichen für den /proc-Vergleich, Startkommando, Argumente)
const DIENSTE: [(&str, &str, &[&str]); 4] = [
    ("matrix-bar", "matrix-bar", &["--reservieren"]),
    ("matrix-dock", "matrix-dock", &["--reservieren"]),
    ("matrix-hintergr", "matrix-hintergrund", &[]),
    ("matrix-mitteilu", "matrix-mitteilungen", &[]),
];

fn lebt(comm: &str) -> bool {
    let Ok(eintraege) = std::fs::read_dir("/proc") else { return true };
    for e in eintraege.flatten() {
        if e.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let pfad = e.path().join("comm");
        if let Ok(c) = std::fs::read_to_string(&pfad) {
            if c.trim() == comm {
                // Zombies zählen nicht (die comm-Falle vom 7.7.).
                if let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) {
                    if let Some(rest) = stat.rsplit(')').next() {
                        if rest.trim().starts_with('Z') {
                            continue;
                        }
                    }
                }
                return true;
            }
        }
    }
    false
}

fn starten(name: &str, args: &[&str]) {
    let heim = std::env::var("HOME").unwrap_or_default();
    let lokal = format!("{heim}/.local/bin/{name}");
    let programm = if std::path::Path::new(&lokal).exists() { lokal } else { name.to_string() };
    if let Ok(mut kind) = std::process::Command::new(programm).args(args).spawn() {
        std::thread::spawn(move || {
            let _ = kind.wait(); // Zombie-Kultur
        });
    }
}

fn main() {
    let mut zuletzt: HashMap<&str, Instant> = HashMap::new();
    // Sitzungs-Anker: der niri-Socket trägt die PID der Compositor-Instanz
    // im Namen — verschwindet er, ist UNSERE Sitzung zu Ende. Ohne diesen
    // Anker überlebt der Wachdienst das Abmelden (er braucht kein Wayland)
    // und belebt in der NÄCHSTEN Sitzung Dienste doppelt (9.7., Laptop:
    // 2 Bars + 2 Docks nach Re-Login).
    let anker = mk::leinwand::anker();
    eprintln!("[wachdienst] wacht über {} Dienste", DIENSTE.len());
    loop {
        if let Some(a) = &anker {
            if !std::path::Path::new(a).exists() {
                eprintln!("[wachdienst] Sitzung beendet — trete ab");
                return;
            }
        }
        for (comm, name, args) in DIENSTE {
            if lebt(comm) {
                continue;
            }
            let frisch = zuletzt
                .get(comm)
                .is_some_and(|t| t.elapsed() < Duration::from_secs(15));
            if frisch {
                continue; // Anlauf-Sperre: kein Flattern
            }
            eprintln!("[wachdienst] {name} gefallen — steht wieder auf");
            starten(name, args);
            zuletzt.insert(comm, Instant::now());
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}
