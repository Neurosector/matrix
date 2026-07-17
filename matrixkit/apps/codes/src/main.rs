//! Matrix Codes — 2FA-Authenticator (TOTP), fünfte MatrixKit-App.
//!
//! Zeigt die 6-stelligen Codes deiner 2FA-Konten mit ablaufendem
//! Countdown-Ring; ein Klick kopiert (Zwischenablage-Recht, bindend).
//! Konten hinzufügen/entfernen liegt hinter dem Passwort-Schloss der
//! Root-Ebene — die Sicherheits-DNA von MatrixKit passt hier perfekt.
//!
//! Die Codes werden von Grund auf in Rust berechnet (totp.rs, mit
//! RFC-6238-Testvektoren belegt). Konten: ~/.config/matrix/codes.conf
//! (0600). Beim ersten Start liegt ein RFC-Beispielkonto bei — löschbar.

mod totp;

use iced::widget::{column, container, row, text, Space};
use iced::{Color, Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use totp::Konto;

const APP_ID: &str = "matrix-codes";

fn main() -> iced::Result {
    if !mk::fenster::einzelinstanz(APP_ID) {
        return Ok(());
    }
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| String::from("Matrix Codes"))
        .subscription(App::subscription)
        .window(mkw::fenster_settings(APP_ID, 400.0, 560.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

fn jetzt() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn conf_pfad() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".config/matrix/codes.conf")
}

/// Konten laden; erster Start bekommt ein RFC-Beispielkonto.
fn konten_laden() -> Vec<Konto> {
    let Ok(inhalt) = std::fs::read_to_string(conf_pfad()) else {
        return vec![Konto::neu(
            "Beispiel (RFC 6238)",
            "MatrixKit",
            "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
            6,
            30,
        )
        .expect("RFC-Beispiel")];
    };
    inhalt
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .filter_map(|zeile| {
            let f: Vec<&str> = zeile.split('\t').collect();
            if f.len() < 5 {
                return None;
            }
            Konto::neu(f[0], f[1], f[2], f[3].parse().unwrap_or(6), f[4].parse().unwrap_or(30))
        })
        .collect()
}

/// Konten atomar + privat (0600) speichern.
fn konten_speichern(konten: &[Konto]) {
    let pfad = conf_pfad();
    if let Some(dir) = pfad.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut inhalt = String::from("# Matrix Codes — 2FA-Geheimnisse (verwaltet von der App)\n");
    for k in konten {
        inhalt.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            k.name, k.issuer, k.secret_b32, k.stellen, k.periode
        ));
    }
    let tmp = pfad.with_extension("conf.neu");
    if std::fs::write(&tmp, &inhalt).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        let _ = std::fs::rename(&tmp, &pfad);
    }
}

