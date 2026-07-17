//! matrix-wache — das Root-Werkzeug des Wächters (Wiederherstellung).
//!
//! Philosophie: Ein vergessenes Passwort macht kein Gerät unbrauchbar.
//! Der Weg zurück ist die KORREKTE Löschung der personenbezogenen Daten
//! des Kontos — niemals deren Preisgabe. Dieses Werkzeug kann Konten
//! analysieren (Speicher-Anteile), löschen und neu anlegen; gelesen oder
//! gerettet wird nichts.
//!
//! Läuft als root (aus der Wiederherstellungs-Sitzung via sudo NOPASSWD
//! für den Benutzer `wache`). Nur std — keine Fremdabhängigkeiten.
//!
//! Befehle:
//!   nutzer                                Menschen-Konten auflisten (name=uid)
//!   analyse --user N                      Speicher-Anteile (kategorie=BYTES)
//!   loeschen --user N                     Konto + Daten vollständig entfernen
//!   werkseinstellung                      ALLE Menschen-Konten entfernen
//!   anlegen --user N --passwort-stdin     frisches Konto (wheel) anlegen

use std::path::Path;
use std::process::Command;

/// Der Wiederherstellungs-Benutzer selbst — niemals Ziel einer Aktion.
const WACHE: &str = "wache";
const HOME_BASIS: &str = "/var/home";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let code = match args.first().map(String::as_str) {
        Some("nutzer") => nutzer(),
        Some("analyse") => match arg("--user") {
            Some(u) => analyse(&u),
            None => fehler("analyse braucht --user <name>"),
        },
        Some("loeschen") => match arg("--user") {
            Some(u) => loeschen(&u),
            None => fehler("loeschen braucht --user <name>"),
        },
        Some("werkseinstellung") => werkseinstellung(),
        Some("anlegen") => match arg("--user") {
            Some(u) if args.iter().any(|a| a == "--passwort-stdin") => anlegen(&u),
            Some(_) => fehler("anlegen braucht --passwort-stdin (Passwort über stdin)"),
            None => fehler("anlegen braucht --user <name>"),
        },
        _ => {
            eprintln!("matrix-wache: nutzer | analyse --user N | loeschen --user N | werkseinstellung | anlegen --user N --passwort-stdin");
            2
        }
    };
    std::process::exit(code);
}

fn fehler(text: &str) -> i32 {
    eprintln!("matrix-wache: {text}");
    2
}

fn ist_root() -> bool {
    std::fs::metadata("/proc/self")
        .map(|m| {
            use std::os::unix::fs::MetadataExt;
            m.uid() == 0
        })
        .unwrap_or(false)
}

/// Menschen-Konten: UID 1000–59999, echtes Home unter /var/home, nie `wache`.
fn menschen() -> Vec<(String, u32)> {
    let Ok(passwd) = std::fs::read_to_string("/etc/passwd") else {
        return Vec::new();
    };
    menschen_aus(&passwd)
}

/// Reine Parse-Logik von menschen() — ohne Datei-I/O, damit testbar.
/// Filtert: nur UID 1000–59999, Home unter /var/home oder /home, keine
/// nologin-/false-Shell, niemals der Recovery-Benutzer `wache`.
fn menschen_aus(passwd: &str) -> Vec<(String, u32)> {
    let mut v = Vec::new();
    for zeile in passwd.lines() {
        let f: Vec<&str> = zeile.split(':').collect();
        if f.len() < 7 {
            continue;
        }
        let (name, uid, home, shell) = (f[0], f[2].parse::<u32>().unwrap_or(0), f[5], f[6]);
        if !(1000..60000).contains(&uid) || name == WACHE {
            continue;
        }
        if !home.starts_with("/var/home") && !home.starts_with("/home") {
            continue;
        }
        if shell.ends_with("nologin") || shell.ends_with("false") {
            continue;
        }
        v.push((name.to_string(), uid));
    }
    v
}

/// Ist dieses Konto vor Löschung geschützt? (root, der Wächter selbst,
/// leerer Name — nie anfassbar, egal was die Aufrufer wollen.)
fn geschuetzt(user: &str) -> bool {
    user == WACHE || user == "root" || user.is_empty()
}

