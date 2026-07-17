//! Matrix Schlüssel — richtet einen USB-Stick als Login-Schlüssel ein.
//!
//! Sechste MatrixKit-App, ganz auf diesen einen Setup-Zweck zugeschnitten:
//! erkennt eingesteckte USB-Sticks, zeigt Status (Schlüssel eingerichtet?
//! Login-Verknüpfung aktiv? Stick steckt?) und führt Schritt für Schritt
//! durchs Einrichten. Das eigentliche Formatieren + die PAM-Verdrahtung
//! macht das root-Werkzeug `matrix-schluessel` — die App ruft es über
//! `sudo` mit dem Account-Passwort auf, das der Nutzer hier eingibt.
//!
//! Modell (Nutzer-Wahl): Stick ODER Passwort, beide gleichwertig; das
//! Passwort bleibt immer als Weg bestehen — kein Aussperren.

use iced::widget::{column, container, Space};
use iced::{Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::time::Duration;

/// USB-Hero im Einrichtungs-Panel — bewusstes Sondermaß.
const USB_SYMBOL: f32 = 34.0;

const APP_ID: &str = "matrix-schluessel-app";
/// Der root-Helfer (Image: /usr/bin, Dev: /usr/local/bin).
const CLI: &str = "matrix-schluessel";

fn main() -> iced::Result {
    if !mk::fenster::einzelinstanz(APP_ID) {
        return Ok(());
    }
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| String::from("Matrix Schlüssel"))
        .subscription(App::subscription)
        .window(mkw::fenster_settings(APP_ID, 440.0, 600.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

fn cli_pfad() -> String {
    for p in ["/usr/local/bin/matrix-schluessel", "/usr/bin/matrix-schluessel"] {
        if std::path::Path::new(p).exists() {
            return p.to_string();
        }
    }
    CLI.to_string()
}

/// Ein erkannter USB-Stick als Einrichtungs-Kandidat.
#[derive(Clone, PartialEq)]
struct Stick {
    pfad: String,
    beschr: String,
    groesse: String,
}

/// USB-Stick-Kandidaten via lsblk finden (nur USB + wechselbar + ganze
/// Platte, niemals die Systemplatten). Reines Lesen, unprivilegiert.
fn sticks_finden() -> Vec<Stick> {
    let Ok(out) = std::process::Command::new("lsblk")
        .args(["-nro", "PATH,TYPE,TRAN,RM,SIZE,VENDOR,MODEL"])
        .output()
    else {
        return Vec::new();
    };
    let mut v = Vec::new();
    for zeile in String::from_utf8_lossy(&out.stdout).lines() {
        // NAME kann Leerzeichen im MODEL haben → gezielt die ersten Felder
        let mut it = zeile.split_whitespace();
        let (Some(pfad), Some(typ), Some(tran), Some(rm), Some(groesse)) =
            (it.next(), it.next(), it.next(), it.next(), it.next())
        else {
            continue;
        };
        if typ != "disk" || tran != "usb" || rm != "1" {
            continue;
        }
        if pfad.ends_with("sda") || pfad.ends_with("sdb") {
            continue;
        }
        // Leere Kartenleser-Slots (0B) sind keine einsetzbaren Sticks
        if groesse == "0B" || groesse == "0" {
            continue;
        }
        // lsblk maskiert Leerzeichen als \x20 — zurückübersetzen
        let rest: String = it.collect::<Vec<_>>().join(" ").replace("\\x20", " ");
        let beschr = rest.split_whitespace().collect::<Vec<_>>().join(" ");
        let beschr = if beschr.is_empty() { "USB-Stick".to_string() } else { beschr };
        v.push(Stick { pfad: pfad.to_string(), beschr, groesse: groesse.to_string() });
    }
    v
}

#[derive(Clone, Default)]
struct Status {
    konto: bool,
    pam: bool,
    stick_da: bool,
}

fn status_lesen() -> Status {
    let mut s = Status::default();
    if let Ok(out) = std::process::Command::new(cli_pfad()).arg("status-json").output() {
        for zeile in String::from_utf8_lossy(&out.stdout).lines() {
            match zeile.split_once('=') {
                Some(("konto", v)) => s.konto = v.trim() == "1",
                Some(("pam", v)) => s.pam = v.trim() == "1",
                Some(("stick", v)) => s.stick_da = !v.trim().is_empty(),
                _ => {}
            }
        }
    }
    s
}

/// Privilegierte Aktion über sudo -S mit dem eingegebenen Passwort.
/// Läuft blockierend im Hintergrund-Thread (via Task::perform).
fn sudo_cli(passwort: String, args: Vec<String>) -> Result<String, String> {
    use std::io::Write;
    let mut kind = std::process::Command::new("sudo")
        .arg("-S")
        .arg("-k")
        .arg(cli_pfad())
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut si) = kind.stdin.take() {
        let _ = writeln!(si, "{passwort}");
    }
    let out = kind.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.status.success() {
        Ok(if text.is_empty() { "Fertig.".into() } else { text })
    } else if err.to_lowercase().contains("incorrect password") || err.contains("Sorry") {
        Err("Passwort falsch.".into())
    } else {
        Err(if err.is_empty() { "Aktion fehlgeschlagen.".into() } else { err })
    }
}

struct App {
    rahmen: mkw::Rahmen,
    status: Status,
    sticks: Vec<Stick>,
    gewaehlt: Option<String>,
    passwort: String,
    meldung: Option<(String, bool)>, // (Text, ist_fehler)
    laeuft: bool,
    /// Phase des Matrix-Puls-Ladezeichens (0..1, läuft nur bei Arbeit).
    puls_phase: f32,
    /// Bestätigungs-Dialog vor destruktiven Aktionen (SwiftUI-Grammatik).
    dialog: mkw::DialogZustand,
    frage: Option<Frage>,
}

/// Welche destruktive Aktion gerade zur Bestätigung ansteht.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Frage {
    Einrichten,
    Entfernen,
}

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Tick,
    /// Eigener 60-fps-Tick für die Dialog-Feder.
    DialogTick,
    Taste(mkw::Taste),
    Waehlen(String),
    Passwort(String),
    Fragen(Frage),
    Bestaetigen,
    Abbrechen,
    Einrichten,
    Entfernen,
    Ergebnis(Result<String, String>),
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        (
            Self {
                rahmen: mkw::Rahmen::neu(APP_ID, &[]),
                status: status_lesen(),
                sticks: sticks_finden(),
                gewaehlt: None,
                passwort: String::new(),
                meldung: None,
                laeuft: false,
                puls_phase: 0.0,
                dialog: {
                    // Dev-Haken wie MATRIXKIT_ROOT_OFFEN: Dialog fuer
                    // Screenshots ohne Klickstrecke oeffnen
                    let mut d = mkw::DialogZustand::neu();
                    if std::env::var("MATRIXKIT_DIALOG_OFFEN").is_ok() {
                        d.oeffnen();
                    }
                    d
                },
                frage: std::env::var("MATRIXKIT_DIALOG_OFFEN")
                    .ok()
                    .map(|_| Frage::Einrichten),
            },
            Task::none(),
        )
    }

    fn benutzer() -> String {
        std::env::var("USER").unwrap_or_else(|_| "nutzer".into())
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(m) => self.rahmen.update(m).map(Msg::Rahmen),
            Msg::Tick => {
                self.rahmen.palette_geaendert();
                if !self.laeuft {
                    self.status = status_lesen();
                    let neu = sticks_finden();
                    if neu != self.sticks {
                        if let Some(g) = &self.gewaehlt {
                            if !neu.iter().any(|s| &s.pfad == g) {
                                self.gewaehlt = None;
                            }
                        }
                        self.sticks = neu;
                    }
                }
                Task::none()
            }
            Msg::DialogTick => {
                self.dialog.tick();
                // 1,2-s-Zyklus — derselbe Puls wie beim Systemstart
                self.puls_phase = (self.puls_phase + 0.016 / 1.2) % 1.0;
                Task::none()
            }
            Msg::Fragen(f) => {
                // Destruktive Aktion? Erst die Voraussetzungen, dann die Rückfrage.
                let bereit = match f {
                    Frage::Einrichten => self.gewaehlt.is_some() && !self.passwort.is_empty(),
                    Frage::Entfernen => !self.passwort.is_empty(),
                };
                if !bereit {
                    self.meldung = Some(("Stick wählen und Passwort eingeben.".into(), true));
                    return Task::none();
                }
                if self.laeuft {
                    return Task::none();
                }
                self.frage = Some(f);
                self.dialog.oeffnen();
                Task::none()
            }
            Msg::Abbrechen => {
                self.dialog.schliessen();
                Task::none()
            }
            Msg::Bestaetigen => {
                self.dialog.schliessen();
                match self.frage.take() {
                    Some(Frage::Einrichten) => self.update(Msg::Einrichten),
                    Some(Frage::Entfernen) => self.update(Msg::Entfernen),
                    None => Task::none(),
                }
            }
            Msg::Waehlen(p) => {
                self.gewaehlt = Some(p);
                self.meldung = None;
                Task::none()
            }
            Msg::Passwort(p) => {
                self.passwort = p;
                Task::none()
            }
            Msg::Einrichten => {
                let (Some(dev), false) = (self.gewaehlt.clone(), self.passwort.is_empty()) else {
                    self.meldung = Some(("Stick wählen und Passwort eingeben.".into(), true));
                    return Task::none();
                };
                self.laeuft = true;
                self.meldung = Some(("Formatiere und richte ein …".into(), false));
                let pw = self.passwort.clone();
                let user = Self::benutzer();
                Task::perform(
                    async move {
                        sudo_cli(
                            pw,
                            vec![
                                "einrichten".into(),
                                "--device".into(),
                                dev,
                                "--user".into(),
                                user,
                            ],
                        )
                    },
                    Msg::Ergebnis,
                )
            }
            Msg::Entfernen => {
                if self.passwort.is_empty() {
                    self.meldung = Some(("Passwort eingeben, um zu entfernen.".into(), true));
                    return Task::none();
                }
                self.laeuft = true;
                let pw = self.passwort.clone();
                let user = Self::benutzer();
                Task::perform(
                    async move {
                        // Erst PAM lösen, dann das Konto-Secret entfernen
                        let a = sudo_cli(pw.clone(), vec!["pam-deaktivieren".into()]);
                        if a.is_err() {
                            return a;
                        }
                        sudo_cli(pw, vec!["entfernen".into(), "--user".into(), user])
                    },
                    Msg::Ergebnis,
                )
            }
            Msg::Ergebnis(r) => {
                self.laeuft = false;
                self.passwort.clear();
                match r {
                    Ok(t) => self.meldung = Some((t, false)),
                    Err(e) => self.meldung = Some((e, true)),
                }
                self.status = status_lesen();
                self.sticks = sticks_finden();
                self.gewaehlt = None;
                Task::none()
            }
            Msg::Taste(t) => {
                // Dialog fängt Esc zuerst; sonst steuert die Root-Ebene.
                if matches!(t, mkw::Taste::Escape) && self.dialog.offen() {
                    self.dialog.schliessen();
                    return Task::none();
                }
                self.rahmen.taste(t);
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        let mut abos = vec![
            self.rahmen.abo().map(Msg::Rahmen),
            mkw::tick("schluessel", Duration::from_secs(2)).map(|_| Msg::Tick),
            mkw::tasten_abo(Msg::Taste),
        ];
        // Der Bestätigungs-Dialog federt unabhängig von der Root-Ebene.
        if self.dialog.animiert() || self.laeuft {
            abos.push(mkw::tick("schl-dialog", Duration::from_millis(16)).map(|_| Msg::DialogTick));
        }
        Subscription::batch(abos)
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;

        // Status-Sektion
        let ja_nein = |b: bool| if b { "Ja" } else { "Nein" };
        let status = mkw::sektion(
            "STATUS",
            vec![
                mkw::zeile_wert("Schlüssel eingerichtet", None, ja_nein(self.status.konto), p),
                mkw::zeile_wert("Login-Verknüpfung aktiv", None, ja_nein(self.status.pam), p),
                mkw::zeile_wert("Stick steckt gerade", None, ja_nein(self.status.stick_da), p),
            ],
            p,
        );

        // Einrichtungs-Sektion
        let mut einr = column![].spacing(mk::spacing::M);
        if self.sticks.is_empty() {
            // Ist noch ein Entfernen-Knopf zu zeigen, reicht ein kompakter
            // Hinweis; die ganz leere Fläche bekommt unten den Leerzustand.
            if self.status.konto {
                einr = einr.push(
                    container(
                        column![
                            mkw::symbol::<Msg>(mkw::symbol::USB, USB_SYMBOL, p.on_surface_variant),
                            Space::new().height(mk::spacing::XS),
                            mkw::txt("Keinen USB-Stick erkannt", mk::typo::KOPF, p.on_surface),
                            mkw::txt("Stecke einen Stick ein — er wird gleich erkannt.", mk::typo::KLEIN, p.on_surface_variant),
                        ]
                        .align_x(iced::Alignment::Center)
                        .spacing(2),
                    )
                    .width(Length::Fill)
                    .padding(mk::spacing::L)
                    .align_x(iced::alignment::Horizontal::Center),
                );
            }
        } else {
            let mut zeilen: Vec<Element<'_, Msg>> = Vec::new();
            for s in &self.sticks {
                let gewaehlt = self.gewaehlt.as_deref() == Some(&s.pfad);
                let marke = if gewaehlt { mkw::symbol::CHECK } else { mkw::symbol::SHIELD };
                let farbe = if gewaehlt { p.primary } else { p.on_surface_variant };
                zeilen.push(
                    iced::widget::button(mkw::zeile::<Msg>(
                        &s.beschr,
                        Some(&s.pfad),
                        Some(mkw::symbol::<Msg>(marke, mk::font_size::LARGE, farbe)),
                        Some(mkw::txt(&s.groesse, mk::typo::FLIESS, p.on_surface_variant).into()),
                        p,
                    ))
                    .padding(0)
                    .on_press(Msg::Waehlen(s.pfad.clone()))
                    .style(move |_, st| {
                        let base = if gewaehlt { p.primary_container } else { p.surface_container_high };
                        let bg = match st {
                            iced::widget::button::Status::Hovered if !gewaehlt => {
                                p.on_surface.over(base, mk::state_layer::HOVER)
                            }
                            _ => base,
                        };
                        // familien-ausnahme: Schlüssel-Aktionsfläche: volle Breite, error-Farbwelt, eigener Inhalt
                        iced::widget::button::Style {
                            background: Some(color(bg).into()),
                            border: mkw::fokus_ring(gewaehlt, mk::CORNER_RADIUS, p),
                            ..Default::default()
                        }
                    })
                    .into(),
                );
            }
            einr = einr.push(mkw::sektion("STICK WÄHLEN (WIRD GELÖSCHT!)", zeilen, p));

            // Passwort + Aktion
            let feld = mkw::textfeld(
                "Account-Passwort",
                &self.passwort,
                "zum Bestätigen der Einrichtung",
                Msg::Passwort,
                Some(Msg::Fragen(Frage::Einrichten)),
                self.meldung.as_ref().and_then(|(t, ist_fehler)| {
                    (*ist_fehler && t.contains("Passwort")).then_some(t.as_str())
                }),
                true,
                p,
            );
            let aktiv = self.gewaehlt.is_some() && !self.passwort.is_empty() && !self.laeuft;
            // Bei laufender Arbeit: der Matrix-Puls (dieselben drei Punkte
            // wie beim Systemstart) statt eines toten Labels.
            let knopf_inhalt: Element<'_, Msg> = if self.laeuft {
                iced::widget::row![
                    mkw::puls(self.puls_phase, p.on_primary),
                    Space::new().width(mk::spacing::S),
                    mkw::txt("Bitte warten …", mk::typo::FLIESS, p.on_primary),
                ]
                .align_y(iced::Alignment::Center)
                .into()
            } else {
                mkw::txt("Als Schlüssel einrichten", mk::typo::FLIESS, p.on_primary)
                    .center()
                    .width(Length::Fill)
                    .into()
            };
            let knopf = iced::widget::button(
                container(knopf_inhalt)
                    .width(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .width(Length::Fill)
            .height(Length::Fixed(42.0))
            .on_press_maybe(aktiv.then_some(Msg::Fragen(Frage::Einrichten)))
            // familien-ausnahme: Schlüssel-Aktionsfläche: volle Breite, error-Farbwelt, eigener Inhalt
            .style(move |_, _| iced::widget::button::Style {
                background: Some(color(if aktiv { p.error } else { p.error.over(p.surface_container, 0.4) }).into()),
                border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                ..Default::default()
            });
            einr = einr.push(feld).push(knopf).push(
                mkw::txt(
                    "Der gewählte Stick wird vollständig formatiert. Danach kannst du dich mit ihm ODER weiterhin mit deinem Passwort anmelden.",
                    mk::typo::KLEIN,
                    p.on_surface_variant,
                ),
            );
        }

        // Entfernen-Möglichkeit, wenn eingerichtet
        if self.status.konto {
            einr = einr.push(Space::new().height(mk::spacing::S)).push(
                iced::widget::button(
                    mkw::txt("Schlüssel-Login entfernen (Passwort nötig)", mk::typo::KLEIN, p.error),
                )
                .padding([mk::spacing::XS as u16, mk::spacing::M as u16])
                .on_press_maybe((!self.passwort.is_empty() && !self.laeuft).then_some(Msg::Fragen(Frage::Entfernen)))
                // familien-ausnahme: Schlüssel-Aktionsfläche: volle Breite, error-Farbwelt, eigener Inhalt
                .style(move |_, st| iced::widget::button::Style {
                    background: matches!(st, iced::widget::button::Status::Hovered)
                        .then(|| color(p.error.over(p.surface_container_high, 0.14)).into()),
                    border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
                    ..Default::default()
                }),
            );
        }

        let meldung = match &self.meldung {
            Some((t, true)) => format!("\u{26a0} {t}"),
            Some((t, false)) => format!("\u{2713} {t}"),
            None => "Stick als Login-Schlüssel einrichten".to_string(),
        };

        // Ganz leer (kein Stick, kein Schlüssel) → Leerzustand-Grammatik
        let mitte: Element<'_, Msg> = if self.sticks.is_empty() && !self.status.konto {
            mkw::leerzustand(
                mkw::symbol::USB,
                "Kein USB-Stick erkannt",
                "Stecke einen Stick ein — er erscheint hier automatisch.\nZum Einrichten wird er vollständig gelöscht.",
                p,
            )
        } else {
            self.rahmen.scrollflaeche(einr.into(), Msg::Rahmen)
        };

        let inhalt = column![
            status,
            Space::new().height(mk::spacing::L),
            mitte,
            mkw::fusszeile(meldung, p),
        ]
        .spacing(0);

        let karte = container(inhalt)
            .padding(mk::spacing::L)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(color(p.surface_container).into()),
                border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                ..Default::default()
            });

        // Bestätigungs-Dialog über dem Karteninhalt (unter dem Root-Panel)
        let karte: Element<'_, Msg> = if self.dialog.sichtbar() {
            let (titel, botschaft, aktion) = match self.frage {
                Some(Frage::Entfernen) => (
                    "Schlüssel-Login entfernen?",
                    "Die Anmeldung mit dem Stick wird abgeschaltet und das hinterlegte \
                     Geheimnis gelöscht. Dein Passwort funktioniert weiterhin."
                        .to_string(),
                    "Entfernen",
                ),
                _ => {
                    let stick = self
                        .gewaehlt
                        .as_ref()
                        .and_then(|g| self.sticks.iter().find(|s| &s.pfad == g))
                        .map(|s| format!("„{}“ ({})", s.beschr, s.groesse))
                        .unwrap_or_else(|| "Der gewählte Stick".to_string());
                    (
                        "Stick wirklich löschen?",
                        format!(
                            "{stick} wird vollständig formatiert und als Login-Schlüssel \
                             eingerichtet. Dein Passwort bleibt weiterhin gültig."
                        ),
                        "Stick löschen",
                    )
                }
            };
            iced::widget::stack![
                karte,
                mkw::bestaetigung(
                    titel,
                    botschaft,
                    aktion,
                    Msg::Bestaetigen,
                    Msg::Abbrechen,
                    &self.dialog,
                    p,
                )
            ]
            .into()
        } else {
            karte.into()
        };

        self.rahmen.fenster(
            "Matrix Schlüssel",
            env!("CARGO_PKG_VERSION"),
            "Richtet einen USB-Stick als Login-Schlüssel ein. Anmeldung dann mit Stick ODER Passwort — das Passwort bleibt immer gültig.",
            karte,
            Msg::Rahmen,
        )
    }
}
