//! Matrix Dateien — der Dateimanager, App #25 (Runde 29).
//!
//! Der Dateimanager-Referenz-Extrakt, live am Referenzsystem abgelesen:
//!
//! * **Anatomie**: Seitenleiste (FAVORITEN / ORTE) | Inhalt mit ‹ › -Verlauf,
//!   sortierbarem Spaltenkopf (Name / Änderungsdatum / Größe / Art, Chevron
//!   zeigt die Richtung), Zeilen mit Zebra-Streifen und Akzent-Pille über die
//!   VOLLE Zeile bei Auswahl — exakt das Dunkel-Modus-Bild des Dateimanager-Referenzs.
//! * **Pfadleiste unten**: Brotkrumen, klickbar — und sie folgt der AUSWAHL
//!   (Dateimanager-Referenz-Detail: das gewählte Objekt hängt hinten dran).
//! * **Interaktion**: Einfachklick wählt, Doppelklick öffnet (Ordner rein,
//!   Dateien über den System-Öffner), Enter benennt um (die Leitbild-Eigenheit!),
//!   ↑/↓ wandern, Esc schließt Menü/Edit, Strg+R lädt frisch.
//! * **Kontextmenü** (Rechtsklick, aus dem echten Dateimanager-Referenz abgeschrieben):
//!   Öffnen | Umbenennen / Duplizieren / Pfad kopieren | In den Papierkorb
//!   legen (rot). Gelöscht wird in den Papierkorb (gio trash), nie hart.

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use iced::widget::{button, column, container, mouse_area, row, Space};
use iced::{Alignment, Element, Font, Length, Subscription, Task};
use matrixkit_theme as mk;
use matrixkit_widgets as mkw;
use mkw::color;
use std::time::Duration;

/// Album-Karten-Symbol: zwischen XLARGE und HERO — bewusstes Sondermaß.
const ALBUM_SYMBOL: f32 = 44.0;

const APP_ID: &str = "matrix-dateien";
/// Doppelklick-Fenster: das Kit-Token (R66; R33-Empirie bestätigt).
const DOPPELKLICK_MS: u128 = mk::eingabe::DOPPELKLICK_MS;

fn main() -> iced::Result {
    if !mk::fenster::einzelinstanz(APP_ID) {
        return Ok(());
    }
    if std::env::var("ICED_PRESENT_MODE").is_err() {
        std::env::set_var("ICED_PRESENT_MODE", "mailbox");
    }
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| String::from("Matrix Dateien"))
        .subscription(App::subscription)
        .window(mkw::fenster_settings(APP_ID, 920.0, 640.0))
        .font(mkw::symbol_font_laden().unwrap_or(std::borrow::Cow::Borrowed(&[])))
        .default_font(Font::with_name("Inter Variable"))
        .run()
}

// ---------------------------------------------------------------- Einträge

#[derive(Debug, Clone)]
struct Eintrag {
    name: String,
    pfad: PathBuf,
    ordner: bool,
    groesse: u64,
    geaendert: Option<SystemTime>,
}

impl Eintrag {
    fn versteckt(&self) -> bool {
        self.name.starts_with('.')
    }

    /// Die „Art"-Spalte des Dateimanager-Referenzs: menschenlesbare Typnamen.
    fn art(&self) -> &'static str {
        if self.ordner {
            return "Ordner";
        }
        let endung = self
            .name
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match endung.as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "svg" | "heic" => "Bild",
            "mp4" | "mkv" | "webm" | "mov" | "avi" => "Film",
            "mp3" | "flac" | "ogg" | "wav" | "m4a" | "opus" => "Musik",
            "pdf" => "PDF-Dokument",
            "txt" | "md" => "Text",
            "rs" => "Rust-Quelltext",
            "toml" | "json" | "kdl" | "yaml" | "yml" | "conf" | "ini" => "Konfiguration",
            "sh" | "bash" => "Shell-Skript",
            "py" => "Python-Skript",
            "zip" | "tar" | "gz" | "xz" | "zst" | "7z" => "Archiv",
            "log" => "Protokolldatei",
            "desktop" => "App-Verknüpfung",
            "iso" | "img" => "Abbild",
            "" => "Dokument",
            _ => "Dokument",
        }
    }

    /// Das Zeilen-Symbol zur Art.
    fn glyph(&self) -> char {
        if self.ordner {
            return mkw::symbol::FOLDER;
        }
        match self.art() {
            "Bild" => mkw::symbol::IMAGE,
            "Film" => mkw::symbol::MOVIE,
            "Musik" => mkw::symbol::MUSIC_NOTE,
            "PDF-Dokument" => mkw::symbol::PDF,
            "Rust-Quelltext" | "Konfiguration" | "Shell-Skript" | "Python-Skript" => {
                mkw::symbol::CODE
            }
            _ => mkw::symbol::DATEI,
        }
    }
}

/// Größen wie der Dateimanager-Referenz — de_CH-Empirie (R35): die Schweiz trennt mit
/// PUNKT („610.7 MB"), nicht mit deutschem Komma.
fn groesse_text(e: &Eintrag) -> String {
    if e.ordner {
        return String::from("—");
    }
    // R65b: die EINE Zahlensprache des Kits.
    mk::format::bytes(e.groesse)
}

/// Datum wie der Dateimanager-Referenz: „18.06.2026, 11:28".
fn datum_text(zeit: Option<SystemTime>) -> String {
    match zeit {
        Some(t) => chrono::DateTime::<chrono::Local>::from(t)
            .format("%d.%m.%Y, %H:%M")
            .to_string(),
        None => String::from("—"),
    }
}

// ------------------------------------------------------------- Sortierung

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spalte {
    Name,
    Datum,
    Groesse,
    Art,
}

fn sortieren(eintraege: &mut [Eintrag], spalte: Spalte, absteigend: bool) {
    eintraege.sort_by(|a, b| {
        let ord = match spalte {
            Spalte::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            Spalte::Datum => a.geaendert.cmp(&b.geaendert),
            Spalte::Groesse => a.groesse.cmp(&b.groesse),
            Spalte::Art => a
                .art()
                .cmp(b.art())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        };
        if absteigend { ord.reverse() } else { ord }
    });
}

fn lesen(pfad: &Path, versteckte: bool, spalte: Spalte, absteigend: bool) -> Vec<Eintrag> {
    let mut aus = Vec::new();
    if let Ok(dir) = std::fs::read_dir(pfad) {
        for e in dir.flatten() {
            let meta = match e.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let eintrag = Eintrag {
                name: e.file_name().to_string_lossy().to_string(),
                pfad: e.path(),
                ordner: meta.is_dir(),
                groesse: meta.len(),
                geaendert: meta.modified().ok(),
            };
            if !versteckte && eintrag.versteckt() {
                continue;
            }
            aus.push(eintrag);
        }
    }
    sortieren(&mut aus, spalte, absteigend);
    aus
}

// -------------------------------------------------------------- Seitenleiste

/// Favoriten wie im Dateimanager-Referenz: (Titel, Pfad relativ zu $HOME, Glyph).
const FAVORITEN: &[(&str, &str, char)] = &[
    ("Schreibtisch", "Schreibtisch", mkw::symbol::DESKTOP),
    ("Dokumente", "Dokumente", mkw::symbol::DATEI),
    ("Downloads", "Downloads", mkw::symbol::DOWNLOAD),
    ("Musik", "Musik", mkw::symbol::MUSIC_NOTE),
];

// ------------------------------------------------------------------ Galerie
//
// Runde 40 (Nutzer): Bilder und Videos erscheinen NICHT mehr getrennt —
// EIN Eintrag „Galerie". Wer ihn (oder die Ordner) betritt, sieht keine
// Dateiliste mehr, sondern eine Foto-App: Alben (= Unterordner, neue per
// „Neues Album"), Thumbnail-Raster, Filter Alle/Bilder/Videos.

