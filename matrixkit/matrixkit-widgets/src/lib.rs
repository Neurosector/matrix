//! MatrixKit-Widgets — die gemeinsamen Fenster-Bausteine aller MatrixKit-Apps.
//!
//! Regel aus dem Designsystem: Ein Widget trifft keine eigenen
//! Designentscheidungen, es referenziert Tokens. Was hier liegt, sieht in
//! jeder App identisch aus — Kohärenz per Konstruktion statt per Disziplin.

use iced::widget::{column, container, mouse_area, row, text, Space};
use iced::{Color, Element, Length, Task};
use matrixkit_theme as mk;

/// mk::Rgba → iced::Color (die eine erlaubte Konvertierungsstelle).
pub fn color(c: mk::Rgba) -> Color {
    Color::from_rgba(c.r, c.g, c.b, c.a)
}

/// Ein Typo-Gewicht → iced-Font-Gewicht (Inter Variable trägt alle drei).
pub fn font_gewicht(g: mk::typo::Gewicht) -> iced::font::Weight {
    match g {
        mk::typo::Gewicht::Normal => iced::font::Weight::Normal,
        mk::typo::Gewicht::Medium => iced::font::Weight::Medium,
        mk::typo::Gewicht::Halbfett => iced::font::Weight::Semibold,
    }
}

/// Text im MatrixKit-Stil: eine semantische ROLLE (mk::typo::*) + Farbrolle,
/// statt roher Größe/Gewicht. Leitbild-Prinzip: Hierarchie ist benannt, nicht
/// gezahlt. `mkw::txt("Titel", mk::typo::TITEL, p.on_surface)`.
pub fn txt<'a>(
    inhalt: impl iced::widget::text::IntoFragment<'a>,
    stil: mk::typo::Stil,
    farbe: mk::Rgba,
) -> iced::widget::Text<'a> {
    text(inhalt)
        .size(stil.groesse * mk::typo::faktor())
        .font(iced::Font {
            weight: font_gewicht(stil.gewicht),
            ..iced::Font::with_name("Inter Variable")
        })
        .color(color(farbe))
}

/// Bildschirmtastatur-Funk (R58): Eingabefelder melden ihren Fokus an den
/// matrix-tastatur-Daemon, damit die Tastatur ERSCHEINT, wenn keine
/// physische angeschlossen ist (Tablet-Leitbild-Grammatik). Der Kanal ist ein
/// Unix-Datagram — verbindungslos, feuervergessend, kostet nichts, wenn
/// niemand lauscht. Der Herzschlag entsteht gratis: iceds Cursor-Blinken
/// zeichnet fokussierte Felder alle 500 ms neu, und der Stil-Closure des
/// Eingabefelds ruft `funken()` — der Daemon deutet Stille als Blur.
pub mod tastatur {
    use std::os::unix::net::UnixDatagram;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    /// Der Rahmen pflegt sie: nur das AKTIVE Fenster darf funken — ein
    /// fokussiertes Feld in einem Hintergrund-Fenster ruft keine Tastatur.
    static FENSTER_AKTIV: AtomicBool = AtomicBool::new(true);
    /// Drossel: höchstens alle 700 ms ein Datagramm (Unix-Millis).
    static ZULETZT: AtomicU64 = AtomicU64::new(0);

    pub fn fenster_aktiv(aktiv: bool) {
        FENSTER_AKTIV.store(aktiv, Ordering::Relaxed);
    }

    /// Der Briefkasten des Daemons.
    pub fn sockel() -> PathBuf {
        let lauf = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(lauf).join("matrix-tastatur.sock")
    }

    /// Ein Wort an den Daemon — Fehler sind egal (kein Daemon = kein Bedarf).
    pub fn senden(wort: &str) {
        if let Ok(s) = UnixDatagram::unbound() {
            let _ = s.send_to(wort.as_bytes(), sockel());
        }
    }

    /// Aus dem Stil-Closure fokussierter Eingabefelder: gedrosselt „auf".
    pub fn funken() {
        if !FENSTER_AKTIV.load(Ordering::Relaxed) {
            return;
        }
        let jetzt = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let vorher = ZULETZT.load(Ordering::Relaxed);
        if jetzt.saturating_sub(vorher) < 700 {
            return;
        }
        if ZULETZT
            .compare_exchange(vorher, jetzt, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        senden("auf");
    }
}

/// Knopf-Grammatik — Leitbild-Prinzip (ButtonStyle × ButtonRole × ControlSize):
/// Ein Knopf wird über drei benannte Achsen beschrieben, nie über eigene
/// Farben. Der STIL sagt, wie präsent er ist; die ROLLE, was er bedeutet
/// (destruktiv = error-Farbwelt); die GRÖSSE seine Dichte-Stufe.
pub mod knopfart {
    /// Wie präsent ist der Knopf? (bordered/borderedProminent/borderless)
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Stil {
        /// Gefüllt mit Akzent — die EINE primäre Aktion einer Fläche.
        Prominent,
        /// Getönte Pille — normale Aktionen (Leitbild „bordered").
        Getoent,
        /// Nur Text/State-Layer — leise Aktionen in Zeilen und Ecken.
        Randlos,
    }
    /// Was bedeutet der Knopf? Rolle färbt, Position entscheidet der Dialog.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Rolle {
        Normal,
        /// Zerstörend (löschen, entfernen) — error-Farbwelt.
        Destruktiv,
    }
    /// Dichte-Stufe (ControlSize: mini/small/regular/large → 3 genügen uns).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Groesse {
        Klein,
        Normal,
        Gross,
    }
}

/// DER MatrixKit-Knopf: Beschriftung + drei Achsen + optionale Aktion
/// (None = gesperrt/stumpf). Gibt den Button zurück, damit Aufrufer noch
/// `.width(...)` setzen können.
pub fn knopf<'a, M: Clone + 'a>(
    label: &'a str,
    stil: knopfart::Stil,
    rolle: knopfart::Rolle,
    groesse: knopfart::Groesse,
    p: mk::Palette,
    on: Option<M>,
) -> iced::widget::Button<'a, M> {
    use knopfart::*;
    let aktiv = on.is_some();
    let (hoehe, pad_h, typo_stil) = match groesse {
        Groesse::Klein => (26.0, mk::spacing::M, mk::typo::KLEIN),
        Groesse::Normal => (mk::control::BUTTON_HEIGHT, mk::spacing::L, mk::typo::FLIESS),
        Groesse::Gross => (44.0, mk::spacing::XL, mk::typo::FLIESS),
    };
    // Farbwelten je Stil × Rolle — alles aus der Palette, nie eigene Werte.
    let (grund, schrift) = match (stil, rolle) {
        (Stil::Prominent, Rolle::Normal) => (Some(p.primary), p.on_primary),
        (Stil::Prominent, Rolle::Destruktiv) => (Some(p.error), p.on_primary),
        (Stil::Getoent, Rolle::Normal) => {
            (Some(p.on_surface.over(p.surface_container_high, 0.08)), p.on_surface)
        }
        (Stil::Getoent, Rolle::Destruktiv) => {
            (Some(p.error.over(p.surface_container_high, 0.12)), p.error)
        }
        (Stil::Randlos, Rolle::Normal) => (None, p.primary),
        (Stil::Randlos, Rolle::Destruktiv) => (None, p.error),
    };
    let schrift_final = if aktiv { schrift } else { p.on_surface_variant };
    iced::widget::button(
        txt(label, typo_stil, schrift_final).center(),
    )
    .height(Length::Fixed(hoehe))
    .padding([0, pad_h as u16])
    .on_press_maybe(on)
    .style(move |_, status| {
        let bg = grund.map(|g| {
            let g = if aktiv { g } else { g.over(p.surface_container_high, 0.5) };
            match status {
                iced::widget::button::Status::Hovered => schrift.over(g, mk::state_layer::HOVER),
                iced::widget::button::Status::Pressed => schrift.over(g, mk::state_layer::PRESSED),
                _ => g,
            }
        });
        // Randlos: State-Layer erscheint erst beim Überfahren
        let bg = bg.or(match status {
            iced::widget::button::Status::Hovered if aktiv => {
                Some(schrift.over(p.surface_container_high, mk::state_layer::HOVER))
            }
            iced::widget::button::Status::Pressed if aktiv => {
                Some(schrift.over(p.surface_container_high, mk::state_layer::PRESSED))
            }
            _ => None,
        });
        iced::widget::button::Style {
            background: bg.map(|b| color(b).into()),
            border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
            ..Default::default()
        }
    })
}

/// Linearer Fortschritt (Leitbild LinearProgressViewStyle): stille Spur,
/// Akzent-Füllung, Token-Radius. `anteil` 0..1.
pub fn fortschritt<'a, M: 'a>(anteil: f32, p: mk::Palette) -> Element<'a, M> {
    let anteil = anteil.clamp(0.0, 1.0);
    let fuellung = container(Space::new().height(Length::Fixed(6.0)))
        .width(Length::FillPortion(((anteil * 1000.0) as u16).max(1)))
        .style(move |_| container::Style {
            background: Some(color(p.primary).into()),
            border: iced::Border { radius: mk::radius::MINI.into(), ..Default::default() },
            ..Default::default()
        });
    let rest = container(Space::new().height(Length::Fixed(6.0)))
        .width(Length::FillPortion((((1.0 - anteil) * 1000.0) as u16).max(1)));
    container(row![fuellung, rest])
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(color(p.on_surface.over(p.surface_container, 0.10)).into()),
            border: iced::Border { radius: mk::radius::MINI.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

/// Elevation — Schatten kommunizieren EBENE (schwebt es?), nie Dramatik
/// (Designsystem Kap. 3). Zwei Stufen: ruhende Karte vs. schwebende Fläche.
pub mod elevation {
    use iced::{Color, Shadow, Vector};

    /// Ruhende Karte (Sektion, Zeile): kaum abgehoben, gibt nur Kante.
    pub fn karte() -> Shadow {
        Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.10),
            offset: Vector::new(0.0, 1.0),
            blur_radius: 5.0,
        }
    }

    /// Schwebende Fläche (Root-Ebene, Dialog): deutlich abgehoben, wie ein
    /// Leitbild-Sheet über dem gedimmten Grund.
    pub fn schwebend() -> Shadow {
        Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.32),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 30.0,
        }
    }
}

/// Gebündelter Fenster-Zustand des Rahmens — stoppt die Parameter-Flut:
/// Aktivität (appearsActive), Ampel-Hover (Glyphen ×/−/+ wie das Leitbild) und
/// die Auftritts-Feder beim App-Start. Eine App hält genau EINEN davon.
pub struct FensterZustand {
    /// Ist das Fenster das aktive („key window")? → mkw::aktiv_abo
    pub aktiv: bool,
    /// Maus über der Ampel-Gruppe? → Glyphen erscheinen in allen dreien.
    pub ampeln_hover: bool,
    /// Auftritts-Feder: der Inhalt setzt sich beim Start sanft (0→1).
    auftritt: mk::motion::Spring,
}

impl FensterZustand {
    pub fn neu() -> Self {
        let mut auftritt = mk::motion::Spring::zackig(0.0);
        if mk::bewegung_reduziert() {
            auftritt = mk::motion::Spring::zackig(1.0);
        } else {
            auftritt.retarget(1.0);
        }
        Self {
            aktiv: true,
            // Dev-Haken für Screenshots: Glyphen ohne echte Maus zeigen
            ampeln_hover: std::env::var("MATRIXKIT_AMPELN_HOVER").is_ok(),
            auftritt,
        }
    }
    pub fn tick(&mut self) {
        self.auftritt.tick(1.0 / 60.0);
    }
    pub fn animiert(&self) -> bool {
        !self.auftritt.is_settled()
    }
    /// Fortschritt des Auftritts (0..~1, mit kleinem Nachschwung).
    pub fn auftritt(&self) -> f32 {
        self.auftritt.value
    }
}

impl Default for FensterZustand {
    fn default() -> Self {
        Self::neu()
    }
}

/// Der MatrixKit-Fenster-Header: Titel links, Schliessen rechts, Flaeche
/// ziehbar (native Compositor-Geste). Identisch in allen Apps.
pub fn theme_header<'a, M: Clone + 'a>(
    title: &'a str,
    p: mk::Palette,
    on_drag: M,
    on_close: M,
    on_title: Option<M>,
    on_ablage: M,
    on_max: M,
    fenster: &FensterZustand,
    on_ampeln_hover: impl Fn(bool) -> M + 'a,
) -> Element<'a, M> {
    // Leitbild-Anordnung: Ampel-Knöpfe links, Titel mittig (klickbar → Root-
    // Ebene), rechts ein optischer Ausgleich. Farben aus der Palette statt
    // fester Ampel-Hexwerte (Hex-Verbot): error/secondary/primary.
    // appearsActive (Referenz-SDK): Ist das Fenster nicht das aktive,
    // verlieren die Ampeln ihre Farbe (einheitlich grau) und der Titel
    // dimmt — der Blick weiß sofort, welches Fenster Eingaben bekommt.
    // Und wie im Leitbild: Schwebt die Maus über der Knopfgruppe, zeigen
    // ALLE drei Lichter ihre Glyphe (× / − / +).
    let aktiv = fenster.aktiv;
    let hover = fenster.ampeln_hover;
    let licht = |farbe: mk::Rgba, msg: M, hinweis: &'a str, glyphe: char| {
        let farbe = if aktiv { farbe } else { p.outline.over(p.surface_container_high, 0.6) };
        let inhalt: Element<'a, M> = if hover {
            // Dunkle Glyphe auf dem Licht — Leitbild-Grammatik
            container(symbol::<M>(glyphe, 9.0, mk::Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.55 }))
                .width(Length::Fixed(12.0))
                .height(Length::Fixed(12.0))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .into()
        } else {
            Space::new().width(Length::Fixed(12.0)).height(Length::Fixed(12.0)).into()
        };
        tipp(
            // MatrixUI HarnessFamily: auch die Ampeln federn unterm Zeiger.
            lupe_stark(
                iced::widget::button(inhalt)
                    .padding(0)
                    .on_press(msg)
                    .style(move |_, status| {
                        let bg = match status {
                            iced::widget::button::Status::Hovered => mk::Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }.over(farbe, 0.18),
                            iced::widget::button::Status::Pressed => mk::Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }.over(farbe, 0.32),
                            _ => farbe,
                        };
                        iced::widget::button::Style {
                            background: Some(color(bg).into()),
                            border: iced::Border { radius: mk::radius::kapsel(12.0).into(), ..Default::default() },
                            ..Default::default()
                        }
                    }),
                1.18,
            ),
            hinweis,
            p,
        )
    };
    // R67-Vermessung (16.7.2026, lebendes Dateimanager-Referenz-Fenster per Pixel-Zoom):
    // das Leitbild faehrt 12 pt Lichter mit 8 pt Luecke (20 pt Mitte-zu-Mitte),
    // ~19 pt Einzug, vertikal mittig — UNSERE Werte (12 + spacing::S)
    // treffen das aufs Pt. Empirisch besiegelt, nicht nur geglaubt.
    // Leitbild: Doppelklick auf die Titelleiste maximiert/stellt wieder her.
    let on_max_dc = on_max.clone();
    let ampeln: Element<'a, M> = mouse_area(
        row![
            licht(p.error, on_close, "Schließen", symbol::CLOSE),
            licht(p.secondary, on_ablage, "In die Ablage (Super+M)", symbol::REMOVE),
            licht(p.primary, on_max, "Maximieren", symbol::ADD),
        ]
        .spacing(mk::spacing::S),
    )
    .on_enter(on_ampeln_hover(true))
    .on_exit(on_ampeln_hover(false))
    .into();

    // Regel: ALLE interaktiven Elemente in Titel/Navigation tragen den
    // Material-State-Layer (Hover 0.12 / Pressed 0.20).
    let titel_farbe = if aktiv { p.on_surface } else { p.on_surface_variant };
    let titel: Element<'a, M> = match on_title {
        Some(msg) => lupe(iced::widget::button(
            txt(title, mk::typo::UNTERTITEL, titel_farbe),
        )
        .padding([2, mk::spacing::S as u16])
        .on_press(msg)
        .style(move |_, status| {
            let base = p.surface_container_high;
            let bg = match status {
                iced::widget::button::Status::Hovered => {
                    Some(color(p.on_surface.over(base, mk::state_layer::HOVER)).into())
                }
                iced::widget::button::Status::Pressed => {
                    Some(color(p.on_surface.over(base, mk::state_layer::PRESSED)).into())
                }
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                border: iced::Border { radius: mk::radius::NORMAL.into(), ..Default::default() },
                ..Default::default()
            }
        })),
        None => txt(title, mk::typo::UNTERTITEL, titel_farbe).into(),
    };

    mouse_area(
        container(
            row![
                container(ampeln).width(Length::Fixed(64.0)),
                Space::new().width(Length::Fill),
                titel,
                Space::new().width(Length::Fill),
                Space::new().width(Length::Fixed(64.0)), // optischer Ausgleich
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([mk::spacing::M as u16, mk::spacing::L as u16])
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(color(p.surface_container_high).into()),
            border: iced::Border {
                radius: iced::border::Radius::default()
                    .top_left(mk::CORNER_RADIUS)
                    .top_right(mk::CORNER_RADIUS),
                ..Default::default()
            },
            ..Default::default()
        }),
    )
    .on_press(on_drag)
    .on_double_click(on_max_dc)
    .into()
}

/// Fenster-Rumpf im Theme: Header + Inhalt + unsichtbare Resize-Griffe
/// (rechts, unten, Ecke — native xdg_toplevel-Geste). DER Standardaufbau
/// jeder MatrixKit-App ohne Fremd-Dekoration.
pub fn app_fenster<'a, M: Clone + 'a>(
    title: &'a str,
    p: mk::Palette,
    inhalt: Element<'a, M>,
    on_drag: M,
    on_close: M,
    resize: impl Fn(iced::window::Direction) -> M + 'a,
    on_title: M,
    on_ablage: M,
    on_max: M,
    root: Option<Element<'a, M>>,
    fenster: &FensterZustand,
    on_ampeln_hover: impl Fn(bool) -> M + 'a,
) -> Element<'a, M> {
    use iced::widget::stack;
    let header = theme_header(
        title,
        p,
        on_drag,
        on_close,
        Some(on_title),
        on_ablage,
        on_max,
        fenster,
        on_ampeln_hover,
    );
    // Auftritt: Der Inhalt setzt sich beim Start sanft von unten (Leitbild-
    // Gefühl — Fenster erscheinen nie schlagartig, sie kommen an).
    let f = fenster.auftritt().clamp(0.0, 1.2);
    let versatz = (10.0 * (1.0 - f)).max(0.0);
    let inhalt = container(inhalt).padding(iced::Padding { top: versatz, ..iced::Padding::ZERO });
    let content = container(column![header, inhalt])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(color(p.surface).into()),
            ..Default::default()
        });

    let grip = 8.0_f32;
    let right = container(
        mouse_area(Space::new().width(Length::Fixed(grip)).height(Length::Fill))
            .on_press(resize(iced::window::Direction::East))
            .interaction(iced::mouse::Interaction::ResizingHorizontally),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Right);
    let bottom = container(
        mouse_area(Space::new().width(Length::Fill).height(Length::Fixed(grip)))
            .on_press(resize(iced::window::Direction::South))
            .interaction(iced::mouse::Interaction::ResizingVertically),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_y(iced::alignment::Vertical::Bottom);
    let corner = container(
        mouse_area(
            Space::new()
                .width(Length::Fixed(grip * 2.0))
                .height(Length::Fixed(grip * 2.0)),
        )
        .on_press(resize(iced::window::Direction::SouthEast))
        .interaction(iced::mouse::Interaction::Grab),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Right)
    .align_y(iced::alignment::Vertical::Bottom);

    match root {
        // Root-Ebene offen: App dimmt aus (wie eine pausierte VM),
        // darueber liegt die Verwaltungsflaeche.
        Some(overlay) => stack![content, overlay].into(),
        None => stack![content, right, bottom, corner].into(),
    }
}

// ============================================================================
// Rahmen — der gemeinsame App-Harness. Absorbiert alles, was JEDE MatrixKit-
// App identisch wiederholte: Fenster-Gesten, Ampel-Aktivität/Hover, die
// Root-Ebene mit Passwort-Schloss, den Palette-Poll samt lebendem Icon und
// die Subscription-Verdrahtung. Eine App hält genau EINEN `Rahmen` und eine
// einzige `Msg::Rahmen(RahmenMsg)`-Variante — neue Rahmen-Features sind
// dann EINE Änderung hier statt sieben in den Apps.
// ============================================================================

/// Die Rahmen-Nachrichten (Fenster + Root-Ebene + Aktivität). Die App
/// wickelt sie in eine eigene Variante: `Msg::Rahmen(RahmenMsg)`.
#[derive(Debug, Clone)]
pub enum RahmenMsg {
    Aktiv(bool),
    AmpelnHover(bool),
    Ziehen,
    Schliessen,
    Resize(iced::window::Direction),
    Ablage,
    Maximieren,
    Groesse(iced::Size),
    RootUmschalten,
    RootPasswort(String),
    RootEntsperren,
    RootEntsperrt(bool),
    Recht(mk::rechte::Recht, bool),
    AnimTick,
    /// Scrollgeist: Maus-Position (der Rahmen trackt sie fürs ganze Fenster).
    Maus(iced::Point),
    GeistScroll(iced::widget::scrollable::Viewport),
    GeistRein,
    GeistRaus,
    /// Scroll-Physik: abgefangenes Rad-/Trackpad-Delta.
    Rad(iced::mouse::ScrollDelta),
    /// Touch-Runde (R59): der Finger zieht den Inhalt (dy je Move).
    Zug(f32),
    /// Finger losgelassen — Geschwindigkeit (px/s) fürs Ausrollen.
    ZugEnde(f32),
    /// HelpLink (Leitbild-Grammatik, Runde 11): öffnet Matrix Hilfe mit dem
    /// App-Namen als Suchbegriff — der ?-Knopf im „Über"-Panel.
    Hilfe(String),
}

/// Der gemeinsame Zustand jeder MatrixKit-Fenster-App.
pub struct Rahmen {
    app_id: &'static str,
    pub palette: mk::Palette,
    watcher: mk::PaletteWatcher,
    /// Lebendes App-Icon (für das „Über"-Panel), folgt der Palette.
    pub icon: Option<iced::widget::image::Handle>,
    pub fenster: FensterZustand,
    pub root: RootZustand,
    pub rechte: mk::rechte::Berechtigungen,
    relevante: Vec<mk::rechte::Recht>,
    /// Der Scrollgeist (Matrix-Original): Scrollbalken neben der Maus.
    pub geist: ScrollGeist,
    /// Scroll-Physik (Leitbild-Gefühl): weiches Rad-Gleiten + Momentum.
    pub roll: RollPhysik,
    roll_id: iced::advanced::widget::Id,
    /// Leinwand-Modus (Desktop-Neuerfindung): Fenster werden nie minimiert —
    /// die −-Ampel legt den PRIVATSCHLEIER über den Inhalt, das Fenster
    /// bleibt an seinem Ort. Gecacht aus ~/.config/matrix/desktop-modus.
    pub leinwand: bool,
    /// Privatschleier aktiv: Inhalt verdeckt, Klick öffnet wieder.
    pub privat: bool,
}

impl Rahmen {
    /// `relevante` = die Berechtigungen, die diese App tatsächlich nutzt.
    pub fn neu(app_id: &'static str, relevante: &[mk::rechte::Recht]) -> Self {
        let palette = mk::Palette::load().unwrap_or_default();
        Self {
            icon: matrixkit_icons::render_png(app_id, &palette)
                .map(iced::widget::image::Handle::from_bytes),
            palette,
            watcher: mk::PaletteWatcher::new(),
            fenster: FensterZustand::neu(),
            root: RootZustand::neu(),
            rechte: mk::rechte::Berechtigungen::laden(app_id),
            relevante: relevante.to_vec(),
            geist: ScrollGeist::neu(),
            roll: RollPhysik::neu(),
            roll_id: iced::advanced::widget::Id::new(app_id),
            // Laufzeit-Wahrheit: der Compositor entscheidet, nicht die Datei
            // (die Session kann sich waehrend der Prozess lebt nie aendern).
            leinwand: session_ist_leinwand(),
            privat: std::env::var("MATRIXKIT_PRIVAT").is_ok(),
            app_id,
        }
    }

    /// Palette-Poll: hat sich das Wallpaper geändert? Lädt neu + rendert das
    /// Icon neu, gibt `true` zurück (die App kann eigene Folgen anhängen).
    /// Aus dem Daten-Tick der App aufrufen (der 2-s-Takt existiert ohnehin).
    pub fn palette_geaendert(&mut self) -> bool {
        if self.watcher.changed() {
            if let Some(p) = mk::Palette::load() {
                self.palette = p;
                // Icon-Rendern ist teuer (SVG-Raster) — während des
                // Paletten-Fades NICHT pro Frame, nur am Ende (die
                // Watcher-Flanke garantiert genau einen End-Aufruf).
                if !mk::uebergang::aktiv() {
                    self.icon = matrixkit_icons::render_png(self.app_id, &p)
                        .map(iced::widget::image::Handle::from_bytes);
                }
                return true;
            }
        }
        false
    }

    /// Läuft gerade eine Rahmen-Animation (Root-Feder oder Auftritt)?
    pub fn animiert(&self) -> bool {
        self.root.animiert() || self.fenster.animiert() || self.geist.animiert() || self.roll.laeuft()
    }

    /// scroll_to-Task für die Rahmen-Scrollfläche (Physik-Antrieb).
    fn roll_zu(&self) -> Task<RahmenMsg> {
        iced::advanced::widget::operate(iced::advanced::widget::operation::scrollable::scroll_to(
            self.roll_id.clone(),
            iced::advanced::widget::operation::scrollable::AbsoluteOffset {
                x: None,
                y: Some(self.roll.ist),
            },
        ))
    }

