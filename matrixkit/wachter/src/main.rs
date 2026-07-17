//! Matrix-Wächter — verhindert, dass Updates das System zurücksetzen.
//!
//! Zirconiums täglicher chezmoi-Lauf schützt lokal veränderte Dateien nur
//! durch ein `yes s` (Skip-Antwort) im Service — eine einzige fragile
//! Stelle. Der Wächter ist die zweite Verteidigungslinie: Er kennt die
//! kritischen Matrix-Zustände und stellt sie SELBSTHEILEND wieder her,
//! egal wodurch sie kippen (Update, Unfall, neues Gerät).
//!
//! Getriggert von systemd (Path-Unit auf config.kdl + settings.json,
//! Stunden-Timer als Auffangnetz). Repariert wird nur, was wirklich
//! abweicht — im Normalfall ist der Lauf ein Leselauf ohne Schreibzugriff.
//!
//! Bewachte Zustände (Wunschwerte überschreibbar in
//! ~/.config/matrix/wachter.conf, Zeile `schluessel=wert`):
//!  - niri-Include-Swap (Titelleisten): zirconium-mit-fensterdeko.kdl
//!  - DMS fontFamily (MatrixKit-Schrift, Standard "Inter Variable")
//!  - DMS soundsEnabled (false — sonst Doppel-Töne neben matrix-klaenge)

use matrixkit_theme as mk;
use std::collections::HashMap;

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/root".into())
}

