//! Matrix Player — App #30 (R71): Videos ansehen, Bildschirmaufnahmen zuerst.
//!
//! Die Leitbild-Blaupause: die Medien-Referenzschicht dekodiert nicht in der App, sondern
//! außer Prozess (einen Medien-Dienstprozess) und reicht fertige Bilder herüber. Hier
//! übernimmt ffmpeg diese Rolle — als Kindprozess, der rohe RGBA-Frames
//! durch eine Pipe schiebt. Das Fenster selbst bleibt reines MatrixKit:
//! Chrome, Palette, Familien, Player-Grammatik (Play/Pause, Zeitleiste,
//! mm:ss-Anzeigen).
//!
//! Ohne Argument öffnet der Player die neueste Bildschirmaufnahme — der
//! kürzeste Weg zu „hat meine Aufnahme funktioniert?".

use iced::widget::{column, container, mouse_area, row, Space};
use iced::{Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const APP_ID: &str = "matrix-player";
/// Decode-Breite: ffmpeg skaliert herunter, die Pipe bleibt schlank.
/// (4K-Aufnahmen wären ~1 GB/s roh — so sind es ~70 MB/s.)
const DECODE_BREITE: u32 = 1024;

fn main() -> iced::Result {
    // Anders als der Rest des Kits: Video braucht VSync (fifo) — mailbox
    // flackert auf NVIDIA, wenn 30×/s neue Texturen präsentiert werden.
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "fifo");
    }
    iced::application(App::new, App::update, App::view)
        .title(|app: &App| match &app.film {
            Some(f) => format!("Matrix Player — {}", f.name()),
            None => String::from("Matrix Player"),
        })
        .subscription(App::subscription)
        .window(mkw::fenster_settings(APP_ID, 960.0, 680.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

// ------------------------------------------------------------- Der Film

/// Steckbrief aus ffprobe: was liegt da, wie lang, wie schnell.
#[derive(Debug, Clone)]
struct Film {
    pfad: PathBuf,
    dauer_s: f64,
    fps: f64,
    breite: u32,
    hoehe: u32,
}

impl Film {
    fn name(&self) -> String {
        self.pfad
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Decode-Maße: Breite fest, Höhe aus dem Seitenverhältnis, gerade
    /// gerundet — MUSS exakt zur ffmpeg-Skalierung passen (Frame-Größe!).
    fn decode_masse(&self) -> (u32, u32) {
        let b = DECODE_BREITE.min(self.breite).max(2) & !1;
        let h = ((self.hoehe as f64 * b as f64 / self.breite as f64).round() as u32).max(2) & !1;
        (b, h)
    }
}

/// ffprobe befragen — Dauer, Maße, Bildrate.
fn sondieren(pfad: &Path) -> Option<Film> {
    let aus = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height,avg_frame_rate:format=duration",
            "-of", "default=nw=1",
        ])
        .arg(pfad)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&aus.stdout).into_owned();
    let mut breite = 0u32;
    let mut hoehe = 0u32;
    let mut fps = 0f64;
    let mut dauer = 0f64;
    for zeile in text.lines() {
        if let Some(w) = zeile.strip_prefix("width=") {
            breite = w.trim().parse().unwrap_or(0);
        } else if let Some(w) = zeile.strip_prefix("height=") {
            hoehe = w.trim().parse().unwrap_or(0);
        } else if let Some(w) = zeile.strip_prefix("duration=") {
            dauer = w.trim().parse().unwrap_or(0.0);
        } else if let Some(w) = zeile.strip_prefix("avg_frame_rate=") {
            let mut teile = w.trim().split('/');
            let z: f64 = teile.next().and_then(|t| t.parse().ok()).unwrap_or(0.0);
            let n: f64 = teile.next().and_then(|t| t.parse().ok()).unwrap_or(1.0);
            if n > 0.0 && z > 0.0 {
                fps = z / n;
            }
        }
    }
    if breite == 0 || hoehe == 0 || dauer <= 0.0 {
        return None;
    }
    Some(Film {
        pfad: pfad.to_path_buf(),
        dauer_s: dauer,
        fps: if fps > 0.0 { fps } else { 30.0 },
        breite,
        hoehe,
    })
}