    /// Tastatur in der Root-Ebene: Esc schließt, Tab wandert, Enter schaltet
    /// den fokussierten Schalter (nur entsperrt) bzw. „Fertig". Gibt `true`
    /// zurück, wenn die Taste verbraucht wurde (Root war offen) — dann soll
    /// die App ihre eigene Fokus-Logik NICHT anwenden.
    pub fn taste(&mut self, t: Taste) -> bool {
        // Strg+, (Leitbild Settings-Szene): aus JEDER App in die Einstellungen —
        // der Rahmen verbraucht die Taste global, noch vor der Root-Ebene.
        if matches!(t, Taste::Einstellungen) {
            einstellungen_oeffnen();
            return true;
        }
        if matches!(t, Taste::Escape) {
            if self.root.offen() {
                self.root.schliessen();
            }
            return self.root.offen(); // war offen → verbraucht
        }
        if !self.root.offen() {
            return false;
        }
        match t {
            Taste::Weiter => self.root.fokus.weiter(),
            Taste::Zurueck => self.root.fokus.zurueck(),
            Taste::Aktivieren => {
                let fertig = self.relevante.len();
                match self.root.fokus.aktuell() {
                    Some(i) if i < self.relevante.len() && self.root.entsperrt => {
                        let r = self.relevante[i];
                        let neu = !self.rechte.erlaubt(r);
                        self.rechte.setzen(r, neu);
                    }
                    Some(i) if i == fertig => self.root.schliessen(),
                    _ => {}
                }
            }
            Taste::Escape => {}
            // Bei offener Root-Ebene gibt es nichts zu suchen — verbraucht,
            // damit die App nicht hinter dem Overlay die Suche fokussiert.
            Taste::Suchen => {}
            // Oben bereits global verbraucht — hier nur für die Vollständigkeit.
            Taste::Einstellungen => {}
            // Die Root-Ebene kennt kein Rückgängig — verbraucht, damit die
            // App nicht hinter dem Overlay Werte zurückdreht.
            Taste::Rueckgaengig => {}
            Taste::Aktualisieren => {}
        }
        true
    }

    pub fn update(&mut self, msg: RahmenMsg) -> Task<RahmenMsg> {
        match msg {
            RahmenMsg::Aktiv(a) => {
                self.fenster.aktiv = a;
                // R58: nur das aktive Fenster darf die Bildschirmtastatur
                // rufen — sonst funkt ein Feld hinter anderen Fenstern.
                tastatur::fenster_aktiv(a);
                Task::none()
            }
            RahmenMsg::AmpelnHover(h) => {
                self.fenster.ampeln_hover = h;
                Task::none()
            }
            RahmenMsg::AnimTick => {
                if mk::uebergang::aktiv() {
                    let _ = self.palette_geaendert();
                }
                self.root.tick();
                self.fenster.tick();
                self.geist.tick();
                let trieb = self.roll.laeuft();
                self.roll.tick(1.0 / 60.0);
                // Geist-Anzeige folgt der Physik live
                if self.roll.max > 0.0 {
                    self.geist.offset = (self.roll.ist / self.roll.max).clamp(0.0, 1.0);
                }
                if trieb || self.roll.laeuft() {
                    self.roll_zu()
                } else {
                    Task::none()
                }
            }
            RahmenMsg::Ziehen => iced::window::latest().and_then(iced::window::drag),
            RahmenMsg::Schliessen => iced::window::latest().and_then(iced::window::close),
            RahmenMsg::Resize(dir) => {
                iced::window::latest().and_then(move |id| iced::window::drag_resize(id, dir))
            }
            RahmenMsg::Ablage => {
                if self.leinwand {
                    // Leinwand-Modus: nie minimieren — Privatschleier kippen
                    self.privat = !self.privat;
                } else {
                    mk::fenster::ablage();
                }
                Task::none()
            }
            RahmenMsg::Maximieren => iced::window::latest().and_then(iced::window::toggle_maximize),
            RahmenMsg::Hilfe(begriff) => {
                hilfe_oeffnen(&begriff);
                Task::none()
            }
            RahmenMsg::Groesse(g) => {
                mk::fenster::groesse_merken(self.app_id, g.width, g.height);
                Task::none()
            }
            RahmenMsg::RootUmschalten => {
                self.root.umschalten();
                self.root.fokus.setze_anzahl(self.relevante.len() + 1);
                Task::none()
            }
            RahmenMsg::RootPasswort(pw) => {
                self.root.passwort = pw;
                self.root.fehlversuch = false;
                Task::none()
            }
            RahmenMsg::RootEntsperren => {
                if self.root.passwort.is_empty() || self.root.pruefung_laeuft {
                    return Task::none();
                }
                self.root.pruefung_laeuft = true;
                let pw = self.root.passwort.clone();
                Task::perform(async move { mk::rechte::passwort_pruefen(&pw) }, RahmenMsg::RootEntsperrt)
            }
            RahmenMsg::RootEntsperrt(ok) => {
                self.root.entsperr_ergebnis(ok);
                Task::none()
            }
            RahmenMsg::Maus(punkt) => {
                self.geist.maus = punkt;
                Task::none()
            }
            RahmenMsg::GeistScroll(vp) => {
                self.geist.scroll(vp);
                self.roll.sync(vp);
                Task::none()
            }
            RahmenMsg::Rad(delta) => {
                self.roll.eingabe(delta);
                // Direktanteile (Trackpad / reduzierte Bewegung) sofort
                // anwenden; das weiche Nachlaufen übernimmt der AnimTick.
                self.roll_zu()
            }
            RahmenMsg::Zug(dy) => {
                // R59: Inhalt klebt am Finger — sofort anwenden, kein Tick.
                self.roll.beruehrung_zug(dy);
                if self.roll.max > 0.0 {
                    self.geist.offset = (self.roll.ist / self.roll.max).clamp(0.0, 1.0);
                }
                self.roll_zu()
            }
            RahmenMsg::ZugEnde(v) => {
                self.roll.beruehrung_ende(v);
                Task::none()
            }
            RahmenMsg::GeistRein => {
                self.geist.betreten();
                Task::none()
            }
            RahmenMsg::GeistRaus => {
                self.geist.verlassen();
                Task::none()
            }
            RahmenMsg::Recht(r, erlaubt) => {
                if self.root.entsperrt {
                    self.rechte.setzen(r, erlaubt);
                }
                Task::none()
            }
        }
    }

    /// Die Rahmen-Subscription: Aktivität, Größe, und — nur solange etwas
    /// federt — der 60-fps-Anim-Tick. Die App batcht das mit ihren eigenen
    /// Abos (Daten-Tick, Tastatur).
    pub fn abo(&self) -> iced::Subscription<RahmenMsg> {
        self.abo_mit(true)
    }

    /// Wie `abo`, aber die Fenstertasten (Strg+W/M) sind abwählbar —
    /// das Terminal (R61) gibt Strg+W der Shell (kill-word), nicht dem
    /// Fenster. Alle anderen Rahmen-Ströme bleiben identisch.
    pub fn abo_mit(&self, fenster_tasten_aktiv: bool) -> iced::Subscription<RahmenMsg> {
        let aktiv = aktiv_abo(RahmenMsg::Aktiv);
        // Paletten-Fade: solange der Übergang läuft, treibt ein
        // 30-ms-Tick den Rahmen — palette_geaendert() im AnimTick
        // liefert die Zwischen-Paletten an JEDE Harness-App.
        let fade = palette_fade_abo().map(|_| RahmenMsg::AnimTick);
        let groesse = iced::window::resize_events().map(|(_, g)| RahmenMsg::Groesse(g));
        // Die Fenstertasten (Leitbild NSWindow, Runde 15): Mod+W/Mod+M-Kultur
        // als Strg+W (performClose) und Strg+M (performMiniaturize — in
        // der Leinwand wird daraus der Privatschleier). Sie wohnen im
        // RAHMEN: jede Harness-App hat sie automatisch.
        let fenster_tasten = iced::keyboard::listen().filter_map(|ereignis| {
            use iced::keyboard::{Event, Key};
            let Event::KeyPressed { key, modifiers, .. } = ereignis else {
                return None;
            };
            if !modifiers.control() {
                return None;
            }
            match key {
                Key::Character(ref c) if c.as_str() == "w" => Some(RahmenMsg::Schliessen),
                Key::Character(ref c) if c.as_str() == "m" => Some(RahmenMsg::Ablage),
                _ => None,
            }
        });
        let fenster_tasten = if fenster_tasten_aktiv {
            fenster_tasten
        } else {
            iced::Subscription::none()
        };
        if self.animiert() {
            let anim = tick("rahmen-anim", std::time::Duration::from_millis(16))
                .map(|_| RahmenMsg::AnimTick);
            iced::Subscription::batch([fade, aktiv, groesse, fenster_tasten, anim])
        } else {
            iced::Subscription::batch([fade, aktiv, groesse, fenster_tasten])
        }
    }

    /// Das komplette Fenster bauen: Header (Ampeln/Titel), Inhalt, Resize-
    /// Griffe und — bei Bedarf — die STANDARD-Root-Ebene (Über + Rechte).
    /// Die App liefert nur ihren Inhalt + „Über"-Text.
    pub fn fenster<'a, M: Clone + 'a>(
        &'a self,
        titel: &'a str,
        version: &'a str,
        beschreibung: &'a str,
        inhalt: Element<'a, M>,
        auf: impl Fn(RahmenMsg) -> M + Clone + 'a,
    ) -> Element<'a, M> {
        let root = self.root.sichtbar().then(|| {
            let auf_r = auf.clone();
            let auf_p = auf.clone();
            root_ansicht(
                RootInfo { name: titel, version, beschreibung, icon: self.icon.clone() },
                self.palette,
                &self.rechte,
                &self.relevante,
                move |r, b| auf_r(RahmenMsg::Recht(r, b)),
                auf(RahmenMsg::RootUmschalten),
                &self.root,
                move |s| auf_p(RahmenMsg::RootPasswort(s)),
                auf(RahmenMsg::RootEntsperren),
                auf(RahmenMsg::Hilfe(titel.to_string())),
            )
        });
        self.huelle(titel, inhalt, root, auf)
    }

    /// Wie `fenster`, aber die App liefert ihr EIGENES Root-Overlay (z. B.
    /// Matrix Codes mit Konten-Verwaltung). `root` = None → kein Overlay.
    pub fn huelle<'a, M: Clone + 'a>(
        &'a self,
        titel: &'a str,
        inhalt: Element<'a, M>,
        root: Option<Element<'a, M>>,
        auf: impl Fn(RahmenMsg) -> M + Clone + 'a,
    ) -> Element<'a, M> {
        let auf_r = auf.clone();
        let auf_h = auf.clone();
        let auf_m = auf.clone();
        // Privatschleier (Leinwand-Modus): der Inhalt wird verdeckt, das
        // Fenster bleibt an seinem Ort — Klick auf die Fläche öffnet wieder.
        let inhalt: Element<'a, M> = if self.privat {
            let p = self.palette;
            mouse_area(
                container(
                    column![
                        symbol::<M>(symbol::VISIBILITY_OFF, 40.0, p.on_surface_variant),
                        Space::new().height(mk::spacing::S),
                        txt("Privat", mk::typo::UNTERTITEL, p.on_surface),
                        Space::new().height(mk::spacing::XXS),
                        txt("Klick öffnet wieder", mk::typo::KLEIN, p.on_surface_variant),
                    ]
                    .spacing(0)
                    .align_x(iced::Alignment::Center),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container).into()),
                    border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                    ..Default::default()
                }),
            )
            .on_press(auf(RahmenMsg::Ablage))
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
        } else {
            inhalt
        };
        let fenster = app_fenster(
            titel,
            self.palette,
            inhalt,
            auf(RahmenMsg::Ziehen),
            auf(RahmenMsg::Schliessen),
            move |dir| auf_r(RahmenMsg::Resize(dir)),
            auf(RahmenMsg::RootUmschalten),
            auf(RahmenMsg::Ablage),
            auf(RahmenMsg::Maximieren),
            root,
            &self.fenster,
            move |h| auf_h(RahmenMsg::AmpelnHover(h)),
        );
        // Scrollgeist-Ebene (Matrix-Original): der Rahmen trackt die Maus
        // fürs ganze Fenster und legt die schwebende Anzeige obenauf.
        let mit_maus = mouse_area(fenster).on_move(move |pt| auf_m(RahmenMsg::Maus(pt)));
        iced::widget::stack![mit_maus, scrollgeist(&self.geist, self.palette)].into()
    }

    /// Scrollfläche im Geist-Stil: Kanten-Fades, KEIN Randbalken — die
    /// Anzeige übernimmt der Scrollgeist des Rahmens. Der Standard für
    /// alle scrollenden Inhalte in Rahmen-Apps.
    pub fn scrollflaeche<'a, M: Clone + 'a>(
        &self,
        inhalt: Element<'a, M>,
        auf: impl Fn(RahmenMsg) -> M + Clone + 'a,
    ) -> Element<'a, M> {
        let auf_s = auf.clone();
        let auf_rad = auf.clone();
        let auf_rein = auf.clone();
        let auf_zug = auf.clone();
        let auf_ende = auf.clone();
        let kern = scrollbereich_geist(
            inhalt,
            self.roll_id.clone(),
            move |vp| auf_s(RahmenMsg::GeistScroll(vp)),
            move |d| auf_rad(RahmenMsg::Rad(d)),
            auf_rein(RahmenMsg::GeistRein),
            auf(RahmenMsg::GeistRaus),
            self.palette,
        );
        // R59: die Wischfläche macht daraus direkte Manipulation — der
        // Finger zieht den Inhalt, Loslassen rollt aus (UIScrollView).
        wischflaeche(
            kern,
            move |dy| auf_zug(RahmenMsg::Zug(dy)),
            move |v| auf_ende(RahmenMsg::ZugEnde(v)),
        )
    }
}

/// Fenster-Einstellungen im MatrixKit-Standard: eigene Dekoration,
/// Wayland-App-ID = Name der .desktop-Datei (Pflichtregel!).
pub fn fenster_settings(app_id: &str, breite: f32, hoehe: f32) -> iced::window::Settings {
    // Fenstergedächtnis (Leitbild): gemerkte Größe gewinnt, Minimum bleibt.
    let (b, h) = mk::fenster::groesse_lesen(app_id)
        .map(|(gb, gh)| (gb.max(breite), gh.max(hoehe)))
        .unwrap_or((breite, hoehe));
    iced::window::Settings {
        size: iced::Size::new(b, h),
        min_size: Some(iced::Size::new(breite, hoehe)),
        decorations: false,
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: String::from(app_id),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Executor-agnostischer Zeitgeber (std::thread + Kanal) — funktioniert in
/// der winit- UND der layershell-Laufzeit. MatrixKit-Standard: NIE
/// iced::time::every (braucht Tokio, das wir nicht fahren).
pub fn tick(id: &'static str, period: std::time::Duration) -> iced::Subscription<std::time::Instant> {
    iced::Subscription::run_with((id, period.as_millis() as u64), make_ticker)
}

fn make_ticker(
    input: &(&'static str, u64),
) -> impl futures::Stream<Item = std::time::Instant> {
    let period = std::time::Duration::from_millis(input.1);
    let (tx, rx) = futures::channel::mpsc::unbounded();
    std::thread::spawn(move || loop {
        std::thread::sleep(period);
        if tx.unbounded_send(std::time::Instant::now()).is_err() {
            break; // App beendet — Thread sauber sterben lassen
        }
    });
    rx
}

/// Paletten-Fade-Abo: solange der Übergang läuft, tickt es mit 30 ms —
/// die App hängt es an ihren Puls und fadet dadurch flüssig. Außerhalb
/// des Fades kostet es nichts (Subscription::none).
/// Rohe Fade-Ticks — der Aufrufer mappt selbst (non-capturing):
/// `mkw::palette_fade_abo().map(|_| Msg::Puls)`.
pub fn palette_fade_abo() -> iced::Subscription<std::time::Instant> {
    if mk::uebergang::aktiv() {
        tick("palette-fade", std::time::Duration::from_millis(30))
    } else {
        iced::Subscription::none()
    }
}

/// Der MatrixKit-Schalter: DMS-Pille (Track 52×30, Kreis 24) aus den Tokens.
pub fn schalter<'a, M: Clone + 'a>(an: bool, p: mk::Palette, on_press: M) -> Element<'a, M> {
    schalter_gesichert(an, p, Some(on_press))
}

/// Schalter mit Schloss-Unterstützung: ohne Nachricht ist er stumpf
/// (gedämpfte Farben, kein Zeiger) — die Optik des Gesperrten.
pub fn schalter_gesichert<'a, M: Clone + 'a>(
    an: bool,
    p: mk::Palette,
    on_press: Option<M>,
) -> Element<'a, M> {
    let gesperrt = on_press.is_none();
    let mut track = if an { p.primary } else { p.on_surface.over(p.surface_container_high, 0.12) };
    let mut kreis = if an { p.on_primary } else { p.outline };
    if gesperrt {
        track = track.over(p.surface_container_high, 0.45);
        kreis = kreis.over(p.surface_container_high, 0.45);
    }
    let pad_seite = 3.0;
    let thumb = container(Space::new().width(Length::Fixed(24.0)).height(Length::Fixed(24.0)))
        .style(move |_| container::Style {
            background: Some(color(kreis).into()),
            border: iced::Border { radius: mk::radius::NORMAL.into(), ..Default::default() },
            ..Default::default()
        });
    let innen = if an {
        row![Space::new().width(Length::Fill), thumb]
    } else {
        row![thumb, Space::new().width(Length::Fill)]
    };
    let pille = container(innen.align_y(iced::Alignment::Center))
        .padding(pad_seite)
        .width(Length::Fixed(mk::control::TOGGLE_TRACK_W))
        .height(Length::Fixed(mk::control::TOGGLE_TRACK_H))
        .style(move |_| container::Style {
            background: Some(color(track).into()),
            border: iced::Border { radius: (mk::control::TOGGLE_TRACK_H / 2.0).into(), ..Default::default() },
            ..Default::default()
        });
    match on_press {
        Some(msg) => lupe(
            mouse_area(pille)
                .on_press(msg)
                .interaction(iced::mouse::Interaction::Pointer),
        ),
        None => pille.into(),
    }
}

/// Der animierte Auftritt der Root-Ebene: eine Feder treibt Schleier-
/// Deckkraft und Panel-Gleiten. Apps halten diesen Zustand und leiten
/// AnimTicks weiter, solange `animiert()` — mehr Verdrahtung braucht es nicht.
pub struct RootZustand {
    ziel: bool,
    feder: mk::motion::Spring,
    reduziert: bool,
    /// Tastatur-Fokus innerhalb der Root-Ebene (Rechte + Fertig).
    pub fokus: Fokus,
    /// Berechtigungs-Schloss: Änderungen erst nach Passwort-Bestätigung.
    pub entsperrt: bool,
    pub passwort: String,
    pub pruefung_laeuft: bool,
    pub fehlversuch: bool,
    /// Wackel-Feder des Schlosses (falsches Passwort) — Leitbild-Feedback.
    wackel: mk::motion::Spring,
}

impl RootZustand {
    pub fn neu() -> Self {
        // Dev-Hilfe: MATRIXKIT_ROOT_OFFEN=1 startet mit offener Root-Ebene
        let offen = std::env::var("MATRIXKIT_ROOT_OFFEN").is_ok();
        Self {
            ziel: offen,
            feder: mk::motion::Spring::zackig(if offen { 1.0 } else { 0.0 }),
            reduziert: mk::bewegung_reduziert(),
            fokus: Fokus::neu(0),
            entsperrt: false,
            passwort: String::new(),
            pruefung_laeuft: false,
            fehlversuch: false,
            wackel: mk::motion::Spring::new(0.0),
        }
    }

    pub fn umschalten(&mut self) {
        self.ziel = !self.ziel;
        self.fokus.loeschen();
        // Beim Öffnen wie beim Schließen: verriegelt (Leitbild-Schloss-Modell)
        self.entsperrt = false;
        self.passwort.clear();
        self.pruefung_laeuft = false;
        self.fehlversuch = false;
        let z = if self.ziel { 1.0 } else { 0.0 };
        if self.reduziert {
            // Barrierefreiheit: Zustaende springen, nichts gleitet
            self.feder = mk::motion::Spring::zackig(z);
        } else {
            self.feder.retarget(z);
        }
    }

    pub fn schliessen(&mut self) {
        if self.ziel {
            self.umschalten();
        }
    }

    pub fn offen(&self) -> bool {
        self.ziel
    }

    pub fn tick(&mut self) {
        self.feder.tick(1.0 / 60.0);
        self.wackel.tick(1.0 / 60.0);
    }

    pub fn animiert(&self) -> bool {
        !self.feder.is_settled() || !self.wackel.is_settled()
    }

    /// Aktueller Wackel-Ausschlag (-1..1) für das Schloss-Symbol.
    pub fn wackel_wert(&self) -> f32 {
        self.wackel.value
    }

    /// Solange sichtbar, muss die App das Overlay rendern (auch beim Abgang).
    pub fn sichtbar(&self) -> bool {
        self.ziel || self.feder.value > 0.01
    }

    pub fn fortschritt(&self) -> f32 {
        self.feder.value
    }

    /// Ergebnis der Passwort-Prüfung übernehmen.
    pub fn entsperr_ergebnis(&mut self, ok: bool) {
        self.pruefung_laeuft = false;
        self.entsperrt = ok;
        self.fehlversuch = !ok;
        if ok {
            self.passwort.clear();
        } else if !self.reduziert {
            // Das Schloss wackelt: harte Feder, wenig Dämpfung = Oszillieren
            self.wackel = mk::motion::Spring {
                value: 1.0,
                velocity: 0.0,
                target: 0.0,
                stiffness: 900.0,
                damping: 7.0,
            };
        }
    }
}

/// Angaben fuer die "Ueber"-Sektion der Root-Ebene.
pub struct RootInfo<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub beschreibung: &'a str,
    /// Das lebende App-Icon (live gerendert) — wie im Leitbild' About.
    pub icon: Option<iced::widget::image::Handle>,
}

