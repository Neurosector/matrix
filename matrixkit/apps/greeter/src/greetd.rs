//! Die greetd-Brücke: das IPC-Protokoll des Login-Daemons.
//!
//! greetd spricht JSON über einen Unix-Socket ($GREETD_SOCK), jede
//! Nachricht mit u32-Längenpräfix (native endian). Wir führen den
//! ganzen Login als EINEN blockierenden Ablauf (läuft im Task-Pool):
//! Session erschaffen → Prompts beantworten → Zielkommando setzen.
//! Endet der Greeter danach, wechselt greetd zur Nutzer-Session.

use serde::Deserialize;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Antwort {
    Success,
    Error {
        #[serde(rename = "error_type")]
        _error_type: String,
        description: String,
    },
    AuthMessage {
        auth_message_type: String,
        #[serde(rename = "auth_message")]
        _auth_message: String,
    },
}

fn senden(strom: &mut UnixStream, nachricht: &serde_json::Value) -> Result<Antwort, String> {
    let daten = nachricht.to_string().into_bytes();
    strom
        .write_all(&(daten.len() as u32).to_ne_bytes())
        .and_then(|_| strom.write_all(&daten))
        .map_err(|e| format!("greetd nicht erreichbar: {e}"))?;
    let mut laenge = [0u8; 4];
    strom
        .read_exact(&mut laenge)
        .map_err(|e| format!("greetd antwortet nicht: {e}"))?;
    let mut puffer = vec![0u8; u32::from_ne_bytes(laenge) as usize];
    strom
        .read_exact(&mut puffer)
        .map_err(|e| format!("greetd-Antwort abgerissen: {e}"))?;
    serde_json::from_slice(&puffer).map_err(|e| format!("greetd-Antwort unlesbar: {e}"))
}

/// Der komplette Login: Nutzer + Passwort gegen PAM, dann `cmd` als
/// Sitzungskommando. Ok = greetd übernimmt, der Greeter darf enden.
/// Für den Recovery-Account (`wache`, passwortlos) bleibt `passwort`
/// leer — greetd meldet dann direkt Erfolg ohne Secret-Prompt.
pub fn einloggen(nutzer: &str, passwort: &str, cmd: &str) -> Result<(), String> {
    let sock = std::env::var("GREETD_SOCK")
        .map_err(|_| String::from("kein GREETD_SOCK — läuft der Greeter unter greetd?"))?;
    let mut strom =
        UnixStream::connect(&sock).map_err(|e| format!("greetd-Socket: {e}"))?;

    let mut antwort = senden(
        &mut strom,
        &serde_json::json!({"type": "create_session", "username": nutzer}),
    )?;

    // Prompt-Schleife: Secrets beantworten wir mit dem Passwort,
    // Info/Fehler quittieren wir leer — bis Erfolg oder Abbruch.
    loop {
        match antwort {
            Antwort::Success => break,
            Antwort::Error { description, .. } => {
                let _ = senden(&mut strom, &serde_json::json!({"type": "cancel_session"}));
                return Err(if description.is_empty() {
                    String::from("Anmeldung abgelehnt")
                } else {
                    description
                });
            }
            Antwort::AuthMessage {
                auth_message_type, ..
            } => {
                let feld = match auth_message_type.as_str() {
                    "secret" => serde_json::json!({
                        "type": "post_auth_message_response",
                        "response": passwort
                    }),
                    // visible/info/error: zur Kenntnis genommen
                    _ => serde_json::json!({
                        "type": "post_auth_message_response",
                        "response": null
                    }),
                };
                antwort = senden(&mut strom, &feld)?;
            }
        }
    }

    // Durch die Shell: Session-Execs dürfen Argumente tragen
    // (der Recovery-Befehl tat es — und starb als Ein-Wort-argv).
    match senden(
        &mut strom,
        &serde_json::json!({"type": "start_session", "cmd": ["sh", "-c", cmd], "env": []}),
    )? {
        Antwort::Success => Ok(()),
        Antwort::Error { description, .. } => Err(description),
        _ => Err(String::from("greetd: unerwartete Antwort auf start_session")),
    }
}