fn bilder_wurzel() -> PathBuf {
    heim().join("Bilder")
}
fn videos_wurzel() -> PathBuf {
    heim().join("Videos")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Galerie {
    Keine,
    /// ~/Bilder ODER ~/Videos: die kombinierte Galerie-Wurzel.
    Wurzel,
    /// Ein Unterordner darin — ein Album.
    Album,
}

fn galerie_von(pfad: &Path) -> Galerie {
    let (b, v) = (bilder_wurzel(), videos_wurzel());
    if pfad == b || pfad == v {
        Galerie::Wurzel
    } else if pfad.starts_with(&b) || pfad.starts_with(&v) {
        Galerie::Album
    } else {
        Galerie::Keine
    }
}

/// Galerie-Thumbs im Hintergrund — das BildKachel-Familienrezept (R44):
/// 16:9-Zuschnitt, 448x252, gebackene Ecken — exakt wie der
/// Hintergrund-Bereich der Matrix Einstellungen.
fn thumbs_erzeugen(pfade: Vec<PathBuf>) -> Vec<(PathBuf, u32, u32, Vec<u8>)> {
    pfade
        .into_iter()
        .filter_map(|pfad| {
            let bild = image::open(&pfad).ok()?;
            let (w, h, roh) = mkw::bild::kachel_thumb(bild);
            Some((pfad, w, h, roh))
        })
        .collect()
}

/// Der Programme-Ordner (R48): das /Applications von Matrix. AppImage
/// hineinlegen = installiert (Start-Menü führt ihn), löschen =
/// deinstalliert. Kit-Apps wohnen derweil versiegelt im Image — exakt
/// Leitbild- Trennung /System/Applications vs. /Applications.
fn programme() -> PathBuf {
    heim().join("Programme")
}

/// AppImage starten: Ausführbit sicherstellen, direkt spawnen,
/// Kind im Thread ernten (Zombie-Lektion 7.7.).
fn appimage_starten(pfad: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(pfad) {
        let mut rechte = meta.permissions();
        if rechte.mode() & 0o111 == 0 {
            rechte.set_mode(rechte.mode() | 0o755);
            let _ = std::fs::set_permissions(pfad, rechte);
        }
    }
    if let Ok(mut kind) = std::process::Command::new(pfad).spawn() {
        std::thread::spawn(move || {
            let _ = kind.wait();
        });
    }
}

fn ist_appimage(pfad: &std::path::Path) -> bool {
    pfad.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("appimage"))
}

/// Der Papierkorb (freedesktop Trash) — Dateimanager-Referenz-Extrakt R35.
fn papierkorb() -> PathBuf {
    heim().join(".local/share/Trash/files")
}

fn heim() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| String::from("/")))
}

/// Eingehängte Datenträger (USB-Sticks etc.) unter /run/media/$USER.
fn datentraeger() -> Vec<(String, PathBuf)> {
    let nutzer = std::env::var("USER").unwrap_or_default();
    let basis = PathBuf::from("/run/media").join(nutzer);
    let mut aus = Vec::new();
    if let Ok(dir) = std::fs::read_dir(&basis) {
        for e in dir.flatten() {
            if e.path().is_dir() {
                aus.push((e.file_name().to_string_lossy().to_string(), e.path()));
            }
        }
    }
    aus.sort();
    aus
}

/// Freier Platz des Dateisystems („234,5 GB verfügbar") — über df, das
/// spricht jedes Dateisystem.
fn frei_text(pfad: &Path) -> String {
    let aus = std::process::Command::new("df")
        .args(["-h", "--output=avail"])
        .arg(pfad)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    match aus.lines().last() {
        Some(z) if !z.is_empty() => format!("{} verfügbar", z.trim()),
        _ => String::new(),
    }
}

/// Einen freien „Name Kopie[.end]"-Pfad neben dem Original finden.
fn kopie_pfad(orig: &Path) -> PathBuf {
    let stamm = orig.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let endung = orig.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
    let eltern = orig.parent().unwrap_or(Path::new("/"));
    for n in 0..99 {
        let name = if n == 0 {
            format!("{stamm} Kopie{endung}")
        } else {
            format!("{stamm} Kopie {n}{endung}")
        };
        let p = eltern.join(&name);
        if !p.exists() {
            return p;
        }
    }
    eltern.join(format!("{stamm} Kopie 99{endung}"))
}

// ------------------------------------------------------------------- App

/// Ein Zieh-Ziel (R68): Ordner (auch Sidebar-Orte, Papierkorb) oder
/// der Programme-Ordner (nimmt nur AppImages — Installation).
#[derive(Clone, Debug, PartialEq)]
enum ZiehZiel {
    Ordner(PathBuf),
    Programme,
}

/// Der laufende Zieh-Vorgang (R68, Drag-&-Drop-Grammatik).
struct Zieh {
    pfad: PathBuf,
    name: String,
    glyph: char,
    ist_appimage: bool,
    start: iced::Point,
    /// Erst nach der 4-pt-Schwelle wird der Druck zum Zug.
    aktiv: bool,
    ziel: Option<ZiehZiel>,
    /// Fürs Spring-Loading: seit wann schwebt der Zug über dem Ziel?
    schwebt_seit: Option<Instant>,
}

struct App {
    rahmen: mkw::Rahmen,
    pfad: PathBuf,
    eintraege: Vec<Eintrag>,
    auswahl: Option<usize>,
    letzter_klick: Option<(usize, Instant)>,
    zieh: Option<Zieh>,
    verlauf: Vec<PathBuf>,
    verlauf_pos: usize,
    spalte: Spalte,
    absteigend: bool,
    suche: String,
    versteckte: bool,
    /// Inline-Umbenennen: (Index, Editiertext).
    umbenennen: Option<(usize, String)>,
    /// Dateimanager-Referenz-Extrakt (R33): Endung geändert → erst der zweite Enter zählt.
    endung_bestaetigen: bool,
    /// WarnOnEmptyTrash (R35): Leeren will einen zweiten, roten Klick.
    leeren_bestaetigen: bool,
    /// Neuer Ordner im Edit (Name).
    neuer_ordner: Option<String>,
    /// Kontextmenü auf Eintrag i (Root-Overlay).
    menue: Option<usize>,
    frei: String,
    orte: Vec<(String, PathBuf)>,
    /// Galerie: fertige Thumbnails (Pfad → Handle) + Filterstufe.
    thumbs: std::collections::HashMap<PathBuf, iced::widget::image::Handle>,
    /// 0 = Alle, 1 = Bilder, 2 = Videos.
    gfilter: usize,
}

#[derive(Debug, Clone)]
enum Msg {
    Rahmen(mkw::RahmenMsg),
    Taste(mkw::Taste),
    Tick,
    Gehe(PathBuf),
    Zurueck,
    Vor,
    Hoch,
    Klick(usize),
    Rechts(usize),
    Installieren(usize),
    MenueZu,
    Suche(String),
    SucheLeeren,
    Sortiere(Spalte),
    Verborgen,
    UmbenennenStart,
    UmbenennenTipp(String),
    UmbenennenFertig,
    NeuOrdnerStart,
    NeuOrdnerTipp(String),
    NeuOrdnerFertig,
    Oeffnen(usize),
    PapierkorbLeeren,
    Loeschen(usize),
    GFilter(usize),
    ThumbsGeladen(Vec<(PathBuf, u32, u32, Vec<u8>)>),
    Duplizieren(usize),
    PfadKopieren(usize),
    /// R68: Zieh-Grammatik — Ziel betreten/verlassen, Loslassen, Spring-Puls.
    ZiehUeber(Option<ZiehZiel>),
    ZiehLoslassen,
    ZiehPuls,
}