/// Die Root-Ebene einer MatrixKit-App: Klick auf den App-Namen dimmt die
/// App aus und zeigt diese Flaeche — "Ueber" (wie im Leitbild) und die
/// BINDENDEN Berechtigungen. Klick auf den Grauschleier schliesst.
pub fn root_ansicht<'a, M: Clone + 'a>(
    info: RootInfo<'a>,
    p: mk::Palette,
    rechte: &mk::rechte::Berechtigungen,
    relevante: &'a [mk::rechte::Recht],
    on_recht: impl Fn(mk::rechte::Recht, bool) -> M,
    on_schliessen: M,
    zustand: &RootZustand,
    on_passwort: impl Fn(String) -> M + 'a,
    on_entsperren: M,
    on_hilfe: M,
) -> Element<'a, M> {
    use iced::widget::stack;
    let fokus = &zustand.fokus;
    let f = zustand.fortschritt().clamp(0.0, 1.2);
    let entsperrt = zustand.entsperrt;

    // Grauschleier blendet mit der Feder ein (Klick darauf schliesst)
    let schleier_alpha = 0.55 * f.min(1.0);
    let schleier = mouse_area(
        container(Space::new().width(Length::Fill).height(Length::Fill)).style(move |_| {
            container::Style {
                background: Some(iced::Color::from_rgba(0.08, 0.08, 0.08, schleier_alpha).into()),
                ..Default::default()
            }
        }),
    )
    .on_press(on_schliessen.clone());

    // "Ueber"-Sektion: Icon links (wie im Leitbild), Titel + Version daneben
    let kopf: Element<'a, M> = match info.icon.clone() {
        Some(handle) => row![
            iced::widget::image(handle)
                .width(Length::Fixed(56.0))
                .height(Length::Fixed(56.0)),
            Space::new().width(mk::spacing::M),
            column![
                text(info.name).size(20).color(color(p.on_surface)),
                Space::new().height(mk::spacing::XXS),
                text(format!("Version {} · MatrixKit-App", info.version))
                    .size(mk::font_size::SMALL)
                    .color(color(p.on_surface_variant)),
                text("Neurosector · System Anomaly")
                    .size(mk::font_size::SMALL)
                    .color(color(p.on_surface_variant)),
            ]
            .spacing(0),
        ]
        .align_y(iced::Alignment::Center)
        .into(),
        None => column![
            text(info.name).size(20).color(color(p.on_surface)),
            Space::new().height(mk::spacing::XXS),
            text(format!("Version {} · MatrixKit-App", info.version))
                .size(mk::font_size::SMALL)
                .color(color(p.on_surface_variant)),
            text("Neurosector · System Anomaly")
                .size(mk::font_size::SMALL)
                .color(color(p.on_surface_variant)),
        ]
        .spacing(0)
        .into(),
    };
    let mut karte = column![
        kopf,
        Space::new().height(mk::spacing::S),
        text(info.beschreibung)
            .size(mk::font_size::MEDIUM)
            .color(color(p.on_surface_variant)),
        Space::new().height(mk::spacing::L),
        trenner(p),
        Space::new().height(mk::spacing::M),
        row![
            symbol::<M>(symbol::SHIELD, mk::font_size::MEDIUM, p.on_surface_variant),
            Space::new().width(mk::spacing::XS),
            text("BERECHTIGUNGEN")
                .size(mk::font_size::SMALL)
                .color(color(p.on_surface_variant)),
        ]
        .align_y(iced::Alignment::Center),
        Space::new().height(mk::spacing::S),
    ]
    .spacing(0);

    if relevante.is_empty() {
        karte = karte.push(
            text("Diese App benötigt keine Berechtigungen.")
                .size(mk::font_size::MEDIUM)
                .color(color(p.on_surface_variant)),
        );
    }
    for (i, r) in relevante.iter().enumerate() {
        let erlaubt = rechte.erlaubt(*r);
        let im_fokus = entsperrt && fokus.ist(i);
        karte = karte.push(
            container(
                row![
                    text(r.anzeige())
                        .size(mk::font_size::MEDIUM)
                        .color(color(if entsperrt { p.on_surface } else { p.on_surface_variant })),
                    Space::new().width(Length::Fill),
                    container(schalter_gesichert(
                        erlaubt,
                        p,
                        entsperrt.then(|| on_recht(*r, !erlaubt)),
                    ))
                    .padding(2)
                    .style(move |_| container::Style {
                        border: fokus_ring(im_fokus, mk::control::TOGGLE_TRACK_H / 2.0 + 2.0, p),
                        ..Default::default()
                    }),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([mk::spacing::XS as u16, 0]),
        );
    }

    // Das Schloss (Leitbild-Modell): Änderungen erst nach Passwort-Bestätigung
    // durch den Benutzeraccount — bindend, ohne Bestätigung keine Änderung.
    if !relevante.is_empty() {
        karte = karte.push(Space::new().height(mk::spacing::S));
        if entsperrt {
            karte = karte.push(
                row![
                    symbol::<M>(symbol::LOCK_OPEN, mk::font_size::MEDIUM, p.primary),
                    Space::new().width(mk::spacing::XS),
                    text("Entsperrt — Änderungen möglich")
                        .size(mk::font_size::SMALL)
                        .color(color(p.on_surface_variant)),
                ]
                .align_y(iced::Alignment::Center),
            );
        } else {
            let feld = iced::widget::text_input(
                if zustand.pruefung_laeuft { "Prüfe …" } else { "Passwort zum Ändern" },
                &zustand.passwort,
            )
            .secure(true)
            .on_input(on_passwort)
            .on_submit(on_entsperren.clone())
            .size(mk::font_size::MEDIUM)
            .padding([mk::spacing::XS as u16, mk::spacing::S as u16])
            .style(move |_, status| iced::widget::text_input::Style {
                background: color(p.surface_container).into(),
                border: iced::Border {
                    radius: (mk::CORNER_RADIUS - 4.0).into(),
                    width: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                        2.0
                    } else {
                        1.0
                    },
                    color: color(if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                        p.primary
                    } else {
                        p.outline.over(p.surface_container_high, 0.4)
                    }),
                },
                icon: color(p.on_surface_variant),
                placeholder: color(p.on_surface_variant),
                value: color(p.on_surface),
                selection: color(p.primary_container),
            });
            let ausschlag = (zustand.wackel_wert() * 6.0).clamp(-6.0, 6.0);
            karte = karte.push(
                row![
                    container(symbol::<M>(symbol::LOCK, mk::font_size::MEDIUM, p.on_surface_variant))
                        .padding(iced::Padding {
                            left: (6.0 + ausschlag).max(0.0),
                            right: (6.0 - ausschlag).max(0.0),
                            ..iced::Padding::ZERO
                        }),
                    feld,
                ]
                .align_y(iced::Alignment::Center),
            );
            if zustand.fehlversuch {
                karte = karte.push(Space::new().height(mk::spacing::XXS));
                karte = karte.push(
                    text("Passwort falsch")
                        .size(mk::font_size::SMALL)
                        .color(color(p.error)),
                );
            }
        }
    }

    karte = karte.push(Space::new().height(mk::spacing::S));
    if !relevante.is_empty() {
        karte = karte.push(
            text("Bindend: Ohne Berechtigung erhält die App keinen Zugriff.")
                .size(mk::font_size::SMALL)
                .color(color(p.on_surface_variant)),
        );
    }
    // HelpLink (Leitbild-Grammatik): der runde ?-Knopf sitzt wie in
    // Leitbild-Dialogen links unten und öffnet Matrix Hilfe mit App-Kontext.
    let hilfe_knopf = iced::widget::button(
        text(symbol::HELP.to_string())
            .font(iced::Font::with_name("Material Symbols Rounded"))
            .size(mk::font_size::MEDIUM)
            .color(color(p.on_surface_variant))
            .center(),
    )
    .width(Length::Fixed(40.0))
    .height(Length::Fixed(40.0))
    .on_press(on_hilfe)
    .style(move |_, status| {
        let base = p.surface_container_high;
        let bg = match status {
            iced::widget::button::Status::Hovered => p.on_surface.over(base, mk::state_layer::HOVER),
            iced::widget::button::Status::Pressed => p.on_surface.over(base, mk::state_layer::PRESSED),
            _ => base,
        };
        iced::widget::button::Style {
            background: Some(color(bg).into()),
            border: iced::Border {
                radius: mk::radius::kapsel(40.0).into(),
                width: 1.0,
                color: color(p.on_surface_variant.over(p.surface_container_high, 0.35)),
            },
            ..Default::default()
        }
    });
    karte = karte
        .push(Space::new().height(mk::spacing::L))
        .push(row![
            hilfe_knopf,
            Space::new().width(mk::spacing::S),
            iced::widget::button(
                text("Fertig")
                    .size(mk::font_size::MEDIUM)
                    .color(color(p.on_primary))
                    .center(),
            )
            .width(Length::Fill)
            .height(Length::Fixed(40.0))
            .on_press(on_schliessen)
            .style({
                let fertig_fokus = fokus.ist(relevante.len());
                move |_, status| {
                    let base = p.primary;
                    let bg = match status {
                        iced::widget::button::Status::Hovered => p.on_primary.over(base, mk::state_layer::HOVER),
                        iced::widget::button::Status::Pressed => p.on_primary.over(base, mk::state_layer::PRESSED),
                        _ => base,
                    };
                    let mut border = fokus_ring(fertig_fokus, mk::CORNER_RADIUS, p);
                    if fertig_fokus {
                        border.color = color(p.on_surface);
                    }
                    iced::widget::button::Style {
                        background: Some(color(bg).into()),
                        border,
                        ..Default::default()
                    }
                }
            }),
        ]);

    // Auftritt: das Panel gleitet von unten ein (Material: emphasized
    // decelerate); der Feder-Ueberschwinger gibt den letzten Millimetern Leben.
    let versatz = (80.0 * (1.0 - f)).max(0.0);
    let panel = container(
        container(
            container(karte)
                .padding(mk::spacing::XL)
                .width(Length::Fixed(320.0))
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container_high).into()),
                    border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                    shadow: elevation::schwebend(),
                    ..Default::default()
                }),
        )
        .padding(iced::Padding { top: versatz, ..iced::Padding::ZERO }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center);

    stack![schleier, panel].into()
}

/// Leerzustand — Leitbild- ContentUnavailableView-Grammatik (Referenz-SDK):
/// grosses Symbol, Titel, Beschreibung, zentriert in der freien Flaeche.
/// Fuer alle "nichts da"-Momente: kein Stick eingesteckt, keine Konten,
/// leere Suche. Nie einen nackten Text in eine leere Flaeche setzen.
pub fn leerzustand<'a, M: 'a>(
    zeichen: char,
    titel: &'a str,
    beschreibung: &'a str,
    p: mk::Palette,
) -> Element<'a, M> {
    container(
        column![
            symbol::<M>(zeichen, 44.0, p.on_surface_variant),
            Space::new().height(mk::spacing::M),
            text(titel)
                .size(mk::font_size::LARGE)
                .color(color(p.on_surface)),
            Space::new().height(mk::spacing::XXS),
            text(beschreibung)
                .size(mk::font_size::MEDIUM)
                .color(color(p.on_surface_variant))
                .center(),
        ]
        .align_x(iced::Alignment::Center)
        .spacing(0),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

/// Zustand eines modalen Dialogs (Bestaetigung) — gleiche Mechanik wie die
/// Root-Ebene: Feder-Auftritt, Bewegung-reduzieren respektiert.
pub struct DialogZustand {
    ziel: bool,
    feder: mk::motion::Spring,
    reduziert: bool,
}

impl DialogZustand {
    pub fn neu() -> Self {
        Self {
            ziel: false,
            feder: mk::motion::Spring::zackig(0.0),
            reduziert: mk::bewegung_reduziert(),
        }
    }
    pub fn oeffnen(&mut self) {
        self.ziel = true;
        if self.reduziert {
            self.feder = mk::motion::Spring::zackig(1.0);
        } else {
            self.feder.retarget(1.0);
        }
    }
    pub fn schliessen(&mut self) {
        self.ziel = false;
        if self.reduziert {
            self.feder = mk::motion::Spring::zackig(0.0);
        } else {
            self.feder.retarget(0.0);
        }
    }
    pub fn offen(&self) -> bool {
        self.ziel
    }
    /// Solange sichtbar, muss die App das Overlay rendern (auch beim Abgang).
    pub fn sichtbar(&self) -> bool {
        self.ziel || self.feder.value > 0.01
    }
    pub fn fortschritt(&self) -> f32 {
        self.feder.value
    }
    pub fn tick(&mut self) {
        self.feder.tick(1.0 / 60.0);
    }
    pub fn animiert(&self) -> bool {
        !self.feder.is_settled()
    }
}

/// Bestaetigungs-Dialog — Leitbild-UIs confirmationDialog-Grammatik (Referenz-SDK):
/// Destruktive Aktionen laufen NIE direkt, sondern durch Titel + Botschaft +
/// rote Aktion + Abbrechen. Klick auf den Schleier oder Esc = Abbrechen
/// (Esc verdrahtet die App ueber tasten_abo). Leitbild-Regel: Der sichere Weg
/// ist der bequeme — Abbrechen liegt zuerst, die rote Aktion zuletzt.
pub fn bestaetigung<'a, M: Clone + 'a>(
    titel: &'a str,
    botschaft: String,
    aktion: &'a str,
    on_bestaetigen: M,
    on_abbrechen: M,
    zustand: &DialogZustand,
    p: mk::Palette,
) -> Element<'a, M> {
    use iced::widget::stack;
    let f = zustand.fortschritt().clamp(0.0, 1.2);

    let schleier_alpha = 0.55 * f.min(1.0);
    let schleier = mouse_area(
        container(Space::new().width(Length::Fill).height(Length::Fill)).style(move |_| {
            container::Style {
                background: Some(iced::Color::from_rgba(0.08, 0.08, 0.08, schleier_alpha).into()),
                ..Default::default()
            }
        }),
    )
    .on_press(on_abbrechen.clone());

    // Leitbild-Dialog-Grammatik: Abbrechen zuerst (getönt/neutral), die
    // destruktive Aktion zuletzt (prominent, rote Rolle).
    let karte = column![
        row![
            symbol::<M>(symbol::WARNUNG, mk::font_size::LARGE, p.error),
            Space::new().width(mk::spacing::XS),
            txt(titel, mk::typo::UNTERTITEL, p.on_surface),
        ]
        .align_y(iced::Alignment::Center),
        Space::new().height(mk::spacing::S),
        txt(botschaft, mk::typo::FLIESS, p.on_surface_variant),
        Space::new().height(mk::spacing::L),
        row![
            knopf("Abbrechen", knopfart::Stil::Getoent, knopfart::Rolle::Normal, knopfart::Groesse::Normal, p, Some(on_abbrechen.clone()))
                .width(Length::Fill),
            Space::new().width(mk::spacing::S),
            knopf(aktion, knopfart::Stil::Prominent, knopfart::Rolle::Destruktiv, knopfart::Groesse::Normal, p, Some(on_bestaetigen))
                .width(Length::Fill),
        ],
    ]
    .spacing(0);

    let versatz = (80.0 * (1.0 - f)).max(0.0);
    let panel = container(
        container(
            container(karte)
                .padding(mk::spacing::XL)
                .width(Length::Fixed(360.0))
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container_high).into()),
                    border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                    shadow: elevation::schwebend(),
                    ..Default::default()
                }),
        )
        .padding(iced::Padding { top: versatz, ..iced::Padding::ZERO }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center);

    stack![schleier, panel].into()
}

/// Die MatrixKit-Fusszeile: eine ruhige Statuszeile am Kartenboden —
/// Modus-Hinweise, Kopier-Feedback, Berechtigungs-Meldungen. Fester
/// Bestandteil des App-Vokabulars (eingefuehrt mit Matrix Farben).
pub fn fusszeile<'a, M: 'a>(inhalt: String, p: mk::Palette) -> Element<'a, M> {
    column![
        Space::new().height(mk::spacing::M),
        txt(inhalt, mk::typo::KLEIN, p.on_surface_variant),
    ]
    .spacing(0)
    .into()
}

/// Material Symbols Rounded — unser SF-Symbols-Gegenstück (aus den
/// DMS-Assets, zur Laufzeit geladen). Codepoints statt Ligaturen.
pub mod symbol {
    pub const CLOSE: char = '\u{e5cd}';
    pub const ARROW_BACK: char = '\u{e5c4}';
    pub const CHECK: char = '\u{e5ca}';
    pub const INFO: char = '\u{e88e}';
    pub const CODE: char = '\u{e86f}';
    pub const PALETTE: char = '\u{e40a}';
    pub const ROCKET_LAUNCH: char = '\u{eb9b}';
    pub const TOUCH_APP: char = '\u{e913}';
    pub const CONTENT_COPY: char = '\u{e14d}';
    pub const SHIELD: char = '\u{e9e0}';
    pub const MONITORING: char = '\u{f190}';
    pub const MENU_BOOK: char = '\u{ea19}';
    pub const PLAY_ARROW: char = '\u{e037}';
    pub const PAUSE: char = '\u{e034}';
    pub const VOLUME_UP: char = '\u{e050}';
    pub const VOLUME_OFF: char = '\u{e04f}';
    pub const GRAPHIC_EQ: char = '\u{e1b8}';
    pub const LOCK: char = '\u{e899}';
    pub const LOCK_OPEN: char = '\u{e898}';
    pub const CHEVRON_RIGHT: char = '\u{e5cc}';
    pub const UNFOLD_MORE: char = '\u{e5d7}';
    pub const ARROW_UPWARD: char = '\u{e5d8}';
    pub const ARROW_DOWNWARD: char = '\u{e5db}';
    pub const WARNUNG: char = '\u{e002}';
    pub const USB: char = '\u{e1e0}';
    pub const NOTIFICATIONS: char = '\u{e7f4}';
    pub const TUNE: char = '\u{e429}';
    pub const BATTERY_FULL: char = '\u{e1a4}';
    pub const BATTERY_CHARGING: char = '\u{e1a3}';
    pub const APPS: char = '\u{e5c3}';
    pub const BRIGHTNESS: char = '\u{e1ac}';
    pub const MIC: char = '\u{e029}';
    pub const MIC_OFF: char = '\u{e02b}';
    pub const POWER: char = '\u{e8ac}';
    pub const DARK_MODE: char = '\u{e51c}';
    pub const RESTART: char = '\u{e5d5}';
    pub const LOGOUT: char = '\u{e9ba}';
    pub const WIFI: char = '\u{e63e}';
    pub const BLUETOOTH: char = '\u{e1a7}';
    pub const KEY: char = '\u{e73c}';
    pub const REMOVE: char = '\u{e15b}';
    pub const ADD: char = '\u{e145}';
    pub const EXPAND_MORE: char = '\u{e5cf}';
    pub const SEARCH: char = '\u{e8b6}';
    pub const VISIBILITY_OFF: char = '\u{e8f5}';
    pub const HELP: char = '\u{e887}';
    pub const SCHEDULE: char = '\u{e8b5}';
    pub const PERSON: char = '\u{e7fd}';
    pub const WIDGETS: char = '\u{e1bd}';
    pub const PUBLIC: char = '\u{e80b}';
    pub const DRAG_INDICATOR: char = '\u{e945}';
    // Datei-Welt (Matrix Dateien, Runde 29)
    pub const FOLDER: char = '\u{e2c7}';
    pub const DATEI: char = '\u{e24d}';
    pub const IMAGE: char = '\u{e3f4}';
    pub const MOVIE: char = '\u{e02c}';
    pub const MUSIC_NOTE: char = '\u{e405}';
    pub const HOME: char = '\u{e88a}';
    pub const DOWNLOAD: char = '\u{e2c4}';
    pub const DELETE: char = '\u{e872}';
    pub const STORAGE: char = '\u{e1db}';
    pub const MEMORY: char = '\u{e322}';
    pub const DESKTOP: char = '\u{e30c}';
    pub const PDF: char = '\u{e415}';
    pub const ARROW_FORWARD: char = '\u{e5c8}';
    pub const NEUER_ORDNER: char = '\u{e2cc}';
}

/// Der Symbol-Font der Shell — Apps übergeben ihn an `.font(...)`.
/// None, wenn die Assets fehlen (dann bleiben Symbole leere Kästchen —
/// auf Matrix-Systemen sind sie immer da).
pub fn symbol_font_laden() -> Option<std::borrow::Cow<'static, [u8]>> {
    std::fs::read(
        "/usr/share/quickshell/dms/assets/fonts/material-design-icons/variablefont/MaterialSymbolsRounded[FILL,GRAD,opsz,wght].ttf",
    )
    .ok()
    .map(std::borrow::Cow::Owned)
}

// ===========================================================================
// Mono- und Karten-Familien (R65) — geboren aus echten Duplikaten:
// Terminal und Installer luden beide Maple Mono von Hand, Installer und
// Übersicht bauten eigene Karten, die Boot-Medium-Kapsel und der
// Geräte-Chip waren Einzelstücke. Ab jetzt: EIN Code-Pfad je Gestalt.
// ===========================================================================

/// Die EINE Mono-Schrift des Systems (Maple Mono NF vom Image).
pub fn mono() -> iced::Font {
    iced::Font::with_name("Maple Mono NF")
}

/// Maple Mono fürs `.font(...)`-Laden im App-main — der Pfad wohnt
/// GENAU EINMAL im Kit.
pub fn mono_font_laden() -> std::borrow::Cow<'static, [u8]> {
    std::fs::read("/usr/share/fonts/maple/MapleMono-NF-Regular.ttf")
        .map(std::borrow::Cow::Owned)
        .unwrap_or(std::borrow::Cow::Borrowed(&[]))
}

/// Karten-Familie: die stille Inhalts-Karte — Füllung nach Größenklasse
/// GROSS, Kit-Hairline, NORMAL-Radius, M-Innenluft.
pub fn karte<'a, M: 'a>(inhalt: Element<'a, M>, p: mk::Palette) -> Element<'a, M> {
    container(inhalt)
        .padding(mk::spacing::M)
        .style(move |_| container::Style {
            background: Some(color(p.fuellung(mk::Fuellung::Gross)).into()),
            border: iced::Border {
                radius: mk::radius::NORMAL.into(),
                width: 1.0,
                color: color(p.outline.mit_alpha(0.25)),
            },
            ..Default::default()
        })
        .into()
}

/// Karte mit Bedeutungsfarbe (Warnung = error, Erfolg = primary):
/// getönter Grund, farbige Hairline — die rote Wand des Installers.
pub fn karte_farbig<'a, M: 'a>(
    inhalt: Element<'a, M>,
    farbe: mk::Rgba,
    p: mk::Palette,
) -> Element<'a, M> {
    container(inhalt)
        .padding(mk::spacing::L)
        .style(move |_| container::Style {
            background: Some(color(farbe.over(p.surface_container_high, 0.10)).into()),
            border: iced::Border {
                radius: mk::radius::NORMAL.into(),
                width: 1.0,
                color: color(farbe),
            },
            ..Default::default()
        })
        .into()
}

/// Status-Kapsel: das stille Abzeichen („Boot-Medium — gesperrt",
/// „Aktiv", Zähler) — ETIKETT-Typo in einer DÜNN gefüllten Kapsel.
pub fn status_kapsel<'a, M: 'a>(
    text_inhalt: impl iced::widget::text::IntoFragment<'a>,
    p: mk::Palette,
) -> Element<'a, M> {
    container(txt(text_inhalt, mk::typo::ETIKETT, p.text_stufe(2)))
        .padding(iced::Padding { top: 3.0, right: 10.0, bottom: 3.0, left: 10.0 })
        .style(move |_| container::Style {
            background: Some(color(p.fuellung(mk::Fuellung::Duenn)).into()),
            border: iced::Border {
                radius: mk::radius::kapsel(22.0).into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Code-Chip: ein technischer Name (Gerät, Pfad, Taste) in Mono auf
/// dünner Füllung — unübersehbar, was gemeint ist.
pub fn code_chip<'a, M: 'a>(
    text_inhalt: impl iced::widget::text::IntoFragment<'a>,
    p: mk::Palette,
) -> Element<'a, M> {
    container(
        text(text_inhalt)
            .font(mono())
            .size(mk::font_size::SMALL)
            .color(color(p.text_stufe(1))),
    )
    .padding(iced::Padding { top: 2.0, right: 8.0, bottom: 2.0, left: 8.0 })
    .style(move |_| container::Style {
        background: Some(color(p.fuellung(mk::Fuellung::Duenn)).into()),
        border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
        ..Default::default()
    })
    .into()
}

/// Konsolen-Karte: Protokoll/Befehlsausgabe in Mono auf der dunkelsten
/// Fläche — die Terminal-Verwandtschaft für jede App, die Logs zeigt.
pub fn konsole<'a, M: 'a>(
    text_inhalt: impl iced::widget::text::IntoFragment<'a>,
    breite: f32,
    p: mk::Palette,
) -> Element<'a, M> {
    container(
        text(text_inhalt)
            .font(mono())
            .size(mk::font_size::SMALL)
            .color(color(p.text_stufe(2))),
    )
    .padding(mk::spacing::M)
    .width(Length::Fixed(breite))
    .style(move |_| container::Style {
        background: Some(color(p.surface).into()),
        border: iced::Border {
            radius: mk::radius::NORMAL.into(),
            width: 1.0,
            color: color(p.outline.mit_alpha(0.25)),
        },
        ..Default::default()
    })
    .into()
}

/// Ein Symbol in Token-Größe und Palettenfarbe.
///
/// R60 (fonttools-vermessen, 16.7.): Material Symbols Rounded trägt
/// ascent 1056 / descent 96 bei 960 upm — die natürliche Zeilenbox ist
/// 1,2 em hoch, iceds Standard macht 1,3 em daraus. In engen Leisten
/// (Bar deckelt Knöpfe auf 32 px) quoll die Box über und iced ließ das
/// UNTERE Padding kollabieren: Symbole rutschten sichtbar tiefer als
/// der Text (Nutzer-Fund). Mit line_height 1,0 liegt die Box exakt
/// baseline..960 und die Glyphen-Mitte (480) IST die Box-Mitte —
/// zentriert per Konstruktion, in jeder Zeile.
pub fn symbol<'a, M: 'a>(zeichen: char, groesse: f32, farbe: mk::Rgba) -> Element<'a, M> {
    symbol_gewicht(zeichen, groesse, farbe, mk::typo::Gewicht::Normal)
}

/// Symbol mit Text-Gewicht (R65b, SF-Symbols-Extrakt): neben halbfettem
/// Text stehen im Leitbild halbfette Symbole — die Strichstärke folgt der
/// Typo. Material Symbols Rounded ist eine VARIABLE Schrift (wght-Achse),
/// das gewünschte Gewicht wandert als Font-Attribut zum Textstapel.
pub fn symbol_gewicht<'a, M: 'a>(
    zeichen: char,
    groesse: f32,
    farbe: mk::Rgba,
    gewicht: mk::typo::Gewicht,
) -> Element<'a, M> {
    text(zeichen.to_string())
        .font(iced::Font {
            weight: font_gewicht(gewicht),
            ..iced::Font::with_name("Material Symbols Rounded")
        })
        .size(groesse * mk::typo::faktor())
        .line_height(iced::widget::text::LineHeight::Relative(1.0))
        .color(color(farbe))
        .into()
}

/// Gestylter Scrollbereich: dünner, runder Balken in Token-Farben —
/// das letzte Fremd-Element in MatrixKit-Fenstern verschwindet.
pub fn scrollbereich<'a, M: 'a>(inhalt: Element<'a, M>, p: mk::Palette) -> Element<'a, M> {
    use iced::widget::scrollable;
    scrollable(inhalt)
        .style(move |_, status| {
            let griff = match status {
                scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => {
                    p.on_surface.over(p.surface_container, 0.45)
                }
                _ => p.on_surface.over(p.surface_container, 0.25),
            };
            let rail = scrollable::Rail {
                background: None,
                border: iced::Border::default(),
                scroller: scrollable::Scroller {
                    background: color(griff).into(),
                    border: iced::Border { radius: mk::radius::MINI.into(), ..Default::default() },
                },
            };
            scrollable::Style {
                container: container::Style::default(),
                vertical_rail: rail,
                horizontal_rail: rail,
                gap: None,
                auto_scroll: scrollable::AutoScroll {
                    background: color(p.surface_container_high).into(),
                    border: iced::Border { radius: mk::radius::NORMAL.into(), ..Default::default() },
                    shadow: iced::Shadow::default(),
                    icon: color(p.primary),
                },
            }
        })
        .height(Length::Fill)
        .into()
}

/// Die MatrixKit-Navigationstasten — EIN Abo für alle Apps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taste {
    Escape,
    /// Tab — Fokus zum nächsten Element
    Weiter,
    /// Shift+Tab — Fokus zum vorherigen Element
    Zurueck,
    /// Enter — fokussiertes Element auslösen
    Aktivieren,
    /// Strg+F — Suche fokussieren (Leitbild keyboardShortcut-Grammatik, Runde 11)
    Suchen,
    /// Strg+, — Matrix Einstellungen öffnen (Leitbild Settings-Szene, Runde 12).
    /// Der Rahmen verbraucht die Taste global; Apps sehen sie nie.
    Einstellungen,
    /// Strg+Z — letzte Änderung zurücknehmen (Leitbild UndoManager, Runde 13).
    Rueckgaengig,
    /// Strg+R — Inhalt frisch laden (Leitbild refreshable, Runde 14).
    Aktualisieren,
}

/// Tastatur-Abo: liefert die Navigationstasten als App-Nachrichten.
/// Fenster-Aktivität (Leitbild-UIs `appearsActive`, Referenz-SDK): true, wenn
/// das Fenster das aktive („key window") ist. Der Rahmen dimmt damit
/// Ampeln und Titel — wie das Leitbild, wo inaktive Fenster ihre Farben verlieren.
pub fn aktiv_abo<M: Send + 'static>(map: fn(bool) -> M) -> iced::Subscription<M> {
    iced::window::events()
        .filter_map(|(_id, ereignis)| match ereignis {
            iced::window::Event::Focused => Some(true),
            iced::window::Event::Unfocused => Some(false),
            _ => None,
        })
        .with(map)
        .map(|(map, aktiv)| map(aktiv))
}

