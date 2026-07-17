//! Der WebKit-Träger von Matrix Web: ein randloses Fenster, NUR Inhalt.
//! Die reine MatrixKit-App (matrix-web) steuert ihn über stdin und
//! koppelt ihn per Compositor unter ihren Inhaltsbereich — der Nutzer
//! sieht EIN Fenster. Endet die Kit-App (stdin-EOF), endet der Träger.
//!
//! Protokoll (zeilenweise):
//!   rein : lade <url> | zurueck | vor | neu
//!   raus : titel <t> | uri <u> | fortschritt <0..1> | nav <0|1> <0|1>

use gtk::glib;
use gtk::prelude::*;
use webkit6::prelude::*;

fn main() {
    glib::set_prgname(Some("matrix-web-inhalt"));
    let app = gtk::Application::builder().build();
    app.connect_activate(aufbauen);
    // Ohne Argumente-Parsing (stdin gehört dem Protokoll).
    app.run_with_args::<&str>(&[]);
}

fn melden(was: &str) {
    use std::io::Write;
    let mut out = std::io::stdout();
    let _ = writeln!(out, "{was}");
    let _ = out.flush();
}

fn aufbauen(app: &gtk::Application) {
    let fenster = gtk::ApplicationWindow::builder()
        .application(app)
        .default_width(1160)
        .default_height(730)
        .decorated(false)
        .build();

    let web = webkit6::WebView::new();
    web.load_uri("https://start.duckduckgo.com");
    fenster.set_child(Some(&web));
    fenster.present();

    // Ereignisse an die Kit-App.
    web.connect_title_notify(|w| melden(&format!("titel {}", w.title().unwrap_or_default())));
    web.connect_uri_notify(|w| melden(&format!("uri {}", w.uri().unwrap_or_default())));
    web.connect_estimated_load_progress_notify(|w| {
        melden(&format!("fortschritt {:.3}", w.estimated_load_progress()));
    });
    {
        let web2 = web.clone();
        web.connect_load_changed(move |w, _| {
            melden(&format!(
                "nav {} {}",
                w.can_go_back() as u8,
                w.can_go_forward() as u8
            ));
            let _ = &web2;
        });
    }

    // Kommandos der Kit-App: stdin-Thread → Kanal → GTK-Puls.
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        for zeile in stdin.lock().lines() {
            match zeile {
                Ok(z) => {
                    if tx.send(z).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        // Eltern weg → der Träger geht mit.
        std::process::exit(0);
    });
    glib::timeout_add_local(std::time::Duration::from_millis(30), move || {
        while let Ok(z) = rx.try_recv() {
            let mut teile = z.splitn(2, ' ');
            match (teile.next(), teile.next()) {
                (Some("lade"), Some(url)) => web.load_uri(url),
                (Some("zurueck"), _) => web.go_back(),
                (Some("vor"), _) => web.go_forward(),
                (Some("neu"), _) => web.reload(),
                _ => {}
            }
        }
        glib::ControlFlow::Continue
    });
}
