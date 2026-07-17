//! Matrix Klänge — Systemklänge einstellen, anhören, abschalten.
//!
//! Vierte MatrixKit-App und Heimat der Klang-Rezepturen (synth.rs):
//! Standard-Modus ist die Einstellungs-App (Master-Schalter, Schalter je
//! Seit der Fusion (R41b) lebt die OBERFLÄCHE in Matrix Einstellungen;
//! dieses Binary ist Daemon und Werkzeug: `--hooks` übersetzt System-
//! ereignisse in Klänge, `--generieren <ordner>` rendert die WAVs — so
//! nutzen Image-Build und App dieselbe Binary und dieselben Rezepturen.
//!
//! Die Schalter sind BINDEND für das System: die Abspiel-Hooks lesen
//! ~/.config/matrix/klaenge.conf, bevor sie einen Klang anfassen.

mod hooks;
mod synth;


fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--generieren") {
        let ziel = args
            .get(i + 1)
            .cloned()
            .unwrap_or_else(|| String::from("/usr/share/matrix/klaenge"));
        if let Err(e) = synth::generieren(std::path::Path::new(&ziel)) {
            eprintln!("Klang-Rendern fehlgeschlagen: {e}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|a| a == "--hooks") {
        hooks::hooks();
        return;
    }
    // Fusion R41b: die Oberfläche wohnt in den Einstellungen — alte
    // Aufrufe (Dock-Pins, Skripte) landen nahtlos im Klänge-Bereich.
    let basis = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| String::from("/tmp"));
    let _ = std::fs::write(format!("{basis}/matrix-einstellungen-bereich"), "klaenge");
    let _ = std::process::Command::new("matrix-einstellungen").spawn().map(|mut kind| {
        std::thread::spawn(move || {
            let _ = kind.wait();
        })
    });
}