/// Tooltip (Leitbild-UIs `.help()`): kurzer Hinweis beim Überfahren mit der
/// Maus. Für Symbol-Knöpfe, deren Bedeutung nicht selbsterklärend ist.
pub fn tipp<'a, M: 'a>(
    inhalt: Element<'a, M>,
    hinweis: &'a str,
    p: mk::Palette,
) -> Element<'a, M> {
    iced::widget::tooltip(
        inhalt,
        container(
            text(hinweis)
                .size(mk::font_size::SMALL)
                .color(color(p.on_surface)),
        )
        .padding([4, 8])
        .style(move |_| container::Style {
            background: Some(color(p.surface_container_high).into()),
            border: iced::Border {
                radius: mk::radius::KLEIN.into(),
                width: 1.0,
                color: color(p.outline.over(p.surface_container_high, 0.4)),
            },
            ..Default::default()
        }),
        iced::widget::tooltip::Position::Bottom,
    )
    .gap(6)
    .into()
}

/// Redaction (Leitbild-UIs `.redacted(.placeholder)`): ein stiller grauer
/// Platzhalter-Balken für Inhalte, die noch geladen werden — die Fläche
/// zeigt die FORM des Kommenden, nie einen Spinner-Zirkus.
pub fn redaktion<'a, M: 'a>(breite: f32, hoehe: f32, p: mk::Palette) -> Element<'a, M> {
    container(Space::new().width(Length::Fixed(breite)).height(Length::Fixed(hoehe)))
        .style(move |_| container::Style {
            background: Some(color(p.on_surface.over(p.surface_container, 0.10)).into()),
            border: iced::Border { radius: (hoehe / 2.0).into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

pub fn tasten_abo<M: Send + 'static>(map: fn(Taste) -> M) -> iced::Subscription<M> {
    // iced verlangt capture-freie map/filter_map-Closures — der fn-Pointer
    // reist deshalb als Daten über Subscription::with.
    iced::keyboard::listen()
        .filter_map(|ereignis| {
            use iced::keyboard::key::Named;
            use iced::keyboard::{Event, Key};
            let Event::KeyPressed { key, modifiers, .. } = ereignis else {
                return None;
            };
            Some(match key {
                Key::Named(Named::Escape) => Taste::Escape,
                Key::Named(Named::Tab) if modifiers.shift() => Taste::Zurueck,
                Key::Named(Named::Tab) => Taste::Weiter,
                // Pfeiltasten navigieren Listen wie im Leitbild (Runde 15).
                Key::Named(Named::ArrowUp) => Taste::Zurueck,
                Key::Named(Named::ArrowDown) => Taste::Weiter,
                Key::Named(Named::Enter) => Taste::Aktivieren,
                Key::Character(ref c) if c.as_str() == "f" && modifiers.control() => Taste::Suchen,
                Key::Character(ref c) if c.as_str() == "," && modifiers.control() => {
                    Taste::Einstellungen
                }
                Key::Character(ref c) if c.as_str() == "z" && modifiers.control() => {
                    Taste::Rueckgaengig
                }
                Key::Character(ref c) if c.as_str() == "r" && modifiers.control() => {
                    Taste::Aktualisieren
                }
                _ => return None,
            })
        })
        .with(map)
        .map(|(map, taste)| map(taste))
}

/// Index-basierter Fokus: Tab wandert, Enter aktiviert, der Ring zeigt wo.
/// (Eigenbau, weil iced-Buttons nicht fokussierbar sind; AccessKit folgt,
/// sobald iced es upstream anbietet.)
#[derive(Debug, Clone, Copy)]
pub struct Fokus {
    pos: Option<usize>,
    pub anzahl: usize,
}

impl Fokus {
    pub fn neu(anzahl: usize) -> Self {
        Self { pos: None, anzahl }
    }
    pub fn setze_anzahl(&mut self, n: usize) {
        self.anzahl = n;
        if let Some(p) = self.pos {
            if p >= n {
                self.pos = n.checked_sub(1);
            }
        }
    }
    pub fn weiter(&mut self) {
        if self.anzahl == 0 {
            return;
        }
        self.pos = Some(match self.pos {
            None => 0,
            Some(p) => (p + 1) % self.anzahl,
        });
    }
    pub fn zurueck(&mut self) {
        if self.anzahl == 0 {
            return;
        }
        self.pos = Some(match self.pos {
            None | Some(0) => self.anzahl - 1,
            Some(p) => p - 1,
        });
    }
    pub fn loeschen(&mut self) {
        self.pos = None;
    }
    pub fn ist(&self, i: usize) -> bool {
        self.pos == Some(i)
    }
    pub fn aktuell(&self) -> Option<usize> {
        self.pos
    }
}

/// Der Fokusring: zwei Signale (Farbe + Stärke), wie das Designsystem
/// es für Fokus verlangt. Auf jede Randdefinition anwendbar.
pub fn fokus_ring(fokussiert: bool, radius: f32, p: mk::Palette) -> iced::Border {
    if fokussiert {
        iced::Border {
            color: color(p.primary),
            width: 2.0,
            radius: radius.into(),
        }
    } else {
        iced::Border { radius: radius.into(), ..Default::default() }
    }
}

// ---------------------------------------------------------------------------
// Formular-Grammatik — das Leitbild-27-Einstellungs-Vokabular (5.7.-Studie):
// Sektions-Titel klein AUSSERHALB der Karte, Karten mit Haarlinien zwischen
// den Zeilen, Beschreibungen klein-grau direkt unter dem Zeilentitel.
// ---------------------------------------------------------------------------

/// Eine Einstellungs-Sektion: Titel + Karte, Zeilen durch Haarlinien getrennt.
pub fn sektion<'a, M: 'a>(
    titel: &'a str,
    zeilen: Vec<Element<'a, M>>,
    p: mk::Palette,
) -> Element<'a, M> {
    let mut karte = column![].spacing(0);
    let n = zeilen.len();
    for (i, z) in zeilen.into_iter().enumerate() {
        karte = karte.push(z);
        if i + 1 < n {
            karte = karte.push(
                container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
                    .padding(iced::Padding { left: mk::spacing::M, ..iced::Padding::ZERO })
                    .style(move |_| container::Style {
                        background: Some(
                            color(p.outline.over(p.surface_container_high, 0.18)).into(),
                        ),
                        ..Default::default()
                    }),
            );
        }
    }
    let mut spalte = column![].spacing(mk::spacing::XS);
    if !titel.is_empty() {
        spalte = spalte.push(
            container(txt(titel, mk::typo::ETIKETT, p.on_surface_variant))
                .padding(iced::Padding { left: mk::spacing::S, ..iced::Padding::ZERO }),
        );
    }
    spalte
        .push(
            container(karte)
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container_high).into()),
                    border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                    shadow: elevation::karte(),
                    ..Default::default()
                }),
        )
        .into()
}

/// Grundzeile: [führendes Element] Titel (+ Beschreibung darunter) … rechtes Element.
pub fn zeile<'a, M: 'a>(
    titel: &'a str,
    beschreibung: Option<&'a str>,
    fuehrend: Option<Element<'a, M>>,
    rechts: Option<Element<'a, M>>,
    p: mk::Palette,
) -> Element<'a, M> {
    let mut textspalte = column![
        txt(titel, mk::typo::FLIESS, p.on_surface),
    ]
    .spacing(2);
    if let Some(b) = beschreibung {
        textspalte = textspalte.push(txt(b, mk::typo::KLEIN, p.on_surface_variant));
    }
    let mut r = row![].spacing(mk::spacing::M).align_y(iced::Alignment::Center);
    if let Some(f) = fuehrend {
        r = r.push(f);
    }
    // Textspalte nimmt den Restplatz und BRICHT UM — lange Beschreibungen
    // duerfen das rechte Element nie zerquetschen.
    r = r.push(container(textspalte).width(Length::Fill));
    if let Some(re) = rechts {
        r = r.push(re);
    }
    container(r)
        .padding(iced::Padding {
            top: mk::spacing::S + 2.0,
            right: mk::spacing::M,
            bottom: mk::spacing::S + 2.0,
            left: mk::spacing::M,
        })
        .width(Length::Fill)
        .into()
}

/// Schalter-Zeile (Option: None = gesperrt/stumpf).
pub fn zeile_schalter<'a, M: Clone + 'a>(
    titel: &'a str,
    beschreibung: Option<&'a str>,
    fuehrend: Option<Element<'a, M>>,
    an: bool,
    p: mk::Palette,
    on: Option<M>,
) -> Element<'a, M> {
    zeile(titel, beschreibung, fuehrend, Some(schalter_gesichert(an, p, on)), p)
}

/// Wert-Zeile: grauer Wert rechts (nur Anzeige).
pub fn zeile_wert<'a, M: 'a>(
    titel: &'a str,
    beschreibung: Option<&'a str>,
    wert: &'a str,
    p: mk::Palette,
) -> Element<'a, M> {
    zeile(
        titel,
        beschreibung,
        None,
        Some(txt(wert, mk::typo::FLIESS, p.on_surface_variant).into()),
        p,
    )
}

/// Knopf-Zeile: graue Pille rechts (Leitbild "Verwalten …").
pub fn zeile_knopf<'a, M: Clone + 'a>(
    titel: &'a str,
    beschreibung: Option<&'a str>,
    knopf: &'a str,
    p: mk::Palette,
    on: M,
) -> Element<'a, M> {
    let pille = iced::widget::button(
        text(knopf).size(mk::font_size::SMALL).color(color(p.on_surface)),
    )
    .padding([4, mk::spacing::M as u16])
    .on_press(on)
    .style(move |_, status| {
        let base = p.on_surface.over(p.surface_container_high, 0.08);
        let bg = match status {
            iced::widget::button::Status::Hovered => p.on_surface.over(base, mk::state_layer::HOVER),
            iced::widget::button::Status::Pressed => p.on_surface.over(base, mk::state_layer::PRESSED),
            _ => base,
        };
        iced::widget::button::Style {
            background: Some(color(bg).into()),
            border: iced::Border { radius: mk::radius::NORMAL.into(), ..Default::default() },
            ..Default::default()
        }
    });
    zeile(titel, beschreibung, None, Some(pille.into()), p)
}

/// Navigations-Zeile: ganze Zeile klickbar, › rechts.
pub fn zeile_navigation<'a, M: Clone + 'a>(
    titel: &'a str,
    beschreibung: Option<&'a str>,
    p: mk::Palette,
    on: M,
) -> Element<'a, M> {
    iced::widget::button(zeile::<M>(
        titel,
        beschreibung,
        None,
        Some(symbol(symbol::CHEVRON_RIGHT, mk::font_size::MEDIUM, p.on_surface_variant)),
        p,
    ))
    .padding(0)
    .on_press(on)
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
        iced::widget::button::Style { background: bg, ..Default::default() }
    })
    .into()
}

/// Auswahl-Zeile: Wert + ⌃⌄ rechts (Pulldown-Optik; öffnet App-Logik).
pub fn zeile_auswahl<'a, M: Clone + 'a>(
    titel: &'a str,
    beschreibung: Option<&'a str>,
    wert: &'a str,
    p: mk::Palette,
    on: M,
) -> Element<'a, M> {
    let rechts = iced::widget::button(
        row![
            text(wert).size(mk::font_size::MEDIUM).color(color(p.on_surface)),
            Space::new().width(mk::spacing::XS),
            symbol::<M>(symbol::UNFOLD_MORE, mk::font_size::SMALL, p.on_surface_variant),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([2, mk::spacing::S as u16])
    .on_press(on)
    .style(move |_, status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => {
                Some(color(p.on_surface.over(p.surface_container_high, mk::state_layer::HOVER)).into())
            }
            _ => None,
        };
        iced::widget::button::Style {
            background: bg,
            border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
            ..Default::default()
        }
    });
    zeile(titel, beschreibung, None, Some(rechts.into()), p)
}

// ---------------------------------------------------------------------------
// Einstellungs-Controls — Leitbild-System-Settings-Vokabular (Runde 6):
// Slider, segmentierte Auswahl, Stepper, aufklappbare Gruppe. Die Bausteine,
// aus denen eine künftige Matrix-Einstellungen-App gebaut wird.
// ---------------------------------------------------------------------------

/// Schieberegler (Leitbild Slider): Akzent-gefüllte Spur, runder Griff.
/// `bereich` = Wertespanne, `wert` = aktueller Wert, `schritt` = Rasterung.
pub fn schieber<'a, M: Clone + 'a>(
    bereich: std::ops::RangeInclusive<f32>,
    wert: f32,
    schritt: f32,
    on_change: impl Fn(f32) -> M + 'a,
    p: mk::Palette,
) -> Element<'a, M> {
    iced::widget::slider(bereich, wert, on_change)
        .step(schritt)
        .height(22.0)
        .style(move |_, status| {
            // Zwei-kanaliges Zustands-Feedback (Leitlinien): Griff wächst bei Hover,
            // beim Ziehen hellt zusätzlich die Rest-Spur auf.
            let (griff, spur) = match status {
                iced::widget::slider::Status::Dragged => (10.0, 0.24),
                iced::widget::slider::Status::Hovered => (10.0, 0.15),
                _ => (9.0, 0.15),
            };
            iced::widget::slider::Style {
                rail: iced::widget::slider::Rail {
                    backgrounds: (
                        color(p.primary).into(),
                        color(p.on_surface.over(p.surface_container, spur)).into(),
                    ),
                    width: 4.0,
                    border: iced::Border { radius: mk::radius::MINI.into(), ..Default::default() },
                },
                handle: iced::widget::slider::Handle {
                    shape: iced::widget::slider::HandleShape::Circle { radius: griff },
                    background: color(p.primary).into(),
                    border_width: 2.0,
                    border_color: color(p.surface_container),
                },
            }
        })
        .into()
}

/// Segmentierte Auswahl (Leitbild Picker `.segmented`): eine getönte Spur mit
/// gleich breiten Segmenten; das aktive ist akzent-gefüllt. Für wenige,
/// gleichrangige Optionen (Hell/Dunkel/Auto, Größenstufen …).
pub fn segmente<'a, M: Clone + 'a>(
    optionen: &'a [&'a str],
    aktiv: usize,
    on_select: impl Fn(usize) -> M + 'a,
    p: mk::Palette,
) -> Element<'a, M> {
    let mut reihe = row![].spacing(2);
    for (i, opt) in optionen.iter().enumerate() {
        let ist_aktiv = i == aktiv;
        let schrift = if ist_aktiv { p.on_primary } else { p.on_surface };
        reihe = reihe.push(
            iced::widget::button(txt(*opt, mk::typo::KLEIN, schrift).center().width(Length::Fill))
                .padding([mk::spacing::XS as u16, mk::spacing::S as u16])
                .width(Length::Fill)
                .on_press(on_select(i))
                .style(move |_, status| {
                    let bg = if ist_aktiv {
                        Some(color(p.primary).into())
                    } else {
                        match status {
                            iced::widget::button::Status::Hovered => {
                                Some(color(p.on_surface.over(p.surface_container_high, mk::state_layer::HOVER)).into())
                            }
                            iced::widget::button::Status::Pressed => {
                                Some(color(p.on_surface.over(p.surface_container_high, mk::state_layer::PRESSED)).into())
                            }
                            _ => None,
                        }
                    };
                    iced::widget::button::Style {
                        background: bg,
                        border: iced::Border { radius: mk::radius::innen(mk::radius::KLEIN, 2.0).into(), ..Default::default() },
                        ..Default::default()
                    }
                }),
        );
    }
    container(reihe)
        .padding(2)
        .style(move |_| container::Style {
            background: Some(color(p.on_surface.over(p.surface_container, 0.08)).into()),
            border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

/// Stepper (Leitbild Stepper): − [Wert] + für kleine numerische Anpassungen.
/// `None` = Grenze erreicht — die Taste dimmt und reagiert nicht (wie das Leitbild).
pub fn stepper<'a, M: Clone + 'a>(
    wert: impl iced::widget::text::IntoFragment<'a>,
    on_minus: Option<M>,
    on_plus: Option<M>,
    p: mk::Palette,
) -> Element<'a, M> {
    let taste = |zeichen: char, msg: Option<M>| {
        let aktiv = msg.is_some();
        let glyph_farbe = if aktiv {
            p.on_surface
        } else {
            p.on_surface_variant.over(p.surface_container_high, 0.5)
        };
        iced::widget::button(symbol::<M>(zeichen, mk::font_size::MEDIUM, glyph_farbe))
            .padding([2, mk::spacing::S as u16])
            .on_press_maybe(msg)
            .style(move |_, status| {
                let base = p.on_surface.over(p.surface_container_high, if aktiv { 0.08 } else { 0.04 });
                let bg = match status {
                    iced::widget::button::Status::Hovered => base.over(p.surface_container, mk::state_layer::HOVER),
                    iced::widget::button::Status::Pressed => base.over(p.surface_container, mk::state_layer::PRESSED),
                    _ => base,
                };
                iced::widget::button::Style {
                    background: Some(color(bg).into()),
                    border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
                    ..Default::default()
                }
            })
    };
    row![
        lupe(taste(symbol::REMOVE, on_minus)),
        container(txt(wert, mk::typo::FLIESS, p.on_surface))
            .width(Length::Fixed(56.0))
            .align_x(iced::alignment::Horizontal::Center),
        lupe(taste(symbol::ADD, on_plus)),
    ]
    .spacing(mk::spacing::XS)
    .align_y(iced::Alignment::Center)
    .into()
}

/// Ein Eintrag im Kontextmenü (Leitbild ContextMenu-Grammatik): Label,
/// optionales Symbol links, destruktive Rolle in error-Farbe.
#[derive(Clone)]
pub struct MenuePunkt<'a, M> {
    pub label: &'a str,
    pub symbol: Option<char>,
    pub destruktiv: bool,
    pub msg: M,
}

/// Geschätzte Maße der Menü-Karte (für Kanten-Flip an Fenster- und
/// Schirmrändern): eine Familien-Zeile ist ~31 pt hoch, dazu Hüllen-
/// und Pillen-Polster. Schätzung reicht — geklappt wird, BEVOR es eng wird.
pub fn menue_hoehe(punkte: usize, mit_kopf: bool) -> f32 {
    let zeilen = punkte as f32 * 31.0;
    let kopf = if mit_kopf { 30.0 } else { 0.0 };
    zeilen + kopf + 4.0 * mk::spacing::S
}

/// Kontextmenü (Leitbild ContextMenu, Runde 10 — der Overlay-Bau): legt über
/// den Inhalt eine unsichtbare Schließ-Fläche und die Menü-Karte an der
/// Klick-Position. Die App hält `Option<Point>` (Rechtsklick öffnet, siehe
/// mouse_area::on_right_press + on_move fürs Positions-Tracking), Esc und
/// Klick daneben schließen.
pub fn kontextmenue<'a, M: Clone + 'a>(
    inhalt: Element<'a, M>,
    punkte: Vec<MenuePunkt<'a, M>>,
    offen_bei: Option<iced::Point>,
    on_schliessen: M,
    p: mk::Palette,
) -> Element<'a, M> {
    use iced::widget::stack;
    let Some(pos) = offen_bei else { return inhalt };

    // Schließ-Fläche: unsichtbar, fängt Links- UND Rechtsklick daneben ab.
    let schleier = mouse_area(
        container(Space::new().width(Length::Fill).height(Length::Fill)),
    )
    .on_press(on_schliessen.clone())
    .on_right_press(on_schliessen);

    // MatrixUI MenuFamily: auch In-App-Kontextmenüs sprechen die EINE
    // Menü-Sprache (destruktiv = Error-Farbe der Familie).
    let anzahl = punkte.len();
    let eintraege: Vec<ui::MenuEintrag<M>> = punkte
        .into_iter()
        .map(|punkt| ui::MenuEintrag::Punkt {
            zeichen: punkt.symbol,
            titel: String::from(punkt.label),
            farbe: punkt.destruktiv.then_some(p.error),
            msg: punkt.msg,
        })
        .collect();

    // Karte an der Klick-Position verankern (leicht versetzt wie das Leitbild) —
    // und wie das Leitbild an den Kanten FLIPPEN: reicht der Platz nach unten/
    // rechts nicht, klappt das Menü über den Klickpunkt nach oben/links.
    let hoehe = menue_hoehe(anzahl, false);
    let platziert = iced::widget::responsive(move |flaeche| {
        let eintraege: Vec<ui::MenuEintrag<M>> = eintraege.clone();
        let karte = ui::menu_family(None, eintraege, p);
        let mut x = pos.x - 4.0;
        let mut y = pos.y - 4.0;
        if x + ui::MENU_BREITE > flaeche.width {
            x = pos.x - ui::MENU_BREITE + 4.0;
        }
        if y + hoehe > flaeche.height {
            y = pos.y - hoehe + 4.0;
        }
        container(karte)
            .padding(iced::Padding {
                left: x.max(0.0),
                top: y.max(0.0),
                ..iced::Padding::ZERO
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    });

    stack![inhalt, schleier, platziert].into()
}

/// Auswahlmenü (Leitbild Picker/Dropdown): getönte Pille mit aktuellem Wert
/// und ⌄; Klick öffnet die Optionsliste. Für mehr als drei gleichrangige
/// Optionen — bei zwei/drei bleiben `segmente` erste Wahl.
pub fn auswahlmenue<'a, T, M>(
    optionen: &'a [T],
    gewaehlt: Option<T>,
    on_select: impl Fn(T) -> M + 'a,
    p: mk::Palette,
) -> Element<'a, M>
where
    T: ToString + PartialEq + Clone + 'a,
    M: Clone + 'a,
{
    iced::widget::pick_list(optionen, gewaehlt, on_select)
        .text_size(mk::font_size::MEDIUM)
        .padding([4, mk::spacing::S as u16])
        .style(move |_, status| iced::widget::pick_list::Style {
            text_color: color(p.on_surface),
            placeholder_color: color(p.on_surface_variant),
            handle_color: color(p.on_surface_variant),
            background: color(p.on_surface.over(
                p.surface_container_high,
                if matches!(status, iced::widget::pick_list::Status::Hovered) {
                    mk::state_layer::HOVER
                } else {
                    0.08
                },
            ))
            .into(),
            border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
        })
        .menu_style(move |_| iced::widget::overlay::menu::Style {
            background: color(p.surface_container_high).into(),
            border: iced::Border {
                radius: mk::radius::KLEIN.into(),
                width: 1.0,
                color: color(p.outline.over(p.surface_container_high, 0.3)),
            },
            text_color: color(p.on_surface),
            selected_text_color: color(p.on_primary),
            selected_background: color(p.primary).into(),
            shadow: elevation::schwebend(),
        })
        .into()
}

/// Formularzeile mit Auswahlmenü rechts.
pub fn zeile_menue<'a, T, M>(
    titel: &'a str,
    beschreibung: Option<&'a str>,
    optionen: &'a [T],
    gewaehlt: Option<T>,
    on_select: impl Fn(T) -> M + 'a,
    p: mk::Palette,
) -> Element<'a, M>
where
    T: ToString + PartialEq + Clone + 'a,
    M: Clone + 'a,
{
    zeile(titel, beschreibung, None, Some(auswahlmenue(optionen, gewaehlt, on_select, p)), p)
}

/// Beschriftetes Textfeld (Leitbild TextField-Grammatik): ETIKETT darüber,
/// getöntes Feld mit Fokus-Doppelsignal (Primärfarbe + dickerer Rand),
/// optionale Fehlerzeile darunter (error-Rolle statt Wackeln).
pub fn textfeld<'a, M: Clone + 'a>(
    label: &'a str,
    wert: &str,
    platzhalter: &str,
    on_input: impl Fn(String) -> M + 'a,
    on_submit: Option<M>,
    fehler: Option<&'a str>,
    geheim: bool,
    p: mk::Palette,
) -> Element<'a, M> {
    let ist_fehler = fehler.is_some();
    let feld = iced::widget::text_input(platzhalter, wert)
        .on_input(on_input)
        .on_submit_maybe(on_submit)
        .secure(geheim)
        .size(mk::font_size::MEDIUM)
        .padding([mk::spacing::XS as u16, mk::spacing::S as u16])
        .style(move |_, status| {
            let fokus = matches!(status, iced::widget::text_input::Status::Focused { .. });
            if fokus {
                // R58: fokussiertes Feld ruft die Bildschirmtastatur —
                // das Cursor-Blinken liefert den 500-ms-Herzschlag gratis.
                tastatur::funken();
            }
            iced::widget::text_input::Style {
                background: color(p.on_surface.over(p.surface_container, 0.08)).into(),
                border: iced::Border {
                    radius: mk::radius::KLEIN.into(),
                    width: if fokus || ist_fehler { 1.5 } else { 1.0 },
                    color: color(if ist_fehler {
                        p.error
                    } else if fokus {
                        p.primary
                    } else {
                        p.outline.over(p.surface_container, 0.35)
                    }),
                },
                icon: color(p.on_surface_variant),
                placeholder: color(p.on_surface_variant),
                value: color(p.on_surface),
                selection: color(p.primary.over(p.surface_container, 0.35)),
            }
        });
    let mut spalte = column![
        txt(label, mk::typo::ETIKETT, p.on_surface_variant),
        Space::new().height(mk::spacing::XXS),
        feld,
    ]
    .spacing(0);
    if let Some(f) = fehler {
        spalte = spalte
            .push(Space::new().height(mk::spacing::XXS))
            .push(txt(f, mk::typo::KLEIN, p.error));
    }
    spalte.into()
}

/// Der Matrix-Puls als universelles Ladezeichen (unbestimmter Fortschritt —
/// dieselben drei Punkte wie beim Systemstart). `phase` treibt die App über
/// ihren Animations-Tick voran (0..1, zyklisch); bei reduzierter Bewegung
/// stehen die Punkte gleichmäßig gedimmt.
pub fn puls<'a, M: 'a>(phase: f32, grundfarbe: mk::Rgba) -> Element<'a, M> {
    let mut reihe = row![].spacing(mk::spacing::XS).align_y(iced::Alignment::Center);
    for i in 0..3 {
        let deck = if mk::bewegung_reduziert() {
            0.55
        } else {
            let ph = phase * std::f32::consts::TAU - i as f32 * 0.9;
            0.30 + 0.70 * (0.5 + 0.5 * ph.sin())
        };
        let farbe = mk::Rgba { a: grundfarbe.a * deck, ..grundfarbe };
        reihe = reihe.push(
            container(Space::new().width(Length::Fixed(8.0)).height(Length::Fixed(8.0))).style(
                move |_| container::Style {
                    background: Some(color(farbe).into()),
                    border: iced::Border { radius: mk::radius::kapsel(8.0).into(), ..Default::default() },
                    ..Default::default()
                },
            ),
        );
    }
    reihe.into()
}

