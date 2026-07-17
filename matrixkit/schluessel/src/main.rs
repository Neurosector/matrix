//! Matrix-Schlüssel — ein USB-Stick als Login-Schlüssel (Besitz-Faktor).
//!
//! Modell „entweder/oder" (Nutzer-Wahl): Am Login zählt der Stick ODER das
//! Passwort — beide sind vollwertige Wege. In PAM als `sufficient` verdrahtet:
//! steckt der richtige Stick, gelingt die Anmeldung ohne Passwort; fehlt er,
//! fragt der Greeter ganz normal nach dem Passwort.
//!
//! WICHTIG (ehrliche Sicherheits-Einordnung): Ein normaler Stick ist ein
//! reiner BESITZ-Faktor — wer ihn kopiert oder stiehlt, kommt rein. Das ist
//! bequem, aber schwächer als ein FIDO2-Hardware-Key. Das Passwort bleibt
//! immer als zweiter, gleichwertiger Weg bestehen (kein Aussperren).
//!
//! Kein Geheimnis liegt je im Repo: das Secret entsteht beim Einrichten,
//! wandert auf den Stick UND (root-only) nach /etc/matrix/schluessel/<user>.
//!
//! Befehle:
//!   matrix-schluessel status
//!   matrix-schluessel einrichten --device /dev/sdX --user NAME
//!   matrix-schluessel entfernen  --user NAME
//!   matrix-schluessel --pam-verify        (von pam_exec, als root)

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const LABEL: &str = "MATRIXKEY";
const KEY_DATEI: &str = "matrix-schluessel.key"; // auf dem Stick
const ETC_DIR: &str = "/etc/matrix/schluessel"; // root-only Ablage der Secrets
const MNT: &str = "/run/matrix-schluessel-mnt";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = if args.iter().any(|a| a == "--pam-verify") {
        pam_verify()
    } else {
        match args.get(1).map(String::as_str) {
            Some("status") => {
                status();
                0
            }
            Some("status-json") => {
                status_json();
                0
            }
            Some("einrichten") => einrichten(&args),
            Some("entfernen") => entfernen(&args),
            Some("pam-aktivieren") => pam_aktivieren(),
            Some("pam-deaktivieren") => pam_deaktivieren(),
            _ => {
                eprintln!(
                    "Matrix-Schlüssel\n  status\n  einrichten --device /dev/sdX --user NAME\n  entfernen  --user NAME\n  --pam-verify"
                );
                2
            }
        }
    };
    std::process::exit(code);
}

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn lsblk(dev: &str, spalten: &str) -> Option<String> {
    let out = Command::new("lsblk").args(["-ndo", spalten, dev]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Zufälliges 32-Byte-Secret als Hex (256 Bit aus /dev/urandom).
fn neues_secret() -> Option<String> {
    use std::io::Read;
    let mut f = std::fs::File::open("/dev/urandom").ok()?;
    let mut buf = [0u8; 32];
    f.read_exact(&mut buf).ok()?;
    Some(buf.iter().map(|b| format!("{b:02x}")).collect())
}

fn etc_pfad(user: &str) -> PathBuf {
    PathBuf::from(ETC_DIR).join(user)
}

// --- status ----------------------------------------------------------------

fn status() {
    println!("Eingerichtete Konten (root-only Secrets in {ETC_DIR}):");
    match std::fs::read_dir(ETC_DIR) {
        Ok(d) => {
            let mut leer = true;
            for e in d.flatten() {
                if let Some(n) = e.file_name().to_str() {
                    println!("  • {n}");
                    leer = false;
                }
            }
            if leer {
                println!("  (keine)");
            }
        }
        Err(_) => println!("  (keine)"),
    }
    match stick_finden() {
        Some(dev) => println!("Matrix-Schlüssel-Stick steckt: {dev}"),
        None => println!("Kein Matrix-Schlüssel-Stick erkannt."),
    }
}

/// Maschinenlesbarer Status für die GUI-App (unprivilegiert aufrufbar).
/// Zeilen: konto=<0/1> · pam=<0/1> · stick=<pfad oder leer>
fn status_json() {
    let user = std::env::var("SUDO_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    let konto = if etc_pfad(&user).exists() { 1 } else { 0 };
    let pam = if pam_aktiv() { 1 } else { 0 };
    let stick = stick_finden().unwrap_or_default();
    println!("konto={konto}\npam={pam}\nstick={stick}");
}

// --- PAM-Verdrahtung (entweder/oder: Stick als sufficient VOR Passwort) ------

const PAM_DATEI: &str = "/etc/pam.d/greetd";

fn pam_zeile() -> String {
    // Eigener Pfad — so zeigt die PAM-Zeile immer auf die echte Binary
    // (Dev: /usr/local/bin, Image: /usr/bin), kein Pfad-Mismatch.
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/usr/bin/matrix-schluessel".into());
    format!("auth       sufficient  pam_exec.so quiet {exe} --pam-verify")
}

fn pam_aktiv() -> bool {
    std::fs::read_to_string(PAM_DATEI)
        .map(|s| s.contains("matrix-schluessel --pam-verify"))
        .unwrap_or(false)
}

fn pam_aktivieren() -> i32 {
    let Ok(inhalt) = std::fs::read_to_string(PAM_DATEI) else {
        eprintln!("Abbruch: {PAM_DATEI} nicht lesbar (als root ausführen).");
        return 1;
    };
    if inhalt.contains("matrix-schluessel --pam-verify") {
        println!("Login-Verknüpfung ist bereits aktiv.");
        return 0;
    }
    // Einmaliges Backup
    let backup = format!("{PAM_DATEI}.matrix-backup");
    if !Path::new(&backup).exists() {
        let _ = std::fs::write(&backup, &inhalt);
    }
    // Unsere Zeile VOR die erste auth-Zeile setzen
    let mut neu = String::new();
    let mut gesetzt = false;
    for zeile in inhalt.lines() {
        if !gesetzt && zeile.trim_start().starts_with("auth") {
            neu.push_str(&pam_zeile());
            neu.push('\n');
            gesetzt = true;
        }
        neu.push_str(zeile);
        neu.push('\n');
    }
    if !gesetzt {
        eprintln!("Abbruch: keine auth-Zeile in {PAM_DATEI} gefunden.");
        return 1;
    }
    // Atomar ersetzen
    let tmp = format!("{PAM_DATEI}.matrix-neu");
    if std::fs::write(&tmp, &neu).is_err() || std::fs::rename(&tmp, PAM_DATEI).is_err() {
        eprintln!("Abbruch: {PAM_DATEI} nicht schreibbar.");
        return 1;
    }
    println!("Login-Verknüpfung aktiviert (Backup: {backup}).");
    0
}

fn pam_deaktivieren() -> i32 {
    let Ok(inhalt) = std::fs::read_to_string(PAM_DATEI) else {
        return 1;
    };
    let neu: String = inhalt
        .lines()
        .filter(|z| !z.contains("matrix-schluessel --pam-verify"))
        .map(|z| format!("{z}\n"))
        .collect();
    let tmp = format!("{PAM_DATEI}.matrix-neu");
    if std::fs::write(&tmp, &neu).is_err() || std::fs::rename(&tmp, PAM_DATEI).is_err() {
        return 1;
    }
    println!("Login-Verknüpfung entfernt (Passwort bleibt unverändert aktiv).");
    0
}

// --- einrichten (FORMATIERT den Stick!) -------------------------------------

fn einrichten(args: &[String]) -> i32 {
    let (Some(dev), Some(user)) = (arg(args, "--device"), arg(args, "--user")) else {
        eprintln!("Aufruf: einrichten --device /dev/sdX --user NAME");
        return 2;
    };

    // --- Sicherheitsprüfungen: NIEMALS eine System-/Nicht-USB-Platte anfassen
    if !dev.starts_with("/dev/sd") && !dev.starts_with("/dev/mmcblk") {
        eprintln!("Abbruch: {dev} ist kein erwartetes USB-Gerät.");
        return 1;
    }
    let tran = lsblk(dev, "TRAN").unwrap_or_default();
    let typ = lsblk(dev, "TYPE").unwrap_or_default();
    let rm = lsblk(dev, "RM").unwrap_or_default();
    let mount = lsblk(dev, "MOUNTPOINT").unwrap_or_default();
    if tran != "usb" {
        eprintln!("Abbruch: {dev} ist nicht per USB angebunden (TRAN={tran}).");
        return 1;
    }
    if typ != "disk" {
        eprintln!("Abbruch: {dev} ist keine ganze Platte (TYPE={typ}).");
        return 1;
    }
    if rm != "1" {
        eprintln!("Abbruch: {dev} ist nicht als wechselbar markiert (RM={rm}).");
        return 1;
    }
    if dev.ends_with("sda") || dev.ends_with("sdb") {
        eprintln!("Abbruch: {dev} sind die internen Systemplatten — verweigert.");
        return 1;
    }
    // Ist irgendeine Partition gerade an einem kritischen Pfad gemountet?
    let alle_mounts = Command::new("lsblk")
        .args(["-nro", "MOUNTPOINT", dev])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    for zeile in alle_mounts.lines().chain(std::iter::once(mount.as_str())) {
        let m = zeile.trim();
        if m == "/" || m == "/home" || m == "/var" || m == "/boot" || m.starts_with("/sysroot") {
            eprintln!("Abbruch: {dev} ist an einem System-Pfad gemountet ({m}).");
            return 1;
        }
    }

    let secret = match neues_secret() {
        Some(s) => s,
        None => {
            eprintln!("Abbruch: konnte kein Secret erzeugen.");
            return 1;
        }
    };

    // Auto-Mount lösen: ein frisch eingesteckter Stick wird von der Sitzung
    // (udisks) eingehängt — dann scheitert wipefs/mkfs mit „Gerät belegt".
    // Alle Einhängepunkte des Geräts sauber aushängen, bevor formatiert wird.
    while let Some(ziel) = findmnt(dev) {
        // erst freundlich über udisks (kennt den Mount), sonst hart per umount
        if !lauf(&["udisksctl", "unmount", "-b", dev]) && !lauf(&["umount", &ziel]) {
            eprintln!("Abbruch: {dev} ist eingehängt ({ziel}) und lässt sich nicht lösen.");
            return 1;
        }
    }

    println!("Formatiere {dev} als Matrix-Schlüssel (FAT32, Label {LABEL}) …");
    if !lauf(&["wipefs", "-a", dev]) {
        eprintln!("Abbruch: wipefs fehlgeschlagen (Gerät belegt?).");
        return 1;
    }
    if !lauf(&["mkfs.vfat", "-n", LABEL, dev]) {
        eprintln!("Abbruch: mkfs.vfat fehlgeschlagen.");
        return 1;
    }

    // Secret auf den Stick schreiben
    let _ = std::fs::create_dir_all(MNT);
    if !lauf(&["mount", dev, MNT]) {
        eprintln!("Abbruch: Stick nicht einhängbar.");
        return 1;
    }
    let stick_ok = std::fs::write(Path::new(MNT).join(KEY_DATEI), &secret).is_ok();
    let _ = Command::new("sync").status();
    let _ = lauf(&["umount", MNT]);
    if !stick_ok {
        eprintln!("Abbruch: Secret nicht auf den Stick schreibbar.");
        return 1;
    }

    // Secret root-only ablegen
    if std::fs::create_dir_all(ETC_DIR).is_err() {
        eprintln!("Abbruch: {ETC_DIR} nicht anlegbar (als root ausführen).");
        return 1;
    }
    let pfad = etc_pfad(user);
    if schreibe_privat(&pfad, &secret).is_err() {
        eprintln!("Abbruch: {} nicht schreibbar.", pfad.display());
        return 1;
    }

    // Login-Verknüpfung idempotent aktivieren (Stick ODER Passwort)
    let _ = pam_aktivieren();

    println!("Fertig. {user} kann sich jetzt mit diesem Stick ODER dem Passwort anmelden.");
    0
}

fn entfernen(args: &[String]) -> i32 {
    let Some(user) = arg(args, "--user") else {
        eprintln!("Aufruf: entfernen --user NAME");
        return 2;
    };
    match std::fs::remove_file(etc_pfad(user)) {
        Ok(_) => {
            println!("Schlüssel-Konto für {user} entfernt (Stick bleibt physisch unverändert).");
            0
        }
        Err(e) => {
            eprintln!("Nichts entfernt: {e}");
            1
        }
    }
}

// --- pam-verify (läuft als root aus dem PAM-Stack) --------------------------

/// 0 = Stick gültig (Login gewähren), 1 = kein/kein passender Stick (Passwort).
fn pam_verify() -> i32 {
    // PAM setzt PAM_USER in der Umgebung von pam_exec.
    let Ok(user) = std::env::var("PAM_USER") else { return 1 };
    let Ok(erwartet) = std::fs::read_to_string(etc_pfad(&user)) else { return 1 };
    let erwartet = erwartet.trim();
    if erwartet.is_empty() {
        return 1;
    }
    let Some(dev) = stick_finden() else { return 1 };

    // Ist der Stick schon eingehängt (z. B. udisks in einer aktiven Sitzung)?
    // Dann von dort lesen und NICHT aushängen. Sonst selbst read-only mounten.
    let bereits = findmnt(&dev);
    let (mnt, selbst_gemountet) = match bereits {
        Some(pfad) => (pfad, false),
        None => {
            let _ = std::fs::create_dir_all(MNT);
            if !lauf(&["mount", "-o", "ro,noexec,nosuid,nodev", &dev, MNT]) {
                return 1;
            }
            (MNT.to_string(), true)
        }
    };

    let gefunden = std::fs::read_to_string(Path::new(&mnt).join(KEY_DATEI))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if selbst_gemountet {
        let _ = lauf(&["umount", MNT]);
    }

    if !gefunden.is_empty() && konstant_gleich(gefunden.as_bytes(), erwartet.as_bytes()) {
        0
    } else {
        1
    }
}

/// Bestehenden Einhängepunkt eines Geräts finden (None = nicht gemountet).
fn findmnt(dev: &str) -> Option<String> {
    let out = Command::new("findmnt").args(["-nfo", "TARGET", dev]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!t.is_empty()).then_some(t)
}

/// Blockgerät mit Label MATRIXKEY finden.
fn stick_finden() -> Option<String> {
    if let Ok(out) = Command::new("blkid").args(["-L", LABEL]).output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    // Fallback über lsblk (NAME + LABEL)
    let out = Command::new("lsblk").args(["-nro", "PATH,LABEL"]).output().ok()?;
    for zeile in String::from_utf8_lossy(&out.stdout).lines() {
        let mut it = zeile.split_whitespace();
        let path = it.next()?;
        if it.next() == Some(LABEL) {
            return Some(path.to_string());
        }
    }
    None
}

// --- Helfer -----------------------------------------------------------------

fn lauf(cmd: &[&str]) -> bool {
    Command::new(cmd[0])
        .args(&cmd[1..])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn schreibe_privat(pfad: &Path, inhalt: &str) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(pfad)?;
    f.write_all(inhalt.as_bytes())
}

/// Laufzeit-konstanter Byte-Vergleich (kein Früh-Abbruch).
fn konstant_gleich(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