struct App {
    rahmen: mkw::Rahmen,
    konten: Vec<Konto>,
    jetzt_s: u64,
    sub_s: f32,
    letzter_tick: std::time::Instant,
    kopiert: Option<usize>,
    eingabe: String,
    eingabe_fehler: bool,
    fokus: mkw::Fokus,
}

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Tick,
    Taste(mkw::Taste),
    Kopieren(usize),
    Eingabe(String),
    Hinzufuegen,
    Entfernen(usize),
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let konten = konten_laden();
        (
            Self {
                rahmen: mkw::Rahmen::neu(APP_ID, &[mk::rechte::Recht::Zwischenablage]),
                fokus: mkw::Fokus::neu(konten.len()),
                konten,
                jetzt_s: jetzt(),
                sub_s: 0.0,
                letzter_tick: std::time::Instant::now(),
                kopiert: None,
                eingabe: String::new(),
                eingabe_fehler: false,
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(m) => self.rahmen.update(m).map(Msg::Rahmen),
            Msg::Tick => {
                let d = self.letzter_tick.elapsed().as_secs_f32();
                self.letzter_tick = std::time::Instant::now();
                self.sub_s += d;
                let n = jetzt();
                if n != self.jetzt_s {
                    self.jetzt_s = n;
                    self.sub_s = 0.0;
                    self.kopiert = None;
                }
                self.rahmen.palette_geaendert();
                Task::none()
            }
            Msg::Kopieren(i) => {
                if !self.rahmen.rechte.erlaubt(mk::rechte::Recht::Zwischenablage) {
                    return Task::none();
                }
                let Some(k) = self.konten.get(i) else { return Task::none() };
                self.kopiert = Some(i);
                iced::clipboard::write(k.code(self.jetzt_s))
            }
            Msg::Eingabe(s) => {
                self.eingabe = s;
                self.eingabe_fehler = false;
                Task::none()
            }
            Msg::Hinzufuegen => {
                if !self.rahmen.root.entsperrt {
                    return Task::none();
                }
                match Konto::aus_eingabe(&self.eingabe) {
                    Some(k) => {
                        self.konten.push(k);
                        konten_speichern(&self.konten);
                        self.eingabe.clear();
                        self.fokus.setze_anzahl(self.konten.len());
                    }
                    None => self.eingabe_fehler = true,
                }
                Task::none()
            }
            Msg::Entfernen(i) => {
                if !self.rahmen.root.entsperrt || i >= self.konten.len() {
                    return Task::none();
                }
                self.konten.remove(i);
                konten_speichern(&self.konten);
                self.fokus.setze_anzahl(self.konten.len());
                Task::none()
            }
            Msg::Taste(t) => {
                if self.rahmen.taste(t) {
                    return Task::none();
                }
                match t {
                    mkw::Taste::Weiter => self.fokus.weiter(),
                    mkw::Taste::Zurueck => self.fokus.zurueck(),
                    mkw::Taste::Aktivieren => {
                        if let Some(i) = self.fokus.aktuell() {
                            return self.update(Msg::Kopieren(i));
                        }
                    }
                    mkw::Taste::Escape => {}
                    mkw::Taste::Suchen => {}
                    mkw::Taste::Einstellungen => {}
                    mkw::Taste::Rueckgaengig => {}
                    mkw::Taste::Aktualisieren => {}
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        Subscription::batch([
            self.rahmen.abo().map(Msg::Rahmen),
            // 200 ms: ruhiger Countdown-Ring ohne nennenswerte Last
            mkw::tick("codes", Duration::from_millis(200)).map(|_| Msg::Tick),
            mkw::tasten_abo(Msg::Taste),
        ])
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;

        let inhalt: Element<'_, Msg> = if self.konten.is_empty() {
            container(
                column![
                    mkw::symbol::<Msg>(mkw::symbol::SHIELD, mk::icon_size::XLARGE, p.on_surface_variant),
                    Space::new().height(mk::spacing::M),
                    mkw::txt("Noch keine Konten", mk::typo::UNTERTITEL, p.on_surface),
                    mkw::txt("Klick auf den App-Namen → Passwort → Konto hinzufügen", mk::typo::KLEIN, p.on_surface_variant),
                ]
                .spacing(mk::spacing::XS)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into()
        } else {
            let mut zeilen: Vec<Element<'_, Msg>> = Vec::new();
            for (i, k) in self.konten.iter().enumerate() {
                zeilen.push(self.konto_zeile(i, k, p));
            }
            self.rahmen.scrollflaeche(mkw::sektion("KONTEN", zeilen, p), Msg::Rahmen)
        };

        let fuss = mkw::fusszeile(
            match self.kopiert {
                Some(i) => format!(
                    "{} kopiert \u{2713}",
                    self.konten.get(i).map(|k| k.issuer_oder_name()).unwrap_or_default()
                ),
                None => String::from("Klick auf einen Code kopiert ihn"),
            },
            p,
        );

        let karte = container(column![inhalt, fuss].spacing(0))
            .padding(mk::spacing::L)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(color(p.surface_container).into()),
                border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                ..Default::default()
            });

        let root = self.rahmen.root.sichtbar().then(|| self.root_ansicht(p));

        self.rahmen.huelle("Matrix Codes", karte.into(), root, Msg::Rahmen)
    }

    /// Eine Konto-Zeile: Countdown-Ring, Name/Issuer, großer Code — klickbar.
    fn konto_zeile<'a>(&'a self, i: usize, k: &'a Konto, p: mk::Palette) -> Element<'a, Msg> {
        let anteil = k.rest_anteil(self.jetzt_s, self.sub_s);
        let knapp = anteil < 0.2;
        let ring_farbe = if knapp { p.error } else { p.primary };
        // Der Countdown-Ring kommt seit Runde 14 aus der Bibliothek (Gauge).
        let ring = mkw::ring::<Msg>(anteil, 34.0, ring_farbe, p);

        let code = k.code(self.jetzt_s);
        let huebsch = if code.len() == 6 {
            format!("{} {}", &code[..3], &code[3..])
        } else {
            code
        };

        let im_fokus = self.fokus.ist(i);
        let inhalt = row![
            ring,
            Space::new().width(mk::spacing::M),
            column![
                mkw::txt(k.issuer_oder_name(), mk::typo::FLIESS, p.on_surface),
                mkw::txt(huebsch, mk::typo::TITEL, ring_farbe),
            ]
            .spacing(0),
            Space::new().width(Length::Fill),
            mkw::symbol::<Msg>(mkw::symbol::CONTENT_COPY, mk::font_size::LARGE, p.on_surface_variant),
        ]
        .align_y(iced::Alignment::Center);

        iced::widget::button(container(inhalt).padding(iced::Padding {
            top: mk::spacing::S,
            right: mk::spacing::M,
            bottom: mk::spacing::S,
            left: mk::spacing::M,
        }))
        .padding(0)
        .on_press(Msg::Kopieren(i))
        .style(move |_, status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => {
                    Some(color(p.on_surface.over(p.surface_container_high, mk::state_layer::HOVER)).into())
                }
                iced::widget::button::Status::Pressed => {
                    Some(color(p.on_surface.over(p.surface_container_high, mk::state_layer::PRESSED)).into())
                }
                _ => None,
            };
            // familien-ausnahme: Symbol-Knopf (CHECK/Segmente) — knopf nimmt nur Text
            iced::widget::button::Style {
                background: bg,
                border: mkw::fokus_ring(im_fokus, mk::CORNER_RADIUS, p),
                ..Default::default()
            }
        })
        .into()
    }

    /// Root-Ebene. Gesperrt: Standard-Ebene (Über + Zwischenablage-Recht +
    /// Passwort-Schloss). Entsperrt: Konten-Verwaltung (hinzufügen/entfernen).
    fn root_ansicht<'a>(&'a self, p: mk::Palette) -> Element<'a, Msg> {
        use iced::widget::stack;
        if !self.rahmen.root.entsperrt {
            return mkw::root_ansicht(
                mkw::RootInfo {
                    name: "Matrix Codes",
                    version: env!("CARGO_PKG_VERSION"),
                    icon: self.rahmen.icon.clone(),
                    beschreibung: "2FA-Authenticator — TOTP-Codes von Grund auf in Rust berechnet. Konten liegen privat unter ~/.config/matrix/codes.conf. Zum Verwalten mit dem Passwort entsperren.",
                },
                p,
                &self.rahmen.rechte,
                &[mk::rechte::Recht::Zwischenablage],
                |r, b| Msg::Rahmen(mkw::RahmenMsg::Recht(r, b)),
                Msg::Rahmen(mkw::RahmenMsg::RootUmschalten),
                &self.rahmen.root,
                |s| Msg::Rahmen(mkw::RahmenMsg::RootPasswort(s)),
                Msg::Rahmen(mkw::RahmenMsg::RootEntsperren),
                Msg::Rahmen(mkw::RahmenMsg::Hilfe("Matrix Codes".into())),
            );
        }

        // Entsperrt: eigene Verwaltungs-Karte
        let schleier = iced::widget::mouse_area(
            container(Space::new().width(Length::Fill).height(Length::Fill)).style(move |_| {
                container::Style {
                    background: Some(Color::from_rgba(0.08, 0.08, 0.08, 0.55).into()),
                    ..Default::default()
                }
            }),
        )
        .on_press(Msg::Rahmen(mkw::RahmenMsg::RootUmschalten));

        let mut liste = column![].spacing(mk::spacing::XS);
        for (i, k) in self.konten.iter().enumerate() {
            liste = liste.push(
                row![
                    mkw::txt(k.issuer_oder_name(), mk::typo::FLIESS, p.on_surface),
                    Space::new().width(Length::Fill),
                    iced::widget::button(mkw::symbol::<Msg>(
                        mkw::symbol::CLOSE,
                        mk::font_size::MEDIUM,
                        p.error,
                    ))
                    .padding(4)
                    .on_press(Msg::Entfernen(i))
                    // familien-ausnahme: Symbol-Knopf (CHECK/Segmente) — knopf nimmt nur Text
                    .style(move |_, status| iced::widget::button::Style {
                        background: matches!(status, iced::widget::button::Status::Hovered)
                            .then(|| color(p.error.over(p.surface_container_high, 0.16)).into()),
                        border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
                        ..Default::default()
                    }),
                ]
                .align_y(iced::Alignment::Center),
            );
        }
        if self.konten.is_empty() {
            liste = liste.push(
                mkw::txt("Noch keine Konten", mk::typo::KLEIN, p.on_surface_variant),
            );
        }

        let feld = mkw::eingabefeld(
            "otpauth://… oder Base32-Geheimnis",
            &self.eingabe,
            Msg::Eingabe,
            Some(Msg::Hinzufuegen),
            false,
            p,
        );

        let karte = column![
            row![
                mkw::symbol::<Msg>(mkw::symbol::LOCK_OPEN, mk::font_size::LARGE, p.primary),
                Space::new().width(mk::spacing::S),
                text("Konten verwalten").size(20).color(color(p.on_surface)),
            ]
            .align_y(iced::Alignment::Center),
            Space::new().height(mk::spacing::M),
            liste,
            Space::new().height(mk::spacing::M),
            row![
                feld,
                Space::new().width(mk::spacing::S),
                iced::widget::button(
                    mkw::symbol::<Msg>(mkw::symbol::CHECK, mk::font_size::LARGE, p.on_primary),
                )
                .padding([mk::spacing::XS as u16, mk::spacing::M as u16])
                .on_press(Msg::Hinzufuegen)
                // familien-ausnahme: Symbol-Knopf (CHECK/Segmente) — knopf nimmt nur Text
                .style(move |_, _| iced::widget::button::Style {
                    background: Some(color(p.primary).into()),
                    border: iced::Border { radius: (mk::CORNER_RADIUS - 4.0).into(), ..Default::default() },
                    ..Default::default()
                }),
            ]
            .align_y(iced::Alignment::Center),
            Space::new().height(mk::spacing::S),
            mkw::txt("otpauth-URI (QR-Text) oder Base32-Geheimnis einfügen", mk::typo::KLEIN, p.on_surface_variant),
            Space::new().height(mk::spacing::L),
            iced::widget::button(
                mkw::txt("Fertig", mk::typo::FLIESS, p.on_primary).center(),
            )
            .width(Length::Fill)
            .height(Length::Fixed(40.0))
            .on_press(Msg::Rahmen(mkw::RahmenMsg::RootUmschalten))
            // familien-ausnahme: Symbol-Knopf (CHECK/Segmente) — knopf nimmt nur Text
            .style(move |_, _| iced::widget::button::Style {
                background: Some(color(p.primary).into()),
                border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                ..Default::default()
            }),
        ]
        .spacing(0);

        let panel = container(
            container(karte)
                .padding(mk::spacing::XL)
                .width(Length::Fixed(340.0))
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container_high).into()),
                    border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                    ..Default::default()
                }),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);

        stack![schleier, panel].into()
    }
}