/// Einfacher Hinweis-Dialog (Leitbild Alert): Titel, Botschaft, EIN Knopf.
/// Für Bestätigungen mit Abbrechen-Weg weiterhin `bestaetigung` nutzen.
pub fn alert<'a, M: Clone + 'a>(
    titel: &'a str,
    botschaft: String,
    on_ok: M,
    zustand: &DialogZustand,
    p: mk::Palette,
) -> Element<'a, M> {
    use iced::widget::stack;
    let f = zustand.fortschritt().clamp(0.0, 1.2);
    let schleier_alpha = 0.55 * f.min(1.0);
    let schleier = mouse_area(
        container(Space::new().width(Length::Fill).height(Length::Fill)).style(move |_| {
            container::Style {
                background: Some(iced::Color::from_rgba(0.08, 0.08, 0.08, schleier_alpha).into()),
                ..Default::default()
            }
        }),
    )
    .on_press(on_ok.clone());

    let karte = column![
        txt(titel, mk::typo::UNTERTITEL, p.on_surface),
        Space::new().height(mk::spacing::S),
        txt(botschaft, mk::typo::FLIESS, p.on_surface_variant),
        Space::new().height(mk::spacing::L),
        knopf("OK", knopfart::Stil::Prominent, knopfart::Rolle::Normal, knopfart::Groesse::Normal, p, Some(on_ok))
            .width(Length::Fill),
    ]
    .spacing(0);

    let versatz = (80.0 * (1.0 - f)).max(0.0);
    let panel = container(
        container(
            container(karte)
                .padding(mk::spacing::XL)
                .width(Length::Fixed(360.0))
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container_high).into()),
                    border: iced::Border { radius: mk::CORNER_RADIUS.into(), ..Default::default() },
                    shadow: elevation::schwebend(),
                    ..Default::default()
                }),
        )
        .padding(iced::Padding { top: versatz, ..iced::Padding::ZERO }),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center);

    stack![schleier, panel].into()
}

/// Suchfeld (Leitbild `.searchable`): Lupe, Kapsel-Feld, ×-Löschen bei Inhalt.
/// Die Esc-Semantik (dismissSearch) verdrahtet die App in ihrem Tasten-Abo
/// mit derselben Nachricht wie `on_leeren`.
/// Eingabefeld-Familie (R47): DAS Textfeld des Kits — ein Padding, ein
/// Radius, eine Fokus-Sprache (primary-Rand). Vorher bauten neun Apps
/// elf eigene text_inputs. `geheim` = Passwort-Punkte.
pub fn eingabefeld<'a, M: Clone + 'a>(
    platzhalter: &str,
    wert: &str,
    on_input: impl Fn(String) -> M + 'a,
    on_submit: Option<M>,
    geheim: bool,
    p: mk::Palette,
) -> Element<'a, M> {
    let mut feld = iced::widget::text_input(platzhalter, wert)
        .on_input(on_input)
        .secure(geheim)
        .size(mk::font_size::MEDIUM)
        .padding(iced::Padding {
            top: 6.0,
            right: mk::spacing::S,
            bottom: 6.0,
            left: mk::spacing::S,
        })
        .style(move |_, status| {
            let fokus = matches!(status, iced::widget::text_input::Status::Focused { .. });
            if fokus {
                // R58: fokussiertes Feld ruft die Bildschirmtastatur —
                // das Cursor-Blinken liefert den 500-ms-Herzschlag gratis.
                tastatur::funken();
            }
            iced::widget::text_input::Style {
                background: color(p.on_surface.over(p.surface_container, 0.08)).into(),
                border: iced::Border {
                    color: color(if fokus {
                        p.primary
                    } else {
                        p.outline.over(p.surface_container, 0.35)
                    }),
                    width: if fokus { 2.0 } else { 1.0 },
                    radius: mk::radius::KLEIN.into(),
                },
                icon: color(p.on_surface_variant),
                placeholder: color(p.on_surface_variant),
                value: color(p.on_surface),
                selection: color(p.primary.over(p.surface_container, 0.35)),
            }
        });
    if let Some(m) = on_submit {
        feld = feld.on_submit(m);
    }
    feld.into()
}

/// Meldungs-Familie (R47): das Ergebnis-Banner — primary = gelungen,
/// error = fehlgeschlagen. Vorher zweimal handgerollt (dev,
/// verbindungen) plus eine dritte Quittungs-Variante.
pub fn meldung<'a, M: 'a>(ok: bool, text: String, p: mk::Palette) -> Element<'a, M> {
    let farbe = if ok { p.primary } else { p.error };
    container(txt(text, mk::typo::FLIESS, farbe))
        .padding(mk::spacing::S)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(color(farbe.over(p.surface_container, 0.12)).into()),
            border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

pub fn suchfeld<'a, M: Clone + 'a>(
    wert: &str,
    platzhalter: &str,
    on_input: impl Fn(String) -> M + 'a,
    on_leeren: M,
    p: mk::Palette,
) -> Element<'a, M> {
    let innen = iced::widget::text_input(platzhalter, wert)
        .id(iced::advanced::widget::Id::new("mkw-suchfeld"))
        .on_input(on_input)
        .size(mk::font_size::MEDIUM)
        .padding(0)
        .width(Length::Fill)
        .style(move |_, status| {
            if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                tastatur::funken(); // R58: auch die Suche ruft die Tastatur
            }
            iced::widget::text_input::Style {
                background: iced::Color::TRANSPARENT.into(),
                border: iced::Border::default(),
                icon: color(p.on_surface_variant),
                placeholder: color(p.on_surface_variant),
                value: color(p.on_surface),
                selection: color(p.primary.over(p.surface_container, 0.35)),
            }
        });
    let mut zeile = row![
        symbol::<M>(symbol::SEARCH, mk::font_size::MEDIUM, p.on_surface_variant),
        Space::new().width(mk::spacing::XS),
        innen,
    ]
    .align_y(iced::Alignment::Center);
    if !wert.is_empty() {
        zeile = zeile.push(
            iced::widget::button(symbol::<M>(symbol::CLOSE, mk::font_size::SMALL, p.on_surface_variant))
                .padding(2)
                .on_press(on_leeren)
                .style(move |_, status| iced::widget::button::Style {
                    background: matches!(status, iced::widget::button::Status::Hovered)
                        .then(|| color(p.on_surface.over(p.surface_container_high, mk::state_layer::HOVER)).into()),
                    border: iced::Border { radius: mk::radius::kapsel(20.0).into(), ..Default::default() },
                    ..Default::default()
                }),
        );
    }
    container(zeile)
        .padding(iced::Padding {
            top: 6.0,
            right: mk::spacing::S,
            bottom: 6.0,
            left: mk::spacing::S,
        })
        .style(move |_| container::Style {
            background: Some(color(p.on_surface.over(p.surface_container, 0.08)).into()),
            border: iced::Border { radius: mk::radius::kapsel(32.0).into(), ..Default::default() },
            ..Default::default()
        })
        .into()
}

/// Die Suche fokussieren (Strg+F, Leitbild keyboardShortcut-Grammatik):
/// setzt den Eingabe-Fokus in das mkw::suchfeld der App.
pub fn suche_fokussieren<M: Send + 'static>() -> Task<M> {
    iced::advanced::widget::operate(iced::advanced::widget::operation::focusable::focus(
        iced::advanced::widget::Id::new("mkw-suchfeld"),
    ))
}

/// Läuft diese Sitzung unter dem Leinwand-Compositor? Laufzeit-WAHRHEIT
/// statt Konfigurationsglaube: wir suchen den niri-leinwand-Prozess des
/// eigenen Nutzers in /proc (comm ist auf 15 Zeichen gekürzt).
pub fn session_ist_leinwand() -> bool {
    let Ok(eintraege) = std::fs::read_dir("/proc") else {
        return false;
    };
    for e in eintraege.flatten() {
        let name = e.file_name();
        let Some(pid) = name.to_str().filter(|s| s.chars().all(|c| c.is_ascii_digit()))
        else {
            continue;
        };
        if let Ok(comm) = std::fs::read_to_string(format!("/proc/{pid}/comm")) {
            if comm.trim() == "niri-leinwan" || comm.trim() == "niri-leinwand" {
                return true;
            }
        }
    }
    false
}

/// Matrix Hilfe mit Suchbegriff öffnen (Leitbild HelpLink, Runde 11).
/// Sucht das Binary an den üblichen Orten (Dev-Setup → System → PATH).
/// Fire-and-forget: schlägt der Start fehl, passiert schlicht nichts —
/// ein fehlender Hilfe-Knopf darf nie die App reißen.
pub fn hilfe_oeffnen(begriff: &str) {
    let kandidaten = [
        std::env::var_os("HOME")
            .map(|h| std::path::PathBuf::from(h).join(".local/bin/matrix-hilfe")),
        Some(std::path::PathBuf::from("/usr/bin/matrix-hilfe")),
    ];
    let programm = kandidaten
        .into_iter()
        .flatten()
        .find(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from("matrix-hilfe"));
    let _ = std::process::Command::new(programm)
        .arg("--suche")
        .arg(begriff)
        .spawn();
}

/// Matrix Einstellungen öffnen (Leitbild Settings-Szene / Strg+, — Runde 12).
/// Läuft die App schon, fokussiert die Einzelinstanz sie nur.
pub fn einstellungen_oeffnen() {
    let kandidaten = [
        std::env::var_os("HOME")
            .map(|h| std::path::PathBuf::from(h).join(".local/bin/matrix-einstellungen")),
        Some(std::path::PathBuf::from("/usr/bin/matrix-einstellungen")),
    ];
    let programm = kandidaten
        .into_iter()
        .flatten()
        .find(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from("matrix-einstellungen"));
    let _ = std::process::Command::new(programm).spawn();
}

/// Abzeichen (Leitbild badge, Runde 12): kleine Zahlen-Kapsel für Listen-
/// zeilen — „wie viele Artikel stecken in dieser Kategorie?"
pub fn abzeichen<'a, M: 'a>(anzahl: usize, p: mk::Palette) -> Element<'a, M> {
    container(
        text(anzahl.to_string())
            .size(mk::font_size::SMALL)
            .color(color(p.on_surface_variant)),
    )
    .padding(iced::Padding {
        top: 1.0,
        right: 7.0,
        bottom: 1.0,
        left: 7.0,
    })
    .style(move |_| container::Style {
        background: Some(color(p.on_surface.over(p.surface_container, 0.08)).into()),
        border: iced::Border {
            radius: mk::radius::kapsel(20.0).into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Eine Tabellen-Spalte (Leitbild Table/TableColumn, Runde 12).
pub struct Spalte {
    pub titel: &'static str,
    /// Anteil an der Gesamtbreite (FillPortion) — 0 = schmal fixiert (80px).
    pub anteil: u16,
    /// Zahlen-Spalten stehen rechtsbündig (Leitbild-Konvention).
    pub rechts: bool,
}

/// Sortierbare Tabelle (Leitbild Table + sortOrder): klickbare Spaltenköpfe
/// mit Sortier-Pfeil, Zebra-Zeilen. Die App hält die Sortierung als
/// Zustand (Spalte + Richtung) und sortiert ihre Daten selbst — wie das
/// Leitbild-UI-sortOrder-Binding.
pub fn tabelle<'a, M: Clone + 'a>(
    spalten: &'a [Spalte],
    zeilen: Vec<Vec<String>>,
    sort_spalte: usize,
    absteigend: bool,
    on_sort: impl Fn(usize) -> M + 'a,
    p: mk::Palette,
) -> Element<'a, M> {
    let zelle_breite = |s: &Spalte| {
        if s.anteil == 0 {
            Length::Fixed(80.0)
        } else {
            Length::FillPortion(s.anteil)
        }
    };

    // Kopfzeile: Klick sortiert, Pfeil zeigt die aktive Richtung
    let mut kopf = iced::widget::Row::new().spacing(mk::spacing::S);
    for (i, s) in spalten.iter().enumerate() {
        let aktiv = i == sort_spalte;
        let pfeil = if !aktiv {
            ""
        } else if absteigend {
            " ↓"
        } else {
            " ↑"
        };
        let beschriftung = text(format!("{}{}", s.titel, pfeil))
            .size(mk::font_size::SMALL)
            .color(color(if aktiv { p.primary } else { p.on_surface_variant }));
        let inhalt = container(beschriftung).width(zelle_breite(s)).align_x(if s.rechts {
            iced::alignment::Horizontal::Right
        } else {
            iced::alignment::Horizontal::Left
        });
        kopf = kopf.push(
            mouse_area(inhalt)
                .interaction(iced::mouse::Interaction::Pointer)
                .on_press(on_sort(i)),
        );
    }

    let mut spalte = column![
        container(kopf).padding(iced::Padding {
            top: 4.0,
            right: mk::spacing::S,
            bottom: 4.0,
            left: mk::spacing::S,
        }),
        container(Space::new().width(Length::Fill).height(1.0)).style(move |_| {
            container::Style {
                background: Some(color(p.on_surface.over(p.surface_container, 0.10)).into()),
                ..Default::default()
            }
        }),
    ];

    // Zebra-Zeilen (Leitbild alternatingRowBackgrounds)
    for (zi, zelle_texte) in zeilen.into_iter().enumerate() {
        let mut zeile = iced::widget::Row::new().spacing(mk::spacing::S);
        for (i, wert) in zelle_texte.into_iter().enumerate() {
            let Some(s) = spalten.get(i) else { break };
            zeile = zeile.push(
                container(
                    text(wert)
                        .size(mk::font_size::SMALL)
                        .color(color(p.on_surface)),
                )
                .width(zelle_breite(s))
                .align_x(if s.rechts {
                    iced::alignment::Horizontal::Right
                } else {
                    iced::alignment::Horizontal::Left
                }),
            );
        }
        let gestreift = zi % 2 == 1;
        spalte = spalte.push(
            container(zeile)
                .padding(iced::Padding {
                    top: 3.0,
                    right: mk::spacing::S,
                    bottom: 3.0,
                    left: mk::spacing::S,
                })
                .style(move |_| container::Style {
                    background: gestreift
                        .then(|| color(p.on_surface.over(p.surface_container, 0.04)).into()),
                    ..Default::default()
                }),
        );
    }
    spalte.into()
}

/// Diagramm-Arten (Swift Charts, Runde 13): LineMark+AreaMark bzw. BarMark.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DiagrammArt {
    Linie,
    Balken,
}

/// Verlaufsdiagramm (Swift Charts): Werte 0..1, jüngster Wert rechts.
/// Charts-Ästhetik: dezente Gitterlinien, Grundlinie (RuleMark), Linie
/// mit Flächenverlauf ODER gerundete Balken — alles aus der Palette.
/// `kapazitaet` = Slots des Zeitfensters (die Kurve wächst nach links).
pub fn diagramm<'a, M: 'a>(
    werte: Vec<f32>,
    kapazitaet: usize,
    art: DiagrammArt,
    p: mk::Palette,
) -> Element<'a, M> {
    iced::widget::canvas(Diagramm {
        werte,
        kapazitaet: kapazitaet.max(2),
        art,
        p,
        _m: std::marker::PhantomData,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

struct Diagramm<M> {
    werte: Vec<f32>,
    kapazitaet: usize,
    art: DiagrammArt,
    p: mk::Palette,
    _m: std::marker::PhantomData<M>,
}

impl<M> iced::widget::canvas::Program<M> for Diagramm<M> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        use iced::widget::canvas::{Frame, Stroke};
        let mut frame = Frame::new(renderer, bounds.size());
        let (w, h) = (bounds.width, bounds.height);
        let p = self.p;

        // Gitter (chartYAxis-Defaults): drei leise Linien bei 25/50/75 %.
        let gitter = color(p.on_surface.over(p.surface_container, 0.06));
        for anteil in [0.25f32, 0.5, 0.75] {
            let y = h - 2.0 - anteil * (h - 4.0);
            let mut g = iced::widget::canvas::path::Builder::new();
            g.move_to(iced::Point::new(0.0, y));
            g.line_to(iced::Point::new(w, y));
            frame.stroke(&g.build(), Stroke::default().with_color(gitter).with_width(1.0));
        }
        // Grundlinie (RuleMark bei 0).
        let grund = color(p.on_surface.over(p.surface_container, 0.15));
        let mut g = iced::widget::canvas::path::Builder::new();
        g.move_to(iced::Point::new(0.0, h - 2.0));
        g.line_to(iced::Point::new(w, h - 2.0));
        frame.stroke(&g.build(), Stroke::default().with_color(grund).with_width(1.0));

        let n = self.werte.len();
        if n >= 1 {
            let step = w / (self.kapazitaet - 1) as f32;
            let x0 = w - step * (n.saturating_sub(1)) as f32;
            let pt = |i: usize| -> iced::Point {
                let v = self.werte[i].clamp(0.0, 1.0);
                iced::Point::new(x0 + step * i as f32, h - 2.0 - v * (h - 4.0))
            };
            match self.art {
                DiagrammArt::Linie if n >= 2 => {
                    let mut linie = iced::widget::canvas::path::Builder::new();
                    linie.move_to(pt(0));
                    for i in 1..n {
                        linie.line_to(pt(i));
                    }
                    let mut flaeche = iced::widget::canvas::path::Builder::new();
                    flaeche.move_to(iced::Point::new(x0, h - 2.0));
                    for i in 0..n {
                        flaeche.line_to(pt(i));
                    }
                    flaeche.line_to(iced::Point::new(x0 + step * (n - 1) as f32, h - 2.0));
                    flaeche.close();
                    frame.fill(
                        &flaeche.build(),
                        color(p.primary.over(p.surface_container, 0.15)),
                    );
                    frame.stroke(
                        &linie.build(),
                        Stroke::default().with_color(color(p.primary)).with_width(1.5),
                    );
                }
                DiagrammArt::Balken => {
                    let dicke = (step - 2.0).max(1.0);
                    for i in 0..n {
                        let spitze = pt(i);
                        frame.fill(
                            &iced::widget::canvas::Path::rounded_rectangle(
                                iced::Point::new(spitze.x - dicke / 2.0, spitze.y),
                                iced::Size::new(dicke, (h - 2.0 - spitze.y).max(0.0)),
                                iced::border::Radius::from(2.0),
                            ),
                            color(p.primary),
                        );
                    }
                }
                _ => {}
            }
        }
        vec![frame.into_geometry()]
    }
}

/// Ring-Anzeige (Leitbild Gauge, Runde 14): ein Anteil 0..1 als
/// Kreissegment auf leiser Spur — Countdown, Füllstand, Fortschritt.
/// Beginnt oben, läuft im Uhrzeigersinn (die Uhren-Konvention).
pub fn ring<'a, M: 'a>(
    anteil: f32,
    groesse: f32,
    farbe: mk::Rgba,
    p: mk::Palette,
) -> Element<'a, M> {
    iced::widget::canvas(RingAnzeige {
        anteil: anteil.clamp(0.0, 1.0),
        fill: color(farbe),
        track: color(p.on_surface.over(p.surface_container_high, 0.12)),
        _m: std::marker::PhantomData,
    })
    .width(Length::Fixed(groesse))
    .height(Length::Fixed(groesse))
    .into()
}

struct RingAnzeige<M> {
    anteil: f32,
    fill: Color,
    track: Color,
    _m: std::marker::PhantomData<M>,
}

impl<M> iced::widget::canvas::Program<M> for RingAnzeige<M> {
    type State = ();
    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        use iced::widget::canvas::{path::Arc, path::Builder, Frame, Stroke};
        let mut frame = Frame::new(renderer, bounds.size());
        let center = iced::Point::new(bounds.width / 2.0, bounds.height / 2.0);
        let breite = (bounds.width.min(bounds.height) * 0.103).max(2.5);
        let radius = bounds.width.min(bounds.height) / 2.0 - breite;
        let mut voll = Builder::new();
        voll.circle(center, radius);
        frame.stroke(&voll.build(), Stroke::default().with_color(self.track).with_width(breite));
        if self.anteil > 0.001 {
            let start = -std::f32::consts::FRAC_PI_2;
            let mut bogen = Builder::new();
            bogen.arc(Arc {
                center,
                radius,
                start_angle: iced::Radians(start),
                end_angle: iced::Radians(start + std::f32::consts::TAU * self.anteil),
            });
            frame.stroke(
                &bogen.build(),
                Stroke::default().with_color(self.fill).with_width(breite),
            );
        }
        vec![frame.into_geometry()]
    }
}

/// Aufklappbare Gruppe (Leitbild DisclosureGroup): klickbare Kopfzeile mit
/// Chevron, die ihren Inhalt zeigt/verbirgt. Für tiefere Einstellungs-Ebenen.
pub fn aufklappen<'a, M: Clone + 'a>(
    titel: &'a str,
    offen: bool,
    on_umschalten: M,
    inhalt: Element<'a, M>,
    p: mk::Palette,
) -> Element<'a, M> {
    let chevron = if offen { symbol::EXPAND_MORE } else { symbol::CHEVRON_RIGHT };
    let kopf = iced::widget::button(
        row![
            txt(titel, mk::typo::KOPF, p.on_surface),
            Space::new().width(Length::Fill),
            symbol::<M>(chevron, mk::font_size::LARGE, p.on_surface_variant),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([mk::spacing::S as u16, mk::spacing::M as u16])
    .width(Length::Fill)
    .on_press(on_umschalten)
    .style(move |_, status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => {
                Some(color(p.on_surface.over(p.surface_container_high, mk::state_layer::HOVER)).into())
            }
            _ => None,
        };
        iced::widget::button::Style {
            background: bg,
            border: iced::Border { radius: mk::radius::KLEIN.into(), ..Default::default() },
            ..Default::default()
        }
    });
    let mut spalte = column![kopf].spacing(0);
    if offen {
        spalte = spalte.push(
            container(inhalt).padding(iced::Padding {
                left: mk::spacing::M,
                top: mk::spacing::XS,
                bottom: mk::spacing::XS,
                ..iced::Padding::ZERO
            }),
        );
    }
    spalte.into()
}

/// Scrollbereich mit Kanten-Fade oben (Leitbild scrollEdgeEffect): der Inhalt
/// läuft sichtbar unter der Kopfzeile aus statt hart zu enden.
pub fn scrollbereich_mit_fade<'a, M: 'a>(
    inhalt: Element<'a, M>,
    p: mk::Palette,
) -> Element<'a, M> {
    use iced::gradient::Linear;
    use iced::widget::stack;
    // Kanten-Fades OBEN und UNTEN (Leitbild scrollEdgeEffect): angeschnittene
    // Zeilen laufen weich aus statt hart durchgeschnitten zu wirken.
    // Winkel 0 = von unten nach oben; Stop 0 liegt am UNTEREN Ende.
    let oben = container(Space::new().width(Length::Fill).height(Length::Fixed(18.0))).style(
        move |_| {
            let g = Linear::new(iced::Radians(0.0))
                .add_stop(0.0, color(mk::Rgba { a: 0.0, ..p.surface_container }))
                .add_stop(1.0, color(p.surface_container));
            container::Style {
                background: Some(iced::Background::Gradient(g.into())),
                ..Default::default()
            }
        },
    );
    let unten = container(Space::new().width(Length::Fill).height(Length::Fixed(18.0))).style(
        move |_| {
            let g = Linear::new(iced::Radians(0.0))
                .add_stop(0.0, color(p.surface_container))
                .add_stop(1.0, color(mk::Rgba { a: 0.0, ..p.surface_container }));
            container::Style {
                background: Some(iced::Background::Gradient(g.into())),
                ..Default::default()
            }
        },
    );
    // Innenabstand in Fade-Höhe: am Scroll-Anschlag beginnt/endet der
    // Inhalt AUSSERHALB der Fade-Zone — nichts wird im Ruhezustand
    // angeschnitten; nur beim Scrollen laufen Zeilen weich unter die Kanten.
    let gepolstert = container(inhalt).padding(iced::Padding {
        top: 18.0,
        bottom: 18.0,
        ..iced::Padding::ZERO
    });
    stack![
        scrollbereich(gepolstert.into(), p),
        container(oben)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Top),
        container(unten)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Bottom),
    ]
    .into()
}

// ---------------------------------------------------------------------------
// Der Scrollgeist — eine Matrix-Erfindung (Nutzer, 7.7.2026): Der
// Scrollbalken klebt nicht am Rand der Anwendung, sondern erscheint NEBEN
// DER MAUS, sobald sie über einem scrollbaren Bereich schwebt. Er ist reine
// Anzeige — „hier kannst du scrollen" (Affordanz) plus „so weit bist du"
// (Position + Sichtfenster-Anteil). Gescrollt wird mit Rad/Trackpad; beim
// Verlassen verschwindet der Geist, der Rand bleibt frei.
// ---------------------------------------------------------------------------

/// Scroll-Physik (das Leitbild-Gefühl): Rad-Rasten SPRINGEN nicht, sie
/// gleiten weich ans Ziel (exponentielles Nachlaufen, ~300 ms); Trackpad-
/// Deltas greifen 1:1 und rollen nach dem Loslassen mit Schwung aus
/// (Momentum mit exponentiellem Abklingen). Lebt im Rahmen; die
/// Scrollfläche liefert Rad-Events und Viewport-Lage.
#[derive(Debug, Clone)]
pub struct RollPhysik {
    /// Wohin die Fläche will (px, nach unten positiv).
    pub ziel: f32,
    /// Wo sie gerade ist.
    pub ist: f32,
    /// Maximales Offset (Inhalt − Sichtfenster).
    pub max: f32,
    /// Ausroll-Geschwindigkeit (px/s, Trackpad-/Finger-Momentum).
    v: f32,
    /// Momentum-Zerfall je Sekunde — Trackpad (Leitbild) und Finger (Tablet-Leitbild)
    /// rollen unterschiedlich aus; die Eingabe wählt.
    zerfall: f32,
    letzte_eingabe: std::time::Instant,
}

impl RollPhysik {
    /// Ein Rad-Rasten scrollt drei Zeilenhöhen — wie das Leitbild gemächlich.
    const RASTEN_PX: f32 = 96.0;
    /// Trackpad-Auslauf (~1,3 s, Leitbild-Gefühl).
    const ZERFALL_TRACKPAD: f32 = 0.045;
    /// Finger-Auslauf: UIScrollViewDecelerationRateNormal = 0,998 je ms
    /// (Touch-Referenz-SDK-Extrakt R59) → 0,998^1000 je Sekunde.
    const ZERFALL_FINGER: f32 = 0.135;

