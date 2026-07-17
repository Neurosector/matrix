//! Klang-Hooks — der Daemon, der Systemereignisse in Klänge übersetzt.
//!
//! Läuft als systemd-User-Service (`matrix-klaenge --hooks`). Vor JEDEM
//! Abspielen wird ~/.config/matrix/klaenge.conf gelesen: die Schalter der
//! App „Matrix Klänge" sind bindend — Master aus oder Ereignis aus heißt,
//! der Klang wird gar nicht erst angefasst.
//!
//! Quellen:
//!  - Anmeldung: Daemon-Start (hängt an graphical-session.target)
//!  - Arbeitsfläche: niri-Event-Stream (WorkspaceActivated)
//!  - Gerät verbunden/getrennt: udevadm monitor (USB)
//!  - Lautstärke: wpctl-Poll auf den Standard-Sink (250 ms)
//!  - Benachrichtigung: dbus-monitor auf org.freedesktop.Notifications

use std::io::BufRead;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/root".into())
}

/// Klangordner wie in der App: Dev-Stand vor Image-Version.
fn klangordner() -> PathBuf {
    let lokal = PathBuf::from(home()).join(".local/share/matrix/klaenge");
    if lokal.join("12-fehler.wav").exists() {
        return lokal;
    }
    PathBuf::from("/usr/share/matrix/klaenge")
}

/// Schalterstand frisch von der Platte — bindend.
fn erlaubt(schluessel: &str) -> bool {
    let pfad = PathBuf::from(home()).join(".config/matrix/klaenge.conf");
    let Ok(inhalt) = std::fs::read_to_string(pfad) else {
        return true; // keine Datei = alles an (Opt-out wie überall)
    };
    for zeile in inhalt.lines() {
        if let Some((k, w)) = zeile.split_once('=') {
            let k = k.trim();
            if (k == "alle" || k == schluessel) && w.trim() == "aus" {
                return false;
            }
        }
    }
    true
}

fn abspielen(ordner: &std::path::Path, schluessel: &str) {
    if !erlaubt(schluessel) {
        return;
    }
    let pfad = ordner.join(format!("{schluessel}.wav"));
    if let Ok(mut kind) = std::process::Command::new("pw-play").arg(&pfad).spawn() {
        std::thread::spawn(move || {
            let _ = kind.wait();
        });
    }
}

/// Zeilen eines Unterprozesses als Ereignisse in den Kanal speisen.
/// Stirbt der Prozess, startet er nach einer Pause neu (Session-Robustheit).
fn zeilen_quelle(
    tx: mpsc::Sender<&'static str>,
    kommando: &'static [&'static str],
    filter: fn(&str) -> Option<&'static str>,
) {
    std::thread::spawn(move || loop {
        let mut cmd = std::process::Command::new(kommando[0]);
        cmd.args(&kommando[1..])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        if let Ok(mut kind) = cmd.spawn() {
            if let Some(out) = kind.stdout.take() {
                for zeile in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                    if let Some(klang) = filter(&zeile) {
                        if tx.send(klang).is_err() {
                            let _ = kind.kill();
                            return;
                        }
                    }
                }
            }
            let _ = kind.wait();
        }
        std::thread::sleep(Duration::from_secs(3));
    });
}

