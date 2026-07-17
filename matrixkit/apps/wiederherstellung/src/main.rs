//! Matrix Wiederherstellung — die geführte Recovery des Wächters (App #7).
//!
//! Erscheint, wenn jemand am Login-Bildschirm sein Passwort vergessen hat.
//! Der Wächter begleitet Schritt für Schritt (wie die macOS-Recovery):
//!
//!   Willkommen → Auswahl (Konto oder ganzes System) → Speicher-Diagramm
//!   (was wird gelöscht) → Bestätigung → Löschung → neues Konto → fertig.
//!
//! Philosophie: Ein vergessenes Passwort macht kein Gerät unbrauchbar.
//! Der Weg zurück ist die KORREKTE, vollständige Löschung der
//! personenbezogenen Daten des Kontos — niemals deren Preisgabe. Das
//! Betriebssystem ist ein unveränderliches Abbild: nach der Löschung der
//! Nutzerdaten ist es wieder die frische Installation.
//!
//! Die eigentlichen Systemeingriffe macht das Root-Werkzeug `matrix-wache`
//! (diese App ruft es über `sudo -n` — in der Recovery-Sitzung darf der
//! Benutzer `wache` es passwortlos aufrufen).

use iced::widget::{column, container, row, Space};
use iced::{Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::time::Duration;

const APP_ID: &str = "matrix-wiederherstellung";
/// Abstand zwischen den lauten Wächter-Rufen während der Frist (Sekunden).
const RUF_INTERVALL: u64 = 5 * 60;

/// Den Wächter-Ruf (Klang 14) laut abspielen — die Umgebung soll hören,
/// dass gerade eine Löschung ansteht. Feuert und vergisst.
fn waechter_ruf() {
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg("f=/usr/share/matrix/klaenge/14-waechter-ruf.wav; [ -e \"$f\" ] || f=/var/cache/dms-greeter/14-waechter-ruf.wav; pw-play \"$f\" 2>/dev/null || paplay \"$f\" 2>/dev/null || aplay -q \"$f\" 2>/dev/null &")
        .spawn();
}

fn main() -> iced::Result {
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| String::from("Matrix Wiederherstellung"))
        .subscription(App::subscription)
        .window(mkw::fenster_settings(APP_ID, 720.0, 560.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

fn wache_pfad() -> String {
    for p in ["/usr/local/bin/matrix-wache", "/usr/bin/matrix-wache"] {
        if std::path::Path::new(p).exists() {
            return p.to_string();
        }
    }
    "matrix-wache".to_string()
}

/// `sudo -n matrix-wache <args>` — Ausgabe (stdout) oder Fehlertext.
fn wache(args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("sudo")
        .arg("-n")
        .arg(wache_pfad())
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "Aktion fehlgeschlagen.".into() } else { err })
    }
}

/// Ergebnis eines Löschversuchs: fertig, noch in der Wächter-Frist
/// (Restsekunden) oder ein echter Fehler.
#[derive(Debug, Clone)]
enum LoeschErgebnis {
    Fertig,
    Frist(u64),
    Fehler(String),
}

/// Löschen bzw. Werkseinstellung anstoßen. Exit 3 + „rest=N" = Frist läuft
/// noch (Schutz gegen unbefugtes Löschen); die App zeigt dann den Countdown.
fn wache_loeschen(ziel: &Ziel) -> LoeschErgebnis {
    let args: Vec<&str> = match ziel {
        Ziel::Konto(u) => vec!["loeschen", "--user", u],
        Ziel::System => vec!["werkseinstellung"],
    };
    let out = match std::process::Command::new("sudo")
        .arg("-n")
        .arg(wache_pfad())
        .args(&args)
        .output()
    {
        Ok(o) => o,
        Err(e) => return LoeschErgebnis::Fehler(e.to_string()),
    };
    if out.status.success() {
        return LoeschErgebnis::Fertig;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    if out.status.code() == Some(3) {
        if let Some(rest) = stdout
            .lines()
            .find_map(|z| z.strip_prefix("rest=").and_then(|v| v.trim().parse::<u64>().ok()))
        {
            return LoeschErgebnis::Frist(rest);
        }
    }
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    LoeschErgebnis::Fehler(if err.is_empty() { "Löschen fehlgeschlagen.".into() } else { err })
}

/// `matrix-wache anlegen` mit Passwort über stdin.
fn wache_anlegen(user: &str, passwort: &str) -> Result<String, String> {
    use std::io::Write;
    let mut kind = std::process::Command::new("sudo")
        .arg("-n")
        .arg(wache_pfad())
        .args(["anlegen", "--user", user, "--passwort-stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut si) = kind.stdin.take() {
        let _ = si.write_all(passwort.as_bytes());
    }
    let out = kind.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "Anlegen fehlgeschlagen.".into() } else { err })
    }
}