    pub fn neu() -> Self {
        Self {
            ziel: 0.0,
            ist: 0.0,
            max: 0.0,
            v: 0.0,
            zerfall: Self::ZERFALL_TRACKPAD,
            letzte_eingabe: std::time::Instant::now() - std::time::Duration::from_secs(60),
        }
    }

    /// Finger zieht (Wischfläche, R59): der Inhalt klebt 1:1 am Finger —
    /// `dy` ist die Fingerbewegung (nach unten positiv).
    pub fn beruehrung_zug(&mut self, dy: f32) {
        self.letzte_eingabe = std::time::Instant::now();
        self.ist = (self.ist - dy).clamp(0.0, self.max);
        self.ziel = self.ist;
        self.v = 0.0;
    }

    /// Finger losgelassen: der Inhalt rollt mit der Fingergeschwindigkeit
    /// weiter und klingt wie UIScrollView ab.
    pub fn beruehrung_ende(&mut self, v_finger: f32) {
        if mk::bewegung_reduziert() {
            return;
        }
        self.v = -v_finger;
        self.zerfall = Self::ZERFALL_FINGER;
        // Kein Anlauf-Delay: das Momentum darf im nächsten Tick greifen.
        self.letzte_eingabe = std::time::Instant::now() - std::time::Duration::from_millis(60);
    }

    /// Lage/Grenzen aus dem Viewport übernehmen (ruhend folgt die Physik
    /// der Realität — Fenstergrößen-Änderungen etc. verschieben nichts).
    pub fn sync(&mut self, vp: iced::widget::scrollable::Viewport) {
        self.max = (vp.content_bounds().height - vp.bounds().height).max(0.0);
        if !self.laeuft() {
            self.ist = vp.absolute_offset().y.clamp(0.0, self.max);
            self.ziel = self.ist;
        }
    }

    /// Rad/Trackpad-Eingabe. Rad (Lines) = weiches Gleiten zum Ziel;
    /// Trackpad (Pixels) = direkt + Schwung fürs Ausrollen danach.
    pub fn eingabe(&mut self, delta: iced::mouse::ScrollDelta) {
        let jetzt = std::time::Instant::now();
        let dt = jetzt.duration_since(self.letzte_eingabe).as_secs_f32().max(0.001);
        self.letzte_eingabe = jetzt;
        match delta {
            iced::mouse::ScrollDelta::Lines { y, .. } => {
                if mk::bewegung_reduziert() {
                    self.ist = (self.ist - y * Self::RASTEN_PX).clamp(0.0, self.max);
                    self.ziel = self.ist;
                } else {
                    self.ziel = (self.ziel - y * Self::RASTEN_PX).clamp(0.0, self.max);
                }
                self.v = 0.0;
            }
            iced::mouse::ScrollDelta::Pixels { y, .. } => {
                // 1:1 unter den Fingern (Leitbild-Trackpad), Schwung merken
                self.zerfall = Self::ZERFALL_TRACKPAD;
                self.ist = (self.ist - y).clamp(0.0, self.max);
                self.ziel = self.ist;
                if dt < 0.2 {
                    self.v = 0.75 * self.v + 0.25 * (-y / dt);
                } else {
                    self.v = 0.0;
                }
            }
        }
    }

    /// 60-fps-Takt: Nachlaufen zum Ziel + Momentum-Ausrollen.
    pub fn tick(&mut self, dt: f32) {
        // Momentum: nach kurzem Trackpad-Stillstand rollt es aus
        let seit = self.letzte_eingabe.elapsed().as_secs_f32();
        if !mk::bewegung_reduziert() && seit > 0.05 && self.v.abs() > 40.0 {
            self.ziel = (self.ziel + self.v * dt).clamp(0.0, self.max);
            // exponentielles Abklingen — Zerfall je Eingabeart (R59)
            self.v *= self.zerfall.powf(dt);
            if self.ziel <= 0.0 || self.ziel >= self.max {
                self.v = 0.0;
            }
        } else if seit > 0.5 {
            self.v = 0.0;
        }
        // Weiches Nachlaufen (Zeitkonstante ~90 ms → ruhig, nie träge)
        let k = 1.0 - (-dt / 0.09f32).exp();
        self.ist += (self.ziel - self.ist) * k;
        if (self.ziel - self.ist).abs() < 0.4 && self.v.abs() <= 40.0 {
            self.ist = self.ziel;
        }
    }

    /// Muss der Takt laufen / ein scroll_to raus?
    pub fn laeuft(&self) -> bool {
        (self.ziel - self.ist).abs() >= 0.4 || self.v.abs() > 40.0
    }
}

/// Zustand des Scrollgeists — lebt im Rahmen (jede App bekommt ihn
/// geschenkt); Ein-/Ausblenden federt sanft.
#[derive(Debug, Clone)]
pub struct ScrollGeist {
    /// Mausposition in Fensterkoordinaten (der Rahmen trackt sie).
    pub maus: iced::Point,
    /// Maus schwebt über dem Scrollbereich.
    pub drin: bool,
    /// Scrollposition 0..1.
    pub offset: f32,
    /// Sichtfenster-Anteil am Inhalt (0..1); ~1 = nichts zu scrollen.
    pub anteil: f32,
    /// Schon eine echte Scroll-Lage empfangen? (Vorher zeigt der Geist
    /// eine neutrale Spur — besser als gar keine Affordanz.)
    pub bekannt: bool,
    /// Deckkraft-Feder (0 = weg, 1 = da) — sanftes Erscheinen/Verschwinden.
    pub deckkraft: mk::motion::Spring,
    /// Rechteck des Scrollbereichs im Fenster — für die Rand-Umkehr wie
    /// beim Fenster-Geist (kippt am Rand auf die andere Seite).
    pub bounds: iced::Rectangle,
}

impl ScrollGeist {
    pub fn neu() -> Self {
        let mut g = Self {
            maus: iced::Point::ORIGIN,
            drin: false,
            offset: 0.0,
            anteil: 0.5,
            bekannt: false,
            deckkraft: mk::motion::Spring::new(0.0),
            bounds: iced::Rectangle { x: 0.0, y: 0.0, width: 0.0, height: 0.0 },
        };
        // Dev-Haken für Screenshots: Geist fixiert sichtbar.
        if std::env::var("MATRIXKIT_GEIST").is_ok() {
            g.drin = true;
            g.bekannt = true;
            g.anteil = 0.45;
            g.offset = 0.35;
            g.maus = iced::Point::new(210.0, 260.0);
            g.deckkraft = mk::motion::Spring::new(1.0);
        }
        g
    }

    /// Scroll-Lage aus einem iced-Viewport übernehmen.
    pub fn scroll(&mut self, vp: iced::widget::scrollable::Viewport) {
        self.offset = vp.relative_offset().y.clamp(0.0, 1.0);
        let (b, c) = (vp.bounds(), vp.content_bounds());
        self.anteil = if c.height > 0.0 { (b.height / c.height).clamp(0.05, 1.0) } else { 1.0 };
        self.bounds = b;
        self.bekannt = true;
    }

    pub fn betreten(&mut self) {
        self.drin = true;
        if mk::bewegung_reduziert() {
            self.deckkraft = mk::motion::Spring::new(1.0);
        } else {
            self.deckkraft.retarget(1.0);
        }
    }

    pub fn verlassen(&mut self) {
        if std::env::var("MATRIXKIT_GEIST").is_ok() {
            return;
        }
        self.drin = false;
        if mk::bewegung_reduziert() {
            self.deckkraft = mk::motion::Spring::new(0.0);
        } else {
            self.deckkraft.retarget(0.0);
        }
    }

    pub fn tick(&mut self) {
        self.deckkraft.tick(1.0 / 60.0);
    }

    pub fn animiert(&self) -> bool {
        !self.deckkraft.is_settled()
    }

    /// Gezeichnet wird, solange Deckkraft da ist (auch beim Ausblenden) —
    /// und nur, wenn es etwas zu scrollen gibt.
    pub fn sichtbar(&self) -> bool {
        self.deckkraft.value > 0.02 && (!self.bekannt || self.anteil < 0.999)
    }
}

/// Scrollbereich für den Scrollgeist: Kanten-Fades wie gehabt, aber OHNE
/// Randbalken — die Lage meldet on_scroll, Betreten/Verlassen die beiden
/// Nachrichten. Der Geist selbst wird per `scrollgeist` über das Fenster
/// gelegt (stack, wie das Kontextmenü).
pub fn scrollbereich_geist<'a, M: Clone + 'a>(
    inhalt: Element<'a, M>,
    id: iced::advanced::widget::Id,
    on_scroll: impl Fn(iced::widget::scrollable::Viewport) -> M + 'a,
    on_rad: impl Fn(iced::mouse::ScrollDelta) -> M + 'a,
    on_betreten: M,
    on_verlassen: M,
    p: mk::Palette,
) -> Element<'a, M> {
    use iced::gradient::Linear;
    use iced::widget::stack;
    let oben = container(Space::new().width(Length::Fill).height(Length::Fixed(18.0))).style(
        move |_| {
            let g = Linear::new(iced::Radians(0.0))
                .add_stop(0.0, color(mk::Rgba { a: 0.0, ..p.surface_container }))
                .add_stop(1.0, color(p.surface_container));
            container::Style {
                background: Some(iced::Background::Gradient(g.into())),
                ..Default::default()
            }
        },
    );
    let unten = container(Space::new().width(Length::Fill).height(Length::Fixed(18.0))).style(
        move |_| {
            let g = Linear::new(iced::Radians(0.0))
                .add_stop(0.0, color(p.surface_container))
                .add_stop(1.0, color(mk::Rgba { a: 0.0, ..p.surface_container }));
            container::Style {
                background: Some(iced::Background::Gradient(g.into())),
                ..Default::default()
            }
        },
    );
    let gepolstert = container(inhalt).padding(iced::Padding {
        top: 18.0,
        bottom: 18.0,
        ..iced::Padding::ZERO
    });
    // Randbalken auf Null — der Geist übernimmt die Anzeige.
    let rollbereich = iced::widget::scrollable(gepolstert)
        .id(id)
        .direction(iced::widget::scrollable::Direction::Vertical(
            iced::widget::scrollable::Scrollbar::new().width(0).scroller_width(0).margin(0),
        ))
        .on_scroll(on_scroll)
        .width(Length::Fill)
        .height(Length::Fill);
    let flaeche = stack![
        rollbereich,
        container(oben)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Top),
        container(unten)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Bottom),
    ];
    // on_scroll fängt das Rad EXKLUSIV ab (capture_event) — die Physik im
    // Rahmen übernimmt und treibt die Fläche per scroll_to.
    mouse_area(flaeche)
        .on_enter(on_betreten)
        .on_exit(on_verlassen)
        .on_scroll(on_rad)
        .into()
}

/// Die schwebende Anzeige des Scrollgeists: schmale Spur rechts neben dem
/// Cursor, Daumen in Akzentfarbe (Länge = Sichtfenster-Anteil, Lage =
/// Scrollposition). Als oberste Stack-Ebene über das Fenster legen.
pub fn scrollgeist<'a, M: 'a>(g: &ScrollGeist, p: mk::Palette) -> Element<'a, M> {
    if !g.sichtbar() {
        return Space::new().into();
    }
    const HOEHE: f32 = 56.0;
    const BREITE: f32 = 6.0;
    let daumen_h = (HOEHE * g.anteil).clamp(12.0, HOEHE);
    let daumen_y = (HOEHE - daumen_h) * g.offset.clamp(0.0, 1.0);
    // Feder-Deckkraft: der Geist blendet weich ein und aus.
    let a = g.deckkraft.value.clamp(0.0, 1.0);

    let daumen = container(Space::new().width(Length::Fixed(BREITE)).height(Length::Fixed(daumen_h)))
        .style(move |_| container::Style {
            background: Some(color(mk::Rgba { a: p.primary.a * a, ..p.primary }).into()),
            border: iced::Border { radius: mk::radius::kapsel(BREITE).into(), ..Default::default() },
            ..Default::default()
        });
    let spur_farbe = p.on_surface.over(p.surface_container, 0.22);
    let spur = container(column![Space::new().height(Length::Fixed(daumen_y)), daumen].spacing(0))
        .width(Length::Fixed(BREITE))
        .height(Length::Fixed(HOEHE))
        .style(move |_| container::Style {
            background: Some(color(mk::Rgba { a: spur_farbe.a * a, ..spur_farbe }).into()),
            border: iced::Border { radius: mk::radius::kapsel(BREITE).into(), ..Default::default() },
            ..Default::default()
        });

    // Rand-Umkehr wie der Fenster-Geist: rechts-unter dem Cursor (Maus+26);
    // am rechten Rand nach links flippen, am unteren Rand hochklemmen.
    const RAND: f32 = 8.0;
    let rechts = if g.bounds.width > 0.0 { g.bounds.x + g.bounds.width } else { g.maus.x + 1e6 };
    let unten = if g.bounds.height > 0.0 { g.bounds.y + g.bounds.height } else { g.maus.y + 1e6 };
    let mut left = g.maus.x + 26.0;
    if left + BREITE > rechts - RAND {
        left = (g.maus.x - 26.0 - BREITE).max(0.0);
    }
    let mut top = g.maus.y + 26.0;
    if top + HOEHE > unten - RAND {
        top = (unten - HOEHE - RAND).max(0.0);
    }

    container(spur)
        .padding(iced::Padding {
            left: left.max(0.0),
            top: top.max(0.0),
            ..iced::Padding::ZERO
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_schloss_zustandsmaschine() {
        let mut r = RootZustand::neu();
        // Frisch: geschlossen, unsichtbar, verriegelt
        assert!(!r.offen());
        assert!(!r.sichtbar());
        assert!(!r.entsperrt);

        // Öffnen: sichtbar, aber immer noch verriegelt (Leitbild-Schloss)
        r.umschalten();
        assert!(r.offen());
        assert!(r.sichtbar());
        assert!(!r.entsperrt, "Root öffnet IMMER verriegelt");

        // Falsches Passwort: Fehlversuch, weiter verriegelt
        r.pruefung_laeuft = true;
        r.entsperr_ergebnis(false);
        assert!(!r.entsperrt);
        assert!(r.fehlversuch);
        assert!(!r.pruefung_laeuft);

        // Richtiges Passwort: entsperrt, Passwort geleert
        r.passwort = "geheim".into();
        r.entsperr_ergebnis(true);
        assert!(r.entsperrt);
        assert!(r.passwort.is_empty(), "Passwort wird nach Erfolg gelöscht");

        // Schließen verriegelt wieder
        r.umschalten();
        assert!(!r.offen());
        assert!(!r.entsperrt, "Schließen verriegelt erneut");
    }

    #[test]
    fn dialog_zustandsmaschine() {
        let mut d = DialogZustand::neu();
        assert!(!d.offen());
        assert!(!d.sichtbar());
        d.oeffnen();
        assert!(d.offen());
        assert!(d.sichtbar());
        d.schliessen();
        assert!(!d.offen());
        // Während der Ausblend-Feder noch sichtbar (Overlay muss rendern)
        assert!(d.sichtbar() || !d.animiert());
    }

    #[test]
    fn redaktion_ist_leerer_platzhalter() {
        // Kein Absturz bei 0-Größen; erzeugt ein Element.
        let p = mk::Palette::default();
        let _: Element<'_, ()> = redaktion(0.0, 0.0, p);
        let _: Element<'_, ()> = redaktion(120.0, 14.0, p);
    }
}

/// Die niri-Brücke — EINE Quelle für Compositor-Wissen der Leisten-Familie
/// (Dock, Bar): Fensterliste, Fokus, Fokussieren. Alle sprechen `niri msg`,
/// niemand parst selbst.
pub mod leinwand {
    use matrixkit_theme as mk;
    /// Ein offenes Fenster, wie der Compositor es sieht.
    #[derive(Debug, Clone)]
    pub struct OffenesFenster {
        pub id: u64,
        pub app_id: String,
        pub titel: String,
        pub fokus: bool,
        /// Lage im Workspace-View (für Andock-Kopplungen wie Matrix Web).
        pub pos: Option<(f64, f64)>,
        pub groesse: Option<(f64, f64)>,
    }

    fn json(args: &[&str]) -> Option<serde_json::Value> {
        let out = mk::leinwand::roh(args)?;
        out.status.success().then(|| ())?;
        serde_json::from_slice(&out.stdout).ok()
    }

    fn fenster_aus(w: &serde_json::Value) -> OffenesFenster {
        let paar = |v: &serde_json::Value| {
            Some((v.get(0)?.as_f64()?, v.get(1)?.as_f64()?))
        };
        OffenesFenster {
            id: w["id"].as_u64().unwrap_or(0),
            app_id: w["app_id"].as_str().unwrap_or("?").to_string(),
            titel: w["title"].as_str().unwrap_or("").to_string(),
            fokus: w["is_focused"].as_bool().unwrap_or(false),
            pos: paar(&w["layout"]["tile_pos_in_workspace_view"]),
            groesse: paar(&w["layout"]["tile_size"]),
        }
    }

    /// Alle offenen Fenster; Fehler heißt leere Liste, nie Absturz.
    pub fn fenster() -> Vec<OffenesFenster> {
        json(&["msg", "--json", "windows"])
            .and_then(|v| v.as_array().map(|a| a.iter().map(fenster_aus).collect()))
            .unwrap_or_default()
    }

    /// Das fokussierte Fenster, falls eines den Fokus hält.
    pub fn fokus() -> Option<OffenesFenster> {
        let v = json(&["msg", "--json", "focused-window"])?;
        v.is_object().then(|| fenster_aus(&v))
    }

    /// Holt ein Fenster in den Fokus (die Leinwand fährt es ganz ins Bild).
    pub fn fokussieren(id: u64) {
        mk::leinwand::fenster_fokussieren(id);
    }
}

/// Die Leisten-Familie (Bar, Dock): gemeinsame Flächen-Grammatik.
/// Leisten sind Layer-Shell-Flächen im Top-Layer (Bottom verhungert in der
/// Leinwand-Session — Befund Task #39), leben aus der Live-Palette und
/// nehmen keine Tastatur.
pub mod leiste {
    use super::*;

    /// Deckkraft der Leisten-Flächen — durchscheinend genug, dass die
    /// Leinwand atmet, dicht genug für Lesbarkeit.
    pub const DECKUNG: f32 = 0.94;

    /// Lebende Deckkraft: `transparenz = reduziert` (Bedienungshilfe,
    /// accessibilityReduceTransparency-Extrakt) macht Flächen deckend.
    pub fn deckung() -> f32 {
        if mk::transparenz_reduziert() {
            1.0
        } else {
            DECKUNG
        }
    }
    /// Hairline-Deckkraft der Kontur.
    pub const KONTUR: f32 = 0.25;

    fn flaeche(p: mk::Palette, radius: f32) -> iced::widget::container::Style {
        iced::widget::container::Style {
            background: Some(color(p.surface_container.mit_alpha(deckung())).into()),
            border: iced::Border {
                color: color(p.outline.mit_alpha(KONTUR)),
                width: 1.0,
                radius: radius.into(),
            },
            // Eigen-Schatten MIT Atemraum: die Leisten-Surfaces sind um
            // SCHATTEN_RAND größer als die Pille (Nutzer-Funde 8.7.:
            // erst geclippte Ecken, dann Compositor-Schatten um die
            // rechteckige Surface — beides falsch; so ist es richtig).
            // R55b: die ENGE Grundschicht des Dreifach-Schattens
            // (schatten_schichten stapelt zwei weitere darüber) — iceds
            // Ein-Schicht-Schatten fällt fast linear ab und wirkt abrupt
            // (Nutzer: „viel zu abrupt"), erst die Summe dreier
            // Schichten nähert den Gauss der Fenster-Schatten.
            shadow: iced::Shadow {
                color: iced::Color { a: 0.32, ..iced::Color::BLACK },
                offset: iced::Vector::new(0.0, 3.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        }
    }

    /// Das durchgehende Band (Bar): volle Kante, keine Rundung.
    pub fn band(p: mk::Palette) -> iced::widget::container::Style {
        flaeche(p, 0.0)
    }

    /// Die schwebende Pille (Dock): Radius konzentrisch zum Inhalt —
    /// `innen_radius` des größten Kindes + dessen Abstand zum Rand.
    pub fn pille(p: mk::Palette, innen_radius: f32, abstand: f32) -> iced::widget::container::Style {
        flaeche(p, innen_radius + abstand)
    }

    /// Dreifach-Schatten (R55b): zwei zusätzliche Hüllen um eine Pille —
    /// mittel und weit, mit sinkendem Alpha. Zusammen mit der engen
    /// Grundschicht der Fläche entsteht der weiche Gauss-Fade der
    /// Fenster-Schatten, den iceds Einzelschatten nicht kann.
    pub fn schatten_schichten<'a, M: 'a>(
        inhalt: iced::Element<'a, M, iced::Theme, iced::Renderer>,
        radius: f32,
    ) -> iced::Element<'a, M, iced::Theme, iced::Renderer> {
        let huelle = |el: iced::Element<'a, M, iced::Theme, iced::Renderer>,
                      blur: f32,
                      alpha: f32,
                      oy: f32| {
            iced::widget::container(el)
                .style(move |_| iced::widget::container::Style {
                    border: iced::Border { radius: radius.into(), ..Default::default() },
                    shadow: iced::Shadow {
                        color: iced::Color { a: alpha, ..iced::Color::BLACK },
                        offset: iced::Vector::new(0.0, oy),
                        blur_radius: blur,
                    },
                    ..Default::default()
                })
                .into()
        };
        huelle(huelle(inhalt, 26.0, 0.20, 6.0), 46.0, 0.10, 9.0)
    }

    /// Atemraum für den Pillen-Schatten: Surfaces sind um diesen Rand
    /// größer als der Inhalt, damit der 18-px-Blur nie geclippt wird.
    pub const SCHATTEN_RAND: f32 = 40.0;

    /// Der EINE Hover-Ton der Leisten-Familie (Anteil on_surface über
    /// surface_container) — für Knöpfe, Menü-Einträge, Auswahl-Zeilen.
    pub const HOVER: f32 = 0.10;

    /// Eine Matrix-App starten: erst der Dev-Stand in ~/.local/bin, dann
    /// das System (PATH). Das Kind wird im Hintergrund GEWARTET — sonst
    /// sammeln langlebige Leisten Zombies, deren comm später jeden
    /// leiste_toggle täuscht (Launcher-Bug vom 7.7.2026).
    pub fn app_starten(name: &str) {
        app_starten_mit(name, &[]);
    }

    /// Wie `app_starten`, mit Argumenten (R69: „matrix-aufnahme film-stopp").
    pub fn app_starten_mit(name: &str, args: &[&str]) {
        let lokal = std::env::var("HOME")
            .map(|h| format!("{h}/.local/bin/{name}"))
            .unwrap_or_default();
        let programm = if std::path::Path::new(&lokal).exists() {
            lokal
        } else {
            name.to_string()
        };
        if let Ok(mut kind) = std::process::Command::new(programm).args(args).spawn() {
            std::thread::spawn(move || {
                let _ = kind.wait();
            });
        }
    }

    /// Knopf-Stil der Leisten-Familie: ruhig-transparent, Hover-Ton,
    /// gerundet. Radius je nach Element (KLEIN für Zeilen-Knöpfe,
    /// GROSS für Dock-Kacheln).
    pub fn knopf_stil(
        p: mk::Palette,
        status: iced::widget::button::Status,
        radius: f32,
    ) -> iced::widget::button::Style {
        use iced::widget::button::{Status, Style};
        let bg = match status {
            Status::Hovered | Status::Pressed => {
                Some(color(p.on_surface.over(p.surface_container, HOVER)).into())
            }
            _ => None,
        };
        Style {
            background: bg,
            border: iced::border::rounded(radius),
            ..Default::default()
        }
    }
}

/// Panel-Toggle der Leisten-Familie: Derselbe Aufruf öffnet und schließt.
/// true = keine andere Instanz lief, wir sind das Panel (weiterlaufen);
/// false = die laufende Instanz wurde beendet — der Aufrufer endet still.
/// (Layer-Surfaces haben kein Fenster zum Fokussieren — anders als
/// mk::fenster::einzelinstanz ist Schließen hier das erwünschte Echo.)
pub fn leiste_toggle() -> bool {
    let ich = std::process::id();
    let name = std::fs::read_to_string(format!("/proc/{ich}/comm")).unwrap_or_default();
    let name = name.trim().to_string();
    if name.is_empty() {
        return true;
    }
    let mut andere = Vec::new();
    if let Ok(eintraege) = std::fs::read_dir("/proc") {
        for e in eintraege.flatten() {
            if let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() {
                if pid != ich && prozess_lebt(pid, &name) {
                    andere.push(pid);
                }
            }
        }
    }
    if andere.is_empty() {
        return true;
    }
    for pid in andere {
        let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
    }
    false
}

/// Gleicher Name UND wirklich am Leben — Zombies (`<defunct>`) behalten
/// ihr comm und haben schon einmal einen Launcher-Toggle getäuscht.
fn prozess_lebt(pid: u32, name: &str) -> bool {
    let Ok(c) = std::fs::read_to_string(format!("/proc/{pid}/comm")) else {
        return false;
    };
    if c.trim() != name {
        return false;
    }
    // /proc/PID/stat: "pid (comm) STATE …" — comm darf ')' enthalten,
    // darum hinter der LETZTEN Klammer lesen.
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| {
            s.rsplit(')')
                .next()
                .and_then(|rest| rest.split_whitespace().next())
                .map(|state| state != "Z")
        })
        .unwrap_or(false)
}

/// Schieberegler im MatrixKit-Stil: Spur und Griff aus der Palette,
/// nie Framework-Blau. Breite/on_release hängt der Aufrufer an.
pub fn regler<'a, M: Clone + 'a>(
    bereich: std::ops::RangeInclusive<f32>,
    wert: f32,
    schritt: f32,
    p: mk::Palette,
    on_change: impl Fn(f32) -> M + 'a,
) -> iced::widget::Slider<'a, f32, M> {
    iced::widget::slider(bereich, wert, on_change)
        .step(schritt)
        .style(move |_theme, status| {
            use iced::widget::slider::{Handle, HandleShape, Rail, Status, Style};
            let griff = match status {
                Status::Dragged => p.primary.over(p.on_surface, 0.15),
                Status::Hovered => p.primary.over(p.on_surface, 0.08),
                _ => p.primary,
            };
            Style {
                rail: Rail {
                    backgrounds: (
                        color(p.primary).into(),
                        color(p.on_surface.over(p.surface_container, 0.12)).into(),
                    ),
                    width: 4.0,
                    border: iced::border::rounded(mk::radius::kapsel(4.0)),
                },
                handle: Handle {
                    shape: HandleShape::Circle { radius: 8.0 },
                    background: color(griff).into(),
                    border_width: 0.0,
                    border_color: iced::Color::TRANSPARENT,
                },
            }
        })
}