/// Ohne Argument: die neueste Bildschirmaufnahme.
fn neueste_aufnahme() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let ordner = PathBuf::from(home).join("Videos/Bildschirmaufnahmen");
    let mut videos: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&ordner)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let mp4 = p
                .extension()
                .map(|x| matches!(x.to_str(), Some("mp4" | "webm" | "mkv" | "mov")))
                .unwrap_or(false);
            if !mp4 {
                return None;
            }
            Some((e.metadata().ok()?.modified().ok()?, p))
        })
        .collect();
    videos.sort();
    videos.pop().map(|(_, p)| p)
}

// --------------------------------------------------------- Das Laufwerk

/// Geteilter Zustand zwischen Decoder-Faden und Fenster.
struct Laufwerk {
    /// Jüngster Frame: (laufende Nummer, Breite, Höhe, RGBA).
    bild: Mutex<Option<(u64, u32, u32, Vec<u8>)>>,
    /// Abspielposition in Millisekunden.
    pos_ms: AtomicU64,
    /// Film zu Ende (der Faden hat EOF gelesen).
    fertig: AtomicBool,
    /// Pause = der Faden liest nicht; die volle Pipe hält ffmpeg an.
    pausiert: AtomicBool,
    /// Jeder Neustart (Sprung) zählt hoch; alte Fäden erkennen sich daran.
    generation: AtomicU64,
}

fn dekodieren(
    lauf: Arc<Laufwerk>,
    mut rohr: std::process::ChildStdout,
    gen: u64,
    ab_s: f64,
    fps: f64,
    b: u32,
    h: u32,
) {
    let frame_len = (b * h * 4) as usize;
    let mut puffer = vec![0u8; frame_len];
    let frame_dauer = Duration::from_secs_f64(1.0 / fps);
    let mut naechster = Instant::now();
    let mut nr: u64 = 0;
    loop {
        if lauf.generation.load(Ordering::SeqCst) != gen {
            return; // ein Sprung hat übernommen
        }
        if lauf.pausiert.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(40));
            naechster = Instant::now();
            continue;
        }
        if rohr.read_exact(&mut puffer).is_err() {
            // EOF: Film zu Ende — es sei denn, wir wurden abgelöst.
            if lauf.generation.load(Ordering::SeqCst) == gen {
                lauf.fertig.store(true, Ordering::SeqCst);
                lauf.pausiert.store(true, Ordering::SeqCst);
            }
            return;
        }
        nr += 1;
        let pos = (ab_s * 1000.0) as u64 + (nr as f64 * 1000.0 / fps) as u64;
        lauf.pos_ms.store(pos, Ordering::SeqCst);
        *lauf.bild.lock().unwrap() = Some((gen * 1_000_000 + nr, b, h, puffer.clone()));
        // Takt: auf Bildrate drosseln; hinken wir weit hinterher, aufschließen.
        naechster += frame_dauer;
        let jetzt = Instant::now();
        if naechster > jetzt {
            std::thread::sleep(naechster - jetzt);
        } else if jetzt - naechster > Duration::from_millis(300) {
            naechster = jetzt;
        }
    }
}

// ------------------------------------------------------------- Die App

