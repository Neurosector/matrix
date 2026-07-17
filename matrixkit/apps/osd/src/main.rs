//! Matrix OSD — App #15, das Tasten-Werkzeug der Leisten-Familie.
//!
//! Seit 2.0 zeichnet das OSD NICHT mehr selbst: `matrix-osd lauter|
//! leiser|stumm|mikro|heller|dunkler` führt die Änderung aus
//! (wpctl/brightnessctl) und schreibt den Stand in den OSD-Kanal
//! (mk::osd). Das DOCK liest ihn und morpht kurz zum OSD — Zeile 1
//! zeigt die Stufe, Zeile 2 sagt, was geändert wurde („Dynamic
//! Dock", Nutzer-Entwurf). Kein Fenster, kein iced, kein Daemon.

use matrixkit_theme as mk;

fn main() {
    let aktion = std::env::args().nth(1).unwrap_or_default();
    // System-Momente (Runde 30): kein OSD, nur der passende Matrix-Klang —
    // synchron, damit der kurzlebige Prozess ihn zu Ende spielt.
    match aktion.as_str() {
        "anmeldung" => {
            mk::feedback::jetzt("anmeldung", "01-anmeldung.wav");
            return;
        }
        "abmeldung" => {
            mk::feedback::jetzt("abmeldung", "02-abmeldung.wav");
            return;
        }
        _ => {}
    }
    let ok = match aktion.as_str() {
        "lauter" => mk::befehl::still("wpctl", &["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", "5%+"]),
        "leiser" => mk::befehl::still("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", "5%-"]),
        "stumm" => mk::befehl::still("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]),
        "mikro" => mk::befehl::still("wpctl", &["set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"]),
        "heller" => mk::befehl::still("brightnessctl", &["--class=backlight", "set", "+10%"]),
        "dunkler" => mk::befehl::still("brightnessctl", &["--class=backlight", "set", "10%-"]),
        _ => {
            eprintln!("matrix-osd lauter|leiser|stumm|mikro|heller|dunkler");
            false
        }
    };
    if !ok {
        return; // kein Backend (z. B. heller am Desktop) — still
    }
    let schritt = matches!(aktion.as_str(), "lauter" | "leiser" | "heller" | "dunkler");
    let stand = match aktion.as_str() {
        "lauter" | "leiser" | "stumm" => ton_stand(mk::osd::Typ::Ton, "@DEFAULT_AUDIO_SINK@"),
        "mikro" => ton_stand(mk::osd::Typ::Mikro, "@DEFAULT_AUDIO_SOURCE@"),
        _ => licht_stand(),
    };
    if let Some(mut stand) = stand {
        stand.schritt = schritt;
        mk::osd::schreiben(stand);
    }
    // Leitbild-Grammatik: beim Ändern der Lautstärke klickt es kurz (05) —
    // NACH dem OSD-Schreiben, damit das Dock sofort morpht.
    if matches!(aktion.as_str(), "lauter" | "leiser") {
        mk::feedback::jetzt("lautstaerke", "05-lautstaerke.wav");
    }
}

/// „Volume: 0.55 [MUTED]" → Stand.
fn ton_stand(typ: mk::osd::Typ, ziel: &str) -> Option<mk::osd::Stand> {
    let z = mk::befehl::erste_zeile("wpctl", &["get-volume", ziel])?;
    let wert: f32 = z.split_whitespace().nth(1)?.parse().ok()?;
    Some(mk::osd::Stand {
        typ,
        prozent: (wert * 100.0).round(),
        stumm: z.contains("MUTED"),
        schritt: false,
    })
}

/// brightnessctl -m: „…,backlight,48000,50%,96000" → Stand.
fn licht_stand() -> Option<mk::osd::Stand> {
    let z = mk::befehl::erste_zeile("brightnessctl", &["-m"])?;
    let prozent = z.split(',').nth(3)?.trim_end_matches('%').parse::<f32>().ok()?;
    Some(mk::osd::Stand {
        typ: mk::osd::Typ::Licht,
        prozent,
        stumm: false,
        schritt: false,
    })
}
