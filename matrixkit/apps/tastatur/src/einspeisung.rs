//! Die Einspeisung — tippt über `zwp_virtual_keyboard_v1` in das
//! FOKUSSIERTE Fenster, egal welches Toolkit dort wohnt (MatrixKit,
//! WebKit, Greeter-Passwortfeld). Der Trick stammt aus wtype: Wir laden
//! eine EIGENE Keymap hoch, in der jedes Zeichen unserer Tasten seinen
//! eigenen Keycode hat — Umlaute und Großbuchstaben inklusive, ganz
//! ohne Modifier-Gymnastik. Ein Tastendruck ist dann genau ein
//! press/release-Paar auf „seinem" Code.

use std::io::{Seek, Write};
use std::os::fd::AsFd;
use std::sync::mpsc;
use std::time::Instant;

use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

/// Was die Fläche der Einspeisung schickt.
#[derive(Debug, Clone)]
pub enum Befehl {
    /// Ein Zeichen unserer Tasten (inkl. Umlaute, Großbuchstaben, Leerzeichen).
    Zeichen(char),
    /// Ein benannter XKB-Keysym: "Return", "BackSpace", "Tab", "Escape".
    Name(&'static str),
}

struct Zustand {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<ZwpVirtualKeyboardManagerV1>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for Zustand {
    fn event(
        zustand: &mut Self,
        registry: &wl_registry::WlRegistry,
        ereignis: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = ereignis {
            match interface.as_str() {
                "wl_seat" => {
                    zustand.seat = Some(registry.bind(name, version.min(7), qh, ()));
                }
                "zwp_virtual_keyboard_manager_v1" => {
                    zustand.manager = Some(registry.bind(name, 1, qh, ()));
                }
                _ => {}
            }
        }
    }
}

wayland_client::delegate_noop!(Zustand: ignore wl_seat::WlSeat);
wayland_client::delegate_noop!(Zustand: ignore ZwpVirtualKeyboardManagerV1);
wayland_client::delegate_noop!(Zustand: ignore ZwpVirtualKeyboardV1);

/// Keysym-Name eines Zeichens: Unicode-Schreibweise (libxkbcommon versteht
/// `U00E4` für ä), nur das Leerzeichen bekommt seinen klassischen Namen.
fn keysym(zeichen: char) -> String {
    if zeichen == ' ' {
        String::from("space")
    } else {
        format!("U{:04X}", zeichen as u32)
    }
}

/// Die komplette Keymap: Eintrag i → XKB-Keycode i+9 → Draht-Code i+1
/// (Wayland zieht 8 ab). Typen/Kompatibilität kommen aus den System-
/// XKB-Daten („complete"), die jeder Compositor ohnehin lädt.
///
/// `variante` prägt einen Dummy-Eintrag auf Keycode 254 (F19/F20, wird
/// nie gesendet): Smithay dedupliziert Keymaps über einen SHA-256 des
/// SERIALISIERTEN Inhalts und seat-GLOBAL — Fenster, die NACH unserem
/// Upload fokussiert werden, kennen nur die echte Tastatur-Keymap und
/// deuten unsere Codes falsch (Nutzer-Fund: „." tippte „q"). Zwei
/// abwechselnde Varianten zwingen vor jedem Tastendruck eine frische
/// Auslieferung an das gerade fokussierte Fenster.
fn keymap_text(eintraege: &[String], variante: bool) -> String {
    let mut s = String::from(
        "xkb_keymap {\nxkb_keycodes \"(unnamed)\" {\nminimum = 8;\nmaximum = 255;\n",
    );
    for i in 0..eintraege.len() {
        s.push_str(&format!("<K{}> = {};\n", i + 1, i + 9));
    }
    s.push_str("<KZZ> = 254;\n");
    s.push_str(
        "};\nxkb_types \"(unnamed)\" { include \"complete\" };\n\
         xkb_compatibility \"(unnamed)\" { include \"complete\" };\n\
         xkb_symbols \"(unnamed)\" {\n",
    );
    for (i, name) in eintraege.iter().enumerate() {
        s.push_str(&format!("key <K{}> {{[ {} ]}};\n", i + 1, name));
    }
    s.push_str(if variante { "key <KZZ> {[ F19 ]};\n" } else { "key <KZZ> {[ F20 ]};\n" });
    s.push_str("};\n};\n");
    s
}

/// Der Einspeisungs-Faden: eigene Wayland-Verbindung, eigene Queue —
/// die iced-Fläche schickt Befehle über den Kanal. Gibt `None` zurück,
/// wenn der Compositor kein Virtual-Keyboard anbietet (dann zeigt die
/// Fläche zwar Tasten, aber tippen ins Leere wäre gelogen — der Rufer
/// darf das melden).
pub fn starten(zeichen: Vec<char>) -> Option<mpsc::Sender<Befehl>> {
    let conn = Connection::connect_to_env().ok()?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let display = conn.display();
    let _registry = display.get_registry(&qh, ());
    let mut zustand = Zustand { seat: None, manager: None };
    queue.roundtrip(&mut zustand).ok()?;
    let seat = zustand.seat.clone()?;
    let manager = zustand.manager.clone()?;
    let vk = manager.create_virtual_keyboard(&seat, &qh, ());

    // Keymap: erst die benannten Spezialtasten, dann alle Zeichen.
    const SPEZIAL: &[&str] = &["Return", "BackSpace", "Tab", "Escape"];
    let mut eintraege: Vec<String> = SPEZIAL.iter().map(|s| s.to_string()).collect();
    let mut zeichen = zeichen;
    zeichen.sort_unstable();
    zeichen.dedup();
    for z in &zeichen {
        eintraege.push(keysym(*z));
    }
    // Beide Varianten als Datei-Deskriptoren vorbereiten (siehe
    // keymap_text: abwechselnd hochladen erzwingt die Auslieferung).
    let lauf = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let mut dateien = Vec::new();
    for variante in [true, false] {
        let text = keymap_text(&eintraege, variante);
        let pfad = std::path::PathBuf::from(&lauf).join(format!(
            "matrix-tastatur-keymap-{}-{}",
            std::process::id(),
            variante
        ));
        let mut datei = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&pfad)
            .ok()?;
        datei.write_all(text.as_bytes()).ok()?;
        datei.rewind().ok()?;
        let _ = std::fs::remove_file(&pfad);
        dateien.push((datei, text.len() as u32));
    }
    vk.keymap(1, dateien[0].0.as_fd(), dateien[0].1);
    // Ein neutraler Modifier-Stand gehört zum Protokoll-Anstand.
    vk.modifiers(0, 0, 0, 0);
    queue.roundtrip(&mut zustand).ok()?;