fn wunsch() -> HashMap<String, String> {
    let mut w = HashMap::from([
        ("fontFamily".to_string(), "Inter Variable".to_string()),
        ("soundsEnabled".to_string(), "false".to_string()),
        (
            "niriInclude".to_string(),
            "zirconium-mit-fensterdeko.kdl".to_string(),
        ),
    ]);
    if let Ok(inhalt) = std::fs::read_to_string(format!("{}/.config/matrix/wachter.conf", home())) {
        for zeile in inhalt.lines() {
            if let Some((k, v)) = zeile.split_once('=') {
                w.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    w
}

fn main() {
    // Dock-Modus: Leitbild-Minimieren — Dock-Klick holt Ablage-Fenster zurück
    if std::env::args().any(|a| a == "--dock") {
        dock_daemon();
        return;
    }
    let w = wunsch();
    let mut reparaturen: Vec<String> = Vec::new();

    // 1) niri: der Include-Swap (Titelleisten + eigene Binds)
    let kdl_pfad = format!("{}/.config/niri/config.kdl", home());
    if let Ok(inhalt) = std::fs::read_to_string(&kdl_pfad) {
        let gewuenscht = format!("include \"{}\"", w["niriInclude"]);
        let zirconium_default =
            "include \"/usr/share/zirconium/zdots/system/niri/zirconium.kdl\"";
        // EXISTENZ-WACHE (R46-Vorfall, Surface): Der Swap zeigt auf eine
        // Datei in ~/.config/niri/ — gibt es sie auf DIESEM Geraet nicht
        // (Laptop kennt kein zirconium-mit-fensterdeko.kdl), wuerde niri
        // die GANZE Config verwerfen: grauer Login. Also: nie auf ein
        // Ziel swappen, das nicht existiert.
        let ziel_da = std::path::Path::new(&format!(
            "{}/.config/niri/{}",
            home(),
            w["niriInclude"]
        ))
        .exists();
        if ziel_da && !inhalt.contains(&gewuenscht) && inhalt.contains(zirconium_default) {
            let neu = inhalt.replace(zirconium_default, &gewuenscht);
            let tmp = format!("{kdl_pfad}.wachter");
            if std::fs::write(&tmp, neu).is_ok() && std::fs::rename(&tmp, &kdl_pfad).is_ok() {
                reparaturen.push(String::from(
                    "niri-Titelleisten-Include wiederhergestellt (Update-Revert erkannt)",
                ));
            }
        }
    }

    // 2) DMS-Einstellungen: Schrift + Doppel-Töne
    let settings_pfad = format!("{}/.config/DankMaterialShell/settings.json", home());
    if let Ok(raw) = std::fs::read_to_string(&settings_pfad) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
            let font_ist = json.get("fontFamily").and_then(|v| v.as_str()).unwrap_or("");
            if font_ist != w["fontFamily"] && dms_setzen("fontFamily", &w["fontFamily"]) {
                reparaturen.push(format!(
                    "DMS-Schrift zurück auf „{}“ (war „{font_ist}“)",
                    w["fontFamily"]
                ));
            }
            let toene_soll = w["soundsEnabled"] == "true";
            let toene_ist = json
                .get("soundsEnabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if toene_ist != toene_soll
                && dms_setzen("soundsEnabled", &w["soundsEnabled"])
            {
                reparaturen.push(String::from(
                    "DMS-Systemtöne wieder abgeschaltet (Doppel-Ton-Schutz)",
                ));
            }
        }
    }

    if reparaturen.is_empty() {
        return;
    }
    for r in &reparaturen {
        eprintln!("matrix-wachter: {r}");
    }
    let _ = std::process::Command::new("notify-send")
        .args([
            "Matrix-Wächter",
            &format!("Zurückgesetzt und repariert:\n• {}", reparaturen.join("\n• ")),
        ])
        .status();
}

/// Einstellung über die laufende Shell setzen — DMS-Zustand nie direkt
/// editieren (Hausregel). Läuft keine Session, greift der nächste Lauf.
fn dms_setzen(schluessel: &str, wert: &str) -> bool {
    std::process::Command::new("dms")
        .args(["ipc", "call", "settings", "set", schluessel, wert])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Dock-Modus: Leitbild-Minimieren auf Matrix.
//
// Die gelbe Ampel legt Fenster in die Ablage-Arbeitsfläche (unsichtbar,
// App bleibt im Dock). Klickt man das Dock-Symbol, fokussiert die Shell
// das Fenster — niri wechselt dann ZUR Ablage. Dieser Daemon dreht das
// um: Er erkennt den Dock-Klick (Fokus auf ein Ablage-Fenster, dessen
// Workspace-Aktivierung jünger als 400 ms ist — empirisch: erst
// WorkspaceActivated, dann WindowFocusChanged) und holt das Fenster auf
// die vorherige Arbeitsfläche ZURÜCK — wie das Leitbild. Wer die Ablage bewusst
// besucht (Super+Shift+M, Aktivierung älter), stöbert ungestört.
// ---------------------------------------------------------------------------

fn dock_daemon() {
    use std::collections::HashMap;
    use std::io::BufRead;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn laufzeit_dir() -> String {
        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into())
    }
    loop {
        // Compositor-Kontakt seit R45 nur über die Leinwand-Fassade.
        if mk::leinwand::socket().is_none() {
            std::thread::sleep(Duration::from_secs(3));
            continue;
        }
        let Ok(mut kind) = mk::leinwand::ereignis_strom(true) else {
            std::thread::sleep(Duration::from_secs(3));
            continue;
        };
        let Some(out) = kind.stdout.take() else {
            let _ = kind.kill();
            continue;
        };

        let mut ablage_id: Option<u64> = None;
        let mut ws_idx: HashMap<u64, u64> = HashMap::new();
        let mut fenster_ws: HashMap<u64, u64> = HashMap::new();
        // Ankunftszeit in der Ablage: das Ablegen selbst fokussiert das
        // Fenster kurz — frisch Angekommene sind KEINE Dock-Klicks.
        let mut ablage_ankunft: HashMap<u64, Instant> = HashMap::new();
        let mut aktive: u64 = 0;
        let mut vorherige: u64 = 0;
        let mut aktiviert_um = Instant::now() - Duration::from_secs(60);
        // Bewusster Ablage-Besuch (Super+Shift+M hinterlässt eine Fahne)
        let mut besuch = false;
        let besuch_fahne = format!("{}/matrix-ablage-besuch", laufzeit_dir());
        // Epoche: jede Workspace-Aktivierung zählt hoch — bricht anstehende
        // Rückholungen ab (Scroll-Durchfahrt durch die Ablage)
        let epoche = Arc::new(AtomicU64::new(0));

        for zeile in std::io::BufReader::new(out).lines().map_while(Result::ok) {
            let Ok(ev) = serde_json::from_str::<serde_json::Value>(&zeile) else { continue };

            if let Some(wc) = ev.get("WorkspacesChanged") {
                if let Some(liste) = wc.get("workspaces").and_then(|w| w.as_array()) {
                    ws_idx.clear();
                    for w in liste {
                        let (Some(id), Some(idx)) = (
                            w.get("id").and_then(|v| v.as_u64()),
                            w.get("idx").and_then(|v| v.as_u64()),
                        ) else {
                            continue;
                        };
                        ws_idx.insert(id, idx);
                        if w.get("name").and_then(|n| n.as_str()) == Some("ablage") {
                            ablage_id = Some(id);
                        }
                        if w.get("is_focused").and_then(|f| f.as_bool()) == Some(true) {
                            aktive = id;
                        }
                    }
                    // Ablage-Ordnung: die Ablage darf nie der ERSTE Workspace
                    // sein. niri legt benannte Workspaces vorn an — dann
                    // startet der Login IN der Ablage, Hochscrollen landet
                    // zuerst dort, und „Minimieren" wird unsichtbar (das
                    // Fenster ist ja schon da). Der Wächter schiebt sie ans
                    // Ende; nur Index 1 wird korrigiert, sonst kein Eingriff.
                    if let Some(aid) = ablage_id {
                        if ws_idx.get(&aid) == Some(&1) && liste.len() > 1 {
                            mk::leinwand::aktion(&[
                                "msg", "action", "move-workspace-to-index",
                                "--reference", "ablage",
                                &liste.len().to_string(),
                            ]);
                        }
                    }
                }
            } else if let Some(wa) = ev.get("WorkspaceActivated") {
                if let Some(id) = wa.get("id").and_then(|v| v.as_u64()) {
                    if id != aktive {
                        vorherige = aktive;
                        aktive = id;
                        aktiviert_um = Instant::now();
                        epoche.fetch_add(1, Ordering::SeqCst);
                        if Some(id) == ablage_id {
                            // Fahne der Mod+Shift+M-Bind? Dann: Stöber-Modus
                            besuch = std::fs::remove_file(&besuch_fahne).is_ok();
                        } else {
                            besuch = false;
                        }
                    }
                }
            } else if let Some(wc) = ev.get("WindowsChanged") {
                if let Some(liste) = wc.get("windows").and_then(|w| w.as_array()) {
                    fenster_ws.clear();
                    for f in liste {
                        if let (Some(id), Some(ws)) = (
                            f.get("id").and_then(|v| v.as_u64()),
                            f.get("workspace_id").and_then(|v| v.as_u64()),
                        ) {
                            fenster_ws.insert(id, ws);
                        }
                    }
                }
            } else if let Some(wo) = ev.get("WindowOpenedOrChanged") {
                if let Some(f) = wo.get("window") {
                    if let (Some(id), Some(ws)) = (
                        f.get("id").and_then(|v| v.as_u64()),
                        f.get("workspace_id").and_then(|v| v.as_u64()),
                    ) {
                        if Some(ws) == ablage_id && fenster_ws.get(&id) != Some(&ws) {
                            ablage_ankunft.insert(id, Instant::now());
                        }
                        fenster_ws.insert(id, ws);
                    }
                }
            } else if let Some(wc) = ev.get("WindowClosed") {
                if let Some(id) = wc.get("id").and_then(|v| v.as_u64()) {
                    fenster_ws.remove(&id);
                }
            } else if let Some(ff) = ev.get("WindowFocusChanged") {
                let (Some(fid), Some(aid)) = (ff.get("id").and_then(|v| v.as_u64()), ablage_id)
                else {
                    continue;
                };
                if fenster_ws.get(&fid) != Some(&aid) {
                    continue;
                }
                // Gerade erst abgelegt? Dann ist dieser Fokus das Ablegen
                // selbst — die Minimierung nicht sofort rückgängig machen.
                if ablage_ankunft
                    .get(&fid)
                    .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
                {
                    continue;
                }
                // Bewusster Besuch (Super+Shift+M): stöbern, nichts zurückholen
                if besuch {
                    continue;
                }
                // Dock-Klick oder bewusster Ablage-Besuch?
                let ziel = if aktive != aid {
                    Some(aktive)
                } else if aktiviert_um.elapsed() < Duration::from_millis(150) && vorherige != aid {
                    Some(vorherige)
                } else {
                    None
                };
                let Some(ziel) = ziel else { continue };
                let Some(idx) = ws_idx.get(&ziel).copied() else { continue };
                // 300 ms Bedenkzeit: kommt in der Zwischenzeit eine weitere
                // Workspace-Aktivierung (Scroll-Durchfahrt), verfällt die
                // Rückholung — ein Dock-Klick bleibt allein.
                let meine_epoche = epoche.load(Ordering::SeqCst);
                let epoche2 = Arc::clone(&epoche);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(300));
                    if epoche2.load(Ordering::SeqCst) != meine_epoche {
                        return;
                    }
                    mk::leinwand::aktion(&[
                        "msg",
                        "action",
                        "move-window-to-workspace",
                        "--window-id",
                        &fid.to_string(),
                        &idx.to_string(),
                    ]);
                });
                fenster_ws.insert(fid, ziel);
            }
        }
        let _ = kind.wait();
        std::thread::sleep(Duration::from_secs(3));
    }
}
