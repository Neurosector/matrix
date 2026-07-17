//! Das System-Tray — Statusleisten-Leitbild-Extrakt (Runde 24, Leitbild-Toolkit Z. 25):
//! Die Bar IST der StatusNotifierWatcher + Host (org.kde-Protokoll,
//! dasselbe zbus-Muster wie der Mitteilungs-Daemon). Items melden sich
//! per DBus an, ihre Icons (IconPixmap ARGB32 bzw. IconName-Lookup)
//! erscheinen als Knöpfe in Zone 3; Klick = Activate, Rechtsklick =
//! SecondaryActivate. Damit fällt die LETZTE fremde Fläche der
//! Systemkarte — auch wenn heute noch keine App ein Item setzt:
//! Das Tray wartet bereit.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct TrayItem {
    pub dienst: String,
    pub pfad: String,
    pub titel: String,
    /// RGBA8-Rohdaten + Kantenlänge (aus IconPixmap) — iced-fertig.
    pub icon: Option<(u32, u32, Vec<u8>)>,
}

pub enum TrayBefehl {
    Aktivieren(String, String),
    Sekundaer(String, String),
}

#[derive(Default)]
struct WatcherStand {
    items: Vec<String>,
}

struct Watcher {
    stand: Arc<Mutex<WatcherStand>>,
    geaendert: mpsc::Sender<()>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(
        &self,
        service: String,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) {
        // Spez-Kultur: manche Clients senden nur einen Pfad — dann ist
        // der Absender der Dienst.
        let eintrag = if service.starts_with('/') {
            let wer = header
                .sender()
                .map(|s| s.to_string())
                .unwrap_or_default();
            format!("{wer}{service}")
        } else if service.starts_with(':') {
            format!("{service}/StatusNotifierItem")
        } else {
            format!("{service}/StatusNotifierItem")
        };
        let mut g = self.stand.lock().unwrap();
        if !g.items.contains(&eintrag) {
            g.items.push(eintrag);
        }
        let _ = self.geaendert.send(());
    }

    fn register_status_notifier_host(&self, _service: String) {}

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.stand.lock().unwrap().items.clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

fn icon_laden(conn: &zbus::blocking::Connection, dienst: &str, pfad: &str) -> Option<(u32, u32, Vec<u8>)> {
    let proxy = zbus::blocking::Proxy::new(
        conn,
        dienst.to_string(),
        pfad.to_string(),
        "org.kde.StatusNotifierItem",
    )
    .ok()?;
    // IconPixmap: a(iiay) — größte Variante nehmen, ARGB→RGBA drehen.
    if let Ok(pixmaps) = proxy.get_property::<Vec<(i32, i32, Vec<u8>)>>("IconPixmap") {
        if let Some((w, h, daten)) = pixmaps.into_iter().max_by_key(|(w, _, _)| *w) {
            if w > 0 && h > 0 && daten.len() >= (w * h * 4) as usize {
                let mut rgba = Vec::with_capacity(daten.len());
                for px in daten.chunks_exact(4) {
                    // Netzwerk-ARGB → RGBA
                    rgba.extend_from_slice(&[px[1], px[2], px[3], px[0]]);
                }
                return Some((w as u32, h as u32, rgba));
            }
        }
    }
    None
}

fn titel_laden(conn: &zbus::blocking::Connection, dienst: &str, pfad: &str) -> String {
    zbus::blocking::Proxy::new(conn, dienst.to_string(), pfad.to_string(), "org.kde.StatusNotifierItem")
        .ok()
        .and_then(|p| p.get_property::<String>("Title").ok())
        .unwrap_or_default()
}

/// Startet Watcher+Host; liefert die lebende Item-Liste und einen
/// Befehls-Sender (Activate). Die Bar pollt die Liste in ihrem Puls.
pub fn starten() -> (Arc<Mutex<Vec<TrayItem>>>, mpsc::Sender<TrayBefehl>) {
    let items: Arc<Mutex<Vec<TrayItem>>> = Arc::new(Mutex::new(Vec::new()));
    let (befehl_tx, befehl_rx) = mpsc::channel::<TrayBefehl>();
    let items_thread = items.clone();

    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<()>();
        let stand = Arc::new(Mutex::new(WatcherStand::default()));
        let mach = || -> zbus::Result<zbus::blocking::Connection> {
            let conn = zbus::blocking::connection::Builder::session()?
                .serve_at(
                    "/StatusNotifierWatcher",
                    Watcher { stand: stand.clone(), geaendert: tx.clone() },
                )?
                .build()?;
            use zbus::fdo::RequestNameFlags;
            let dbus = zbus::blocking::fdo::DBusProxy::new(&conn)?;
            let _ = dbus.request_name(
                "org.kde.StatusNotifierWatcher".try_into().unwrap(),
                RequestNameFlags::AllowReplacement | RequestNameFlags::ReplaceExisting,
            );
            Ok(conn)
        };
        let Ok(conn) = mach() else {
            eprintln!("matrix-bar: Tray-Watcher-Start scheiterte");
            return;
        };
        loop {
            // Befehle (Klicks) ausführen.
            while let Ok(b) = befehl_rx.try_recv() {
                match b {
                    TrayBefehl::Aktivieren(d, p) => {
                        if let Ok(proxy) = zbus::blocking::Proxy::new(
                            &conn, d, p, "org.kde.StatusNotifierItem",
                        ) {
                            let _ = proxy.call_method("Activate", &(0i32, 0i32));
                        }
                    }
                    TrayBefehl::Sekundaer(d, p) => {
                        if let Ok(proxy) = zbus::blocking::Proxy::new(
                            &conn, d, p, "org.kde.StatusNotifierItem",
                        ) {
                            let _ = proxy.call_method("SecondaryActivate", &(0i32, 0i32));
                        }
                    }
                }
            }
            // Registrierungen → Items samt Icons neu einlesen.
            if rx.try_recv().is_ok() {
                let liste = stand.lock().unwrap().items.clone();
                let mut neu = Vec::new();
                for eintrag in liste {
                    let (dienst, pfad) = match eintrag.find('/') {
                        Some(i) => (eintrag[..i].to_string(), eintrag[i..].to_string()),
                        None => (eintrag.clone(), String::from("/StatusNotifierItem")),
                    };
                    neu.push(TrayItem {
                        icon: icon_laden(&conn, &dienst, &pfad),
                        titel: titel_laden(&conn, &dienst, &pfad),
                        dienst,
                        pfad,
                    });
                }
                *items_thread.lock().unwrap() = neu;
            }
            std::thread::sleep(std::time::Duration::from_millis(120));
        }
    });

    (items, befehl_tx)
}