impl App {
    fn new() -> (Self, Task<Msg>) {
        let _ = std::fs::create_dir_all(programme());
        let pfad = heim();
        let spalte = Spalte::Name;
        let eintraege = lesen(&pfad, false, spalte, false);
        let frei = frei_text(&pfad);
        let app = Self {
            rahmen: mkw::Rahmen::neu(APP_ID, &[]),
            verlauf: vec![pfad.clone()],
            verlauf_pos: 0,
            pfad,
            eintraege,
            auswahl: None,
            letzter_klick: None,
            zieh: None,
            spalte,
            absteigend: false,
            suche: String::new(),
            versteckte: false,
            umbenennen: None,
            endung_bestaetigen: false,
            leeren_bestaetigen: false,
            neuer_ordner: None,
            menue: None,
            frei,
            orte: datentraeger(),
            thumbs: std::collections::HashMap::new(),
            // Programme-Ordner (R48) existiert ab dem ersten Start.
            gfilter: 0,
        };
        (app, Task::none())
    }

    fn neu_laden(&mut self) {
        self.eintraege = if galerie_von(&self.pfad) == Galerie::Wurzel {
            // Die Galerie-Wurzel vereint ~/Bilder und ~/Videos.
            let mut alle = lesen(&bilder_wurzel(), self.versteckte, self.spalte, self.absteigend);
            alle.extend(lesen(&videos_wurzel(), self.versteckte, self.spalte, self.absteigend));
            sortieren(&mut alle, self.spalte, self.absteigend);
            alle
        } else {
            lesen(&self.pfad, self.versteckte, self.spalte, self.absteigend)
        };
        if let Some(i) = self.auswahl {
            if i >= self.sichtbare().len() {
                self.auswahl = None;
            }
        }
    }

    /// Die Zeilen nach Suchfilter — die Ansicht arbeitet NUR auf dieser Liste.
    fn sichtbare(&self) -> Vec<usize> {
        let s = self.suche.to_lowercase();
        self.eintraege
            .iter()
            .enumerate()
            .filter(|(_, e)| s.is_empty() || e.name.to_lowercase().contains(&s))
            .map(|(i, _)| i)
            .collect()
    }

    /// Fehlende Galerie-Thumbs im Hintergrund erzeugen (max 150 je Ladung).
    fn thumbs_nachladen(&self) -> Task<Msg> {
        if galerie_von(&self.pfad) == Galerie::Keine {
            return Task::none();
        }
        let fehlend: Vec<PathBuf> = self
            .eintraege
            .iter()
            .filter(|e| !e.ordner && e.art() == "Bild" && !self.thumbs.contains_key(&e.pfad))
            .map(|e| e.pfad.clone())
            .take(150)
            .collect();
        if fehlend.is_empty() {
            return Task::none();
        }
        Task::perform(async move { thumbs_erzeugen(fehlend) }, Msg::ThumbsGeladen)
    }

    fn gehe(&mut self, ziel: PathBuf) {
        if !ziel.is_dir() {
            return;
        }
        // Verlauf wie ein Browser: alles hinter der Position verfällt.
        self.verlauf.truncate(self.verlauf_pos + 1);
        self.verlauf.push(ziel.clone());
        self.verlauf_pos = self.verlauf.len() - 1;
        self.pfad = ziel;
        self.auswahl = None;
        self.suche.clear();
        self.menue = None;
        self.umbenennen = None;
        self.neuer_ordner = None;
        self.frei = frei_text(&self.pfad);
        self.leeren_bestaetigen = false;
        self.neu_laden();
    }

    fn oeffnen(&mut self, i: usize) {
        let Some(e) = self.eintraege.get(i).cloned() else { return };
        if e.ordner {
            self.gehe(e.pfad);
        } else if ist_appimage(&e.pfad) {
            // Programme starten wie das Leitbild: Doppelklick genügt (R48).
            appimage_starten(&e.pfad);
        } else {
            // System-Öffner; Kind im Thread warten (Zombie-Lektion 7.7.).
            if let Ok(mut kind) = std::process::Command::new("xdg-open").arg(&e.pfad).spawn() {
                std::thread::spawn(move || {
                    let _ = kind.wait();
                });
            }
        }
    }

    fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Rahmen(m) => {
                // R68: die fensterweite Maus des Rahmens treibt den Zug —
                // erst jenseits der 4-pt-Schwelle wird der Druck zum Zug.
                if let mkw::RahmenMsg::Maus(pt) = &m {
                    if let Some(z) = &mut self.zieh {
                        if !z.aktiv {
                            let dx = pt.x - z.start.x;
                            let dy = pt.y - z.start.y;
                            if (dx * dx + dy * dy).sqrt() > mk::eingabe::ZIEH_SCHWELLE {
                                z.aktiv = true;
                            }
                        }
                    }
                }
                return self.rahmen.update(m).map(Msg::Rahmen);
            }
            Msg::Tick => {
                self.rahmen.palette_geaendert();
                self.orte = datentraeger();
            }
            Msg::Taste(t) => {
                if self.rahmen.taste(t) {
                    return Task::none();
                }
                match t {
                    mkw::Taste::Escape => {
                        self.zieh = None;
                        self.menue = None;
                        self.umbenennen = None;
                        self.neuer_ordner = None;
                        self.endung_bestaetigen = false;
                        self.leeren_bestaetigen = false;
                    }
                    // ↑/↓ wandern durch die sichtbaren Zeilen (Runde 15).
                    mkw::Taste::Zurueck | mkw::Taste::Weiter
                        if self.umbenennen.is_none() && self.neuer_ordner.is_none() =>
                    {
                        let sicht = self.sichtbare();
                        if sicht.is_empty() {
                            return Task::none();
                        }
                        let pos = self
                            .auswahl
                            .and_then(|a| sicht.iter().position(|&i| i == a));
                        let neu = match (t, pos) {
                            (mkw::Taste::Weiter, Some(p)) if p + 1 < sicht.len() => p + 1,
                            (mkw::Taste::Weiter, None) => 0,
                            (mkw::Taste::Zurueck, Some(p)) if p > 0 => p - 1,
                            (mkw::Taste::Zurueck, None) => sicht.len() - 1,
                            (_, Some(p)) => p,
                            _ => 0,
                        };
                        self.auswahl = Some(sicht[neu]);
                    }
                    // Enter benennt um — die Leitbild-Eigenheit (nur bei Auswahl,
                    // und nicht während ein Edit läuft).
                    mkw::Taste::Aktivieren
                        if self.umbenennen.is_none()
                            && self.neuer_ordner.is_none()
                            && self.auswahl.is_some() =>
                    {
                        return self.update(Msg::UmbenennenStart);
                    }
                    mkw::Taste::Aktualisieren => {
                        self.neu_laden();
                        self.frei = frei_text(&self.pfad);
                    }
                    _ => {}
                }
            }
            Msg::Gehe(p) => {
                self.gehe(p);
                return self.thumbs_nachladen();
            }
            Msg::Zurueck => {
                if self.verlauf_pos > 0 {
                    self.verlauf_pos -= 1;
                    self.pfad = self.verlauf[self.verlauf_pos].clone();
                    self.auswahl = None;
                    self.neu_laden();
                    self.frei = frei_text(&self.pfad);
                }
            }
            Msg::Vor => {
                if self.verlauf_pos + 1 < self.verlauf.len() {
                    self.verlauf_pos += 1;
                    self.pfad = self.verlauf[self.verlauf_pos].clone();
                    self.auswahl = None;
                    self.neu_laden();
                    self.frei = frei_text(&self.pfad);
                }
            }
            Msg::Hoch => {
                if let Some(eltern) = self.pfad.parent() {
                    self.gehe(eltern.to_path_buf());
                    return self.thumbs_nachladen();
                }
            }
            Msg::Klick(i) => {
                let jetzt = Instant::now();
                let doppel = self
                    .letzter_klick
                    .is_some_and(|(li, lz)| li == i && jetzt.duration_since(lz).as_millis() < DOPPELKLICK_MS);
                self.letzter_klick = Some((i, jetzt));
                self.menue = None;
                if doppel {
                    self.zieh = None;
                    self.oeffnen(i);
                } else {
                    self.auswahl = Some(i);
                    // R68: jeder Druck ist ein Zieh-Kandidat.
                    if let Some(e) = self.eintraege.get(i) {
                        self.zieh = Some(Zieh {
                            pfad: e.pfad.clone(),
                            name: e.name.clone(),
                            glyph: e.glyph(),
                            ist_appimage: !e.ordner && ist_appimage(&e.pfad),
                            start: self.rahmen.geist.maus,
                            aktiv: false,
                            ziel: None,
                            schwebt_seit: None,
                        });
                    }
                }
            }
            Msg::Rechts(i) => {
                self.auswahl = Some(i);
                self.menue = Some(i);
            }
            Msg::MenueZu => self.menue = None,
            // R48: „In Programme installieren" — die Leitbild-Geste als Klick.
            Msg::Installieren(i) => {
                self.menue = None;
                if let Some(e) = self.eintraege.get(i).cloned() {
                    let ziel = programme().join(e.pfad.file_name().unwrap_or_default());
                    let _ = std::fs::create_dir_all(programme());
                    // rename zuerst (gleiche Platte), sonst kopieren+löschen.
                    if std::fs::rename(&e.pfad, &ziel).is_err() {
                        if std::fs::copy(&e.pfad, &ziel).is_ok() {
                            let _ = std::fs::remove_file(&e.pfad);
                        }
                    }
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = std::fs::metadata(&ziel) {
                        let mut rechte = meta.permissions();
                        rechte.set_mode(rechte.mode() | 0o755);
                        let _ = std::fs::set_permissions(&ziel, rechte);
                    }
                    // Leitbild-Moment: hinspringen und das Ergebnis zeigen.
                    self.gehe(programme());
                }
            }
            Msg::ZiehUeber(ziel) => {
                if let Some(z) = &mut self.zieh {
                    if z.aktiv {
                        // Programme nimmt nur AppImages; nichts zieht auf sich selbst.
                        let ziel = match ziel {
                            Some(ZiehZiel::Programme) if !z.ist_appimage => None,
                            Some(ZiehZiel::Ordner(p)) if p == z.pfad => None,
                            andere => andere,
                        };
                        if ziel != z.ziel {
                            z.schwebt_seit = ziel.is_some().then(Instant::now);
                            z.ziel = ziel;
                        }
                    }
                }
            }
            Msg::ZiehPuls => {
                // Spring-Loading (0,5 s, Leitbild-Empirie): der Ordner öffnet
                // sich unterm schwebenden Zug — der Zug lebt weiter.
                let spring = self.zieh.as_ref().and_then(|z| {
                    if !z.aktiv {
                        return None;
                    }
                    match (&z.ziel, z.schwebt_seit) {
                        (Some(ZiehZiel::Ordner(p)), Some(seit))
                            if seit.elapsed().as_millis() as u64
                                >= mk::eingabe::SPRING_LADEN_MS =>
                        {
                            Some(p.clone())
                        }
                        _ => None,
                    }
                });
                if let Some(pfad) = spring {
                    if let Some(z) = &mut self.zieh {
                        z.ziel = None;
                        z.schwebt_seit = None;
                    }
                    self.gehe(pfad);
                    return self.thumbs_nachladen();
                }
            }
            Msg::ZiehLoslassen => {
                if let Some(z) = self.zieh.take() {
                    if z.aktiv {
                        match z.ziel {
                            Some(ZiehZiel::Programme) => {
                                // Installation per Zug — dieselbe Mechanik
                                // wie das Kontextmenü (R48).
                                let ziel = programme()
                                    .join(z.pfad.file_name().unwrap_or_default());
                                let _ = std::fs::create_dir_all(programme());
                                if std::fs::rename(&z.pfad, &ziel).is_err() {
                                    if std::fs::copy(&z.pfad, &ziel).is_ok() {
                                        let _ = std::fs::remove_file(&z.pfad);
                                    }
                                }
                                use std::os::unix::fs::PermissionsExt;
                                if let Ok(meta) = std::fs::metadata(&ziel) {
                                    let mut rechte = meta.permissions();
                                    rechte.set_mode(rechte.mode() | 0o755);
                                    let _ = std::fs::set_permissions(&ziel, rechte);
                                }
                                self.gehe(programme());
                                return self.thumbs_nachladen();
                            }
                            Some(ZiehZiel::Ordner(ordner)) => {
                                let ziel = ordner.join(&z.name);
                                // rename zuerst (gleiche Platte), sonst
                                // kopieren+löschen (nur Dateien).
                                if std::fs::rename(&z.pfad, &ziel).is_err()
                                    && !z.pfad.is_dir()
                                    && std::fs::copy(&z.pfad, &ziel).is_ok()
                                {
                                    let _ = std::fs::remove_file(&z.pfad);
                                }
                                self.neu_laden();
                                self.auswahl = None;
                            }
                            None => {}
                        }
                    }
                }
            }
            Msg::Suche(s) => {
                self.suche = s;
                self.auswahl = None;
            }
            Msg::SucheLeeren => self.suche.clear(),
            Msg::Sortiere(sp) => {
                if self.spalte == sp {
                    self.absteigend = !self.absteigend;
                } else {
                    self.spalte = sp;
                    self.absteigend = false;
                }
                self.neu_laden();
            }
            Msg::Verborgen => {
                self.versteckte = !self.versteckte;
                self.neu_laden();
            }
            Msg::UmbenennenStart => {
                if let Some(i) = self.auswahl {
                    if let Some(e) = self.eintraege.get(i) {
                        self.umbenennen = Some((i, e.name.clone()));
                        self.menue = None;
                    }
                }
            }
            Msg::UmbenennenTipp(s) => {
                if let Some((_, text)) = &mut self.umbenennen {
                    *text = s;
                }
                self.endung_bestaetigen = false;
            }
            Msg::UmbenennenFertig => {
                // Dateimanager-Referenz-Extrakt (R33, FXEnableExtensionChangeWarning): wer
                // die ENDUNG ändert, meint es vielleicht nicht so — der
                // erste Enter warnt, erst der zweite benennt um.
                if let Some((i, text)) = self.umbenennen.clone() {
                    let neu = text.trim().to_string();
                    let endung = |n: &str| n.rsplit_once('.').map(|(_, e)| e.to_lowercase());
                    let alt_endung = self.eintraege.get(i).and_then(|e| endung(&e.name));
                    if !self.endung_bestaetigen
                        && self.eintraege.get(i).is_some_and(|e| e.name != neu)
                        && alt_endung != endung(&neu)
                    {
                        self.endung_bestaetigen = true;
                        return Task::none();
                    }
                }
                self.endung_bestaetigen = false;
                if let Some((i, neu)) = self.umbenennen.take() {
                    let neu = neu.trim();
                    if let Some(e) = self.eintraege.get(i) {
                        if !neu.is_empty() && neu != e.name && !neu.contains('/') {
                            let ziel = self.pfad.join(neu);
                            if !ziel.exists() {
                                let _ = std::fs::rename(&e.pfad, &ziel);
                            }
                        }
                    }
                    self.neu_laden();
                }
            }
            Msg::NeuOrdnerStart => {
                self.neuer_ordner = Some(String::from("Neuer Ordner"));
            }
            Msg::NeuOrdnerTipp(s) => self.neuer_ordner = Some(s),
            Msg::NeuOrdnerFertig => {
                if let Some(name) = self.neuer_ordner.take() {
                    let name = name.trim();
                    if !name.is_empty() && !name.contains('/') {
                        let _ = std::fs::create_dir(self.pfad.join(name));
                        self.neu_laden();
                    }
                }
            }
            Msg::Oeffnen(i) => {
                self.menue = None;
                self.oeffnen(i);
                return self.thumbs_nachladen();
            }
            Msg::PapierkorbLeeren => {
                // WarnOnEmptyTrash (Dateimanager-Referenz-Default AN): erst warnen, dann
                // leeren — und der Papierkorb-Klang (10) besiegelt es.
                if !self.leeren_bestaetigen {
                    self.leeren_bestaetigen = true;
                } else {
                    self.leeren_bestaetigen = false;
                    let _ = mk::befehl::still("gio", &["trash", "--empty"]);
                    std::thread::spawn(|| {
                        mk::feedback::jetzt("papierkorb", "10-papierkorb.wav");
                    });
                    self.neu_laden();
                }
            }
            Msg::Loeschen(i) => {
                // In den Papierkorb, nie hart löschen (gio spricht Trash).
                if let Some(e) = self.eintraege.get(i) {
                    let _ = mk::befehl::still("gio", &["trash", &e.pfad.to_string_lossy()]);
                    // Der Dateimanager-Referenz-Plopp (R37) — fehlt die Datei (altes
                    // Image), bleibt es einfach still.
                    std::thread::spawn(|| {
                        mk::feedback::jetzt("papierkorb", "15-wurf.wav");
                    });
                }
                self.menue = None;
                self.auswahl = None;
                self.neu_laden();
            }
            Msg::Duplizieren(i) => {
                if let Some(e) = self.eintraege.get(i) {
                    let ziel = kopie_pfad(&e.pfad);
                    let _ = mk::befehl::still(
                        "cp",
                        &["-a", &e.pfad.to_string_lossy(), &ziel.to_string_lossy()],
                    );
                }
                self.menue = None;
                self.neu_laden();
            }
            Msg::GFilter(stufe) => self.gfilter = stufe.min(2),
            Msg::ThumbsGeladen(fertig) => {
                for (pfad, w, h, rgba) in fertig {
                    self.thumbs
                        .insert(pfad, iced::widget::image::Handle::from_rgba(w, h, rgba));
                }
                // Verdunstung gegen RAM-Fraß: jede Kachel wiegt ~450 KB
                // (448x252 RGBA). Beim Wandern durch Alben nur behalten,
                // was der aktuelle Ordner zeigt — sonst wächst die Map
                // grenzenlos.
                if self.thumbs.len() > 240 {
                    let aktuelle: std::collections::HashSet<PathBuf> =
                        self.eintraege.iter().map(|e| e.pfad.clone()).collect();
                    self.thumbs.retain(|k, _| aktuelle.contains(k));
                }
            }
            Msg::PfadKopieren(i) => {
                if let Some(e) = self.eintraege.get(i) {
                    let _ = std::process::Command::new("wl-copy")
                        .arg("--")
                        .arg(e.pfad.as_os_str())
                        .spawn()
                        .map(|mut kind| {
                            std::thread::spawn(move || {
                                let _ = kind.wait();
                            })
                        });
                }
                self.menue = None;
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Msg> {
        let mut alle = vec![
            self.rahmen.abo().map(Msg::Rahmen),
            mkw::tick("dateien", Duration::from_secs(4)).map(|_| Msg::Tick),
            mkw::tasten_abo(Msg::Taste),
        ];
        // R68: nur während eines Zugs — Loslass-Ohr und Spring-Puls.
        if self.zieh.is_some() {
            alle.push(iced::event::listen_with(|ereignis, _, _| {
                matches!(
                    ereignis,
                    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                        iced::mouse::Button::Left
                    )) | iced::Event::Touch(iced::touch::Event::FingerLifted { .. })
                )
                .then_some(Msg::ZiehLoslassen)
            }));
        }
        if self.zieh.as_ref().is_some_and(|z| z.aktiv) {
            alle.push(
                mkw::tick("dateien-zieh", Duration::from_millis(100)).map(|_| Msg::ZiehPuls),
            );
        }
        Subscription::batch(alle)
    }

    // -------------------------------------------------------- Seitenleiste

    fn seitenpunkt<'a>(
        &self,
        glyph: char,
        titel: &'a str,
        ziel: PathBuf,
    ) -> Element<'a, Msg> {
        // DIESELBE Family wie die Hilfe-Sidebar (Nutzer, 14.7.): der
        // Kit-Baustein sidebar_eintrag — identische Höhe, Paletten-Pille
        // (primary_container) fürs Aktive, gleicher Hover, gleiche Lupe.
        // R68: Sidebar-Orte sind Zieh-Ziele; Programme nimmt AppImages.
        let zieh_ziel = if ziel == programme() {
            ZiehZiel::Programme
        } else {
            ZiehZiel::Ordner(ziel.clone())
        };
        let zieh_hier = self.zieh.as_ref().is_some_and(|z| {
            z.aktiv && z.ziel.as_ref() == Some(&zieh_ziel)
        });
        mouse_area(mkw::sidebar_eintrag(
            mkw::SidebarPunkt { zeichen: glyph, titel, anzahl: None },
            self.pfad == ziel || zieh_hier,
            false,
            Msg::Gehe(ziel),
            self.rahmen.palette,
        ))
        .on_enter(Msg::ZiehUeber(Some(zieh_ziel)))
        .on_exit(Msg::ZiehUeber(None))
        .into()
    }

    fn seitenleiste(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let heim = heim();
        let mut spalte = column![
            mkw::txt("FAVORITEN", mk::typo::ETIKETT, p.on_surface_variant),
        ]
        .spacing(2);
        for (titel, rel, glyph) in FAVORITEN {
            spalte = spalte.push(self.seitenpunkt(*glyph, titel, heim.join(rel)));
        }
        // Galerie statt Bilder+Videos (R40) — aktiv im GANZEN Galerie-Bereich.
        spalte = spalte.push(mkw::sidebar_eintrag(
            mkw::SidebarPunkt { zeichen: mkw::symbol::IMAGE, titel: "Galerie", anzahl: None },
            galerie_von(&self.pfad) != Galerie::Keine,
            false,
            Msg::Gehe(bilder_wurzel()),
            self.rahmen.palette,
        ));
        spalte = spalte.push(Space::new().height(mk::spacing::M));
        spalte = spalte.push(mkw::txt("ORTE", mk::typo::ETIKETT, p.on_surface_variant));
        spalte = spalte.push(self.seitenpunkt(mkw::symbol::HOME, "Zuhause", heim.clone()));
        spalte = spalte.push(self.seitenpunkt(mkw::symbol::APPS, "Programme", programme()));
        spalte = spalte.push(self.seitenpunkt(mkw::symbol::STORAGE, "Matrix", PathBuf::from("/")));
        spalte = spalte.push(self.seitenpunkt(mkw::symbol::DELETE, "Papierkorb", papierkorb()));
        for (name, pfad) in &self.orte {
            spalte = spalte.push(self.seitenpunkt(mkw::symbol::USB, name.as_str(), pfad.clone()));
        }
        // DERSELBE Sidebar-Grund wie Matrix Hilfe (Familien-Baustein).
        let _ = p;
        mkw::sidebar_flaeche(spalte.into(), mkw::ui::SIDEBAR_BREITE, self.rahmen.palette)
    }

    // ------------------------------------------------------------- Inhalt

    fn werkzeugleiste(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let titel = self
            .pfad
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| String::from("Matrix"));
        let mut rechts: Vec<Element<'_, Msg>> = Vec::new();
        if self.pfad == papierkorb() && !self.eintraege.is_empty() {
            rechts.push(mkw::lupe(mkw::knopf(
                if self.leeren_bestaetigen { "Wirklich leeren?" } else { "Leeren …" },
                mkw::knopfart::Stil::Getoent,
                mkw::knopfart::Rolle::Destruktiv,
                mkw::knopfart::Groesse::Klein,
                p,
                Some(Msg::PapierkorbLeeren),
            )));
        }
        rechts.push(mkw::ui::werkzeug_knopf(mkw::symbol::NEUER_ORDNER, Some(Msg::NeuOrdnerStart), p));
        rechts.push(mkw::ui::werkzeug_knopf(mkw::symbol::VISIBILITY_OFF, Some(Msg::Verborgen), p));
        // Die Werkzeugleisten-Familie (mkw::ui) — Dateien war ihr Erstgeborener.
        mkw::ui::werkzeugleiste(
            titel,
            (self.verlauf_pos > 0).then_some(Msg::Zurueck),
            (self.verlauf_pos + 1 < self.verlauf.len()).then_some(Msg::Vor),
            vec![mkw::ui::werkzeug_knopf(
                mkw::symbol::ARROW_UPWARD,
                self.pfad.parent().map(|_| Msg::Hoch),
                p,
            )],
            rechts,
            &self.suche,
            Msg::Suche,
            Msg::SucheLeeren,
            p,
        )
    }

    fn spaltenkopf(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let kopf = |titel: &'static str, sp: Spalte, breite: Option<f32>| -> Element<'_, Msg> {
            let aktiv = self.spalte == sp;
            let mut zeile = row![mkw::txt(
                titel,
                mk::typo::KLEIN,
                if aktiv { p.on_surface } else { p.on_surface_variant },
            )]
            .spacing(2)
            .align_y(Alignment::Center);
            if aktiv {
                zeile = zeile.push(mkw::symbol::<Msg>(
                    if self.absteigend { mkw::symbol::ARROW_DOWNWARD } else { mkw::symbol::ARROW_UPWARD },
                    11.0,
                    p.on_surface_variant,
                ));
            }
            let b = button(zeile)
                .padding(iced::Padding { left: 4.0, right: 4.0, top: 2.0, bottom: 2.0 })
                .style(move |_, status| mkw::leiste::knopf_stil(p, status, mk::radius::KLEIN))
                .on_press(Msg::Sortiere(sp));
            match breite {
                Some(w) => container(b).width(Length::Fixed(w)).into(),
                None => container(b).width(Length::Fill).into(),
            }
        };
        container(
            row![
                Space::new().width(Length::Fixed(30.0)),
                kopf("Name", Spalte::Name, None),
                kopf("Änderungsdatum", Spalte::Datum, Some(160.0)),
                kopf("Größe", Spalte::Groesse, Some(84.0)),
                kopf("Art", Spalte::Art, Some(120.0)),
            ]
            .align_y(Alignment::Center),
        )
        .padding(iced::Padding { bottom: 2.0, ..iced::Padding::ZERO })
        .into()
    }

    fn zeile(&self, i: usize, zebra: bool) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let e = &self.eintraege[i];
        let gewaehlt = self.auswahl == Some(i);
        let (vorne, neben) = if gewaehlt {
            (p.on_primary, p.on_primary)
        } else {
            (p.on_surface, p.on_surface_variant)
        };

        // Name — oder das Inline-Umbenennen-Feld (Enter bestätigt).
        let name: Element<'_, Msg> = match &self.umbenennen {
            Some((ui, text)) if *ui == i => mkw::eingabefeld(
                "Name",
                text,
                Msg::UmbenennenTipp,
                Some(Msg::UmbenennenFertig),
                false,
                p,
            ),
            _ => mkw::txt(&e.name, mk::typo::KLEIN, vorne).into(),
        };

        let inhalt = row![
            container(mkw::symbol::<Msg>(e.glyph(), mk::icon_size::SMALL, if gewaehlt { vorne } else { p.primary }))
                .width(Length::Fixed(30.0)),
            container(name).width(Length::Fill),
            container(mkw::txt(datum_text(e.geaendert), mk::typo::KLEIN, neben))
                .width(Length::Fixed(160.0)),
            container(
                container(mkw::txt(groesse_text(e), mk::typo::KLEIN, neben))
                    .align_x(iced::alignment::Horizontal::Right)
                    .width(Length::Fill)
                    .padding(iced::Padding { right: mk::spacing::M, ..iced::Padding::ZERO })
            )
            .width(Length::Fixed(84.0)),
            container(mkw::txt(e.art(), mk::typo::KLEIN, neben)).width(Length::Fixed(120.0)),
        ]
        .align_y(Alignment::Center);

        // R68: schwebt ein Zug über diesem Ordner, leuchtet der Ring.
        let zieh_hier = self.zieh.as_ref().is_some_and(|z| {
            z.aktiv && z.ziel == Some(ZiehZiel::Ordner(e.pfad.clone()))
        });
        // Dateimanager-Referenz-Bild: Auswahl = Akzent-Pille volle Zeile; sonst Zebra.
        let flaeche = container(inhalt)
            .width(Length::Fill)
            .padding(iced::Padding { left: 6.0, right: 6.0, top: 4.0, bottom: 4.0 })
            .style(move |_| container::Style {
                background: if gewaehlt {
                    Some(color(p.primary).into())
                } else if zieh_hier {
                    Some(color(p.primary.over(p.surface_container_high, 0.18)).into())
                } else if zebra {
                    Some(color(p.on_surface.over(p.surface_container_high, 0.04)).into())
                } else {
                    None
                },
                border: iced::Border {
                    radius: mk::radius::KLEIN.into(),
                    width: if zieh_hier { 2.0 } else { 0.0 },
                    color: color(p.primary),
                },
                ..Default::default()
            });

        let zieh_ziel = e
            .ordner
            .then(|| ZiehZiel::Ordner(e.pfad.clone()));
        mouse_area(flaeche)
            .on_press(Msg::Klick(i))
            .on_right_press(Msg::Rechts(i))
            .on_enter(Msg::ZiehUeber(zieh_ziel))
            .on_exit(Msg::ZiehUeber(None))
            .into()
    }

    fn pfadleiste(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let mut zeile = row![].spacing(2).align_y(Alignment::Center);
        let mut bisher = PathBuf::from("/");
        let mut segmente: Vec<(String, PathBuf)> = vec![(String::from("Matrix"), bisher.clone())];
        for teil in self.pfad.components() {
            if let std::path::Component::Normal(n) = teil {
                bisher = bisher.join(n);
                segmente.push((n.to_string_lossy().to_string(), bisher.clone()));
            }
        }
        let n = segmente.len();
        for (idx, (name, ziel)) in segmente.into_iter().enumerate() {
            if idx > 0 {
                zeile = zeile.push(mkw::txt("›", mk::typo::ETIKETT, p.outline));
            }
            let letzte = idx + 1 == n;
            zeile = zeile.push(
                button(mkw::txt(
                    name,
                    mk::typo::ETIKETT,
                    if letzte { p.on_surface } else { p.on_surface_variant },
                ))
                .padding(iced::Padding { left: 4.0, right: 4.0, top: 1.0, bottom: 1.0 })
                .style(move |_, status| mkw::leiste::knopf_stil(p, status, mk::radius::KLEIN))
                .on_press(Msg::Gehe(ziel)),
            );
        }
        // Dateimanager-Referenz-Detail: die Auswahl hängt hinten an der Pfadleiste.
        if let Some(e) = self.auswahl.and_then(|i| self.eintraege.get(i)) {
            zeile = zeile.push(mkw::txt("›", mk::typo::ETIKETT, p.outline));
            zeile = zeile.push(mkw::txt(&e.name, mk::typo::ETIKETT, p.on_surface));
        }
        container(zeile).width(Length::Fill).into()
    }

    /// Kontextmenü als Root-Overlay — die Dateimanager-Referenz-Einträge in der EINEN
    /// Menü-Sprache der MenuFamily.
    fn kontext(&self, i: usize) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let ordner = self.eintraege.get(i).is_some_and(|e| e.ordner);
        let installierbar = self
            .eintraege
            .get(i)
            .is_some_and(|e| !e.ordner && ist_appimage(&e.pfad) && !e.pfad.starts_with(programme()));
        let mut eintraege: Vec<mkw::ui::MenuEintrag<Msg>> = vec![
            mkw::ui::MenuEintrag::Punkt {
                zeichen: Some(if ordner { mkw::symbol::FOLDER } else { mkw::symbol::PLAY_ARROW }),
                titel: String::from("Öffnen"),
                farbe: None,
                msg: Msg::Oeffnen(i),
            },
            mkw::ui::MenuEintrag::Trenner,
            mkw::ui::MenuEintrag::Punkt {
                zeichen: None,
                titel: String::from("Umbenennen"),
                farbe: None,
                msg: Msg::UmbenennenStart,
            },
            mkw::ui::MenuEintrag::Punkt {
                zeichen: None,
                titel: String::from("Duplizieren"),
                farbe: None,
                msg: Msg::Duplizieren(i),
            },
            mkw::ui::MenuEintrag::Punkt {
                zeichen: None,
                titel: String::from("Pfad kopieren"),
                farbe: None,
                msg: Msg::PfadKopieren(i),
            },
            mkw::ui::MenuEintrag::Trenner,
            mkw::ui::MenuEintrag::Punkt {
                zeichen: Some(mkw::symbol::DELETE),
                titel: String::from("In den Papierkorb legen"),
                farbe: Some(p.error),
                msg: Msg::Loeschen(i),
            },
        ];
        if installierbar {
            eintraege.insert(
                1,
                mkw::ui::MenuEintrag::Punkt {
                    zeichen: Some(mkw::symbol::APPS),
                    titel: String::from("In Programme installieren"),
                    farbe: None,
                    msg: Msg::Installieren(i),
                },
            );
        }
        mouse_area(
            container(mkw::ui::menu_family(None, eintraege, p))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .on_press(Msg::MenueZu)
        .on_right_press(Msg::MenueZu)
        .into()
    }

    /// Eine Album-Karte (Ordner in der Galerie): großer Ordner, Name.
    fn album_karte(&self, i: usize) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let e = &self.eintraege[i];
        let gewaehlt = self.auswahl == Some(i);
        let inhalt = column![
            mkw::symbol::<Msg>(mkw::symbol::FOLDER, ALBUM_SYMBOL, p.primary),
            mkw::txt(&e.name, mk::typo::KLEIN, p.on_surface),
        ]
        .spacing(mk::spacing::XXS)
        .align_x(Alignment::Center);
        mouse_area(
            container(inhalt)
                .width(Length::Fixed(110.0))
                .padding(mk::spacing::S)
                .style(move |_| container::Style {
                    background: Some(color(if gewaehlt {
                        p.primary.over(p.surface_container_high, 0.14)
                    } else {
                        p.surface_container_high
                    }).into()),
                    border: iced::Border {
                        radius: mk::radius::NORMAL.into(),
                        width: if gewaehlt { 1.5 } else { 0.0 },
                        color: color(p.primary),
                    },
                    ..Default::default()
                }),
        )
        .on_press(Msg::Klick(i))
        .on_right_press(Msg::Rechts(i))
        .into()
    }

    /// Eine Medien-Kachel: Bild-Thumb (sobald geladen) oder Film-Karte.
    fn medien_kachel(&self, i: usize) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let e = &self.eintraege[i];
        let gewaehlt = self.auswahl == Some(i);
        // BildKachel-Familie (mkw::ui): dieselbe Anatomie wie der
        // Hintergrund-Bereich der Einstellungen — 112x63, gebackene Ecken.
        let kachel = mkw::ui::bild_kachel(
            self.thumbs.get(&e.pfad).cloned(),
            if e.art() == "Film" { mkw::symbol::MOVIE } else { mkw::symbol::IMAGE },
            e.name.clone(),
            gewaehlt,
            None,
            p,
        );
        mouse_area(container(kachel).padding(2))
            .on_press(Msg::Klick(i))
            .on_right_press(Msg::Rechts(i))
            .into()
    }

    /// Die Galerie-Ansicht (R40): Alben oben, Medienraster darunter —
    /// keine Dateiliste. Filter: Alle / Bilder / Videos.
    fn galerie_ansicht(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;
        let sicht = self.sichtbare();

        // Kopf: Filter-Segmente + Neues Album.
        let mut kopf = row![
            container(mkw::segmente(
                &["Alle", "Bilder", "Videos"],
                self.gfilter,
                Msg::GFilter,
                p,
            ))
            .width(Length::Fixed(260.0)),
            Space::new().width(Length::Fill),
            mkw::knopf(
                "Neues Album",
                mkw::knopfart::Stil::Getoent,
                mkw::knopfart::Rolle::Normal,
                mkw::knopfart::Groesse::Klein,
                p,
                Some(Msg::NeuOrdnerStart),
            ),
        ]
        .spacing(mk::spacing::S)
        .align_y(Alignment::Center);
        let _ = &mut kopf;

        let mut spalte = column![kopf].spacing(mk::spacing::M);

        // Album anlegen / Umbenennen — Inline-Eingaben der Galerie.
        if let Some(name) = &self.neuer_ordner {
            spalte = spalte.push(
                row![
                    mkw::txt("Neues Album:", mk::typo::KLEIN, p.on_surface_variant),
                    container(mkw::eingabefeld(
                        "Albumname",
                        name,
                        Msg::NeuOrdnerTipp,
                        Some(Msg::NeuOrdnerFertig),
                        false,
                        p,
                    ))
                    .width(Length::Fixed(240.0)),
                ]
                .spacing(mk::spacing::S)
                .align_y(Alignment::Center),
            );
        }
        if let Some((i, text)) = &self.umbenennen {
            if self.eintraege.get(*i).is_some() {
                spalte = spalte.push(
                    row![
                        mkw::txt("Umbenennen:", mk::typo::KLEIN, p.on_surface_variant),
                        container(mkw::eingabefeld(
                            "Name",
                            text,
                            Msg::UmbenennenTipp,
                            Some(Msg::UmbenennenFertig),
                            false,
                            p,
                        ))
                        .width(Length::Fixed(240.0)),
                    ]
                    .spacing(mk::spacing::S)
                    .align_y(Alignment::Center),
                );
            }
        }

        // Alben (Ordner) als Karten-Reihe.
        let alben: Vec<usize> = sicht
            .iter()
            .copied()
            .filter(|&i| self.eintraege[i].ordner)
            .collect();
        if !alben.is_empty() {
            spalte = spalte.push(mkw::txt("ALBEN", mk::typo::ETIKETT, p.on_surface_variant));
            let mut reihe = row![].spacing(mk::spacing::S);
            let mut gitter = column![].spacing(mk::spacing::S);
            for (n, i) in alben.iter().enumerate() {
                reihe = reihe.push(self.album_karte(*i));
                if (n + 1) % 5 == 0 {
                    gitter = gitter.push(reihe);
                    reihe = row![].spacing(mk::spacing::S);
                }
            }
            gitter = gitter.push(reihe);
            spalte = spalte.push(gitter);
        }

        // Medien nach Filter.
        let medien: Vec<usize> = sicht
            .iter()
            .copied()
            .filter(|&i| {
                let e = &self.eintraege[i];
                if e.ordner {
                    return false;
                }
                match self.gfilter {
                    1 => e.art() == "Bild",
                    2 => e.art() == "Film",
                    _ => e.art() == "Bild" || e.art() == "Film",
                }
            })
            .collect();
        if !medien.is_empty() {
            spalte = spalte.push(mkw::txt(
                match self.gfilter {
                    1 => "BILDER",
                    2 => "VIDEOS",
                    _ => "MEDIEN",
                },
                mk::typo::ETIKETT,
                p.on_surface_variant,
            ));
            let mut reihe = row![].spacing(mk::spacing::S);
            let mut gitter = column![].spacing(mk::spacing::S);
            for (n, i) in medien.iter().enumerate() {
                reihe = reihe.push(self.medien_kachel(*i));
                if (n + 1) % 5 == 0 {
                    gitter = gitter.push(reihe);
                    reihe = row![].spacing(mk::spacing::S);
                }
            }
            gitter = gitter.push(reihe);
            spalte = spalte.push(gitter);
        } else if alben.is_empty() {
            spalte = spalte.push(
                container(mkw::txt(
                    "Noch keine Bilder oder Videos — lege welche in die Galerie.",
                    mk::typo::KLEIN,
                    p.on_surface_variant,
                ))
                .center_x(Length::Fill)
                .padding(mk::spacing::XL),
            );
        }

        spalte.into()
    }

    fn view(&self) -> Element<'_, Msg> {
        let p = self.rahmen.palette;

        // Zeilen (gefiltert), Zebra über die SICHTBARE Position.
        let sicht = self.sichtbare();
        let mut liste = column![].spacing(1);
        // „Neuer Ordner"-Edit als erste Zeile.
        if let Some(name) = &self.neuer_ordner {
            liste = liste.push(
                row![
                    container(mkw::symbol::<Msg>(mkw::symbol::FOLDER, mk::icon_size::SMALL, p.primary))
                        .width(Length::Fixed(30.0)),
                    mkw::eingabefeld(
                        "Neuer Ordner",
                        name,
                        Msg::NeuOrdnerTipp,
                        Some(Msg::NeuOrdnerFertig),
                        false,
                        p,
                    ),
                ]
                .align_y(Alignment::Center),
            );
        }
        for (pos, i) in sicht.iter().enumerate() {
            liste = liste.push(self.zeile(*i, pos % 2 == 1));
        }
        if sicht.is_empty() && self.neuer_ordner.is_none() {
            liste = liste.push(
                container(mkw::txt(
                    if self.suche.is_empty() { "Dieser Ordner ist leer" } else { "Keine Treffer" },
                    mk::typo::KLEIN,
                    p.on_surface_variant,
                ))
                .center_x(Length::Fill)
                .padding(mk::spacing::XL),
            );
        }

        // Galerie-Bereich (R40): Foto-App statt Dateiliste.
        let inhalt = if galerie_von(&self.pfad) != Galerie::Keine {
            column![
                self.werkzeugleiste(),
                Space::new().height(mk::spacing::S),
                self.rahmen.scrollflaeche(self.galerie_ansicht(), Msg::Rahmen),
                Space::new().height(mk::spacing::XS),
                self.pfadleiste(),
            ]
        } else {
            column![
                self.werkzeugleiste(),
                Space::new().height(mk::spacing::S),
                self.spaltenkopf(),
                self.rahmen.scrollflaeche(liste.into(), Msg::Rahmen),
                Space::new().height(mk::spacing::XS),
                self.pfadleiste(),
            ]
        };

        // Die SidebarFamily-Anatomie wie in Matrix Hilfe: dunkle Leiste
        // nackt am Rand, rechts DIESELBE helle Detail-Karte (Familien-
        // Baustein detail_karte) — die Fußzeile wohnt in der Karte.
        let detail = column![
            inhalt,
            mkw::fusszeile(
                if self.leeren_bestaetigen {
                    String::from("Papierkorb unwiderruflich leeren? Noch ein Klick bestätigt — Esc bricht ab")
                } else if self.endung_bestaetigen {
                    String::from("Dateiendung geändert — Enter bestätigt, Esc bricht ab")
                } else {
                    format!(
                        "{} Objekte · {}",
                        self.sichtbare().len(),
                        if self.frei.is_empty() { String::from("—") } else { self.frei.clone() }
                    )
                },
                p,
            ),
        ]
        .spacing(mk::spacing::S);

        let karte = row![
            self.seitenleiste(),
            mkw::ui::detail_karte(detail.into(), p),
        ]
        .height(Length::Fill);

        // R68: der Zieh-Geist schwebt mit ~0,7 Deckkraft unterm Zeiger.
        let karte: Element<'_, Msg> = if let Some(z) = self.zieh.as_ref().filter(|z| z.aktiv) {
            let maus = self.rahmen.geist.maus;
            let geist = container(
                container(
                    row![
                        mkw::symbol::<Msg>(z.glyph, mk::icon_size::SMALL, p.primary),
                        mkw::txt(&z.name, mk::typo::KLEIN, p.on_surface),
                    ]
                    .spacing(mk::spacing::XS)
                    .align_y(Alignment::Center),
                )
                .padding(iced::Padding {
                    top: 4.0,
                    right: mk::spacing::S,
                    bottom: 4.0,
                    left: mk::spacing::S,
                })
                .style(move |_| container::Style {
                    background: Some(
                        color(p.surface_container_high.mit_alpha(mk::eingabe::ZIEH_GEIST_ALPHA))
                            .into(),
                    ),
                    border: iced::Border {
                        radius: mk::radius::KLEIN.into(),
                        width: 1.0,
                        color: color(p.outline.mit_alpha(0.25)),
                    },
                    ..Default::default()
                }),
            )
            .padding(iced::Padding {
                left: (maus.x - 10.0).max(0.0),
                top: (maus.y - 52.0).max(0.0),
                ..iced::Padding::ZERO
            });
            iced::widget::stack![karte, geist].into()
        } else {
            karte.into()
        };
        let root = self.menue.map(|i| self.kontext(i));
        self.rahmen.huelle("Matrix Dateien", karte, root, Msg::Rahmen)
    }
}