    // Draht-Code eines Befehls in UNSERER Keymap.
    let code_von = move |befehl: &Befehl| -> Option<u32> {
        let name = match befehl {
            Befehl::Name(n) => (*n).to_string(),
            Befehl::Zeichen(z) => keysym(*z),
        };
        eintraege.iter().position(|e| *e == name).map(|i| i as u32 + 1)
    };

    let (sender, empfaenger) = mpsc::channel::<Befehl>();
    std::thread::spawn(move || {
        let start = Instant::now();
        let mut naechste = 1usize; // Start-Upload war dateien[0]
        for befehl in empfaenger {
            let Some(code) = code_von(&befehl) else { continue };
            // Vor JEDEM Tastendruck die jeweils andere Keymap-Variante:
            // frischer SHA → Smithay MUSS sie dem fokussierten Fenster
            // ausliefern, bevor der Key ankommt (Protokoll-Reihenfolge).
            let (datei, laenge) = &dateien[naechste];
            vk.keymap(1, datei.as_fd(), *laenge);
            naechste = 1 - naechste;
            let t = start.elapsed().as_millis() as u32;
            vk.key(t, code, 1);
            vk.key(t.saturating_add(8), code, 0);
            if queue.roundtrip(&mut zustand).is_err() {
                break; // Compositor weg — Faden endet leise.
            }
        }
    });
    Some(sender)
}

/// `--tipptest <text>`: blind in das fokussierte Fenster tippen —
/// der Beweis, dass die Einspeisung trägt, ohne dass jemand die
/// Fläche berühren muss.
pub fn tipptest(text: &str) {
    let Some(kanal) = starten(text.chars().collect()) else {
        eprintln!("tipptest: kein Virtual-Keyboard am Compositor");
        std::process::exit(1);
    };
    for z in text.chars() {
        let _ = kanal.send(Befehl::Zeichen(z));
    }
    // Der Faden arbeitet den Kanal ab; kurz warten, dann sauber gehen.
    std::thread::sleep(std::time::Duration::from_millis(400));
}