/// Frist-Entscheidung (rein, ohne Datei-I/O). `start` = wann die Frist
/// begann (None = noch nie). Ok(()) = darf löschen; Err(rest) = warten.
fn frist_entscheidung(jetzt: u64, start: Option<u64>) -> Result<(), u64> {
    match start {
        Some(s) => {
            let vergangen = jetzt.saturating_sub(s);
            if vergangen >= FRIST_SEKUNDEN {
                Ok(())
            } else {
                Err(FRIST_SEKUNDEN - vergangen)
            }
        }
        None => Err(FRIST_SEKUNDEN),
    }
}

fn nutzer() -> i32 {
    for (name, uid) in menschen() {
        println!("{name}={uid}");
    }
    0
}

/// Bytes eines Pfads (du -sb; 0 wenn nicht vorhanden).
fn groesse(pfad: &Path) -> u64 {
    if !pfad.exists() {
        return 0;
    }
    Command::new("du")
        .args(["-sb", pfad.to_str().unwrap_or("")])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0)
}

/// Speicher-Anteile eines Kontos: was gelöscht würde, ehrlich beziffert.
/// Deutsche + englische XDG-Ordnernamen (beide zählen).
fn analyse(user: &str) -> i32 {
    if !menschen().iter().any(|(n, _)| n == user) {
        return fehler(&format!("„{user}“ ist kein Menschen-Konto auf diesem System."));
    }
    let home = Path::new(HOME_BASIS).join(user);
    let kategorien: &[(&str, &[&str])] = &[
        ("bilder", &["Bilder", "Pictures"]),
        ("musik", &["Musik", "Music"]),
        ("dokumente", &["Dokumente", "Documents"]),
        ("videos", &["Videos"]),
        ("downloads", &["Downloads"]),
        ("schreibtisch", &["Schreibtisch", "Desktop"]),
    ];
    let gesamt = groesse(&home);
    let mut kategorisiert = 0u64;
    for (schluessel, ordner) in kategorien {
        let bytes: u64 = ordner.iter().map(|o| groesse(&home.join(o))).sum();
        kategorisiert += bytes;
        println!("{schluessel}={bytes}");
    }
    println!("rest={}", gesamt.saturating_sub(kategorisiert));
    println!("gesamt={gesamt}");
    0
}

