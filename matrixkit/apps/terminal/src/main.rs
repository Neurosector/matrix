//! Matrix Terminal — App #28 (R61), die eigene Kommandozeile.
//!
//! Architektur wie bei den Großen: `alacritty_terminal` ist das
//! Maschinenhaus (PTY, Escape-Parser, Zellen-Gitter, Scrollback) —
//! eine BIBLIOTHEK, kein fremdes Fenster. MatrixKit rendert das Gitter
//! selbst: Rahmen mit Ampeln, lebende Palette, Maple Mono NF vom
//! Image. Die ANSI-Grundfarben sind auf die Matrix-Dunkelwelt
//! gestimmt; Vorder-/Hintergrund folgen der Palette live.
//!
//! Bewusste Abweichungen vom Kit-Standard:
//! * KEINE Einzelinstanz — Terminals will man im Rudel.
//! * Strg+W gehört der Shell (kill-word), nicht dem Fenster —
//!   der Rahmen läuft mit `abo_mit(false)`.

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex, OnceLock};

use alacritty_terminal::event::{Event as TermEvent, EventListener, Notify, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Msg as SchleifenMsg, Notifier};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::{Config as TermConfig, Term};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use iced::widget::{column, container};
use iced::{Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use matrixkit_widgets::color;

const APP_ID: &str = "matrix-terminal";

/// Maple Mono NF bei 13 px: Vorschub 0,6 em, Zeile fest 18 px — die
/// Zellmaße, aus denen Spalten und Zeilen des PTY errechnet werden.
const SCHRIFT: f32 = 13.0;
const ZELLE_B: f32 = SCHRIFT * 0.6;
const ZELLE_H: f32 = 18.0;
/// Innenluft des Gitters + Platz des Rahmen-Kopfes (Ampeln/Titel).
const RAND_X: f32 = 28.0;
const RAND_Y: f32 = 68.0;


fn main() -> iced::Result {
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    iced::application(App::new, App::update, App::view)
        .title(|a: &App| a.titel.clone())
        .subscription(App::subscription)
        .window(mkw::fenster_settings(APP_ID, 840.0, 560.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .font(mkw::mono_font_laden())
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

// ------------------------------------------------------ Maschinenhaus

/// Gittermaß fürs Backend.
#[derive(Clone, Copy)]
struct Mass {
    spalten: usize,
    zeilen: usize,
}

impl Dimensions for Mass {
    fn total_lines(&self) -> usize {
        self.zeilen
    }
    fn screen_lines(&self) -> usize {
        self.zeilen
    }
    fn columns(&self) -> usize {
        self.spalten
    }
}

/// Terminal-Ereignisse wandern über einen Kanal in die iced-Welt.
#[derive(Clone)]
struct Bote(mpsc::Sender<TermEvent>);

impl EventListener for Bote {
    fn send_event(&self, ereignis: TermEvent) {
        let _ = self.0.send(ereignis);
    }
}

/// Der Empfänger für die iced-Subscription (einmalig gesteckt).
static EREIGNISSE: OnceLock<Mutex<mpsc::Receiver<TermEvent>>> = OnceLock::new();

fn ereignis_abo() -> Subscription<TermEvent> {
    Subscription::run(|| {
        let (tx, rx) = iced::futures::channel::mpsc::unbounded();
        std::thread::spawn(move || {
            let Some(quelle) = EREIGNISSE.get() else { return };
            let quelle = quelle.lock().unwrap();
            while let Ok(e) = quelle.recv() {
                if tx.unbounded_send(e).is_err() {
                    return;
                }
            }
        });
        rx
    })
}

struct Maschine {
    term: Arc<FairMutex<Term<Bote>>>,
    schreiber: Notifier,
    mass: Mass,
}

impl Maschine {
    fn starten(mass: Mass) -> Self {
        let (tx, rx) = mpsc::channel();
        let _ = EREIGNISSE.set(Mutex::new(rx));
        let bote = Bote(tx);

        let mut umgebung = HashMap::new();
        // Universelles terminfo — alacritty-Eintrag fehlt auf dem Image.
        umgebung.insert(String::from("TERM"), String::from("xterm-256color"));
        umgebung.insert(String::from("COLORTERM"), String::from("truecolor"));

        let fenster = WindowSize {
            num_cols: mass.spalten as u16,
            num_lines: mass.zeilen as u16,
            cell_width: ZELLE_B as u16,
            cell_height: ZELLE_H as u16,
        };
        let optionen = tty::Options {
            shell: None, // Login-Shell des Nutzers
            working_directory: None,
            hold: false,
            env: umgebung,
        };
        let pty = tty::new(&optionen, fenster, 0).expect("matrix-terminal: PTY");
        let term = Arc::new(FairMutex::new(Term::new(
            TermConfig::default(),
            &mass,
            bote.clone(),
        )));
        let schleife = EventLoop::new(term.clone(), bote, pty, false, false)
            .expect("matrix-terminal: Ereignisschleife");
        let schreiber = Notifier(schleife.channel());
        let _ = schleife.spawn();
        Self { term, schreiber, mass }
    }

    fn tippen(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            self.schreiber.notify(bytes);
        }
    }

    fn messen(&mut self, breite: f32, hoehe: f32) {
        let spalten = ((breite - RAND_X) / ZELLE_B).floor().max(20.0) as usize;
        let zeilen = ((hoehe - RAND_Y) / ZELLE_H).floor().max(5.0) as usize;
        if spalten == self.mass.spalten && zeilen == self.mass.zeilen {
            return;
        }
        self.mass = Mass { spalten, zeilen };
        let fenster = WindowSize {
            num_cols: spalten as u16,
            num_lines: zeilen as u16,
            cell_width: ZELLE_B as u16,
            cell_height: ZELLE_H as u16,
        };
        let _ = self.schreiber.0.send(SchleifenMsg::Resize(fenster));
        self.term.lock().resize(self.mass);
    }
}

// ------------------------------------------------------------ Farben

/// Die 16 ANSI-Grundfarben in der Matrix-Dunkelwelt — gedeckte, aber
/// unterscheidbare Töne (Material-Verwandtschaft der Kit-Palette).
const ANSI: [(u8, u8, u8); 16] = [
    (0x2a, 0x2c, 0x33), // schwarz
    (0xf2, 0x8b, 0x82), // rot
    (0x81, 0xc9, 0x95), // grün
    (0xfd, 0xd6, 0x63), // gelb
    (0x8a, 0xb4, 0xf8), // blau
    (0xd7, 0xae, 0xfb), // magenta
    (0x78, 0xd9, 0xec), // cyan
    (0xdd, 0xe1, 0xe6), // weiß
    (0x48, 0x4b, 0x55), // hell-schwarz
    (0xf6, 0xae, 0xa9),
    (0xa8, 0xda, 0xb5),
    (0xfd, 0xe2, 0x93),
    (0xae, 0xcb, 0xfa),
    (0xe4, 0xc7, 0xfc),
    (0xa1, 0xe4, 0xf2),
    (0xf8, 0xf9, 0xfa), // hell-weiß
];

fn indexfarbe(i: u8) -> iced::Color {
    match i {
        0..=15 => {
            let (r, g, b) = ANSI[i as usize];
            iced::Color::from_rgb8(r, g, b)
        }
        16..=231 => {
            // 6×6×6-Würfel
            let i = i - 16;
            let stufe = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
            iced::Color::from_rgb8(stufe(i / 36), stufe((i / 6) % 6), stufe(i % 6))
        }
        _ => {
            let g = 8 + (i - 232) * 10;
            iced::Color::from_rgb8(g, g, g)
        }
    }
}

fn ansifarbe(farbe: AnsiColor, p: mk::Palette, vordergrund: bool) -> Option<iced::Color> {
    match farbe {
        AnsiColor::Spec(rgb) => Some(iced::Color::from_rgb8(rgb.r, rgb.g, rgb.b)),
        AnsiColor::Indexed(i) => Some(indexfarbe(i)),
        AnsiColor::Named(n) => match n {
            NamedColor::Foreground => Some(color(p.on_surface)),
            NamedColor::Background => {
                // Standard-Hintergrund = Fensterfläche → kein Span-Grund nötig.
                if vordergrund { Some(color(p.surface)) } else { None }
            }
            NamedColor::Cursor => Some(color(p.primary)),
            NamedColor::BrightForeground => Some(color(p.on_surface)),
            NamedColor::DimForeground => Some(color(p.on_surface_variant)),
            _ => {
                let i = n as usize;
                (i < 16).then(|| {
                    let (r, g, b) = ANSI[i];
                    iced::Color::from_rgb8(r, g, b)
                })
            }
        },
    }
}

// ------------------------------------------------------- Tasten → PTY

/// Eine gedrückte Taste in die Bytes übersetzen, die eine xterm-artige
/// Anwendung erwartet. `text` ist iceds fertige Übersetzung (Shift,
/// AltGr, Umlaute) — Steuerkombinationen gehen vor.
fn taste_zu_bytes(
    key: &iced::keyboard::Key,
    mods: iced::keyboard::Modifiers,
    text: Option<&str>,
) -> Option<Vec<u8>> {
    use iced::keyboard::key::Named;
    use iced::keyboard::Key;

    // Steuerzeichen: Strg+a..z → 0x01..0x1a (Strg+Shift bleibt frei
    // für künftiges Kopieren/Einfügen).
    if mods.control() && !mods.shift() {
        if let Key::Character(c) = key {
            let mut zeichen = c.chars();
            if let (Some(z), None) = (zeichen.next(), zeichen.next()) {
                let klein = z.to_ascii_lowercase();
                if klein.is_ascii_lowercase() {
                    return Some(vec![(klein as u8) & 0x1f]);
                }
                match klein {
                    ' ' | '@' => return Some(vec![0x00]),
                    '[' => return Some(vec![0x1b]),
                    '\\' => return Some(vec![0x1c]),
                    ']' => return Some(vec![0x1d]),
                    _ => {}
                }
            }
        }
    }

    let esc = |s: &str| Some(format!("\x1b{s}").into_bytes());
    if let Key::Named(n) = key {
        return match n {
            Named::Enter => Some(b"\r".to_vec()),
            Named::Backspace => Some(vec![0x7f]),
            Named::Tab if mods.shift() => esc("[Z"),
            Named::Tab => Some(b"\t".to_vec()),
            Named::Escape => Some(vec![0x1b]),
            Named::ArrowUp => esc("[A"),
            Named::ArrowDown => esc("[B"),
            Named::ArrowRight => esc("[C"),
            Named::ArrowLeft => esc("[D"),
            Named::Home => esc("[H"),
            Named::End => esc("[F"),
            Named::PageUp => esc("[5~"),
            Named::PageDown => esc("[6~"),
            Named::Delete => esc("[3~"),
            Named::Insert => esc("[2~"),
            Named::Space => Some(b" ".to_vec()),
            _ => None,
        };
    }

    // Normaler Text — Alt schickt das ESC-Präfix voraus (Meta).
    let t = text?;
    if t.is_empty() || t.chars().next().is_some_and(|c| (c as u32) < 0x20) {
        return None;
    }
    let mut bytes = Vec::new();
    if mods.alt() {
        bytes.push(0x1b);
    }
    bytes.extend_from_slice(t.as_bytes());
    Some(bytes)
}

// --------------------------------------------------------------- App

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Term(std::sync::Arc<TermEvent>),
    Taste(iced::keyboard::Key, iced::keyboard::Modifiers, Option<String>),
    Rad(f32),
    Eingefuegt(Option<String>),
}

struct App {
    rahmen: mkw::Rahmen,
    maschine: Maschine,
    titel: String,
    /// Fensterinnenmaß — für die Gitter-Vermessung.
    flaeche: (f32, f32),
    /// Die Shell ist gegangen — das Fenster folgt ihr.
    beendet: bool,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let mass = Mass {
            spalten: ((840.0 - RAND_X) / ZELLE_B) as usize,
            zeilen: ((560.0 - RAND_Y) / ZELLE_H) as usize,
        };
        (
            App {
                rahmen: mkw::Rahmen::neu(APP_ID, &[]),
                maschine: Maschine::starten(mass),
                titel: String::from("Matrix Terminal"),
                flaeche: (840.0, 560.0),
                beendet: false,
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(r) => {
                if let mkw::RahmenMsg::Groesse(g) = &r {
                    self.flaeche = (g.width, g.height);
                    self.maschine.messen(g.width, g.height);
                }
                self.rahmen.update(r).map(Msg::Rahmen)
            }
            Msg::Term(ereignis) => {
                match &*ereignis {
                    TermEvent::PtyWrite(text) => {
                        self.maschine.tippen(text.clone().into_bytes())
                    }
                    TermEvent::Title(t) => self.titel = format!("{t} — Matrix Terminal"),
                    TermEvent::Bell => {
                        let _ = mk::feedback::jetzt("terminal", "04-benachrichtigung-leise.wav");
                    }
                    TermEvent::Exit => {
                        self.beendet = true;
                        return iced::window::latest().and_then(iced::window::close);
                    }
                    _ => {} // Wakeup & Co: das Neuzeichnen passiert ohnehin
                }
                Task::none()
            }
            Msg::Taste(key, mods, text) => {
                if self.rahmen.root.offen() {
                    return Task::none(); // Root-Ebene fängt die Tastatur
                }
                // Einfügen: Strg+Shift+V — die Terminal-Konvention.
                if mods.control() && mods.shift() {
                    if let iced::keyboard::Key::Character(c) = &key {
                        if c.as_str().eq_ignore_ascii_case("v") {
                            return iced::clipboard::read().map(Msg::Eingefuegt);
                        }
                    }
                }
                if let Some(bytes) = taste_zu_bytes(&key, mods, text.as_deref()) {
                    // Eingabe springt ans Ende des Verlaufs (Terminal-Sitte).
                    self.maschine.term.lock().scroll_display(Scroll::Bottom);
                    self.maschine.tippen(bytes);
                }
                Task::none()
            }
            Msg::Rad(zeilen) => {
                self.maschine
                    .term
                    .lock()
                    .scroll_display(Scroll::Delta(zeilen as i32));
                Task::none()
            }
            Msg::Eingefuegt(inhalt) => {
                if let Some(t) = inhalt {
                    self.maschine.tippen(t.into_bytes());
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        if self.beendet {
            return Subscription::none();
        }
        let tasten = iced::event::listen_with(|ereignis, _status, _fenster| {
            use iced::keyboard::Event as K;
            use iced::mouse::Event as M;
            match ereignis {
                iced::Event::Keyboard(K::KeyPressed { key, modifiers, text, .. }) => Some(
                    Msg::Taste(key, modifiers, text.map(|t| t.to_string())),
                ),
                iced::Event::Mouse(M::WheelScrolled { delta }) => {
                    let zeilen = match delta {
                        iced::mouse::ScrollDelta::Lines { y, .. } => y * 3.0,
                        iced::mouse::ScrollDelta::Pixels { y, .. } => y / ZELLE_H,
                    };
                    (zeilen.abs() >= 0.5).then(|| Msg::Rad(zeilen))
                }
                _ => None,
            }
        });
        Subscription::batch([
            // Strg+W gehört der Shell — Rahmen ohne Fenstertasten.
            self.rahmen.abo_mit(false).map(Msg::Rahmen),
            ereignis_abo().map(|e| Msg::Term(std::sync::Arc::new(e))),
            tasten,
        ])
    }

    /// Das Gitter als rich_text-Zeilen: Läufe gleicher Farbe werden zu
    /// EINEM Span — 24 Zeilen × wenige Läufe statt 1920 Einzelzellen.
    fn gitter(&self) -> Element<'_, Msg> {
        use alacritty_terminal::term::cell::Flags;
        use iced::widget::span;
        let p = self.rahmen.palette;
        let term = self.maschine.term.lock();
        let inhalt = term.renderable_content();
        let cursor = inhalt.cursor.point;
        let zeilen_gesamt = self.maschine.mass.zeilen;
        let spalten = self.maschine.mass.spalten;

        // Zellen in Zeilenpuffer einsammeln.
        let mut gitter: Vec<Vec<(char, Option<iced::Color>, Option<iced::Color>)>> =
            vec![vec![(' ', None, None); spalten]; zeilen_gesamt];
        for indexed in inhalt.display_iter {
            let zeile = indexed.point.line.0;
            let spalte = indexed.point.column.0;
            if zeile < 0 || zeile as usize >= zeilen_gesamt || spalte >= spalten {
                continue;
            }
            let zelle = &indexed;
            if zelle.flags.contains(Flags::HIDDEN) {
                continue;
            }
            let mut fg = ansifarbe(zelle.fg, p, true);
            let mut bg = ansifarbe(zelle.bg, p, false);
            if zelle.flags.contains(Flags::INVERSE) {
                let alt_fg = fg;
                fg = Some(bg.unwrap_or(color(p.surface)));
                bg = Some(alt_fg.unwrap_or(color(p.on_surface)));
            }
            if zelle.flags.contains(Flags::DIM) {
                if let Some(f) = fg.as_mut() {
                    f.a *= 0.6;
                }
            }
            // Cursor: der Block in Primary — die eine Kit-Akzentstelle.
            if zeile == cursor.line.0 && spalte == cursor.column.0 {
                fg = Some(color(p.on_primary));
                bg = Some(color(p.primary));
            }
            gitter[zeile as usize][spalte] = (zelle.c, fg, bg);
        }
        drop(term);

        let mut spaltenbau = column![].spacing(0.0);
        for zeile in &gitter {
            let mut spans = Vec::new();
            let mut lauf = String::new();
            let mut lauf_stil: (Option<iced::Color>, Option<iced::Color>) = (None, None);
            for (z, fg, bg) in zeile {
                let stil = (*fg, *bg);
                if stil != lauf_stil && !lauf.is_empty() {
                    spans.push(span_bauen(std::mem::take(&mut lauf), lauf_stil, p));
                }
                lauf_stil = stil;
                lauf.push(*z);
            }
            if !lauf.is_empty() {
                spans.push(span_bauen(lauf, lauf_stil, p));
            }
            spaltenbau = spaltenbau.push(
                iced::widget::rich_text(spans)
                    .font(mkw::mono())
                    .size(SCHRIFT)
                    .line_height(iced::widget::text::LineHeight::Absolute(ZELLE_H.into()))
                    .wrapping(iced::widget::text::Wrapping::None),
            );
        }

        fn span_bauen<'a>(
            text: String,
            (fg, bg): (Option<iced::Color>, Option<iced::Color>),
            p: mk::Palette,
        ) -> iced::widget::text::Span<'a, Msg> {
            let mut s = span(text).color(fg.unwrap_or(color(p.on_surface)));
            if let Some(b) = bg {
                s = s.background(b);
            }
            s
        }

        container(spaltenbau)
            .padding(iced::Padding { top: 6.0, right: 10.0, bottom: 10.0, left: 12.0 })
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view(&self) -> Element<'_, Msg> {
        self.rahmen.fenster(
            "Matrix Terminal",
            "1.0",
            "Die Kommandozeile in der MatrixKit-Grammatik — \
             alacritty-Maschinenhaus, Matrix-Karosserie.",
            self.gitter(),
            Msg::Rahmen,
        )
    }
}