fn nutzer_lesen() -> Vec<String> {
    wache(&["nutzer"])
        .map(|s| {
            s.lines()
                .filter_map(|z| z.split('=').next().map(|n| n.trim().to_string()))
                .filter(|n| !n.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Eine Speicher-Kategorie mit Bytes.
#[derive(Debug, Clone)]
struct Anteil {
    schluessel: String,
    label: String,
    bytes: u64,
}

fn label_fuer(schluessel: &str) -> &'static str {
    match schluessel {
        "bilder" => "Bilder",
        "musik" => "Musik",
        "dokumente" => "Dokumente",
        "videos" => "Videos",
        "downloads" => "Downloads",
        "schreibtisch" => "Schreibtisch",
        "rest" => "Sonstiges",
        _ => "Sonstiges",
    }
}

fn analyse_lesen(user: &str) -> (Vec<Anteil>, u64) {
    let mut anteile = Vec::new();
    let mut gesamt = 0u64;
    if let Ok(s) = wache(&["analyse", "--user", user]) {
        for z in s.lines() {
            if let Some((k, v)) = z.split_once('=') {
                let bytes: u64 = v.trim().parse().unwrap_or(0);
                if k == "gesamt" {
                    gesamt = bytes;
                } else {
                    anteile.push(Anteil {
                        schluessel: k.to_string(),
                        label: label_fuer(k).to_string(),
                        bytes,
                    });
                }
            }
        }
    }
    // größte zuerst — das Diagramm liest sich dann natürlich
    anteile.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    (anteile, gesamt)
}

/// Bytes menschenlesbar — delegiert an die Formatter-Kultur (mk::format).
fn menge(bytes: u64) -> String {
    mk::format::bytes(bytes)
}

/// Farbe je Kategorie aus der Palette (nie feste Hexwerte).
fn kat_farbe(schluessel: &str, p: mk::Palette) -> mk::Rgba {
    match schluessel {
        "bilder" => p.primary,
        "musik" => p.secondary,
        "dokumente" => p.tertiary,
        "videos" => p.primary_container,
        "downloads" => p.on_surface_variant,
        "schreibtisch" => p.outline,
        _ => p.surface_container_high,
    }
}

#[derive(Clone, PartialEq)]
enum Schritt {
    Willkommen,
    Auswahl,
    Analyse,
    /// Wächter-Frist: Wartezeit vor der Löschung (Schutz vor Fremden).
    Frist,
    Loeschung,
    Neuanlage,
    Fertig,
}

/// Was zurückgesetzt wird.
#[derive(Clone, PartialEq)]
enum Ziel {
    Konto(String),
    System,
}

struct App {
    palette: mk::Palette,
    watcher: mk::PaletteWatcher,
    icon: Option<iced::widget::image::Handle>,
    schritt: Schritt,
    nutzer: Vec<String>,
    ziel: Option<Ziel>,
    anteile: Vec<Anteil>,
    gesamt: u64,
    dialog: mkw::DialogZustand,
    laeuft: bool,
    /// Analyse läuft noch (Redaction-Platzhalter statt Zahlen).
    laden: bool,
    meldung: Option<(String, bool)>,
    // Neuanlage
    neu_name: String,
    neu_pw: String,
    neu_pw2: String,
    puls: mk::motion::Spring,
    /// Wächter-Frist: verbleibende Sekunden bis zur erlaubten Löschung.
    frist_rest: u64,
    /// Sekunden seit dem letzten Wächter-Ruf während der Frist.
    seit_ruf: u64,
}

#[derive(Debug, Clone)]
enum Msg {
    Tick,
    AnimTick,
    Taste(mkw::Taste),
    Weiter,
    Zurueck,
    ZielKonto(String),
    ZielSystem,
    Analyse((Vec<Anteil>, u64)),
    LoeschenFragen,
    LoeschenAbbrechen,
    LoeschenBestaetigen,
    /// Ergebnis eines Löschversuchs (fertig / Frist läuft / Fehler).
    LoeschAntwort(LoeschErgebnis),
    /// 1-Sekunden-Takt während der Wächter-Frist (Countdown + Ruf + Poll).
    FristTakt,
    /// Frist abbrechen und zurück zum Diagramm.
    FristAbbrechen,
    NeuName(String),
    NeuPw(String),
    NeuPw2(String),
    Anlegen,
    Angelegt(Result<String, String>),
    Beenden,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let palette = mk::Palette::load().unwrap_or_default();
        // Dev-Sprung für Screenshots: MATRIX_WH_SCHRITT=auswahl|analyse|neuanlage|fertig
        let (schritt, anteile, gesamt, ziel) = match std::env::var("MATRIX_WH_SCHRITT").ok().as_deref() {
            Some("auswahl") => (Schritt::Auswahl, Vec::new(), 0, None),
            Some("analyse") => {
                let u = nutzer_lesen().into_iter().next().unwrap_or_else(|| "nutzer".into());
                let (a, g) = analyse_lesen(&u);
                (Schritt::Analyse, a, g, Some(Ziel::Konto(u)))
            }
            Some("neuanlage") => (Schritt::Neuanlage, Vec::new(), 0, None),
            Some("fertig") => (Schritt::Fertig, Vec::new(), 0, None),
            // Dev-Screenshot: ziel=None → der Poll ruht, der Countdown bleibt stehen
            Some("frist") => (Schritt::Frist, Vec::new(), 0, None),
            _ => (Schritt::Willkommen, Vec::new(), 0, None),
        };
        let frist_rest = if schritt == Schritt::Frist { 1740 } else { 0 };
        (
            Self {
                icon: matrixkit_icons::render_png(APP_ID, &palette)
                    .map(iced::widget::image::Handle::from_bytes),
                palette,
                watcher: mk::PaletteWatcher::new(),
                schritt,
                nutzer: nutzer_lesen(),
                ziel,
                anteile,
                gesamt,
                dialog: mkw::DialogZustand::neu(),
                laeuft: false,
                laden: false,
                meldung: None,
                neu_name: String::new(),
                neu_pw: String::new(),
                neu_pw2: String::new(),
                puls: mk::motion::Spring::federnd(0.0),
                frist_rest,
                seit_ruf: 0,
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Tick => {
                if self.watcher.changed() {
                    if let Some(p) = mk::Palette::load() {
                        self.palette = p;
                        self.icon = matrixkit_icons::render_png(APP_ID, &p)
                            .map(iced::widget::image::Handle::from_bytes);
                    }
                }
                // Der Wächter pulsiert leise, solange er begleitet
                if self.puls.is_settled() {
                    self.puls.retarget(if self.puls.value > 0.5 { 0.0 } else { 1.0 });
                }
                self.puls.tick(1.0 / 60.0);
                self.dialog.tick();
                Task::none()
            }
            Msg::AnimTick => {
                self.puls.tick(1.0 / 60.0);
                self.dialog.tick();
                Task::none()
            }
            Msg::Weiter => {
                self.schritt = match self.schritt {
                    Schritt::Willkommen => {
                        self.nutzer = nutzer_lesen();
                        Schritt::Auswahl
                    }
                    _ => self.schritt.clone(),
                };
                Task::none()
            }
            Msg::Zurueck => {
                self.meldung = None;
                self.schritt = match self.schritt {
                    Schritt::Analyse => Schritt::Auswahl,
                    Schritt::Auswahl => Schritt::Willkommen,
                    ref other => other.clone(),
                };
                Task::none()
            }
            Msg::ZielKonto(u) => {
                self.ziel = Some(Ziel::Konto(u.clone()));
                self.schritt = Schritt::Analyse;
                // Die Vermessung (du über das ganze Home) darf die UI nie
                // blockieren — Platzhalter zeigen solange die FORM (Redaction).
                self.laden = true;
                self.anteile = Vec::new();
                self.gesamt = 0;
                Task::perform(async move { analyse_lesen(&u) }, Msg::Analyse)
            }
            Msg::ZielSystem => {
                self.ziel = Some(Ziel::System);
                self.schritt = Schritt::Analyse;
                self.laden = true;
                self.anteile = Vec::new();
                self.gesamt = 0;
                Task::perform(
                    async move {
                        // Für das Diagramm alle Konten zusammenrechnen
                        let mut summe: std::collections::BTreeMap<String, u64> =
                            std::collections::BTreeMap::new();
                        let mut gesamt = 0u64;
                        for u in nutzer_lesen() {
                            let (a, g) = analyse_lesen(&u);
                            gesamt += g;
                            for an in a {
                                *summe.entry(an.schluessel).or_default() += an.bytes;
                            }
                        }
                        let mut anteile: Vec<Anteil> = summe
                            .into_iter()
                            .map(|(k, bytes)| Anteil { label: label_fuer(&k).to_string(), schluessel: k, bytes })
                            .collect();
                        anteile.sort_by(|a, b| b.bytes.cmp(&a.bytes));
                        (anteile, gesamt)
                    },
                    Msg::Analyse,
                )
            }
            Msg::Analyse((anteile, gesamt)) => {
                self.laden = false;
                self.anteile = anteile;
                self.gesamt = gesamt;
                Task::none()
            }
            Msg::LoeschenFragen => {
                self.dialog.oeffnen();
                Task::none()
            }
            Msg::LoeschenAbbrechen => {
                self.dialog.schliessen();
                Task::none()
            }
            Msg::LoeschenBestaetigen => {
                self.dialog.schliessen();
                self.laeuft = true;
                self.schritt = Schritt::Loeschung;
                self.meldung = Some(("Der Wächter prüft …".into(), false));
                let Some(ziel) = self.ziel.clone() else { return Task::none() };
                Task::perform(async move { wache_loeschen(&ziel) }, Msg::LoeschAntwort)
            }
            Msg::LoeschAntwort(erg) => {
                self.laeuft = false;
                match erg {
                    LoeschErgebnis::Fertig => {
                        self.meldung = Some(("Löschung abgeschlossen. Jetzt kann ein frisches Konto entstehen.".into(), false));
                        self.schritt = Schritt::Neuanlage;
                        Task::none()
                    }
                    LoeschErgebnis::Frist(rest) => {
                        // Schutz greift: warten (oder Login-Schlüssel einstecken).
                        // Beim ERSTEN Eintritt sofort einmal rufen; bei den
                        // periodischen Re-Checks nur die Restzeit aktualisieren.
                        if self.schritt != Schritt::Frist {
                            self.seit_ruf = RUF_INTERVALL;
                            self.schritt = Schritt::Frist;
                            self.meldung = None;
                        }
                        self.frist_rest = rest;
                        Task::none()
                    }
                    LoeschErgebnis::Fehler(e) => {
                        self.meldung = Some((e, true));
                        self.schritt = Schritt::Analyse;
                        Task::none()
                    }
                }
            }
            Msg::FristTakt => {
                if self.schritt != Schritt::Frist {
                    return Task::none();
                }
                self.frist_rest = self.frist_rest.saturating_sub(1);
                self.seit_ruf += 1;
                // Der Wächter ruft regelmäßig laut — die Umgebung hört mit.
                if self.seit_ruf >= RUF_INTERVALL {
                    self.seit_ruf = 0;
                    waechter_ruf();
                }
                // Alle paar Sekunden neu versuchen: Zeit abgelaufen ODER
                // Login-Schlüssel eingesteckt → das Werkzeug lässt uns durch.
                if self.frist_rest == 0 || self.frist_rest % 3 == 0 {
                    let Some(ziel) = self.ziel.clone() else { return Task::none() };
                    return Task::perform(async move { wache_loeschen(&ziel) }, Msg::LoeschAntwort);
                }
                Task::none()
            }
            Msg::FristAbbrechen => {
                self.schritt = Schritt::Analyse;
                self.meldung = None;
                Task::none()
            }
            Msg::NeuName(n) => {
                self.neu_name = n.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-' || *c == '_').collect();
                Task::none()
            }
            Msg::NeuPw(p) => {
                self.neu_pw = p;
                Task::none()
            }
            Msg::NeuPw2(p) => {
                self.neu_pw2 = p;
                Task::none()
            }
            Msg::Anlegen => {
                if self.neu_name.is_empty() {
                    self.meldung = Some(("Bitte einen Kontonamen wählen.".into(), true));
                    return Task::none();
                }
                if self.neu_pw.len() < 4 {
                    self.meldung = Some(("Das Passwort braucht mindestens 4 Zeichen.".into(), true));
                    return Task::none();
                }
                if self.neu_pw != self.neu_pw2 {
                    self.meldung = Some(("Die beiden Passwörter stimmen nicht überein.".into(), true));
                    return Task::none();
                }
                self.laeuft = true;
                self.meldung = Some(("Lege das Konto an …".into(), false));
                let (name, pw) = (self.neu_name.clone(), self.neu_pw.clone());
                Task::perform(async move { wache_anlegen(&name, &pw) }, Msg::Angelegt)
            }
            Msg::Angelegt(r) => {
                self.laeuft = false;
                match r {
                    Ok(_) => {
                        self.neu_pw.clear();
                        self.neu_pw2.clear();
                        self.meldung = None;
                        self.schritt = Schritt::Fertig;
                    }
                    Err(e) => self.meldung = Some((e, true)),
                }
                Task::none()
            }
            Msg::Beenden => {
                // Zurück zum Login-Bildschirm: die ganze Recovery-Sitzung
                // sauber beenden (nicht nur die App — sonst bliebe ein leerer
                // niri-Hintergrund). niri beendet sich → greetd zeigt wieder
                // den Login. Robust, egal wie die App gestartet wurde.
                mk::leinwand::abmelden(false);
                iced::exit()
            }
            Msg::Taste(t) => {
                if matches!(t, mkw::Taste::Escape) && self.dialog.offen() {
                    self.dialog.schliessen();
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        let takt = mkw::tick("wiederherstellung", Duration::from_secs(2)).map(|_| Msg::Tick);
        let tasten = mkw::tasten_abo(Msg::Taste);
        let mut abos = vec![takt, tasten];
        if self.dialog.animiert() || !self.puls.is_settled() {
            abos.push(mkw::tick("wh-anim", Duration::from_millis(16)).map(|_| Msg::AnimTick));
        }
        // Während der Wächter-Frist tickt ein 1-Sekunden-Countdown (der auch
        // den Login-Schlüssel und den Zeitablauf poll't).
        if self.schritt == Schritt::Frist {
            abos.push(mkw::tick("wh-frist", Duration::from_secs(1)).map(|_| Msg::FristTakt));
        }
        Subscription::batch(abos)
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let inhalt = match self.schritt {
            Schritt::Willkommen => self.willkommen(),
            Schritt::Auswahl => self.auswahl(),
            Schritt::Analyse => self.analyse_ansicht(),
            Schritt::Frist => self.frist_ansicht(),
            Schritt::Loeschung => self.loeschung(),
            Schritt::Neuanlage => self.neuanlage(),
            Schritt::Fertig => self.fertig(),
        };

        // Inhalt in einer zentrierten Spalte begrenzter Breite — sonst
        // streut er im Vollbild oben-links auseinander (Recovery läuft in
        // einer eigenen, maximierten niri-Sitzung).
        let karte = container(
            container(inhalt)
                .max_width(560.0)
                .height(Length::Fill)
                .padding(mk::spacing::XXL),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center);

        let mit_dialog: Element<'_, Msg> = if self.dialog.sichtbar() {
            let botschaft = match &self.ziel {
                Some(Ziel::System) => "ALLE Konten und ihre persönlichen Daten werden vollständig gelöscht. Das System wird auf Werkseinstellung zurückgesetzt — eine frische Installation. Dieser Schritt lässt sich nicht rückgängig machen.".to_string(),
                Some(Ziel::Konto(u)) => format!("Das Konto „{u}“ und alle seine persönlichen Daten ({}) werden vollständig gelöscht. Andere Konten bleiben unberührt. Dieser Schritt lässt sich nicht rückgängig machen.", menge(self.gesamt)),
                None => "Es ist kein Ziel gewählt.".to_string(),
            };
            iced::widget::stack![
                karte,
                mkw::bestaetigung(
                    "Wirklich unwiderruflich löschen?",
                    botschaft,
                    "Endgültig löschen",
                    Msg::LoeschenBestaetigen,
                    Msg::LoeschenAbbrechen,
                    &self.dialog,
                    p,
                )
            ]
            .into()
        } else {
            karte.into()
        };

        // Vollflächiger Wiederherstellungs-Grund (kein Fenster-Chrome —
        // das ist eine eigene Sitzung, kein App-Fenster).
        container(mit_dialog)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(color(p.surface).into()),
                ..Default::default()
            })
            .into()
    }
}

// --- Bausteine ---------------------------------------------------------------

impl App {
    /// Kopf mit Wächter-Icon + Titel — auf jedem Schritt gleich.
    fn wachter_kopf<'a>(&'a self, titel: String, unter: &'a str) -> Element<'a, Msg> {
        let p = self.palette;
        let glanz = 0.6 + 0.4 * self.puls.value; // leises Pulsieren
        let icon: Element<'_, Msg> = match self.icon.clone() {
            Some(h) => container(iced::widget::image(h).width(Length::Fixed(64.0)).height(Length::Fixed(64.0)))
                .style(move |_| container::Style {
                    border: iced::Border { radius: mk::radius::GROSS.into(), ..Default::default() },
                    background: Some(color(mk::Rgba { a: 0.10 * glanz, ..p.primary }).into()),
                    ..Default::default()
                })
                .padding(6)
                .into(),
            None => Space::new().width(Length::Fixed(64.0)).height(Length::Fixed(64.0)).into(),
        };
        row![
            icon,
            Space::new().width(mk::spacing::L),
            column![
                mkw::txt("DER WÄCHTER", mk::typo::ETIKETT, p.primary),
                Space::new().height(mk::spacing::XXS),
                mkw::txt(titel, mk::typo::TITEL, p.on_surface),
                Space::new().height(mk::spacing::XXS),
                mkw::txt(unter, mk::typo::FLIESS, p.on_surface_variant),
            ]
            .spacing(0),
        ]
        .align_y(iced::Alignment::Center)
        .into()
    }

    fn knopf<'a>(&self, label: &'a str, grund: mk::Rgba, schrift: mk::Rgba, msg: Option<Msg>) -> Element<'a, Msg> {
        let p = self.palette;
        let aktiv = msg.is_some();
        iced::widget::button(mkw::txt(label, mk::typo::FLIESS, schrift).center())
            .width(Length::Fixed(220.0))
            .height(Length::Fixed(44.0))
            .on_press_maybe(msg)
            .style(move |_, status| {
                let base = if aktiv { grund } else { grund.over(p.surface, 0.5) };
                let bg = match status {
                    iced::widget::button::Status::Hovered => schrift.over(base, mk::state_layer::HOVER),
                    iced::widget::button::Status::Pressed => schrift.over(base, mk::state_layer::PRESSED),
                    _ => base,
                };
                // familien-ausnahme: Recovery-Großknöpfe: Hero-Maße mit eigenem Inhalt
                iced::widget::button::Style {
                    background: Some(color(bg).into()),
                    border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                    ..Default::default()
                }
            })
            .into()
    }

    fn text_knopf<'a>(&self, label: &'a str, msg: Msg) -> Element<'a, Msg> {
        let p = self.palette;
        iced::widget::button(mkw::txt(label, mk::typo::FLIESS, p.on_surface_variant).center())
            .width(Length::Fixed(220.0))
            .height(Length::Fixed(44.0))
            .on_press(msg)
            .style(move |_, status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(color(p.on_surface.over(p.surface, mk::state_layer::HOVER)).into()),
                    _ => None,
                };
                // familien-ausnahme: Recovery-Großknöpfe: Hero-Maße mit eigenem Inhalt
                iced::widget::button::Style {
                    background: bg,
                    border: iced::Border { radius: mk::CORNER_RADIUS.into(), width: 1.0, color: color(p.outline.over(p.surface, 0.4)) },
                    ..Default::default()
                }
            })
            .into()
    }

    fn absatz<'a>(&self, s: &'a str) -> Element<'a, Msg> {
        mkw::txt(s, mk::typo::FLIESS, self.palette.on_surface_variant).into()
    }

    fn willkommen(&self) -> Element<'_, Msg> {
        column![
            self.wachter_kopf("Wiederherstellung".to_string(), "Der Wächter begleitet dich zurück ins System."),
            Space::new().height(mk::spacing::XL),
            self.absatz("Du hast dein Passwort vergessen — das ist kein Grund, dieses Gerät aufzugeben. Ein vergessenes Passwort macht bei Matrix niemals ein Gerät unbrauchbar."),
            Space::new().height(mk::spacing::M),
            self.absatz("Was der Wächter NICHT kann: deine Dateien lesen oder retten. Ein vergessenes Passwort öffnet keine Hintertür zu deinen Daten — das schützt dich."),
            Space::new().height(mk::spacing::M),
            self.absatz("Der Weg zurück ist die vollständige, korrekte Löschung der Daten dieses Kontos. Danach richtest du ein frisches Konto ein. Der Wächter zeigt dir vorher genau, was gelöscht wird."),
            Space::new().height(Length::Fill),
            row![
                self.text_knopf("Zurück zum Login", Msg::Beenden),
                Space::new().width(mk::spacing::M),
                self.knopf("Fortfahren", self.palette.primary, self.palette.on_primary, Some(Msg::Weiter)),
            ],
        ]
        .into()
    }

    fn auswahl(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let mehrere = self.nutzer.len() > 1;
        let mut liste = column![].spacing(mk::spacing::S);
        for u in &self.nutzer {
            liste = liste.push(
                iced::widget::button(
                    row![
                        mkw::symbol::<Msg>(mkw::symbol::LOCK, mk::font_size::LARGE, p.on_surface_variant),
                        Space::new().width(mk::spacing::M),
                        column![
                            mkw::txt(u.as_str(), mk::typo::UNTERTITEL, p.on_surface),
                            mkw::txt("Nur dieses Konto und seine Daten zurücksetzen", mk::typo::KLEIN, p.on_surface_variant),
                        ].spacing(0),
                    ].align_y(iced::Alignment::Center),
                )
                .width(Length::Fill)
                .padding(mk::spacing::M)
                .on_press(Msg::ZielKonto(u.clone()))
                .style(move |_, st| {
                    let bg = match st {
                        iced::widget::button::Status::Hovered => p.on_surface.over(p.surface_container, mk::state_layer::HOVER),
                        _ => p.surface_container,
                    };
                    // familien-ausnahme: Recovery-Großknöpfe: Hero-Maße mit eigenem Inhalt
                    iced::widget::button::Style {
                        background: Some(color(bg).into()),
                        border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                        ..Default::default()
                    }
                }),
            );
        }

        let unter = if mehrere {
            "Mehrere Personen nutzen diesen PC. Setze nur dein Konto zurück — die anderen bleiben unberührt. Oder das ganze System."
        } else {
            "Setze dein Konto zurück — oder das ganze System auf Werkseinstellung."
        };

        column![
            self.wachter_kopf("Was soll zurückgesetzt werden?".to_string(), unter),
            Space::new().height(mk::spacing::L),
            liste,
            Space::new().height(mk::spacing::M),
            iced::widget::button(
                row![
                    mkw::symbol::<Msg>(mkw::symbol::WARNUNG, mk::font_size::LARGE, p.error),
                    Space::new().width(mk::spacing::M),
                    column![
                        mkw::txt("Ganzes System (Werkseinstellung)", mk::typo::UNTERTITEL, p.error),
                        mkw::txt("Alle Konten und Daten löschen — frische Installation", mk::typo::KLEIN, p.on_surface_variant),
                    ].spacing(0),
                ].align_y(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .padding(mk::spacing::M)
            .on_press(Msg::ZielSystem)
            .style(move |_, st| {
                let bg = match st {
                    iced::widget::button::Status::Hovered => p.error.over(p.surface_container, 0.14),
                    _ => p.surface_container,
                };
                // familien-ausnahme: Recovery-Großknöpfe: Hero-Maße mit eigenem Inhalt
                iced::widget::button::Style {
                    background: Some(color(bg).into()),
                    border: iced::Border { radius: mk::CORNER_RADIUS.into(), width: 1.0, color: color(p.error.over(p.surface_container, 0.4)) },
                    ..Default::default()
                }
            }),
            Space::new().height(Length::Fill),
            row![ self.text_knopf("Zurück", Msg::Zurueck) ],
        ]
        .into()
    }

    /// Das Speicher-Diagramm: gestapelter Balken + Legende mit GB je Kategorie.
    fn analyse_ansicht(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let titel = match &self.ziel {
            Some(Ziel::Konto(u)) => format!("Das wird von „{u}“ gelöscht"),
            _ => "Das wird vom gesamten System gelöscht".to_string(),
        };

        // Redaction (SwiftUI .redacted(.placeholder)): solange die
        // Vermessung läuft, zeigen stille graue Balken die FORM des
        // Diagramms — kein Einfrieren, kein Spinner.
        if self.laden {
            let mut skelett = column![].spacing(mk::spacing::S);
            for breite in [180.0, 140.0, 160.0, 120.0] {
                skelett = skelett.push(
                    row![
                        mkw::redaktion::<Msg>(14.0, 14.0, p),
                        Space::new().width(mk::spacing::M),
                        mkw::redaktion::<Msg>(breite, 14.0, p),
                        Space::new().width(Length::Fill),
                        mkw::redaktion::<Msg>(56.0, 14.0, p),
                    ]
                    .align_y(iced::Alignment::Center),
                );
            }
            return column![
                self.wachter_kopf(titel, "Der Wächter vermisst die Daten …"),
                Space::new().height(mk::spacing::L),
                row![
                    mkw::txt("Gesamt", mk::typo::UNTERTITEL, p.on_surface),
                    Space::new().width(Length::Fill),
                    mkw::redaktion::<Msg>(72.0, 18.0, p),
                ]
                .align_y(iced::Alignment::Center),
                Space::new().height(mk::spacing::S),
                mkw::redaktion::<Msg>(496.0, 28.0, p),
                Space::new().height(mk::spacing::L),
                skelett,
                Space::new().height(Length::Fill),
                row![self.text_knopf("Zurück", Msg::Zurueck)],
            ]
            .into();
        }

        // Gestapelter Balken (proportionale Breiten). Winzige Anteile
        // bekommen eine Mindestbreite, damit sie sichtbar bleiben.
        let sichtbar: Vec<&Anteil> = self.anteile.iter().filter(|a| a.bytes > 0).collect();
        let summe: u64 = sichtbar.iter().map(|a| a.bytes).sum::<u64>().max(1);
        let mut balken = row![].spacing(2);
        if sichtbar.is_empty() {
            balken = balken.push(
                container(Space::new().height(Length::Fixed(28.0)))
                    .width(Length::Fill)
                    .style(move |_| container::Style {
                        background: Some(color(p.surface_container_high).into()),
                        border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
                        ..Default::default()
                    }),
            );
        }
        for a in &sichtbar {
            let anteil = (a.bytes as f32 / summe as f32).max(0.02);
            let farbe = kat_farbe(&a.schluessel, p);
            balken = balken.push(
                container(Space::new().height(Length::Fixed(28.0)))
                    .width(Length::FillPortion((anteil * 1000.0) as u16))
                    .style(move |_| container::Style {
                        background: Some(color(farbe).into()),
                        border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
                        ..Default::default()
                    }),
            );
        }

        // Legende: Farbe · Kategorie · Menge
        let mut legende = column![].spacing(mk::spacing::XS);
        for a in &sichtbar {
            let farbe = kat_farbe(&a.schluessel, p);
            legende = legende.push(
                row![
                    container(Space::new().width(Length::Fixed(14.0)).height(Length::Fixed(14.0)))
                        .style(move |_| container::Style {
                            background: Some(color(farbe).into()),
                            border: iced::Border { radius: mk::radius::MINI.into(), ..Default::default() },
                            ..Default::default()
                        }),
                    Space::new().width(mk::spacing::M),
                    mkw::txt(a.label.clone(), mk::typo::FLIESS, p.on_surface),
                    Space::new().width(Length::Fill),
                    mkw::txt(menge(a.bytes), mk::typo::FLIESS, p.on_surface_variant),
                ]
                .align_y(iced::Alignment::Center),
            );
        }
        if sichtbar.is_empty() {
            legende = legende.push(self.absatz("Dieses Konto hat keine nennenswerten persönlichen Daten angelegt."));
        }

        let inhalt = column![
            self.wachter_kopf(titel, "Sieh genau, was du aufgibst — bevor etwas gelöscht wird."),
            Space::new().height(mk::spacing::L),
            row![
                mkw::txt("Gesamt", mk::typo::UNTERTITEL, p.on_surface),
                Space::new().width(Length::Fill),
                mkw::txt(menge(self.gesamt), mk::typo::UNTERTITEL, p.on_surface),
            ],
            Space::new().height(mk::spacing::S),
            balken,
            Space::new().height(mk::spacing::L),
            legende,
        ];

        column![
            mkw::scrollbereich_mit_fade(inhalt.into(), p),
            self.fehler_zeile(),
            Space::new().height(mk::spacing::M),
            row![
                self.text_knopf("Zurück", Msg::Zurueck),
                Space::new().width(mk::spacing::M),
                self.knopf("Daten löschen", p.error, p.on_primary, Some(Msg::LoeschenFragen)),
            ],
        ]
        .into()
    }

    fn loeschung(&self) -> Element<'_, Msg> {
        column![
            Space::new().height(Length::Fill),
            self.wachter_kopf("Der Wächter arbeitet …".to_string(), "Die Daten werden sauber und vollständig entfernt."),
            Space::new().height(mk::spacing::L),
            self.absatz("Bitte den PC jetzt nicht ausschalten."),
            Space::new().height(Length::Fill),
        ]
        .align_x(iced::Alignment::Center)
        .into()
    }

    /// Die Wächter-Frist: Wartezeit als Schutz gegen unbefugtes Löschen.
    /// Großer Countdown, Erklärung, Login-Schlüssel-Schnellweg, Abbrechen.
    fn frist_ansicht(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let uhr = mk::format::dauer_mmss(self.frist_rest);
        // 30 Minuten = 1800 s; der Balken füllt sich, während die Frist läuft.
        let anteil = 1.0 - (self.frist_rest as f32 / 1800.0).clamp(0.0, 1.0);
        column![
            Space::new().height(Length::Fill),
            self.wachter_kopf(
                "Sicherheits-Wartezeit".to_string(),
                "Der Wächter schützt vor unbefugtem Löschen.",
            ),
            Space::new().height(mk::spacing::XL),
            mkw::txt(uhr, mk::typo::Stil { groesse: 64.0, gewicht: mk::typo::Gewicht::Halbfett }, p.primary),
            Space::new().height(mk::spacing::S),
            mkw::txt("bis die Löschung freigegeben wird", mk::typo::HINWEIS, p.on_surface_variant),
            Space::new().height(mk::spacing::M),
            container(mkw::fortschritt(anteil, p)).max_width(320.0),
            Space::new().height(mk::spacing::XL),
            container(
                column![
                    row![
                        mkw::symbol::<Msg>(mkw::symbol::KEY, mk::font_size::LARGE, p.primary),
                        Space::new().width(mk::spacing::S),
                        mkw::txt("Bist du der Besitzer?", mk::typo::KOPF, p.on_surface),
                    ].align_y(iced::Alignment::Center),
                    Space::new().height(mk::spacing::XXS),
                    mkw::txt("Steck deinen Login-Schlüssel ein — der Wächter erkennt ihn und löscht sofort, ohne Wartezeit.", mk::typo::KLEIN, p.on_surface_variant),
                ].spacing(0)
            )
            .max_width(420.0)
            .padding(mk::spacing::M)
            .style(move |_| container::Style {
                background: Some(color(p.surface_container).into()),
                border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                ..Default::default()
            }),
            Space::new().height(mk::spacing::M),
            self.absatz("Der Wächter ruft währenddessen laut — wer hier unbefugt löschen will, wird gehört."),
            Space::new().height(Length::Fill),
            row![self.text_knopf("Abbrechen", Msg::FristAbbrechen)],
        ]
        .align_x(iced::Alignment::Center)
        .into()
    }

    fn neuanlage(&self) -> Element<'_, Msg> {
        let p = self.palette;
        let feld = |platz: &'static str, wert: &str, msg: fn(String) -> Msg, secure: bool| {
            mkw::eingabefeld(platz, wert, msg, None, secure, p)
        };
        let bereit = !self.neu_name.is_empty() && self.neu_pw.len() >= 4 && !self.laeuft;
        column![
            self.wachter_kopf("Ein frischer Start".to_string(), "Lege ein neues Konto an — dieser PC gehört jetzt dir."),
            Space::new().height(mk::spacing::XL),
            mkw::txt("Kontoname (klein geschrieben)", mk::typo::KLEIN, p.on_surface_variant),
            Space::new().height(mk::spacing::XXS),
            feld("z. B. nicolas", &self.neu_name, Msg::NeuName, false),
            Space::new().height(mk::spacing::M),
            mkw::txt("Passwort", mk::typo::KLEIN, p.on_surface_variant),
            Space::new().height(mk::spacing::XXS),
            feld("Neues Passwort", &self.neu_pw, Msg::NeuPw, true),
            Space::new().height(mk::spacing::S),
            feld("Passwort wiederholen", &self.neu_pw2, Msg::NeuPw2, true),
            self.fehler_zeile(),
            Space::new().height(Length::Fill),
            row![
                self.knopf(if self.laeuft { "Lege an …" } else { "Konto anlegen" }, p.primary, p.on_primary, bereit.then_some(Msg::Anlegen)),
            ],
        ]
        .into()
    }

    fn fertig(&self) -> Element<'_, Msg> {
        let p = self.palette;
        column![
            Space::new().height(Length::Fill),
            self.wachter_kopf("Willkommen zurück".to_string(), "Das Konto ist bereit. Melde dich jetzt neu an."),
            Space::new().height(mk::spacing::L),
            self.absatz("Der Wächter hat die alten Daten vollständig entfernt und ein frisches Konto vorbereitet. Ab jetzt bewacht er wieder still das System."),
            Space::new().height(mk::spacing::XL),
            row![ self.knopf("Zur Anmeldung", p.primary, p.on_primary, Some(Msg::Beenden)) ],
            Space::new().height(Length::Fill),
        ]
        .align_x(iced::Alignment::Center)
        .into()
    }

    fn fehler_zeile(&self) -> Element<'_, Msg> {
        match &self.meldung {
            Some((t, true)) => column![
                Space::new().height(mk::spacing::M),
                mkw::txt(format!("\u{26a0} {t}"), mk::typo::KLEIN, self.palette.error),
            ].into(),
            Some((t, false)) => column![
                Space::new().height(mk::spacing::M),
                mkw::txt(t.clone(), mk::typo::KLEIN, self.palette.on_surface_variant),
            ].into(),
            None => Space::new().height(Length::Fixed(0.0)).into(),
        }
    }
}