struct App {
    rahmen: mkw::Rahmen,
    film: Option<Film>,
    lauf: Arc<Laufwerk>,
    kind: Arc<Mutex<Option<Child>>>,
    /// Jüngst hochgeladenes Bild (Frame-Nummer + GPU-Handle).
    handle: Option<iced::widget::image::Handle>,
    /// Das Vorgänger-Handle lebt einen Takt weiter: sein Ableben dürfte
    /// sonst die Textur freigeben, während sie noch auf dem Schirm ist.
    handle_alt: Option<iced::widget::image::Handle>,
    letzte_nr: u64,
    /// Beim Ziehen der Zeitleiste: Zielposition statt Live-Position.
    ziehe: Option<f32>,
}

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Taste(mkw::Taste),
    Tick,
    PlayPause,
    Ziehen(f32),
    Springen,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let pfad = std::env::args()
            .nth(1)
            .map(PathBuf::from)
            .or_else(neueste_aufnahme);
        let film = pfad.as_deref().and_then(sondieren);
        let app = Self {
            rahmen: mkw::Rahmen::neu(APP_ID, &[]),
            film,
            lauf: Arc::new(Laufwerk {
                bild: Mutex::new(None),
                pos_ms: AtomicU64::new(0),
                fertig: AtomicBool::new(false),
                pausiert: AtomicBool::new(false),
                generation: AtomicU64::new(0),
            }),
            kind: Arc::new(Mutex::new(None)),
            handle: None,
            handle_alt: None,
            letzte_nr: 0,
            ziehe: None,
        };
        if app.film.is_some() {
            app.abspielen(0.0);
        }
        (app, Task::none())
    }

    /// Decoder (neu) anwerfen — ab Sekunde `ab_s`. Der alte Kindprozess
    /// wird beendet; sein Faden erkennt die neue Generation und geht.
    fn abspielen(&self, ab_s: f64) {
        let Some(film) = &self.film else { return };
        let gen = self.lauf.generation.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some(mut alt) = self.kind.lock().unwrap().take() {
            let _ = alt.kill();
            let _ = alt.wait();
        }
        self.lauf.fertig.store(false, Ordering::SeqCst);
        self.lauf.pausiert.store(false, Ordering::SeqCst);
        self.lauf
            .pos_ms
            .store((ab_s * 1000.0) as u64, Ordering::SeqCst);
        let (b, h) = film.decode_masse();
        let Ok(mut kind) = Command::new("ffmpeg")
            .args(["-v", "error", "-ss"])
            .arg(format!("{ab_s:.3}"))
            .arg("-i")
            .arg(&film.pfad)
            .args(["-vf", &format!("scale={b}:{h}"), "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            return;
        };
        let rohr = kind.stdout.take();
        *self.kind.lock().unwrap() = Some(kind);
        if let Some(rohr) = rohr {
            let lauf = Arc::clone(&self.lauf);
            let fps = film.fps;
            std::thread::spawn(move || dekodieren(lauf, rohr, gen, ab_s, fps, b, h));
        }
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(m) => self.rahmen.update(m).map(Msg::Rahmen),
            Msg::Taste(t) => {
                if self.rahmen.taste(t) {
                    return Task::none();
                }
                if matches!(t, mkw::Taste::Aktivieren) {
                    return self.update(Msg::PlayPause);
                }
                Task::none()
            }
            Msg::Tick => {
                // Jüngsten Frame auf die GPU heben — nur wenn er neu ist.
                if let Some((nr, b, h, rgba)) = self.lauf.bild.lock().unwrap().as_ref() {
                    if *nr != self.letzte_nr {
                        self.letzte_nr = *nr;
                        self.handle_alt = self.handle.take();
                        self.handle = Some(iced::widget::image::Handle::from_rgba(
                            *b,
                            *h,
                            rgba.clone(),
                        ));
                    }
                }
                Task::none()
            }
            Msg::PlayPause => {
                if self.lauf.fertig.load(Ordering::SeqCst) {
                    // Zu Ende: von vorn (Replay).
                    self.abspielen(0.0);
                } else {
                    let neu = !self.lauf.pausiert.load(Ordering::SeqCst);
                    self.lauf.pausiert.store(neu, Ordering::SeqCst);
                }
                Task::none()
            }
            Msg::Ziehen(v) => {
                self.ziehe = Some(v);
                Task::none()
            }
            Msg::Springen => {
                if let Some(ziel) = self.ziehe.take() {
                    self.abspielen(ziel as f64);
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Msg> {
        let mut abos = vec![
            self.rahmen.abo().map(Msg::Rahmen),
            mkw::tasten_abo(Msg::Taste),
        ];
        // Frame-Puls nur solange gespielt wird — Pause kostet nichts.
        if self.film.is_some() && !self.lauf.pausiert.load(Ordering::SeqCst) {
            abos.push(mkw::tick("player", Duration::from_millis(33)).map(|_| Msg::Tick));
        }
        Subscription::batch(abos)
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;

        let inhalt: Element<'_, Msg> = match &self.film {
            None => mkw::leerzustand(
                mkw::symbol::PLAY_ARROW,
                "Kein Video",
                "matrix-player <Datei> — oder erst eine Bildschirmaufnahme machen (Mod+Shift+5).",
                p,
            ),
            Some(film) => {
                let dauer = film.dauer_s as f32;
                let fertig = self.lauf.fertig.load(Ordering::SeqCst);
                let pausiert = self.lauf.pausiert.load(Ordering::SeqCst);
                let pos = self
                    .ziehe
                    .unwrap_or(self.lauf.pos_ms.load(Ordering::SeqCst) as f32 / 1000.0)
                    .min(dauer);

                // Die Bühne: der Frame, mittig, auf schwarzem Samt. Der
                // Vorgänger-Frame liegt als Unterlage im Stapel: verliert
                // der Renderer das neue Bild für einen Takt (NVIDIA-
                // Textur-Wechsel), zeigt sich das vorige statt Schwarz.
                let bild = |h: &iced::widget::image::Handle| {
                    iced::widget::image(h.clone())
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .content_fit(iced::ContentFit::Contain)
                };
                let buehne: Element<'_, Msg> = match (&self.handle, &self.handle_alt) {
                    (Some(neu), Some(alt)) => iced::widget::stack![bild(alt), bild(neu)]
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into(),
                    (Some(neu), None) => bild(neu).into(),
                    _ => Space::new().width(Length::Fill).height(Length::Fill).into(),
                };
                let buehne = container(buehne)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(1.0)
                    .style(|_| container::Style {
                        background: Some(iced::Color::BLACK.into()),
                        border: iced::Border {
                            radius: mk::radius::NORMAL.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    });

                // Player-Leitbild-Zeile: Play/Pause, Zeit, Leiste, Restzeit.
                let zeichen = if fertig {
                    mkw::symbol::RESTART
                } else if pausiert {
                    mkw::symbol::PLAY_ARROW
                } else {
                    mkw::symbol::PAUSE
                };
                let knopf_sym: Element<'_, Msg> =
                    container(mkw::symbol::<Msg>(zeichen, mk::icon_size::NORMAL, p.on_surface))
                        .padding(4.0)
                        .into();
                let knopf = mouse_area(mkw::lupe(knopf_sym))
                    .on_press(Msg::PlayPause)
                    .interaction(iced::mouse::Interaction::Pointer);

                let leiste = mkw::regler(0.0..=dauer, pos, 0.05, p, Msg::Ziehen)
                    .on_release(Msg::Springen)
                    .width(Length::Fill);

                let steuer = row![
                    knopf,
                    Space::new().width(mk::spacing::M),
                    mkw::txt(
                        mk::format::dauer_mmss(pos as u64),
                        mk::typo::HINWEIS,
                        p.on_surface_variant
                    ),
                    Space::new().width(mk::spacing::M),
                    leiste,
                    Space::new().width(mk::spacing::M),
                    mkw::txt(
                        format!("\u{2212}{}", mk::format::dauer_mmss((dauer - pos).max(0.0) as u64)),
                        mk::typo::HINWEIS,
                        p.on_surface_variant
                    ),
                ]
                .align_y(iced::Alignment::Center);

                column![
                    buehne,
                    Space::new().height(mk::spacing::M),
                    steuer,
                    Space::new().height(mk::spacing::XS),
                    mkw::fusszeile(
                        format!(
                            "{} · {}×{} · {:.0} fps",
                            film.name(),
                            film.breite,
                            film.hoehe,
                            film.fps
                        ),
                        p
                    ),
                ]
                .into()
            }
        };

        let inhalt = container(inhalt)
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
            });

        self.rahmen.fenster(
            "Matrix Player",
            env!("CARGO_PKG_VERSION"),
            "Videos ansehen — Bildschirmaufnahmen zuerst. Der Decoder läuft außer Prozess, wie im Leitbild.",
            inhalt.into(),
            Msg::Rahmen,
        )
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(mut kind) = self.kind.lock().unwrap().take() {
            let _ = kind.kill();
            let _ = kind.wait();
        }
    }
}