// ------------------------------------------------------------------ Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(name: &str, ordner: bool, groesse: u64) -> Eintrag {
        Eintrag {
            name: name.into(),
            pfad: PathBuf::from("/tmp").join(name),
            ordner,
            groesse,
            geaendert: None,
        }
    }

    #[test]
    fn groessen_lesen_sich_wie_im_finder() {
        assert_eq!(groesse_text(&probe("a", false, 610_700_000)), "610.7 MB");
        assert_eq!(groesse_text(&probe("a", false, 26_000)), "26 KB");
        assert_eq!(groesse_text(&probe("a", false, 512)), "512 Byte");
        assert_eq!(groesse_text(&probe("a", true, 4096)), "—");
    }

    #[test]
    fn art_erkennt_typen() {
        assert_eq!(probe("bild.PNG", false, 1).art(), "Bild");
        assert_eq!(probe("main.rs", false, 1).art(), "Rust-Quelltext");
        assert_eq!(probe("ordner", true, 0).art(), "Ordner");
        assert_eq!(probe("irgendwas.xyz", false, 1).art(), "Dokument");
    }

    #[test]
    fn sortierung_und_richtung() {
        let mut v = vec![probe("b", false, 2), probe("A", false, 3), probe("c", true, 1)];
        sortieren(&mut v, Spalte::Name, false);
        assert_eq!(v[0].name, "A");
        sortieren(&mut v, Spalte::Groesse, true);
        assert_eq!(v[0].groesse, 3);
    }

    #[test]
    fn kopie_pfad_weicht_aus() {
        let p = kopie_pfad(Path::new("/nirgends/bericht.txt"));
        assert_eq!(p, PathBuf::from("/nirgends/bericht Kopie.txt"));
    }

    #[test]
    fn versteckt_am_punkt() {
        assert!(probe(".geheim", false, 1).versteckt());
        assert!(!probe("offen", false, 1).versteckt());
    }
}