/// OSD-Bausteine der Leisten-Familie (Dynamic Dock, Kap. 10/11):
/// EIN Symbol-Mapping und EIN Stufen-Balken — wer den OSD-Kanal
/// (mk::osd) anzeigt, rendert ihn hiermit.
pub mod osd_anzeige {
    use super::*;

    /// Symbol + „voll"-Flag (false = gedimmte Darstellung bei stumm).
    pub fn zeichen(stand: mk::osd::Stand) -> (char, bool) {
        match (stand.typ, stand.stumm) {
            (mk::osd::Typ::Ton, false) => (symbol::VOLUME_UP, true),
            (mk::osd::Typ::Ton, true) => (symbol::VOLUME_OFF, false),
            (mk::osd::Typ::Mikro, false) => (symbol::MIC, true),
            (mk::osd::Typ::Mikro, true) => (symbol::MIC_OFF, false),
            (mk::osd::Typ::Licht, _) => (symbol::BRIGHTNESS, true),
        }
    }

    /// Der Stufen-Balken: Regler-Farbwelt ohne Griff (Spur + Füllung),
    /// Kapsel-Rundung. `voll = false` dimmt die Füllung (stumm).
    pub fn stufen_balken<'a, M: 'a>(
        p: mk::Palette,
        anteil: f32,
        voll: bool,
        breite: f32,
    ) -> Element<'a, M> {
        let anteil = anteil.clamp(0.0, 1.0);
        let fuellung = if voll {
            p.primary
        } else {
            p.on_surface_variant.mit_alpha(0.6)
        };
        container(
            container(Space::new())
                .width(Length::Fixed(breite * anteil))
                .height(Length::Fixed(8.0))
                .style(move |_| container::Style {
                    background: Some(color(fuellung).into()),
                    border: iced::border::rounded(mk::radius::kapsel(8.0)),
                    ..Default::default()
                }),
        )
        .width(Length::Fixed(breite))
        .style(move |_| container::Style {
            background: Some(color(p.on_surface.over(p.surface_container, 0.12)).into()),
            border: iced::border::rounded(mk::radius::kapsel(8.0)),
            ..Default::default()
        })
        .into()
    }
}

/// Der niri-Ereignis-Strom: EIN `niri msg event-stream`-Prozess je App,
/// jeder Ereignis-Schwall wird zu genau EINEM Ping (100-ms-Drossel).
/// Leisten reagieren damit SOFORT auf Fenster-/Fokus-Wechsel statt zu
/// pollen; ein seltener Poll bleibt als Fallback guter Ton. Reißt der
/// Strom ab (Compositor-Neustart), verbindet er sich selbst neu.
pub fn leinwand_strom() -> iced::Subscription<()> {
    use iced::futures::{SinkExt, StreamExt};
    iced::Subscription::run(|| {
        iced::stream::channel(64, |mut out: iced::futures::channel::mpsc::Sender<()>| async move {
            let (tx, mut rx) = iced::futures::channel::mpsc::unbounded::<()>();
            std::thread::spawn(move || loop {
                if let Ok(mut kind) = mk::leinwand::ereignis_strom(false) {
                    if let Some(stdout) = kind.stdout.take() {
                        use std::io::BufRead;
                        let mut zeilen = std::io::BufReader::new(stdout).lines();
                        while let Some(Ok(_)) = zeilen.next() {
                            if tx.unbounded_send(()).is_err() {
                                let _ = kind.kill();
                                return; // App ist weg — Thread geht mit
                            }
                            // Schwall-Drossel: Folge-Ereignisse der
                            // nächsten 100 ms bündeln sich im Puffer.
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                    let _ = kind.wait(); // kein Zombie
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            });
            while let Some(()) = rx.next().await {
                // Puffer leeren: ein Schwall = ein Ping.
                #[allow(deprecated)]
                while rx.try_next().ok().flatten().is_some() {}
                let _ = out.send(()).await;
            }
        })
    })
}

/// Das Leitbild-UI-`Label`-Extrakt (Leitbild-Runde 17): Symbol + Text sind EIN
/// benanntes Paar mit fester Grammatik (Abstand, Grundlinie) — nie mehr
/// ad-hoc zusammengesetzte rows in Leisten, Menüs und Zeilen.
pub fn etikett<'a, M: 'a>(
    zeichen: char,
    inhalt: impl iced::widget::text::IntoFragment<'a>,
    stil: mk::typo::Stil,
    farbe: mk::Rgba,
) -> Element<'a, M> {
    row![
        // R65b: das Symbol trägt das GEWICHT seines Textes (SF-Symbols).
        symbol_gewicht(zeichen, stil.groesse + 3.0, farbe, stil.gewicht),
        txt(inhalt, stil, farbe),
    ]
    .spacing(mk::spacing::XS)
    .align_y(iced::Alignment::Center)
    .into()
}

/// Das Leitbild-UI-`Divider`-Extrakt: der EINE stille Trennstrich.
pub fn trenner<'a, M: 'a>(p: mk::Palette) -> Element<'a, M> {
    container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
        .padding(iced::Padding {
            left: mk::spacing::S,
            right: mk::spacing::S,
            top: 2.0,
            bottom: 2.0,
        })
        .style(move |_| container::Style {
            background: Some(color(p.outline.mit_alpha(0.25)).into()),
            ..Default::default()
        })
        .into()
}

/// Das `TimelineView(.everyMinute)`-Extrakt: ein Tick EXAKT zur vollen
/// Minute, dann minütlich — Uhren zeigen nie eine abgelaufene Minute
/// und niemand pollt im Sekundentakt.
pub fn tick_zur_minute() -> iced::Subscription<()> {
    use iced::futures::{SinkExt, StreamExt};
    iced::Subscription::run(|| {
        iced::stream::channel(4, |mut out: iced::futures::channel::mpsc::Sender<()>| async move {
            let (tx, mut rx) = iced::futures::channel::mpsc::unbounded::<()>();
            std::thread::spawn(move || loop {
                let jetzt = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let bis_zur_minute = 60 - (jetzt % 60);
                std::thread::sleep(std::time::Duration::from_secs(bis_zur_minute.max(1)));
                if tx.unbounded_send(()).is_err() {
                    return;
                }
            });
            while let Some(()) = rx.next().await {
                let _ = out.send(()).await;
            }
        })
    })
}

// ===========================================================================
// Die Lupe — Kit-weite Hover-Magnification (Nutzer, 8.7.2026).
// Ein Wrapper-Widget: sein Inhalt SKALIERT beim Überfahren federnd auf
// (Renderer::with_transformation, um die Mitte), OHNE das Layout zu
// verschieben — die Klickfläche bleibt exakt, nur die Optik „poppt".
// bewegung=reduziert schaltet es hart aus. Alles Anklickbare kann sich
// damit umgeben: mkw::lupe(button(...)).
// ===========================================================================

/// Standard-Vergrößerung beim Hover (subtil, wie das Touch-Leitbild/Leitbild-Druckknöpfe).
pub const LUPE_MAX: f32 = 1.07;

pub fn lupe<'a, M: 'a>(inhalt: impl Into<Element<'a, M>>) -> Element<'a, M> {
    Lupe { inhalt: inhalt.into(), max: LUPE_MAX }.into()
}

/// Wie `lupe`, aber mit eigener Maximal-Skalierung (Dock-Kacheln mögen mehr).
pub fn lupe_stark<'a, M: 'a>(inhalt: impl Into<Element<'a, M>>, max: f32) -> Element<'a, M> {
    Lupe { inhalt: inhalt.into(), max }.into()
}

struct Lupe<'a, M> {
    inhalt: Element<'a, M>,
    max: f32,
}

#[derive(Default)]
struct LupeZustand {
    hover: bool,
    skala: f32,
    letzte: Option<std::time::Instant>,
}

impl<'a, M> iced::advanced::Widget<M, iced::Theme, iced::Renderer> for Lupe<'a, M> {
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<LupeZustand>()
    }
    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(LupeZustand { skala: 1.0, ..Default::default() })
    }
    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.inhalt)]
    }
    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.inhalt));
    }
    fn size(&self) -> iced::Size<iced::Length> {
        self.inhalt.as_widget().size()
    }
    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.inhalt
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, M>,
        viewport: &iced::Rectangle,
    ) {
        let hart = mk::bewegung_reduziert();
        let z = tree.state.downcast_mut::<LupeZustand>();
        let drueber = cursor.is_over(layout.bounds());
        if drueber != z.hover {
            z.hover = drueber;
            if hart {
                z.skala = if drueber { self.max } else { 1.0 };
            } else {
                shell.request_redraw();
            }
        }
        // Feder auf jedem Frame (RedrawRequested) nachziehen.
        if let iced::Event::Window(iced::window::Event::RedrawRequested(jetzt)) = event {
            if !hart {
                let dt = z
                    .letzte
                    .map(|l| jetzt.duration_since(l).as_secs_f32())
                    .unwrap_or(0.0)
                    .clamp(0.0, 0.05);
                z.letzte = Some(*jetzt);
                let ziel = if z.hover { self.max } else { 1.0 };
                let k = 1.0 - (-dt / 0.06f32).exp();
                z.skala += (ziel - z.skala) * k;
                if (ziel - z.skala).abs() < 0.001 {
                    z.skala = ziel;
                } else {
                    shell.request_redraw();
                }
            }
        }
        self.inhalt.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }
    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        use iced::advanced::Renderer as _;
        let z = tree.state.downcast_ref::<LupeZustand>();
        let s = z.skala;
        let zeichne = |r: &mut iced::Renderer| {
            self.inhalt
                .as_widget()
                .draw(&tree.children[0], r, theme, style, layout, cursor, viewport);
        };
        if s <= 1.001 {
            zeichne(renderer);
        } else {
            let b = layout.bounds();
            let (cx, cy) = (b.center_x(), b.center_y());
            let t = iced::Transformation::translate((1.0 - s) * cx, (1.0 - s) * cy)
                * iced::Transformation::scale(s);
            renderer.with_transformation(t, zeichne);
        }
    }
    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        self.inhalt
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }
    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.inhalt
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.inhalt.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, M: 'a> From<Lupe<'a, M>> for Element<'a, M> {
    fn from(l: Lupe<'a, M>) -> Self {
        Element::new(l)
    }
}

// ===========================================================================
// Die Touch-Runde (R59) — UIScrollView-/UIKeyboard-Extrakte aus dem
// Touch-Referenz-SDK 27.0 (UIScrollView.h auf Nutzer-Referenzsystem, 16.7.2026):
// * „if the user then drags far enough, we switch back to dragging and
//   cancel any tracking in the subview" — ein Drag beginnt nach einem
//   Slop und BRICHT dann das Tracking der Unteransicht AB. Deshalb
//   klickt ein Flick auf dem Tablet-Leitbild nie versehentlich einen Schalter.
// * Nach dem Loslassen rollt der Inhalt mit decelerationRate aus
//   (UIScrollViewDecelerationRateNormal — 0,998 je Millisekunde).
// * Die Leitbild-Tastatur tippt beim BERÜHREN, nicht beim Loslassen —
//   die halbe gefühlte Latenz („this has no effect on presses").
// iced kennt kein Touch-Cancel-Event; das Abbrechen bildet die
// Wischfläche mit einem synthetischen FingerLost nach — dieselbe
// Wirkung: das Kind vergisst den Finger, ohne zu feuern.
// ===========================================================================

/// Finger-Slop: so weit darf ein Tap wandern, bevor er zum Drag wird.
const WISCH_SLOP: f32 = 10.0;

/// Wischfläche: macht aus jedem Inhalt direkte Manipulation. `on_zug`
/// bekommt die Fingerbewegung (dy je Move, nach unten positiv),
/// `on_ende` die Loslass-Geschwindigkeit in px/s fürs Ausrollen.
/// Mäuse und Trackpads laufen unverändert am Widget vorbei.
pub fn wischflaeche<'a, M: Clone + 'a>(
    inhalt: impl Into<Element<'a, M>>,
    on_zug: impl Fn(f32) -> M + 'a,
    on_ende: impl Fn(f32) -> M + 'a,
) -> Element<'a, M> {
    Element::new(Wischflaeche {
        inhalt: inhalt.into(),
        on_zug: Box::new(on_zug),
        on_ende: Box::new(on_ende),
    })
}

struct Wischflaeche<'a, M> {
    inhalt: Element<'a, M>,
    on_zug: Box<dyn Fn(f32) -> M + 'a>,
    on_ende: Box<dyn Fn(f32) -> M + 'a>,
}

#[derive(Default)]
struct WischZustand {
    finger: Option<iced::touch::Finger>,
    start_y: f32,
    letzte_y: f32,
    letzte_zeit: Option<std::time::Instant>,
    zieht: bool,
    /// Geglättete Fingergeschwindigkeit (px/s, nach unten positiv).
    v: f32,
}

impl<'a, M: Clone> iced::advanced::Widget<M, iced::Theme, iced::Renderer> for Wischflaeche<'a, M> {
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<WischZustand>()
    }
    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(WischZustand::default())
    }
    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.inhalt)]
    }
    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.inhalt));
    }
    fn size(&self) -> iced::Size<iced::Length> {
        self.inhalt.as_widget().size()
    }
    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.inhalt
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, M>,
        viewport: &iced::Rectangle,
    ) {
        use iced::touch::Event as T;
        if let iced::Event::Touch(t) = event {
            let z = tree.state.downcast_mut::<WischZustand>();
            match t {
                T::FingerPressed { id, position } => {
                    if z.finger.is_none() && layout.bounds().contains(*position) {
                        z.finger = Some(*id);
                        z.start_y = position.y;
                        z.letzte_y = position.y;
                        z.letzte_zeit = Some(std::time::Instant::now());
                        z.zieht = false;
                        z.v = 0.0;
                    }
                    // weiterreichen — ein Tap soll normal wirken
                }
                T::FingerMoved { id, position } if z.finger == Some(*id) => {
                    if !z.zieht && (position.y - z.start_y).abs() > WISCH_SLOP {
                        z.zieht = true;
                        // UIScrollView-Cancel: das Kind verliert den Finger,
                        // bevor der Drag es versehentlich feuern lässt.
                        let verloren =
                            iced::Event::Touch(T::FingerLost { id: *id, position: *position });
                        self.inhalt.as_widget_mut().update(
                            &mut tree.children[0],
                            &verloren,
                            layout,
                            cursor,
                            renderer,
                            clipboard,
                            shell,
                            viewport,
                        );
                    }
                    if z.zieht {
                        let dy = position.y - z.letzte_y;
                        let jetzt = std::time::Instant::now();
                        if let Some(vor) = z.letzte_zeit {
                            let dt = jetzt.duration_since(vor).as_secs_f32();
                            if dt > 0.0005 {
                                z.v = 0.7 * z.v + 0.3 * (dy / dt);
                            }
                        }
                        z.letzte_y = position.y;
                        z.letzte_zeit = Some(jetzt);
                        if dy != 0.0 {
                            shell.publish((self.on_zug)(dy));
                        }
                        shell.capture_event();
                        return; // das Kind sieht den Drag nicht
                    }
                }
                T::FingerLifted { id, .. } | T::FingerLost { id, .. }
                    if z.finger == Some(*id) =>
                {
                    let zog = z.zieht;
                    let v = z.v;
                    z.finger = None;
                    z.zieht = false;
                    z.v = 0.0;
                    if zog {
                        // Kind wurde beim Drag-Beginn schon gecancelt.
                        shell.publish((self.on_ende)(v));
                        shell.capture_event();
                        return;
                    }
                }
                _ => {}
            }
        }
        self.inhalt.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }
    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.inhalt
            .as_widget()
            .draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }
    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        self.inhalt
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }
    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.inhalt
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.inhalt.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, M: Clone + 'a> From<Wischflaeche<'a, M>> for Element<'a, M> {
    fn from(w: Wischflaeche<'a, M>) -> Self {
        Element::new(w)
    }
}

/// Sofort-Taste (UIKeyboard-Extrakt): feuert beim BERÜHREN statt beim
/// Loslassen — für Tasten, deren Tempo zählt (Bildschirmtastatur).
/// Das Kind (der Knopf) zeigt weiter seinen Gedrückt-Zustand, feuert
/// aber für Touch nicht selbst (FingerLost statt FingerLifted); Maus
/// und Trackpad laufen unverändert durch das Kind.
pub fn sofort_taste<'a, M: Clone + 'a>(
    inhalt: impl Into<Element<'a, M>>,
    msg: M,
) -> Element<'a, M> {
    Element::new(SofortTaste { inhalt: inhalt.into(), msg })
}

struct SofortTaste<'a, M> {
    inhalt: Element<'a, M>,
    msg: M,
}

#[derive(Default)]
struct SofortZustand {
    finger: Option<iced::touch::Finger>,
}

impl<'a, M: Clone> iced::advanced::Widget<M, iced::Theme, iced::Renderer> for SofortTaste<'a, M> {
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<SofortZustand>()
    }
    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(SofortZustand::default())
    }
    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.inhalt)]
    }
    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.inhalt));
    }
    fn size(&self) -> iced::Size<iced::Length> {
        self.inhalt.as_widget().size()
    }
    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.inhalt
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, M>,
        viewport: &iced::Rectangle,
    ) {
        use iced::touch::Event as T;
        if let iced::Event::Touch(t) = event {
            let z = tree.state.downcast_mut::<SofortZustand>();
            match t {
                T::FingerPressed { id, position } => {
                    if z.finger.is_none() && layout.bounds().contains(*position) {
                        z.finger = Some(*id);
                        // JETZT feuern — nicht erst beim Loslassen.
                        shell.publish(self.msg.clone());
                        // Der Knopf darunter darf „gedrückt" zeigen.
                    }
                }
                T::FingerLifted { id, position } | T::FingerLost { id, position }
                    if z.finger == Some(*id) =>
                {
                    z.finger = None;
                    // Dem Kind ein FingerLost statt des Lifted geben —
                    // Optik zurücksetzen, ohne dass es selbst feuert.
                    let verloren =
                        iced::Event::Touch(T::FingerLost { id: *id, position: *position });
                    self.inhalt.as_widget_mut().update(
                        &mut tree.children[0],
                        &verloren,
                        layout,
                        cursor,
                        renderer,
                        clipboard,
                        shell,
                        viewport,
                    );
                    shell.capture_event();
                    return;
                }
                _ => {}
            }
        }
        self.inhalt.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }
    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
    ) {
        self.inhalt
            .as_widget()
            .draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);
    }
    fn mouse_interaction(
        &self,
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &iced::Renderer,
    ) -> iced::advanced::mouse::Interaction {
        self.inhalt
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }
    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.inhalt
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.inhalt.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, M: Clone + 'a> From<SofortTaste<'a, M>> for Element<'a, M> {
    fn from(s: SofortTaste<'a, M>) -> Self {
        Element::new(s)
    }
}

// ===========================================================================
// Die Kit-Sidebar (Nutzer-Vereinheitlichung, 8.7.2026): EIN Baustein für
// alle Seitenleisten — dunkler Grund (die Hilfe-Optik), Lupe auf jedem
// Eintrag (die Leinwand-Haptik), Aktiv-Zustand in Primary-Container,
// optionales Zahlen-Abzeichen (Leitbild badge) und Fokusring fürs
// Tastaturmodell. Apps beschreiben nur noch ihre Einträge.
// ===========================================================================

pub struct SidebarPunkt<'a> {
    pub zeichen: char,
    pub titel: &'a str,
    /// Zahl rechts (z. B. Artikel je Kategorie) — None = kein Abzeichen.
    pub anzahl: Option<usize>,
}

/// EIN Sidebar-Eintrag — der Familien-Baustein, den `sidebar()` UND jede
/// App mit eigener Leisten-Anatomie (Dateien) nutzen: gleiche Höhe,
/// gleiche Paletten-Pille (primary_container) fürs Aktive, gleicher
/// Hover, gleiche Lupe. EIN Code-Pfad = garantiert identisch.
pub fn sidebar_eintrag<'a, M: Clone + 'a>(
    punkt: SidebarPunkt<'a>,
    ist_aktiv: bool,
    im_fokus: bool,
    msg: M,
    p: mk::Palette,
) -> Element<'a, M> {
    let textfarbe = if ist_aktiv { p.on_primary_container } else { p.on_surface };
    let mut zeile = row![
        symbol::<M>(punkt.zeichen, mk::font_size::LARGE, textfarbe),
        Space::new().width(mk::spacing::S),
        txt(punkt.titel, mk::typo::FLIESS, textfarbe),
        Space::new().width(Length::Fill),
    ]
    .align_y(iced::Alignment::Center);
    if let Some(n) = punkt.anzahl {
        zeile = zeile.push(abzeichen::<M>(n, p));
    }
    lupe(
        iced::widget::button(zeile)
            .width(Length::Fill)
            .padding([mk::spacing::S as u16, mk::spacing::M as u16])
            .on_press(msg)
            .style(move |_, status| {
                let base = if ist_aktiv { p.primary_container } else { p.surface };
                let bg = match status {
                    iced::widget::button::Status::Hovered if !ist_aktiv => {
                        p.on_surface.over(base, mk::state_layer::HOVER)
                    }
                    iced::widget::button::Status::Pressed if !ist_aktiv => {
                        p.on_surface.over(base, mk::state_layer::PRESSED)
                    }
                    _ => base,
                };
                iced::widget::button::Style {
                    background: (ist_aktiv
                        || !matches!(status, iced::widget::button::Status::Active))
                    .then(|| color(bg).into()),
                    border: if im_fokus {
                        fokus_ring(true, mk::CORNER_RADIUS, p)
                    } else {
                        iced::Border {
                            radius: mk::CORNER_RADIUS.into(),
                            ..Default::default()
                        }
                    },
                    ..Default::default()
                }
            }),
    )
}

pub fn sidebar<'a, M: Clone + 'a>(
    punkte: Vec<SidebarPunkt<'a>>,
    aktiv: usize,
    fokus: Option<usize>,
    breite: f32,
    on_wahl: impl Fn(usize) -> M + 'a,
    p: mk::Palette,
) -> Element<'a, M> {
    let mut seite = column![].spacing(mk::spacing::XS);
    for (i, punkt) in punkte.into_iter().enumerate() {
        seite = seite.push(sidebar_eintrag(punkt, i == aktiv, fokus == Some(i), on_wahl(i), p));
    }
    sidebar_flaeche(seite.into(), breite, p)
}