fn lauf(cmd: &[&str]) -> bool {
    Command::new(cmd[0])
        .args(&cmd[1..])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Die Wächter-Frist: Löschungen ohne Besitz-Nachweis warten 30 Minuten
/// (Schutz gegen „Freunde am unbewachten PC" — niemand harrt eine halbe
/// Stunde neben einem rufenden Rechner aus). Der Login-Stick beweist
/// Besitz (Geheimnis-Prüfung) und überspringt die Frist. Durchgesetzt
/// HIER im Root-Werkzeug, nicht nur in der App — Zeitstempel persistent
/// in /etc/matrix/wache-frist (überlebt Neustarts, Uhr stellt am Greeter
/// niemand ohne Anmeldung um).
const FRIST_SEKUNDEN: u64 = 30 * 60;
const FRIST_DATEI: &str = "/etc/matrix/wache-frist";

fn jetzt_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Beweist der eingesteckte Login-Stick den Besitz? Für ein Konto: sein
/// hinterlegtes Geheimnis; für die Werkseinstellung: irgendein Konto des
/// Systems (wer IRGENDEINEN gültigen Schlüssel besitzt, gehört hierher).
fn stick_beweist(ziel: &str) -> bool {
    let schluessel_cli = ["/usr/bin/matrix-schluessel", "/usr/local/bin/matrix-schluessel"]
        .iter()
        .find(|p| Path::new(p).exists())
        .copied();
    let Some(cli) = schluessel_cli else { return false };
    let pruefe = |user: &str| -> bool {
        Command::new(cli)
            .arg("--pam-verify")
            .env("PAM_USER", user)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    if ziel == "@system" {
        menschen().iter().any(|(n, _)| pruefe(n))
    } else {
        pruefe(ziel)
    }
}

/// Frist prüfen/starten. Ok(()) = Löschung darf laufen; Err(rest) = noch
/// warten (rest Sekunden). Erster Aufruf startet die Frist.
fn frist_pruefen(ziel: &str) -> Result<(), u64> {
    if stick_beweist(ziel) {
        eprintln!("Login-Schlüssel erkannt — Besitz nachgewiesen, keine Wartezeit.");
        frist_entfernen(ziel);
        return Ok(());
    }
    let inhalt = std::fs::read_to_string(FRIST_DATEI).unwrap_or_default();
    for zeile in inhalt.lines() {
        if let Some((z, ts)) = zeile.split_once('=') {
            if z.trim() == ziel {
                let start: u64 = ts.trim().parse().unwrap_or(0);
                return match frist_entscheidung(jetzt_s(), Some(start)) {
                    Ok(()) => {
                        frist_entfernen(ziel);
                        Ok(())
                    }
                    Err(rest) => Err(rest),
                };
            }
        }
    }
    // Frist beginnt jetzt
    let mut neu = inhalt;
    neu.push_str(&format!("{ziel}={}\n", jetzt_s()));
    let _ = std::fs::create_dir_all("/etc/matrix");
    let _ = std::fs::write(FRIST_DATEI, neu);
    Err(FRIST_SEKUNDEN)
}

fn frist_entfernen(ziel: &str) {
    let Ok(inhalt) = std::fs::read_to_string(FRIST_DATEI) else { return };
    let neu: String = inhalt
        .lines()
        .filter(|z| z.split_once('=').map(|(k, _)| k.trim() != ziel).unwrap_or(true))
        .map(|z| format!("{z}\n"))
        .collect();
    let _ = std::fs::write(FRIST_DATEI, neu);
}

/// Ein Konto vollständig entfernen: Prozesse beenden, Konto + Home löschen,
/// alle Systemspuren (AccountsService, Greeter-Slot, Login-Schlüssel) mit.
fn loeschen(user: &str) -> i32 {
    if !ist_root() {
        return fehler("loeschen braucht root (sudo).");
    }
    if geschuetzt(user) {
        return fehler("Dieses Konto ist geschützt.");
    }
    if !menschen().iter().any(|(n, _)| n == user) {
        return fehler(&format!("„{user}“ ist kein Menschen-Konto auf diesem System."));
    }
    if let Err(rest) = frist_pruefen(user) {
        println!("rest={rest}");
        eprintln!("Wächter-Frist läuft: Löschung erst in {} Min möglich — oder Login-Schlüssel einstecken.", rest.div_ceil(60));
        return 3;
    }
    loeschen_erzwungen(user)
}

/// Der eigentliche Löschvorgang — NUR nach bestandener Frist-/Besitz-Prüfung
/// aufrufen (loeschen bzw. werkseinstellung erledigen das).
fn loeschen_erzwungen(user: &str) -> i32 {
    eprintln!("Beende Prozesse von {user} …");
    let _ = lauf(&["loginctl", "terminate-user", user]);
    let _ = lauf(&["pkill", "-9", "-u", user]);
    std::thread::sleep(std::time::Duration::from_millis(800));

    eprintln!("Lösche Konto und Daten von {user} …");
    if !lauf(&["userdel", "-r", user]) {
        // userdel -r scheitert z. B. bei fehlendem Mail-Spool — Home dann direkt
        let home = format!("{HOME_BASIS}/{user}");
        if Path::new(&home).exists() && !lauf(&["rm", "-rf", "--one-file-system", &home]) {
            return fehler("Konnte das Home-Verzeichnis nicht löschen.");
        }
        let _ = lauf(&["userdel", user]);
    }
    // Systemspuren
    for pfad in [
        format!("/var/lib/AccountsService/users/{user}"),
        format!("/var/lib/AccountsService/icons/{user}"),
        format!("/var/cache/dms-greeter/users/{user}"),
        format!("/etc/matrix/schluessel/{user}"),
    ] {
        let p = Path::new(&pfad);
        if p.is_dir() {
            let _ = std::fs::remove_dir_all(p);
        } else if p.exists() {
            let _ = std::fs::remove_file(p);
        }
    }
    // SSD: freigegebene Blöcke trimmen — Teil der „korrekten Löschung"
    let _ = lauf(&["fstrim", "--quiet-unsupported", HOME_BASIS]);
    println!("Konto {user} vollständig entfernt.");
    0
}

/// Werkseinstellung: alle Menschen-Konten entfernen. Das Betriebssystem
/// selbst ist ein unveränderliches Abbild — nach der Löschung der
/// Nutzerdaten ist es wieder die frische Installation.
fn werkseinstellung() -> i32 {
    if !ist_root() {
        return fehler("werkseinstellung braucht root (sudo).");
    }
    let alle = menschen();
    if alle.is_empty() {
        println!("Keine Menschen-Konten vorhanden.");
        return 0;
    }
    if let Err(rest) = frist_pruefen("@system") {
        println!("rest={rest}");
        eprintln!("Wächter-Frist läuft: Werkseinstellung erst in {} Min möglich — oder Login-Schlüssel einstecken.", rest.div_ceil(60));
        return 3;
    }
    let mut code = 0;
    for (name, _) in alle {
        if loeschen_erzwungen(&name) != 0 {
            code = 1;
        }
    }
    if code == 0 {
        println!("Werkseinstellung abgeschlossen — das System ist frisch.");
    }
    code
}

/// Die MatrixKit-Apps als Dock-Pins ins Home des Nutzers legen (DMS liest
/// pinnedApps aus ~/.local/state/DankMaterialShell/session.json). Nur
/// anlegen, wenn noch keine Session-Datei existiert — nie überschreiben.
fn matrix_pins_setzen(user: &str) {
    let dir = format!("{HOME_BASIS}/{user}/.local/state/DankMaterialShell");
    let datei = format!("{dir}/session.json");
    if Path::new(&datei).exists() {
        return; // aus skel schon da — nicht anfassen
    }
    let inhalt = "{\n  \"pinnedApps\": [\n    \"matrix-sysmon\",\n    \"matrix-farben\",\n    \"matrix-klaenge\",\n    \"matrix-codes\",\n    \"matrix-schluessel-app\",\n    \"matrix-hilfe\",\n    \"org.mozilla.firefox\",\n    \"org.gnome.Nautilus\"\n  ],\n  \"barPinnedApps\": [],\n  \"configVersion\": 3\n}\n";
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if std::fs::write(&datei, inhalt).is_ok() {
        // dem neuen Nutzer übereignen (wir laufen als root)
        let _ = lauf(&["chown", "-R", &format!("{user}:{user}"),
            &format!("{HOME_BASIS}/{user}/.local")]);
    }
}

/// Frisches Konto anlegen (Admin/wheel wie das Erstkonto); Passwort über
/// stdin (nie als Argument — Argumente sind in der Prozessliste sichtbar).
fn anlegen(user: &str) -> i32 {
    if !ist_root() {
        return fehler("anlegen braucht root (sudo).");
    }
    if user == WACHE
        || user == "root"
        || user.is_empty()
        || user.len() > 32
        || !user
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        || !user.chars().next().unwrap_or('0').is_ascii_lowercase()
    {
        return fehler("Kontoname: klein geschrieben, beginnt mit Buchstabe, a–z 0–9 - _ (max 32).");
    }
    if menschen().iter().any(|(n, _)| n == user) {
        return fehler(&format!("„{user}“ existiert bereits."));
    }
    let mut passwort = String::new();
    use std::io::Read;
    if std::io::stdin().read_to_string(&mut passwort).is_err() {
        return fehler("Konnte das Passwort nicht lesen.");
    }
    let passwort = passwort.trim_end_matches('\n');
    if passwort.len() < 4 {
        return fehler("Passwort zu kurz (mindestens 4 Zeichen).");
    }
    if !lauf(&["useradd", "-m", "-G", "wheel", "-s", "/bin/bash", user]) {
        return fehler("useradd fehlgeschlagen.");
    }
    // Passwort setzen: chpasswd liest name:passwort von stdin
    use std::io::Write;
    let Ok(mut kind) = Command::new("chpasswd")
        .stdin(std::process::Stdio::piped())
        .spawn()
    else {
        return fehler("chpasswd nicht startbar.");
    };
    if let Some(mut si) = kind.stdin.take() {
        let _ = writeln!(si, "{user}:{passwort}");
    }
    if !kind.wait().map(|s| s.success()).unwrap_or(false) {
        return fehler("Passwort setzen fehlgeschlagen.");
    }
    // MatrixKit-zentriert: die Dock-Pins ins frische Home setzen. `useradd
    // -m` kopiert zwar /etc/skel (das die Pins mitbringt) — hier stellen wir
    // sicher, dass sie da sind, falls skel mal fehlt. Idempotent.
    matrix_pins_setzen(user);
    println!("Konto {user} angelegt (Admin). Anmeldung ab sofort möglich.");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ein realistischer /etc/passwd-Ausschnitt (bootc-Layout).
    const PASSWD: &str = "\
root:x:0:0:root:/root:/bin/bash
bin:x:1:1:bin:/bin:/usr/sbin/nologin
wache:x:766:766:Matrix Waechter:/var/lib/matrix-wache:/bin/bash
greeter:x:767:767::/var/lib/greeter:/usr/sbin/nologin
benutzer:x:1000:1000::/var/home/benutzer:/bin/bash
knoblauch:x:1001:1001::/var/home/knoblauch:/bin/bash
service:x:1002:1002::/var/home/service:/usr/sbin/nologin
altdir:x:1003:1003::/opt/altdir:/bin/bash
nobody:x:65534:65534:Nobody:/:/usr/sbin/nologin";

    #[test]
    fn menschen_filtert_system_und_wache() {
        let m = menschen_aus(PASSWD);
        let namen: Vec<&str> = m.iter().map(|(n, _)| n.as_str()).collect();
        // Nur echte Menschen-Konten mit Login-Shell und Home unter /var/home
        assert_eq!(namen, vec!["benutzer", "knoblauch"]);
        // Ausdrücklich NICHT dabei:
        assert!(!namen.contains(&"root"), "root darf nie Ziel sein");
        assert!(!namen.contains(&"wache"), "der Wächter selbst nie");
        assert!(!namen.contains(&"greeter"), "System-Benutzer (nologin) nie");
        assert!(!namen.contains(&"service"), "nologin-Shell nie");
        assert!(!namen.contains(&"altdir"), "Home außerhalb /var/home,/home nie");
        assert!(!namen.contains(&"nobody"), "UID außerhalb 1000–59999 nie");
    }

    #[test]
    fn geschuetzte_konten_sind_tabu() {
        assert!(geschuetzt("root"));
        assert!(geschuetzt("wache"));
        assert!(geschuetzt(""));
        assert!(!geschuetzt("benutzer"));
        assert!(!geschuetzt("knoblauch"));
    }

    #[test]
    fn frist_mathematik() {
        // Nie gestartet → volle Frist warten
        assert_eq!(frist_entscheidung(10_000, None), Err(FRIST_SEKUNDEN));
        // Gerade gestartet → fast volle Frist
        assert_eq!(frist_entscheidung(1_000, Some(1_000)), Err(FRIST_SEKUNDEN));
        // Halb um → halbe Restzeit
        assert_eq!(
            frist_entscheidung(1_000 + FRIST_SEKUNDEN / 2, Some(1_000)),
            Err(FRIST_SEKUNDEN / 2)
        );
        // Exakt abgelaufen → erlaubt
        assert_eq!(frist_entscheidung(1_000 + FRIST_SEKUNDEN, Some(1_000)), Ok(()));
        // Lange her → erlaubt
        assert_eq!(frist_entscheidung(999_999, Some(1_000)), Ok(()));
        // Uhr zurückgestellt (jetzt < start) → kein Unterlauf, weiter warten
        assert_eq!(frist_entscheidung(500, Some(1_000)), Err(FRIST_SEKUNDEN));
    }

    #[test]
    fn frist_ist_dreissig_minuten() {
        assert_eq!(FRIST_SEKUNDEN, 1800);
    }
}