pub fn hooks() {
    let ordner = klangordner();
    let (tx, rx) = mpsc::channel::<&'static str>();

    // 1) Anmeldung: der Daemon startet mit der Session
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            let _ = tx.send("01-anmeldung");
        });
    }

    // 2) Arbeitsflächen-Wechsel über den niri-Event-Stream.
    //    Der Socket-Pfad variiert — vor dem Start entdecken.
    {
        let tx = tx.clone();
        std::thread::spawn(move || loop {
            // Compositor-Kontakt seit R45 nur über die Leinwand-Fassade.
            {
                if let Ok(mut kind) = matrixkit_theme::leinwand::ereignis_strom(true) {
                    if let Some(out) = kind.stdout.take() {
                        let mut erster = true;
                        for zeile in std::io::BufReader::new(out).lines().map_while(Result::ok) {
                            if zeile.contains("\"WorkspaceActivated\"") {
                                // den Anfangszustand nicht vertonen
                                if erster {
                                    erster = false;
                                    continue;
                                }
                                if tx.send("08-arbeitsflaeche").is_err() {
                                    let _ = kind.kill();
                                    return;
                                }
                            }
                        }
                    }
                    let _ = kind.wait();
                }
            }
            std::thread::sleep(Duration::from_secs(3));
        });
    }

    // 3) USB angesteckt/abgezogen
    zeilen_quelle(
        tx.clone(),
        &["udevadm", "monitor", "--udev", "--subsystem-match=usb"],
        |zeile| {
            if !zeile.contains("(usb)") {
                return None;
            }
            if zeile.contains(" add ") {
                Some("06-geraet-verbunden")
            } else if zeile.contains(" remove ") {
                Some("07-geraet-getrennt")
            } else {
                None
            }
        },
    );

    // 4) Benachrichtigungen (DMS ist der Server, spielt aber selbst nichts).
    //    busctl statt dbus-monitor — Letzteres ist auf Matrix nicht installiert.
    zeilen_quelle(
        tx.clone(),
        &[
            "busctl",
            "--user",
            "monitor",
            "--json=short",
            "--match",
            "type='method_call',interface='org.freedesktop.Notifications',member='Notify'",
        ],
        |zeile| {
            if !(zeile.contains("\"member\":\"Notify\"")
                && zeile.contains("org.freedesktop.Notifications"))
            {
                return None;
            }
            // Nutzer-Fund (15.7.): niris Screenshot-Meldung ist KEINE
            // Post — sie ist der Auslöser. Vorher klang jedes
            // Bildschirmfoto wie eine Benachrichtigung (03), weil kein
            // Hook den Screenshot kannte; 09 existierte nur als Datei.
            if zeile.contains("creenshot") || zeile.contains("ildschirmfoto") {
                Some("09-screenshot")
            } else {
                Some("03-benachrichtigung")
            }
        },
    );

    // 5) Lautstärke: Poll auf den Standard-Sink — Änderung = Blip
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let lesen = || -> Option<String> {
                let out = std::process::Command::new("wpctl")
                    .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
                    .output()
                    .ok()?;
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            };
            let mut zuletzt = lesen();
            loop {
                std::thread::sleep(Duration::from_millis(250));
                let jetzt = lesen();
                if jetzt.is_some() && zuletzt.is_some() && jetzt != zuletzt {
                    let _ = tx.send("05-lautstaerke");
                }
                if jetzt.is_some() {
                    zuletzt = jetzt;
                }
            }
        });
    }

    // Verbraucher: entprellen (Ereignis-Stürme wie USB-Enumeration), abspielen
    let mut zuletzt_gespielt: std::collections::HashMap<&'static str, Instant> =
        std::collections::HashMap::new();
    // Wächter-Zustand: steckt der Login-Stick (Label MATRIXKEY) gerade?
    let schluessel_da = || std::path::Path::new("/dev/disk/by-label/MATRIXKEY").exists();
    let mut schluessel = schluessel_da();
    for klang in rx {
        // Login-Stick erkannt? Dann spricht der Wächter statt des
        // USB-Klangs — Sicherheits-Feedback, hörbar für die Umgebung.
        let mut klang = klang;
        if klang == "06-geraet-verbunden" {
            // udev meldet vor dem Blockgerät — kurz warten, dann prüfen
            std::thread::sleep(Duration::from_millis(700));
            let jetzt_da = schluessel_da();
            if jetzt_da && !schluessel {
                klang = "13-schluessel-erkannt";
            }
            schluessel = jetzt_da;
        } else if klang == "07-geraet-getrennt" {
            schluessel = schluessel_da();
        }
        let jetzt = Instant::now();
        let sperre = match klang {
            "06-geraet-verbunden" | "07-geraet-getrennt" => Duration::from_millis(1200),
            "13-schluessel-erkannt" => Duration::from_millis(3000),
            "05-lautstaerke" => Duration::from_millis(200),
            _ => Duration::from_millis(400),
        };
        if let Some(vorher) = zuletzt_gespielt.get(klang) {
            if jetzt.duration_since(*vorher) < sperre {
                continue;
            }
        }
        zuletzt_gespielt.insert(klang, jetzt);
        abspielen(&ordner, klang);
    }
}