/// Der dunkle Sidebar-Grund der Hilfe-Optik als eigener Familien-Baustein —
/// für `sidebar()` UND Apps mit eigener Leisten-Struktur (Dateien:
/// Sektionen). Gleiche Fläche, gleiche linke Rundung, EIN Code-Pfad.
pub fn sidebar_flaeche<'a, M: 'a>(
    inhalt: Element<'a, M>,
    breite: f32,
    p: mk::Palette,
) -> Element<'a, M> {
    container(inhalt)
        .padding(mk::spacing::S)
        .width(Length::Fixed(breite))
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(color(p.surface).into()),
            border: iced::Border {
                radius: iced::border::Radius {
                    top_left: mk::CORNER_RADIUS,
                    bottom_left: mk::CORNER_RADIUS,
                    top_right: 0.0,
                    bottom_right: 0.0,
                },
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

// ===========================================================================
// MatrixUI — die Muster-Schicht über den MatrixKit-Bausteinen
// (Nutzer-Direktive, 8.7.2026): Während mkw die BAUSTEINE liefert,
// liefert MatrixUI die verbindlichen FAMILIEN — Baustein + Anatomie in
// einem Griff, damit Vereinheitlichung nicht von Disziplin abhängt.
// Erstes Mitglied: die SidebarFamily.
// ===========================================================================

pub mod ui {
    use super::*;

    /// Die SidebarFamily: dunkle Kit-Sidebar nackt am Fensterrand,
    /// rechts die helle Detail-Karte (padding L, Radius) — die
    /// verbindliche Anatomie aus Hilfe/Leinwand. Apps liefern Einträge
    /// und Detail-Inhalt; Breite und Einbettung gehören der Familie.
    pub const SIDEBAR_BREITE: f32 = 190.0;

    pub fn sidebar_family<'a, M: Clone + 'a>(
        punkte: Vec<SidebarPunkt<'a>>,
        aktiv: usize,
        fokus: Option<usize>,
        on_wahl: impl Fn(usize) -> M + 'a,
        detail: Element<'a, M>,
        p: mk::Palette,
    ) -> Element<'a, M> {
        let leiste = sidebar(punkte, aktiv, fokus, SIDEBAR_BREITE, on_wahl, p);
        row![leiste, detail_karte(detail, p)].into()
    }

    /// Die helle Detail-Karte der SidebarFamily als eigener Baustein —
    /// derselbe View-Grund für Hilfe UND Apps mit eigener Leisten-Struktur.
    pub fn detail_karte<'a, M: 'a>(inhalt: Element<'a, M>, p: mk::Palette) -> Element<'a, M> {
        container(inhalt)
            .padding(mk::spacing::L)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(color(p.surface_container).into()),
                border: iced::Border {
                    radius: mk::CORNER_RADIUS.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }

    /// BildKachel-Familie, Widget-Seite: 112x63-Thumb (16:9, Ecken im
    /// Bild gebacken — Rezept in mkw::bild), Name darunter (max. 16
    /// Zeichen), Auswahl = 2px-primary-Rand, optionales Badge oben
    /// rechts. Erstgeboren im Hintergrund-Bereich; die Galerie in
    /// Matrix Dateien trägt dieselbe Anatomie.
    pub const BILD_KACHEL_B: f32 = 112.0;
    pub const BILD_KACHEL_H: f32 = 63.0;

    pub fn bild_kachel<'a, M: 'a>(
        handle: Option<iced::widget::image::Handle>,
        platzhalter: char,
        name: String,
        gewaehlt: bool,
        badge: Option<String>,
        p: mk::Palette,
    ) -> Element<'a, M> {
        use iced::widget::stack;
        let flaeche: Element<'a, M> = match handle {
            Some(h) => iced::widget::image(h)
                .width(Length::Fixed(BILD_KACHEL_B))
                .height(Length::Fixed(BILD_KACHEL_H))
                .content_fit(iced::ContentFit::Fill)
                .into(),
            None => container(symbol::<M>(platzhalter, 28.0, p.on_surface_variant))
                .center_x(Length::Fixed(BILD_KACHEL_B))
                .center_y(Length::Fixed(BILD_KACHEL_H))
                .style(move |_| container::Style {
                    background: Some(color(p.surface_container_high).into()),
                    border: iced::Border { radius: (mk::radius::KLEIN - 2.0).into(), ..Default::default() },
                    ..Default::default()
                })
                .into(),
        };
        let mut lagen = stack![flaeche];
        if let Some(b) = badge {
            lagen = lagen.push(
                container(
                    container(txt(b, mk::typo::ETIKETT, p.on_primary))
                        .padding(iced::Padding { top: 1.0, right: 6.0, bottom: 1.0, left: 6.0 })
                        .style(move |_| container::Style {
                            background: Some(color(p.primary).into()),
                            border: iced::Border { radius: mk::radius::kapsel(18.0).into(), ..Default::default() },
                            ..Default::default()
                        }),
                )
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Right)
                .padding(3),
            );
        }
        let mut kurz: String = name.chars().take(16).collect();
        if name.chars().count() > 16 {
            kurz.push('\u{2026}');
        }
        let namensfarbe = if gewaehlt { p.primary } else { p.on_surface_variant };
        iced::widget::column![
            container(lagen).padding(2).style(move |_| container::Style {
                border: iced::Border {
                    color: color(p.primary),
                    width: if gewaehlt { 2.0 } else { 0.0 },
                    radius: mk::radius::KLEIN.into(),
                },
                ..Default::default()
            }),
            txt(kurz, mk::typo::ETIKETT, namensfarbe),
        ]
        .spacing(2)
        .align_x(iced::Alignment::Center)
        .into()
    }

    /// Werkzeug-Knopf der Toolbar-Familie: Symbol-Knopf, gedimmt und
    /// tot wenn `msg` fehlt (Leitbild-Grammatik: inaktive Chevrons bleiben
    /// sichtbar). Erstgeboren in Matrix Dateien, jetzt Familien-Baustein.
    pub fn werkzeug_knopf<'a, M: Clone + 'a>(
        zeichen: char,
        msg: Option<M>,
        p: mk::Palette,
    ) -> Element<'a, M> {
        let an = msg.is_some();
        let mut b = iced::widget::button(symbol::<M>(
            zeichen,
            18.0,
            if an { p.on_surface } else { p.outline },
        ))
        .padding(iced::Padding { left: 6.0, right: 6.0, top: 4.0, bottom: 4.0 })
        .style(move |_, status| leiste::knopf_stil(p, status, mk::radius::KLEIN));
        if let Some(m) = msg {
            b = b.on_press(m);
        }
        lupe(b)
    }

    /// Werkzeugleisten-Familie (Leitbild-Toolbar-Extrakt, erstgeboren in
    /// Matrix Dateien): ◀ ▶ [eigene Navigation] Titel … [rechte Knöpfe]
    /// [Suchfeld 200 px]. Gehört an den KOPF der Detail-Karte.
    #[allow(clippy::too_many_arguments)]
    pub fn werkzeugleiste<'a, M: Clone + 'a>(
        titel: String,
        zurueck: Option<M>,
        vor: Option<M>,
        navigation_extra: Vec<Element<'a, M>>,
        rechts: Vec<Element<'a, M>>,
        suche: &str,
        on_suche: impl Fn(String) -> M + 'a,
        such_leeren: M,
        p: mk::Palette,
    ) -> Element<'a, M> {
        let mut zeile = row![
            werkzeug_knopf(symbol::ARROW_BACK, zurueck, p),
            werkzeug_knopf(symbol::ARROW_FORWARD, vor, p),
        ]
        .spacing(mk::spacing::XS)
        .align_y(iced::Alignment::Center);
        for e in navigation_extra {
            zeile = zeile.push(e);
        }
        zeile = zeile
            .push(Space::new().width(mk::spacing::S))
            .push(txt(titel, mk::typo::KOPF, p.on_surface))
            .push(Space::new().width(Length::Fill));
        for e in rechts {
            zeile = zeile.push(e);
        }
        zeile
            .push(
                container(suchfeld(suche, "Suchen", on_suche, such_leeren, p))
                    .width(Length::Fixed(200.0)),
            )
            .into()
    }

    /// Die SidebarFamily-Eintrags-Optik als freier Baustein (Nutzer,
    /// 8.7.: „Top-Bar-Widgets gleich wie die Sidebar-Elemente"):
    /// Symbol+Text-Grammatik, State-Layer-Hover, Aktiv-Pille in
    /// Primary-Container, CORNER_RADIUS, Lupe. Für Leisten-Widgets und
    /// überall, wo Einträge wie Sidebar-Einträge aussehen sollen.
    pub fn familien_knopf<'a, M: Clone + 'a>(
        zeichen: Option<char>,
        titel: Option<&'a str>,
        aktiv: bool,
        msg: M,
        p: mk::Palette,
        stil: mk::typo::Stil,
    ) -> Element<'a, M> {
        let textfarbe = if aktiv { p.on_primary_container } else { p.on_surface };
        let mut zeile = row![].spacing(mk::spacing::S).align_y(iced::Alignment::Center);
        if let Some(z) = zeichen {
            zeile = zeile.push(symbol::<M>(z, mk::font_size::LARGE, textfarbe));
        }
        if let Some(t) = titel {
            // R60: enge Zeilenbox — in gedeckelten Leisten (Bar: 32 px)
            // darf der Knopf nie höher wollen, als er darf; sonst
            // kollabiert iced still das untere Padding (Schiefstand).
            zeile = zeile.push(
                txt(t, stil, textfarbe)
                    .line_height(iced::widget::text::LineHeight::Relative(1.0)),
            );
        }
        lupe(
            iced::widget::button(zeile)
                // Exakt die Sidebar-Eintragshöhe (Nutzer, 8.7.):
                // gleiches Padding wie SidebarPunkt-Knöpfe.
                .padding([mk::spacing::S as u16, mk::spacing::M as u16])
                .on_press(msg)
                .style(move |_, status| {
                    let base = if aktiv { p.primary_container } else { p.surface };
                    let bg = match status {
                        iced::widget::button::Status::Hovered if !aktiv => {
                            p.on_surface.over(base, mk::state_layer::HOVER)
                        }
                        iced::widget::button::Status::Pressed if !aktiv => {
                            p.on_surface.over(base, mk::state_layer::PRESSED)
                        }
                        _ => base,
                    };
                    iced::widget::button::Style {
                        background: (aktiv
                            || !matches!(status, iced::widget::button::Status::Active))
                        .then(|| color(bg).into()),
                        border: iced::Border {
                            radius: mk::CORNER_RADIUS.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
        )
    }

    /// Die MenuFamily (Nutzer, 8.7.): EIN Menü-Look für ALLE Menüs —
    /// Sitzungsmenü, Dock-Kontext, Leinwand-Kontext, In-App-Kontextmenüs.
    /// Einträge tragen die MatrixUI-Grammatik (Symbol+Text, State-Layer,
    /// CORNER_RADIUS, Lupe), Gefahr-Einträge kommen in Error-Farbe, die
    /// Hülle ist die Kit-Pille mit Schatten. Breite ist Familien-Norm.
    pub const MENU_BREITE: f32 = 230.0;

    #[derive(Clone)]
    pub enum MenuEintrag<M> {
        Punkt {
            zeichen: Option<char>,
            titel: String,
            /// None = on_surface; Some(error) für Gefahr/Bestätigung.
            farbe: Option<mk::Rgba>,
            msg: M,
        },
        /// Punkt mit Tastenkürzel rechts (Leitbild-MenüItem.keyEquivalent,
        /// Runde 23) — nur für ECHTE Binds verwenden.
        PunktMitKuerzel {
            zeichen: Option<char>,
            titel: String,
            kuerzel: String,
            farbe: Option<mk::Rgba>,
            msg: M,
        },
        /// Ausgegrauter Punkt (Vermessungs-Runde 36): das Referenzsystem VERSTECKT
        /// deaktivierte Einträge nie — sie bleiben sichtbar-grau stehen,
        /// damit das Menü seine Form behält und man lernt, was es gäbe.
        Inaktiv {
            zeichen: Option<char>,
            titel: String,
        },
        /// Punkt mit Live-Badge rechts (Apfel-Menü-Grammatik, Runde 28:
        /// „Systemeinstellungen …  [1 Update]") — das Menü trägt Status.
        PunktMitBadge {
            zeichen: Option<char>,
            titel: String,
            badge: String,
            farbe: Option<mk::Rgba>,
            msg: M,
        },
        Trenner,
    }

    /// Die EINE schwebende Hülle der MenuFamily — für Menüs UND Panels
    /// (Zentrale, Mitteilungs-Verlauf): gleicher Radius, gleiches
    /// Padding, gleicher Schatten. Nie wieder drei Pillen-Dialekte.
    pub fn panel_huelle<'a, M: 'a>(
        inhalt: Element<'a, M>,
        breite: f32,
        p: mk::Palette,
    ) -> Element<'a, M> {
        // R55c (Nutzer): auch Menüs tragen den Dreifach-Schatten —
        // die Familie erbt den Gauss-Fade an der Wurzel, nicht jede
        // Hülle einzeln.
        leiste::schatten_schichten(
            container(container(inhalt).padding(mk::spacing::S))
                .width(Length::Fixed(breite))
                .style(move |_| leiste::pille(p, mk::radius::KLEIN, mk::spacing::S))
                .into(),
            mk::radius::KLEIN + mk::spacing::S,
        )
    }

    /// Der Panel-Kopf der Familie: Titel links, Aktions-Knöpfe rechts
    /// (Apps bauen sie mit ui::nav_knopf bzw. ui::kopf_text_knopf),
    /// darunter der Trenner.
    pub fn panel_kopf<'a, M: Clone + 'a>(
        titel: &'a str,
        aktionen: Vec<Element<'a, M>>,
        p: mk::Palette,
    ) -> Element<'a, M> {
        let mut zeile = row![
            txt(titel, mk::typo::KOPF, p.on_surface),
            Space::new().width(Length::Fill),
        ]
        .spacing(mk::spacing::XS)
        .align_y(iced::Alignment::Center);
        for a in aktionen {
            zeile = zeile.push(a);
        }
        iced::widget::column![zeile, trenner(p)]
            .spacing(mk::spacing::XS)
            .into()
    }

    /// Text-Aktion für Panel-Köpfe („Leeren") — Familien-Grammatik + Lupe.
    pub fn kopf_text_knopf<'a, M: Clone + 'a>(
        label: &'a str,
        msg: M,
        p: mk::Palette,
    ) -> Element<'a, M> {
        lupe(
            iced::widget::button(txt(label, mk::typo::KLEIN, p.on_surface_variant))
                .padding([2, mk::spacing::S as u16])
                .on_press(msg)
                .style(move |_, status| leiste::knopf_stil(p, status, mk::radius::KLEIN)),
        )
    }

    /// Der Menü-Punkt-Knopf (R66, Leitbild-Menü-Extrakt — empirisch am Referenzsystem):
    /// Ein gewählter Eintrag BLINKT zweimal (~0,2 s), bevor seine
    /// Nachricht feuert — Leitbild- Klick-Quittung seit 1984, in jedem
    /// Menü. iced-Buttons können den Klick nicht verzögern, ohne den
    /// Hover zu verlieren; darum ist dies ein eigenes Widget: es malt
    /// Hover-/Blitz-Grund selbst, fängt den Klick, spielt den Blitz
    /// (aus — an — aus) und publiziert DANN. `bewegung reduziert`
    /// feuert sofort.
    pub fn menu_punkt_knopf<'a, M: Clone + 'a>(
        inhalt: Element<'a, M>,
        msg: M,
        p: mk::Palette,
    ) -> Element<'a, M> {
        Element::new(MenuPunktKnopf { inhalt, msg, p })
    }

    struct MenuPunktKnopf<'a, M> {
        inhalt: Element<'a, M>,
        msg: M,
        p: mk::Palette,
    }

    #[derive(Default)]
    struct BlitzZustand {
        hover: bool,
        /// Blitz-Beginn; None = ruhend.
        seit: Option<std::time::Instant>,
    }

    impl<'a, M: Clone> iced::advanced::Widget<M, iced::Theme, iced::Renderer>
        for MenuPunktKnopf<'a, M>
    {
        fn tag(&self) -> iced::advanced::widget::tree::Tag {
            iced::advanced::widget::tree::Tag::of::<BlitzZustand>()
        }
        fn state(&self) -> iced::advanced::widget::tree::State {
            iced::advanced::widget::tree::State::new(BlitzZustand::default())
        }
        fn children(&self) -> Vec<iced::advanced::widget::Tree> {
            vec![iced::advanced::widget::Tree::new(&self.inhalt)]
        }
        fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
            tree.diff_children(std::slice::from_ref(&self.inhalt));
        }
        fn size(&self) -> iced::Size<iced::Length> {
            self.inhalt.as_widget().size()
        }
        fn layout(
            &mut self,
            tree: &mut iced::advanced::widget::Tree,
            renderer: &iced::Renderer,
            limits: &iced::advanced::layout::Limits,
        ) -> iced::advanced::layout::Node {
            self.inhalt
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits)
        }
        fn update(
            &mut self,
            tree: &mut iced::advanced::widget::Tree,
            event: &iced::Event,
            layout: iced::advanced::Layout<'_>,
            cursor: iced::advanced::mouse::Cursor,
            renderer: &iced::Renderer,
            clipboard: &mut dyn iced::advanced::Clipboard,
            shell: &mut iced::advanced::Shell<'_, M>,
            viewport: &iced::Rectangle,
        ) {
            let z = tree.state.downcast_mut::<BlitzZustand>();
            let drueber = cursor.is_over(layout.bounds());
            if drueber != z.hover {
                z.hover = drueber;
                shell.request_redraw();
            }
            let gedrueckt = matches!(
                event,
                iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                    iced::mouse::Button::Left
                ))
            ) || matches!(event, iced::Event::Touch(iced::touch::Event::FingerPressed { .. }));
            if gedrueckt && drueber && z.seit.is_none() {
                if mk::bewegung_reduziert() {
                    shell.publish(self.msg.clone());
                } else {
                    z.seit = Some(std::time::Instant::now());
                    shell.request_redraw();
                }
                shell.capture_event();
                return;
            }
            if let iced::Event::Window(iced::window::Event::RedrawRequested(_)) = event {
                if let Some(seit) = z.seit {
                    if seit.elapsed().as_millis() as u64 >= mk::eingabe::MENU_BLITZ_MS {
                        z.seit = None;
                        shell.publish(self.msg.clone());
                    } else {
                        shell.request_redraw();
                    }
                }
            }
            self.inhalt.as_widget_mut().update(
                &mut tree.children[0],
                event,
                layout,
                cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
        }
        fn draw(
            &self,
            tree: &iced::advanced::widget::Tree,
            renderer: &mut iced::Renderer,
            theme: &iced::Theme,
            style: &iced::advanced::renderer::Style,
            layout: iced::advanced::Layout<'_>,
            cursor: iced::advanced::mouse::Cursor,
            viewport: &iced::Rectangle,
        ) {
            use iced::advanced::Renderer as _;
            let z = tree.state.downcast_ref::<BlitzZustand>();
            // Blitz-Phasen (aus — AN — aus) bzw. ruhender Hover-Grund.
            let grund = if let Some(seit) = z.seit {
                let t = seit.elapsed().as_millis() as u64;
                let an = t >= mk::eingabe::MENU_BLITZ_MS / 3
                    && t < 2 * mk::eingabe::MENU_BLITZ_MS / 3;
                an.then(|| {
                    self.p
                        .on_surface
                        .over(self.p.surface_container_high, mk::state_layer::PRESSED)
                })
            } else if z.hover {
                Some(
                    self.p
                        .on_surface
                        .over(self.p.surface_container_high, mk::state_layer::HOVER),
                )
            } else {
                None
            };
            if let Some(g) = grund {
                renderer.fill_quad(
                    iced::advanced::renderer::Quad {
                        bounds: layout.bounds(),
                        border: iced::Border {
                            radius: mk::CORNER_RADIUS.into(),
                            ..Default::default()
                        },
                        shadow: iced::Shadow::default(),
                        snap: false,
                    },
                    color(g),
                );
            }
            self.inhalt.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                style,
                layout,
                cursor,
                viewport,
            );
        }
        fn mouse_interaction(
            &self,
            _tree: &iced::advanced::widget::Tree,
            layout: iced::advanced::Layout<'_>,
            cursor: iced::advanced::mouse::Cursor,
            _viewport: &iced::Rectangle,
            _renderer: &iced::Renderer,
        ) -> iced::advanced::mouse::Interaction {
            if cursor.is_over(layout.bounds()) {
                iced::advanced::mouse::Interaction::Pointer
            } else {
                iced::advanced::mouse::Interaction::default()
            }
        }
        fn operate(
            &mut self,
            tree: &mut iced::advanced::widget::Tree,
            layout: iced::advanced::Layout<'_>,
            renderer: &iced::Renderer,
            operation: &mut dyn iced::advanced::widget::Operation,
        ) {
            self.inhalt
                .as_widget_mut()
                .operate(&mut tree.children[0], layout, renderer, operation);
        }
        fn overlay<'b>(
            &'b mut self,
            tree: &'b mut iced::advanced::widget::Tree,
            layout: iced::advanced::Layout<'b>,
            renderer: &iced::Renderer,
            viewport: &iced::Rectangle,
            translation: iced::Vector,
        ) -> Option<iced::advanced::overlay::Element<'b, M, iced::Theme, iced::Renderer>>
        {
            self.inhalt.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            )
        }
    }

    pub fn menu_family<'a, M: Clone + 'a>(
        kopf: Option<Element<'a, M>>,
        eintraege: Vec<MenuEintrag<M>>,
        p: mk::Palette,
    ) -> Element<'a, M> {
        let mut spalte = iced::widget::column![].spacing(mk::spacing::XXS);
        if let Some(k) = kopf {
            spalte = spalte.push(
                container(k).padding([4, mk::spacing::S as u16]),
            );
        }
        for e in eintraege {
            match e {
                MenuEintrag::Trenner => {
                    spalte = spalte.push(trenner(p));
                }
                MenuEintrag::Inaktiv { zeichen, titel } => {
                    let f = p.outline;
                    let inhalt: Element<'a, M> = match zeichen {
                        Some(z) => etikett(z, titel, mk::typo::FLIESS, f),
                        None => txt(titel, mk::typo::FLIESS, f).into(),
                    };
                    spalte = spalte.push(
                        container(inhalt)
                            .width(Length::Fill)
                            .padding([5, mk::spacing::M as u16]),
                    );
                }
                MenuEintrag::PunktMitBadge { zeichen, titel, badge, farbe, msg } => {
                    let f = farbe.unwrap_or(p.on_surface);
                    let links: Element<'a, M> = match zeichen {
                        Some(z) => etikett(z, titel, mk::typo::FLIESS, f),
                        None => txt(titel, mk::typo::FLIESS, f).into(),
                    };
                    // Badge-Pille rechts, tonal — wie „1 Update" im Apfel-Menü.
                    let pille = container(txt(badge, mk::typo::KLEIN, p.on_surface))
                        .padding(iced::Padding {
                            left: mk::spacing::S,
                            right: mk::spacing::S,
                            top: 1.0,
                            bottom: 1.0,
                        })
                        .style(move |_| container::Style {
                            background: Some(
                                color(p.on_surface.over(p.surface_container_high, 0.10)).into(),
                            ),
                            border: iced::Border {
                                radius: 999.0_f32.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        });
                    let inhalt = row![links, Space::new().width(Length::Fill), pille]
                        .align_y(iced::Alignment::Center);
                    // R66: der Familien-Knopf mit Menü-Blitz.
                    spalte = spalte.push(lupe(menu_punkt_knopf(
                        container(inhalt)
                            .width(Length::Fill)
                            .padding([5, mk::spacing::M as u16])
                            .into(),
                        msg,
                        p,
                    )));
                }
                MenuEintrag::PunktMitKuerzel { zeichen, titel, kuerzel, farbe, msg } => {
                    let f = farbe.unwrap_or(p.on_surface);
                    let links: Element<'a, M> = match zeichen {
                        Some(z) => etikett(z, titel, mk::typo::FLIESS, f),
                        None => txt(titel, mk::typo::FLIESS, f).into(),
                    };
                    let inhalt = row![
                        links,
                        Space::new().width(Length::Fill),
                        txt(kuerzel, mk::typo::KLEIN, p.on_surface_variant),
                    ]
                    .align_y(iced::Alignment::Center);
                    // R66: der Familien-Knopf mit Menü-Blitz.
                    spalte = spalte.push(lupe(menu_punkt_knopf(
                        container(inhalt)
                            .width(Length::Fill)
                            .padding([5, mk::spacing::M as u16])
                            .into(),
                        msg,
                        p,
                    )));
                }
                MenuEintrag::Punkt { zeichen, titel, farbe, msg } => {
                    let f = farbe.unwrap_or(p.on_surface);
                    let inhalt: Element<'a, M> = match zeichen {
                        Some(z) => etikett(z, titel, mk::typo::FLIESS, f),
                        None => txt(titel, mk::typo::FLIESS, f).into(),
                    };
                    // R66: der Familien-Knopf mit Menü-Blitz.
                    spalte = spalte.push(lupe(menu_punkt_knopf(
                        container(inhalt)
                            .width(Length::Fill)
                            .padding([5, mk::spacing::M as u16])
                            .into(),
                        msg,
                        p,
                    )));
                }
            }
        }
        panel_huelle(spalte.into(), MENU_BREITE, p)
    }

    /// HarnessFamily: der Fenster-Navigations-Knopf (‹ › ⟳ …) — Symbol
    /// + Kit-Knopf-Stil + Lupe. Für Kopfzeilen (Web-Navigation,
    /// Verlaufs-Pfeile) — inaktive Knöpfe sind stumpf und lupenlos.
    pub fn nav_knopf<'a, M: Clone + 'a>(
        zeichen: char,
        aktiv: bool,
        msg: M,
        p: mk::Palette,
    ) -> Element<'a, M> {
        let farbe = if aktiv {
            p.on_surface
        } else {
            p.on_surface_variant.mit_alpha(0.4)
        };
        let mut k = iced::widget::button(symbol::<M>(
            zeichen,
            mk::icon_size::SMALL + 2.0,
            farbe,
        ))
        .padding([2, mk::spacing::S as u16])
        .style(move |_, status| leiste::knopf_stil(p, status, mk::radius::KLEIN));
        if aktiv {
            k = k.on_press(msg);
            lupe(k)
        } else {
            k.into()
        }
    }
}

// ===========================================================================
// BildKachel-Familie, Pixel-Seite (Feature "bild"): das verbindliche
// Thumb-Rezept aus dem Hintergrund-Bereich (R38/R44) — zentrierter
// 16:9-Zuschnitt, 448x252 CatmullRom, Ecken DIREKT ins Bild gebacken
// (iced clippt Rasterbilder nicht an gerundeten Rahmen). Angezeigt wird
// die Kachel mit 112x63 — die 24px-Backrundung wirkt dort als 6px.
// ===========================================================================

#[cfg(feature = "bild")]
pub mod bild {
    /// Kreisbogen-Ecken mit 1px-Weichzeichnung in den Alphakanal stanzen.
    pub fn ecken_runden(bild: &mut image::RgbaImage, radius: f32) {
        let (w, h) = bild.dimensions();
        let r = radius.min(w as f32 / 2.0).min(h as f32 / 2.0);
        for y in 0..h {
            for x in 0..w {
                let fx = x as f32 + 0.5;
                let fy = y as f32 + 0.5;
                let dx = if fx < r {
                    r - fx
                } else if fx > w as f32 - r {
                    fx - (w as f32 - r)
                } else {
                    continue;
                };
                let dy = if fy < r {
                    r - fy
                } else if fy > h as f32 - r {
                    fy - (h as f32 - r)
                } else {
                    continue;
                };
                let d = (dx * dx + dy * dy).sqrt();
                if d > r {
                    bild.get_pixel_mut(x, y).0[3] = 0;
                } else if d > r - 1.0 {
                    let alt = bild.get_pixel(x, y).0[3] as f32;
                    bild.get_pixel_mut(x, y).0[3] = (alt * (r - d)) as u8;
                }
            }
        }
    }

    /// Das Familien-Thumb: 16:9-Zuschnitt zentriert, 448x252, gebackene
    /// Ecken (24px). Liefert (Breite, Höhe, RGBA) für Handle::from_rgba.
    pub fn kachel_thumb(bild: image::DynamicImage) -> (u32, u32, Vec<u8>) {
        let (bw, bh) = (bild.width(), bild.height());
        let ziel_seite = 16.0 / 9.0;
        let (cw, ch) = if (bw as f32 / bh.max(1) as f32) > ziel_seite {
            (((bh as f32) * ziel_seite) as u32, bh)
        } else {
            (bw, ((bw as f32) / ziel_seite) as u32)
        };
        let geschnitten = bild.crop_imm((bw - cw) / 2, (bh - ch) / 2, cw.max(1), ch.max(1));
        let thumb = geschnitten.resize_exact(448, 252, image::imageops::FilterType::CatmullRom);
        let mut rgba = thumb.to_rgba8();
        ecken_runden(&mut rgba, 24.0);
        let (w, h) = rgba.dimensions();
        (w, h, rgba.into_raw())
    }
}
