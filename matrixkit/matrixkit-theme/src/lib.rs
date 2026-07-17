//! matrixkit-theme — das Fundament der Matrix-UI.
//!
//! Enthält: Design-Tokens (aus dem DMS-Quellcode extrahiert, siehe DESIGNSYSTEM.md),
//! die Live-Palette (liest matugen/DMS-Farben, folgt Wallpaper & Hell/Dunkel) und
//! den Bewegungs-Kern (Material-Kurven + Feder).

use serde::Deserialize;
use std::path::PathBuf;
use std::time::SystemTime;

// ============================================================================
// Tokens — Abstände, Radien, Größen (Quelle: DMS Theme.qml)
// ============================================================================

pub mod spacing {
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const S: f32 = 8.0;
    pub const M: f32 = 12.0;
    pub const L: f32 = 16.0;
    pub const XL: f32 = 24.0;
    pub const XXL: f32 = 40.0;
}

pub const CORNER_RADIUS: f32 = radius::NORMAL;

/// Rundungen — Matrix' Ecken-DNA, aus dem Leitbild destilliert.
///
/// Leitbild lehrt drei Dinge über Ecken:
/// 1. **Kontinuierliche Krümmung** (`RoundedCornerStyle.continuous` ist
///    Leitbild- Default): Ecken sind Superellipsen (Squircles), keine
///    Kreisbögen — der Grund, warum Leitbild-Ecken „weicher" wirken. Unsere
///    ICONS haben das bereits (tiny-skia-Superellipse, n=4.6). UI-Flächen
///    nutzen iceds Kreisbogen (echte Squircle-Flächen wären ein
///    Custom-Shape-Umbau — dokumentiert, für später).
/// 2. **EIN Basis-Radius** fürs ganze System (Kohärenz-Signal) — bei uns
///    NORMAL. Andere Radien leiten sich davon ab, nie willkürlich.
/// 3. **Konzentrizität** (`ContainerRelativeShape`/`ConcentricRectangle`):
///    Ein Element IN einem gerundeten Container rundet so, dass die Ecken
///    konzentrisch verlaufen — innen = außen − Abstand. Das lässt
///    verschachtelte Flächen sauber ineinandergreifen statt zu „beißen".
pub mod radius {
    /// Der EINE Basis-Radius: Karten, Knöpfe, Sektionen, Fenster-Ecken.
    pub const NORMAL: f32 = 12.0;
    /// Kleine Flächen: Chips, Pillen, Tooltips, Auswahl-Knöpfe.
    pub const KLEIN: f32 = 8.0;
    /// Winzige Elemente: Scroll-Griff, Fortschrittsbalken, Platzhalter.
    pub const MINI: f32 = 4.0;
    /// Große schwebende Sheets (Dialoge, Panels): eine Spur weicher.
    pub const GROSS: f32 = 16.0;

    /// Kapsel/Kreis: eine Fläche der Höhe `h` wird voll gerundet (Ampeln,
    /// Farb-Swatches, runde Indikatoren) — width/2 bzw. height/2.
    pub fn kapsel(hoehe: f32) -> f32 {
        hoehe / 2.0
    }

    /// KONZENTRISCHER Innenradius: der Radius eines Elements INNERHALB eines
    /// gerundeten Containers (äußerer Radius `aussen`, Innenabstand
    /// `abstand`), damit die Ecken konzentrisch verlaufen. Nie unter MINI —
    /// ein zu kleiner Innenradius wirkt kantig-falsch.
    pub fn innen(aussen: f32, abstand: f32) -> f32 {
        (aussen - abstand).max(MINI)
    }
}

pub mod font_size {
    pub const SMALL: f32 = 12.0;
    pub const MEDIUM: f32 = 14.0;
    pub const LARGE: f32 = 16.0;
    pub const XLARGE: f32 = 20.0;
}

/// Semantische Typografie — das Kern-UI-Wissen aus dem Leitbild (Font.TextStyle).
///
/// Leitbild- eigentliche Lehre: Hierarchie ist SEMANTISCH, nicht metrisch. Man
/// schreibt nie „Größe 14", sondern „das ist ein `body`" — und Größe,
/// Gewicht und Zeilenfall kommen als GEBÜNDELTE Rolle. So bleibt die
/// Typografie über die ganze Oberfläche kohärent, egal wer sie baut.
///
/// Hier auf Matrix übertragen (Schrift: Inter Variable, an die bestehende
/// DMS-Skala 12/14/16/20 angelehnt). Farbe bleibt bewusst GETRENNT (kommt
/// aus der Palette) — Form und Bedeutung sind zwei Achsen.
pub mod typo {
    /// Schriftgewicht, framework-neutral (wie Rgba). Nur die drei Stufen,
    /// die Leitbild- Text-Styles wirklich unterscheiden.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Gewicht {
        Normal,
        Medium,
        Halbfett,
    }

    /// Ein Text-Stil = Größe + Gewicht, benannt nach seiner ROLLE.
    #[derive(Clone, Copy, Debug)]
    pub struct Stil {
        pub groesse: f32,
        pub gewicht: Gewicht,
    }

    // Leitbild Font.TextStyle → Matrix-Rolle (Auswahl der 13 Leitbild-Styles, die
    // eine Desktop-App wirklich braucht):

    /// `largeTitle` — der eine große Moment (Recovery-Countdown, Willkommen).
    pub const GROSSTITEL: Stil = Stil { groesse: 28.0, gewicht: Gewicht::Halbfett };
    /// `title` — App-Name im Über-Panel, Schritt-Überschriften.
    pub const TITEL: Stil = Stil { groesse: 20.0, gewicht: Gewicht::Halbfett };
    /// `title3` — Abschnitts-Überschrift innerhalb einer Ansicht.
    pub const UNTERTITEL: Stil = Stil { groesse: 16.0, gewicht: Gewicht::Halbfett };
    /// `headline` — betonte Zeile (hervorgehobener Eintrag, aktive Auswahl).
    pub const KOPF: Stil = Stil { groesse: 14.0, gewicht: Gewicht::Halbfett };
    /// `body` — der Standard-Fließtext. Die meiste UI.
    pub const FLIESS: Stil = Stil { groesse: 14.0, gewicht: Gewicht::Normal };
    /// `callout` — etwas kleinerer Begleittext neben dem Hauptinhalt.
    pub const HINWEIS: Stil = Stil { groesse: 13.0, gewicht: Gewicht::Normal };
    /// `footnote`/`subheadline` — Beschreibungen, Fußzeile, Sekundäres.
    pub const KLEIN: Stil = Stil { groesse: 12.0, gewicht: Gewicht::Normal };
    /// `caption` — ALL-CAPS-Sektionsüberschriften, winzige Labels (leicht
    /// betont, damit sie als Struktur lesbar bleiben trotz kleiner Größe).
    pub const ETIKETT: Stil = Stil { groesse: 12.0, gewicht: Gewicht::Medium };

    /// Das dynamicTypeSize-Extrakt (Leitbild-Runde 20): EIN lebender
    /// Faktor skaliert die gesamte semantische Skala. Stufen wie das Leitbild:
    /// klein 0,9 · normal 1,0 · gross 1,15 · sehr-gross 1,3 —
    /// ~/.config/matrix/textgroesse. Gecacht (1-s-Frische), weil txt()
    /// in jedem View-Aufbau vielfach läuft.
    pub fn faktor() -> f32 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static CACHE: AtomicU64 = AtomicU64::new(0);
        let jetzt = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let gepackt = CACHE.load(Ordering::Relaxed);
        let (stand, wert_bits) = (gepackt >> 32, (gepackt & 0xffff_ffff) as u32);
        if stand == jetzt && wert_bits != 0 {
            return f32::from_bits(wert_bits);
        }
        let f: f32 = match super::einstellung::lesen("textgroesse").as_deref() {
            Some("klein") => 0.9,
            Some("gross") => 1.15,
            Some("sehr-gross") => 1.3,
            _ => 1.0,
        };
        CACHE.store((jetzt << 32) | u64::from(f.to_bits()), Ordering::Relaxed);
        f
    }

    /// Die Leitbild-Stufen in Anzeige-Reihenfolge (für ±-Zeilen).
    pub const STUFEN: [&str; 4] = ["klein", "normal", "gross", "sehr-gross"];
}

pub mod icon_size {
    pub const XSMALL: f32 = 13.0;
    pub const SMALL: f32 = 16.0;
    /// Zeilen-Symbole (Listen, Toolbar-Glyphen).
    pub const MEDIUM: f32 = 18.0;
    pub const NORMAL: f32 = 24.0;
    pub const LARGE: f32 = 32.0;
    /// Platzhalter-Symbole (leere Kacheln, Vorschau).
    pub const XLARGE: f32 = 40.0;
    /// Hero-Symbole (Greeter, Dialoge, Leerzustände).
    pub const HERO: f32 = 48.0;
}

/// Einstellungs-Kultur (Leitbild-UI AppStorage, Runde 9): EINE Quelle für
/// Nutzer-Einstellungen — einfache lesbare Dateien unter
/// ~/.config/matrix/<name>. Apps lesen und schreiben nur hierüber;
/// nichts ist versteckt, alles lässt sich von Hand ändern oder sichern.
pub mod einstellung {
    use std::path::PathBuf;

    fn pfad(name: &str) -> PathBuf {
        let basis = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
            });
        basis.join("matrix").join(name)
    }

    /// Wert lesen (getrimmt); None = nie gesetzt.
    pub fn lesen(name: &str) -> Option<String> {
        std::fs::read_to_string(pfad(name)).ok().map(|s| s.trim().to_string())
    }

    /// Wert schreiben (legt ~/.config/matrix an; best effort wie AppStorage).
    pub fn schreiben(name: &str, wert: &str) {
        let p = pfad(name);
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, wert);
    }
}

/// Formatter-Kultur (Foundation ByteCountFormatStyle & Co., Runde 8):
/// Werte werden SYSTEMWEIT gleich formatiert — Apps formatieren nie selbst.
/// de_CH (R65c): die Schweiz trennt Dezimalen mit PUNKT („4.5 GB") —
/// die R35-Empirie aus Matrix Dateien gilt jetzt für die ganze Familie
/// (vorher sprach sie deutsches Komma und Dateien musste ausscheren).
pub mod format {
    /// de_CH: Punkt bleibt Punkt — der Haken existiert nur noch als
    /// EINE Stelle, falls je eine andere Sprachkultur einzieht.
    fn komma(s: String) -> String {
        s
    }

    /// Ein Byte-Wert im Dateimanager-Referenz-Stil: „512 Byte", „980 KB", „610.7 MB",
    /// „512 GB". R68-Empirie-Korrektur: der ECHTE Dateimanager-Referenz rundet NICHT ab
    /// 100 (R35-Beobachtung: „610.7 MB") — er arbeitet adaptiv wie
    /// ByteCountFormatter: MB eine Dezimale, GB/TB bis zwei, und
    /// überflüssige Nullen fallen weg („512 GB", nie „512.0 GB").
    pub fn bytes(b: u64) -> String {
        fn zahl(w: f64, max_dez: usize) -> String {
            let mut s = format!("{w:.*}", max_dez);
            if s.contains('.') {
                while s.ends_with('0') {
                    s.pop();
                }
                if s.ends_with('.') {
                    s.pop();
                }
            }
            s
        }
        const STUFEN: [(f64, &str, usize); 4] =
            [(1e12, "TB", 2), (1e9, "GB", 2), (1e6, "MB", 1), (1e3, "KB", 0)];
        for (grenze, einheit, dez) in STUFEN {
            if (b as f64) >= grenze {
                let w = b as f64 / grenze;
                return komma(format!("{} {einheit}", zahl(w, dez)));
            }
        }
        format!("{b} Byte")
    }

    /// Wertepaar mit GEMEINSAMER Einheit (Speicher-Anzeigen): „2,4 / 15,5 GB".
    /// Die Einheit bestimmt der größere Wert — so bleibt das Paar vergleichbar.
    pub fn bytes_paar(teil: u64, gesamt: u64) -> String {
        const STUFEN: [(f64, &str); 4] = [(1e12, "TB"), (1e9, "GB"), (1e6, "MB"), (1e3, "KB")];
        let max = teil.max(gesamt) as f64;
        for (grenze, einheit) in STUFEN {
            if max >= grenze {
                let (a, b) = (teil as f64 / grenze, gesamt as f64 / grenze);
                let zahl = |w: f64| {
                    if b >= 100.0 { format!("{w:.0}") } else { komma(format!("{w:.1}")) }
                };
                return format!("{} / {} {einheit}", zahl(a), zahl(b));
            }
        }
        format!("{teil} / {gesamt} Byte")
    }

    /// Arbeitsspeicher-Stil (ByteCountFormatter „.memory"): binär (1024er),
    /// umgangssprachliche Einheiten — 16 GiB heißen wie im Leitbild „16 GB".
    pub fn bytes_speicher(b: u64) -> String {
        let gib = b as f64 / 1_073_741_824.0;
        if gib >= 100.0 {
            format!("{gib:.0} GB")
        } else if gib >= 1.0 {
            komma(format!("{gib:.1} GB"))
        } else {
            // Unter 1 GiB wählt der Formatter die Einheit selbst (Leitbild
            // ByteCountFormatter) — Prozess-RSS wäre sonst überall „0,0 GB".
            let mib = b as f64 / 1_048_576.0;
            format!("{mib:.0} MB")
        }
    }

    /// Speicher-Paar mit gemeinsamer binärer Einheit: „2,4 / 15,5 GB".
    pub fn bytes_speicher_paar(teil: u64, gesamt: u64) -> String {
        let g = |b: u64| b as f64 / 1_073_741_824.0;
        let (a, b) = (g(teil), g(gesamt));
        let zahl = |w: f64| {
            if b >= 100.0 { format!("{w:.0}") } else { komma(format!("{w:.1}")) }
        };
        format!("{} / {} GB", zahl(a), zahl(b))
    }

    /// Datenrate: „1,2 MB/s", „830 kB/s", „0 B/s" — dezimal wie bytes().
    pub fn rate(bytes_pro_s: f64) -> String {
        let b = bytes_pro_s.max(0.0);
        if b < 1e3 {
            return format!("{b:.0} B/s");
        }
        format!("{}/s", bytes(b as u64))
    }

    /// Countdown/Restzeit als MM:SS: „28:48", „0:07".
    pub fn dauer_mmss(sekunden: u64) -> String {
        format!("{}:{:02}", sekunden / 60, sekunden % 60)
    }

    /// Zeitspannen wie das Leitbild sie erzählt (Betriebszeit): knapp,
    /// größte Einheit zuerst — aus der Übersicht eingewandert (R65b).
    pub fn dauer(sekunden: u64) -> String {
        let (t, h, m) = (
            sekunden / 86_400,
            (sekunden % 86_400) / 3600,
            (sekunden % 3600) / 60,
        );
        match (t, h) {
            (0, 0) => format!("{m} Min."),
            (0, _) => format!("{h} Std. {m} Min."),
            _ => format!("{t} T. {h} Std."),
        }
    }
}

pub mod control {
    pub const BAR_HEIGHT: f32 = 48.0;
    pub const BUTTON_HEIGHT: f32 = 40.0;
    pub const BUTTON_MIN_WIDTH: f32 = 64.0;
    pub const SLIDER_HEIGHT: f32 = 48.0;
    pub const TOGGLE_TRACK_W: f32 = 52.0;
    pub const TOGGLE_TRACK_H: f32 = 30.0;
    pub const TOGGLE_THUMB: f32 = 24.0;
}

/// Material State-Layers: Deckkraft der Textfarbe ÜBER der Fläche.
pub mod state_layer {
    pub const HOVER: f32 = 0.12;
    pub const PRESSED: f32 = 0.20;
}

// ============================================================================
// Bewegung — Dauern, Material-Kurven, Feder (Quelle: DMS Anims.qml)
// ============================================================================

pub mod motion {
    /// Dauern in Millisekunden.
    pub const DUR_SHORT: u64 = 200;
    pub const DUR_MED: u64 = 450;
    pub const DUR_LONG: u64 = 600;
    /// Standard-Distanz für Herein-Gleiten (px).
    pub const SLIDE_PX: f32 = 80.0;

    /// Kubische Bezier-Kurve (x1, y1, x2, y2) — wie CSS cubic-bezier.
    #[derive(Clone, Copy, Debug)]
    pub struct Bezier(pub f32, pub f32, pub f32, pub f32);

    pub const STANDARD: Bezier = Bezier(0.20, 0.00, 0.00, 1.00);
    pub const STANDARD_DECEL: Bezier = Bezier(0.00, 0.00, 0.00, 1.00);
    pub const STANDARD_ACCEL: Bezier = Bezier(0.30, 0.00, 1.00, 1.00);
    pub const EMPH_DECEL: Bezier = Bezier(0.05, 0.70, 0.10, 1.00);
    pub const EMPH_ACCEL: Bezier = Bezier(0.30, 0.00, 0.80, 0.15);
    /// Räumliche Bewegung mit Überschwinger (y1 > 1!) — Popouts, Dock-Hover.
    pub const EXPRESSIVE_SPATIAL: Bezier = Bezier(0.38, 1.21, 0.22, 1.00);
    pub const EXPRESSIVE_FAST: Bezier = Bezier(0.34, 1.50, 0.20, 1.00);
    pub const EXPRESSIVE_FX: Bezier = Bezier(0.34, 0.80, 0.34, 1.00);

    impl Bezier {
        /// Wertet die Kurve bei Fortschritt t ∈ [0,1] aus (Newton-Iteration über x).
        pub fn eval(&self, t: f32) -> f32 {
            if t <= 0.0 { return 0.0; }
            if t >= 1.0 { return 1.0; }
            let (x1, y1, x2, y2) = (self.0, self.1, self.2, self.3);
            let cubic = |a: f32, b: f32, s: f32| {
                // Bezier mit P0=0, P3=1: 3(1-s)²s·a + 3(1-s)s²·b + s³
                3.0 * (1.0 - s) * (1.0 - s) * s * a + 3.0 * (1.0 - s) * s * s * b + s * s * s
            };
            // s so finden, dass x(s) = t
            let mut s = t;
            for _ in 0..8 {
                let x = cubic(x1, x2, s) - t;
                let dx = 3.0 * (1.0 - s) * (1.0 - s) * x1
                    + 6.0 * (1.0 - s) * s * (x2 - x1)
                    + 3.0 * s * s * (1.0 - x2);
                if dx.abs() < 1e-6 { break; }
                s = (s - x / dx).clamp(0.0, 1.0);
            }
            cubic(y1, y2, s)
        }
    }

    /// Unterbrechbare Feder — der Kern des „Leitbild-Gefühls".
    /// Ziel jederzeit änderbar; die Bewegung geht nahtlos vom aktuellen
    /// Zustand (Position UND Geschwindigkeit) weiter.
    #[derive(Clone, Copy, Debug)]
    pub struct Spring {
        pub value: f32,
        pub velocity: f32,
        pub target: f32,
        pub stiffness: f32,
        pub damping: f32,
    }

    impl Spring {
        pub fn new(value: f32) -> Self {
            Self { value, velocity: 0.0, target: value, stiffness: 170.0, damping: 22.0 }
        }
        /// Leitbild-UI-Parametrisierung (Referenz-SDK, swiftinterface-verifiziert):
        /// `dauer` = wahrgenommene Dauer in s, `sprung` ∈ [-1..1] = Nachschwung.
        /// stiffness = (2π/dauer)², damping = (1−sprung)·4π/dauer für sprung ≥ 0,
        /// sonst 4π/(dauer·(1+sprung)).
        pub fn mit(value: f32, dauer: f32, sprung: f32) -> Self {
            let omega = 2.0 * std::f32::consts::PI / dauer;
            let damping = if sprung >= 0.0 {
                (1.0 - sprung) * 4.0 * std::f32::consts::PI / dauer
            } else {
                4.0 * std::f32::consts::PI / (dauer * (1.0 + sprung))
            };
            Self { value, velocity: 0.0, target: value, stiffness: omega * omega, damping }
        }
        /// das Leitbild `smooth`: kein Nachschwung (0,5 s / sprung 0).
        pub fn glatt(value: f32) -> Self { Self::mit(value, 0.5, 0.0) }
        /// das Leitbild `snappy`: kleiner Nachschwung (0,5 s / sprung 0,15).
        pub fn zackig(value: f32) -> Self { Self::mit(value, 0.5, 0.15) }
        /// das Leitbild `bouncy`: deutlicher Nachschwung (0,5 s / sprung 0,3).
        pub fn federnd(value: f32) -> Self { Self::mit(value, 0.5, 0.3) }
        pub fn retarget(&mut self, target: f32) { self.target = target; }
        /// true, wenn die Feder ihr Ziel erreicht hat und ruht.
        pub fn is_settled(&self) -> bool {
            self.velocity.abs() < 0.001 && (self.target - self.value).abs() < 0.001
        }
        /// Einen Zeitschritt integrieren (dt in Sekunden). true = noch in Bewegung.
        pub fn tick(&mut self, dt: f32) -> bool {
            let force = self.stiffness * (self.target - self.value) - self.damping * self.velocity;
            self.velocity += force * dt;
            self.value += self.velocity * dt;
            if self.velocity.abs() < 0.001 && (self.target - self.value).abs() < 0.001 {
                self.value = self.target;
                self.velocity = 0.0;
                false
            } else {
                true
            }
        }
    }
}

// ============================================================================
// Live-Palette — folgt Wallpaper & Hell/Dunkel-Modus des Systems
// ============================================================================

/// Farbe als 0..1-RGBA, Framework-neutral.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    /// Dieselbe Farbe, anteilig durchscheinend (multipliziert das
    /// vorhandene Alpha) — Farb-Operationen wohnen im Theme, nicht in Apps.
    /// Linear mischen (für den Paletten-Fade).
    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        let m = |x: f32, y: f32| x + (y - x) * t;
        Self { r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: m(a.a, b.a) }
    }

    pub fn mit_alpha(self, a: f32) -> Self {
        Rgba { a: self.a * a, ..self }
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let h = hex.trim_start_matches('#');
        let (r, g, b, a) = match h.len() {
            6 => (
                u8::from_str_radix(&h[0..2], 16).ok()?,
                u8::from_str_radix(&h[2..4], 16).ok()?,
                u8::from_str_radix(&h[4..6], 16).ok()?,
                255u8,
            ),
            8 => (
                u8::from_str_radix(&h[0..2], 16).ok()?,
                u8::from_str_radix(&h[2..4], 16).ok()?,
                u8::from_str_radix(&h[4..6], 16).ok()?,
                u8::from_str_radix(&h[6..8], 16).ok()?,
            ),
            _ => return None,
        };
        Some(Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        })
    }

    /// Hex-Notation (#rrggbb) — z. B. für Anzeige und Zwischenablage.
    pub fn hex(&self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8
        )
    }

    /// State-Layer: diese Farbe mit gegebener Deckkraft über eine Basis legen.
    pub fn over(&self, base: Rgba, alpha: f32) -> Rgba {
        Rgba {
            r: self.r * alpha + base.r * (1.0 - alpha),
            g: self.g * alpha + base.g * (1.0 - alpha),
            b: self.b * alpha + base.b * (1.0 - alpha),
            a: 1.0,
        }
    }
}

/// Die Material-3-Rollen, die MatrixKit-Apps brauchen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub is_light: bool,
    pub primary: Rgba,
    pub on_primary: Rgba,
    pub primary_container: Rgba,
    pub on_primary_container: Rgba,
    pub secondary: Rgba,
    pub tertiary: Rgba,
    pub surface: Rgba,
    pub on_surface: Rgba,
    pub on_surface_variant: Rgba,
    pub surface_container: Rgba,
    pub surface_container_high: Rgba,
    pub outline: Rgba,
    pub error: Rgba,
}

impl Default for Palette {
    /// Fallback (dunkel, neutrale Töne) falls DMS-Daten fehlen.
    fn default() -> Self {
        let hex = |s| Rgba::from_hex(s).unwrap();
        Self {
            is_light: false,
            primary: hex("#cbcb76"),
            on_primary: hex("#323200"),
            primary_container: hex("#4a4a21"),
            on_primary_container: hex("#e8e78f"),
            // Fallback wie im Ernstfall: sekundär/tertiär folgen primary
            secondary: hex("#cbcb76"),
            tertiary: hex("#cbcb76"),
            surface: hex("#14140c"),
            on_surface: hex("#e6e3d5"),
            on_surface_variant: hex("#cac7b6"),
            surface_container: hex("#202018"),
            surface_container_high: hex("#2b2b22"),
            outline: hex("#939182"),
            error: hex("#ffb4ab"),
        }
    }
}

#[derive(Deserialize)]
struct DmsColorsFile {
    colors: DmsModes,
}
#[derive(Deserialize)]
struct DmsModes {
    dark: serde_json::Map<String, serde_json::Value>,
    light: serde_json::Map<String, serde_json::Value>,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into()))
}

pub fn colors_path() -> PathBuf {
    home().join(".cache/DankMaterialShell/dms-colors.json")
}

pub fn session_path() -> PathBuf {
    home().join(".local/state/DankMaterialShell/session.json")
}

/// Hell/Dunkel umschalten — OHNE DMS: isLightMode in session.json
/// flippen. Die Palette trägt beide Modi; alle Watcher greifen sofort.
pub fn hell_umschalten() {
    let pfad = session_path();
    let mut v: serde_json::Value = std::fs::read_to_string(&pfad)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let jetzt = v.get("isLightMode").and_then(|b| b.as_bool()).unwrap_or(false);
    v["isLightMode"] = serde_json::Value::Bool(!jetzt);
    if let Some(eltern) = pfad.parent() {
        let _ = std::fs::create_dir_all(eltern);
    }
    let _ = std::fs::write(&pfad, v.to_string());
}

fn is_light_mode() -> bool {
    std::fs::read_to_string(session_path())
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("isLightMode").and_then(|b| b.as_bool()))
        .unwrap_or(false)
}

/// Der Paletten-Fade (Nutzer, 8.7.2026): Beim Hintergrundwechsel
/// springt das System nicht in die neuen Farben, es GLEITET (~450 ms).
/// Als Filter in Palette::load(): der statische Zustand merkt sich
/// von/ziel/start; solange der Übergang läuft, liefert load()
/// Zwischen-Paletten und PaletteWatcher::changed() bleibt true — jede
/// pollende Fläche fadet automatisch. bewegung=reduziert schaltet hart.
pub mod uebergang {
    use super::*;
    use std::sync::{OnceLock, RwLock};

    pub const DAUER_MS: u128 = 450;

    struct Ueb {
        von: Palette,
        ziel: Palette,
        start: std::time::Instant,
    }
    fn zelle() -> &'static RwLock<Option<Ueb>> {
        static Z: OnceLock<RwLock<Option<Ueb>>> = OnceLock::new();
        Z.get_or_init(|| RwLock::new(None))
    }

    /// Läuft gerade ein Fade? (Treibt Watcher + Schnell-Ticks.)
    pub fn aktiv() -> bool {
        // +80 ms Puffer: der LETZTE Tick nach t=1 muss noch feuern,
        // damit die Fläche exakt auf dem Ziel landet (sonst bliebe sie
        // auf der 97-%-Zwischenpalette hängen).
        zelle()
            .read()
            .ok()
            .and_then(|g| g.as_ref().map(|u| u.start.elapsed().as_millis() < DAUER_MS + 80))
            .unwrap_or(false)
    }

    pub(super) fn filter(roh: Palette) -> Palette {
        if super::bewegung_reduziert() {
            *zelle().write().unwrap() = None;
            return roh;
        }
        let mut g = zelle().write().unwrap();
        match g.as_mut() {
            None => {
                // Erster Kontakt: still übernehmen (App-Start fadet nicht).
                *g = Some(Ueb { von: roh, ziel: roh, start: std::time::Instant::now() });
                roh
            }
            Some(u) => {
                let t = (u.start.elapsed().as_millis() as f32 / DAUER_MS as f32).min(1.0);
                let ist = Palette::lerp(u.von, u.ziel, t);
                if roh != u.ziel {
                    // Neues Ziel: vom aktuellen IST aus weitergleiten.
                    u.von = ist;
                    u.ziel = roh;
                    u.start = std::time::Instant::now();
                    return ist;
                }
                if t >= 1.0 { u.ziel } else { ist }
            }
        }
    }
}

impl Palette {
    /// Linear mischen — is_light springt hart in der Mitte (bool).
    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        let m = |x: Rgba, y: Rgba| Rgba::lerp(x, y, t);
        Palette {
            is_light: if t < 0.5 { a.is_light } else { b.is_light },
            primary: m(a.primary, b.primary),
            on_primary: m(a.on_primary, b.on_primary),
            primary_container: m(a.primary_container, b.primary_container),
            on_primary_container: m(a.on_primary_container, b.on_primary_container),
            secondary: m(a.secondary, b.secondary),
            tertiary: m(a.tertiary, b.tertiary),
            surface: m(a.surface, b.surface),
            on_surface: m(a.on_surface, b.on_surface),
            on_surface_variant: m(a.on_surface_variant, b.on_surface_variant),
            surface_container: m(a.surface_container, b.surface_container),
            surface_container_high: m(a.surface_container_high, b.surface_container_high),
            outline: m(a.outline, b.outline),
            error: m(a.error, b.error),
        }
    }

    /// Lädt die aktuelle System-Palette (DMS/matugen). None bei fehlenden Dateien.
    pub fn load() -> Option<Self> {
        let raw = std::fs::read_to_string(colors_path()).ok()?;
        let file: DmsColorsFile = serde_json::from_str(&raw).ok()?;
        let light = is_light_mode();
        let mode = if light { &file.colors.light } else { &file.colors.dark };
        let get = |key: &str| -> Option<Rgba> {
            mode.get(key).and_then(|v| v.as_str()).and_then(Rgba::from_hex)
        };
        let d = Palette { is_light: light, ..Default::default() };
        Some(uebergang::filter(Self::mit_kontrast(Palette {
            is_light: light,
            primary: get("primary").unwrap_or(d.primary),
            on_primary: get("on_primary").unwrap_or(d.on_primary),
            primary_container: get("primary_container").unwrap_or(d.primary_container),
            on_primary_container: get("on_primary_container").unwrap_or(d.on_primary_container),
            secondary: get("secondary").unwrap_or(d.secondary),
            tertiary: get("tertiary").unwrap_or(d.tertiary),
            surface: get("surface").unwrap_or(d.surface),
            on_surface: get("on_surface").unwrap_or(d.on_surface),
            on_surface_variant: get("on_surface_variant").unwrap_or(d.on_surface_variant),
            surface_container: get("surface_container").unwrap_or(d.surface_container),
            surface_container_high: get("surface_container_high").unwrap_or(d.surface_container_high),
            outline: get("outline").unwrap_or(d.outline),
            error: get("error").unwrap_or(d.error),
        })))
    }

    /// ColorSchemeContrast: bei kontrast=hoch rücken Sekundärtöne an die
    /// Primärtöne und Konturen werden kräftiger — systemweit, weil ALLE
    /// Apps ihre Farben hierdurch beziehen.
    fn mit_kontrast(mut p: Palette) -> Palette {
        if !kontrast_hoch() {
            return p;
        }
        p.on_surface_variant = p.on_surface.over(p.on_surface_variant, 0.45);
        p.outline = p.on_surface.over(p.outline, 0.35);
        p
    }
}

impl Palette {
    /// Wie `load()`, wartet aber Schreib-Wettläufe ab: der Farb-Sync feuert,
    /// WÄHREND matugen die JSON noch schreibt — halb geschriebene Dateien
    /// parsen nicht und lieferten früher stumm Fallback-Farben.
    pub fn load_settled(max_wait: std::time::Duration) -> Option<Self> {
        let start = std::time::Instant::now();
        loop {
            if let Some(p) = Self::load() {
                return Some(p);
            }
            if start.elapsed() >= max_wait {
                return None;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
}

/// Eingabe-Gefühl (R66, empirisch vom Referenzsystem gemessen, defaults read -g):
/// die Zahlen, die das Leitbild nirgends dokumentiert und überall gleich hält.
pub mod eingabe {
    /// Doppelklick-Fenster: Leitbild-Standard 0,5 s (Threshold ungesetzt =
    /// Default; deckt sich mit der R33-Empirie aus Matrix Dateien).
    pub const DOPPELKLICK_MS: u128 = 500;
    /// Tastenwiederholung: InitialKeyRepeat 25×15 ms, KeyRepeat 6×15 ms —
    /// 375 ms Anlauf, ~11/s. In niri gesetzt und am 16.7. per wev auf
    /// dem PC BESTÄTIGT (repeat_info: rate 11, delay 375).
    pub const WIEDERHOLUNG_ANLAUF_MS: u64 = 375;
    pub const WIEDERHOLUNG_RATE: u8 = 11;
    /// Menü-Blitz: der gewählte Eintrag blinkt ~0,2 s, BEVOR das Menü
    /// schließt — seit 1984 die Klick-Quittung jedes Leitbild-Menüs.
    pub const MENU_BLITZ_MS: u64 = 200;
    /// Zieh-Grammatik (R68): 4-pt-Schwelle, bevor ein Druck zum Zug wird
    /// (Leitbild-Toolkit-Konstante); Spring-Loading 0,5 s (empirisch:
    /// springing.delay = 0.5, am Referenzsystem ausgelesen); der Zieh-Geist
    /// schwebt mit ~0,7 Deckkraft.
    pub const ZIEH_SCHWELLE: f32 = 4.0;
    pub const SPRING_LADEN_MS: u64 = 500;
    pub const ZIEH_GEIST_ALPHA: f32 = 0.7;
}

/// Lesebreite (R65b, readableContentGuide-Extrakt): Fließtext läuft
/// nie breiter als ~70 Zeichen — Leitbild- stillste Konsistenzregel.
pub const LESEBREITE: f32 = 560.0;

/// Füll-Stufen für Formen (R65, UIColor.systemFill-Extrakt aus dem
/// Touch-Leitbild-27-SDK): „systemFillColor is appropriate for filling thin and
/// small shapes … quaternary … for filling large areas containing
/// complex content" — je GRÖSSER die Fläche, desto ZARTER die Füllung.
/// Vorher mischte jede App ihre Alphas selbst (0.08/0.12/0.14/0.5 …).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fuellung {
    /// Dünne, kleine Formen (Chips, Tastatur-Tasten, Feld-Gründe).
    Duenn,
    /// Mittlere Formen (Zeilen-Hover, kleine Karten).
    Mittel,
    /// Große Formen (Karten, Panels).
    Gross,
    /// Weite Flächen mit eigenem Inhalt (Konsolen, Listen-Gründe).
    Weit,
}

impl Palette {
    /// Text-Betonungsstufen (R65, UIColor.label*-Extrakt): VIER Stufen
    /// statt zwei — Leitbild- eigentliches Konsistenz-Werkzeug ist die
    /// BENANNTE Hierarchie. 1 = Inhalt, 2 = Begleittext, 3 = Nebensatz,
    /// 4 = kaum mehr als Struktur (Wasserzeichen, Platzhalter-Deko).
    pub fn text_stufe(&self, stufe: u8) -> Rgba {
        match stufe {
            1 => self.on_surface,
            2 => self.on_surface_variant,
            3 => self.on_surface_variant.mit_alpha(0.62),
            _ => self.on_surface_variant.mit_alpha(0.38),
        }
    }

    /// Die Füllfarbe einer Form nach ihrer Größenklasse (s. `Fuellung`).
    pub fn fuellung(&self, f: Fuellung) -> Rgba {
        let anteil = match f {
            Fuellung::Duenn => 0.14,
            Fuellung::Mittel => 0.10,
            Fuellung::Gross => 0.07,
            Fuellung::Weit => 0.045,
        };
        self.on_surface.over(self.surface_container, anteil)
    }
}

/// Leichtgewichtiger Änderungs-Wächter: meldet, wenn Palette ODER Modus
/// sich geändert haben (mtime-Poll — bewusst ohne notify-Abhängigkeit).
pub struct PaletteWatcher {
    last_colors: Option<SystemTime>,
    last_session: Option<SystemTime>,
    /// Bedienungshilfen (kontrast/transparenz) wirken wie ein
    /// Palettenwechsel — der Watcher meldet auch sie.
    last_a11y: (Option<SystemTime>, Option<SystemTime>),
    last_text: Option<SystemTime>,
    last_fade: bool,
}

fn a11y_pfad(name: &str) -> PathBuf {
    let basis = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
        });
    basis.join("matrix").join(name)
}

impl PaletteWatcher {
    pub fn new() -> Self {
        Self {
            last_colors: mtime(&colors_path()),
            last_session: mtime(&session_path()),
            last_a11y: (mtime(&a11y_pfad("kontrast")), mtime(&a11y_pfad("transparenz"))),
            last_text: mtime(&a11y_pfad("textgroesse")),
            last_fade: false,
        }
    }
    /// true, wenn sich seit dem letzten Aufruf etwas geändert hat.
    pub fn changed(&mut self) -> bool {
        let c = mtime(&colors_path());
        let s = mtime(&session_path());
        let a = (mtime(&a11y_pfad("kontrast")), mtime(&a11y_pfad("transparenz")));
        let t = mtime(&a11y_pfad("textgroesse"));
        let fade = uebergang::aktiv();
        let changed = c != self.last_colors
            || s != self.last_session
            || a != self.last_a11y
            || t != self.last_text
            || fade
            // Flanke: der erste Blick NACH dem Fade — für das eine
            // finale Nachladen (Icons frisch backen, Ziel exakt setzen).
            || self.last_fade;
        self.last_fade = fade;
        self.last_colors = c;
        self.last_session = s;
        self.last_a11y = a;
        self.last_text = t;
        changed
    }
}

impl Default for PaletteWatcher {
    fn default() -> Self { Self::new() }
}

fn mtime(p: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(p).ok().and_then(|m| m.modified().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        let c = Rgba::from_hex("#ffb59b").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!(Rgba::from_hex("kaputt").is_none());
    }

    #[test]
    fn format_bytes_referenz_stil() {
        assert_eq!(format::bytes(0), "0 Byte");
        assert_eq!(format::bytes(512), "512 Byte");
        assert_eq!(format::bytes(980_000), "980 KB");
        assert_eq!(format::bytes(4_100_000_000), "4.1 GB");
        assert_eq!(format::bytes(234_000_000), "234 MB");
        assert_eq!(format::bytes_paar(2_400_000_000, 15_500_000_000), "2.4 / 15.5 GB");
        assert_eq!(format::bytes_paar(47_000_000_000, 230_000_000_000), "47 / 230 GB");
        assert_eq!(format::bytes_speicher(17_179_869_184), "16.0 GB");
        assert_eq!(format::bytes_speicher(52_428_800), "50 MB");
        // Relative Zeit (Runde 14)
        assert_eq!(zeit::relativ(1000, 1030), "gerade eben");
        assert_eq!(zeit::relativ(1000, 1000 + 5 * 60), "vor 5 Min.");
        assert_eq!(zeit::relativ(1000, 1000 + 2 * 3600), "vor 2 Std.");
        assert_eq!(zeit::relativ(1000, 1000 + 30 * 3600), "gestern");
        assert_eq!(zeit::relativ(1000, 1000 + 5 * 86400), "vor 5 Tagen");
        // Rückgängig-Stapel (Runde 13): LIFO, benannt, 20er-Limit
        let mut st = rueckgaengig::Stapel::neu();
        assert!(st.leer());
        for i in 0..25 {
            st.merken("Wert", i);
        }
        assert_eq!(st.zurueck(), Some((String::from("Wert"), 24)));
        let mut letzter = 0;
        while let Some((_, w)) = st.zurueck() {
            letzter = w;
        }
        assert_eq!(letzter, 5); // die ältesten 5 sind dem Limit gewichen
        assert!(st.leer());
        assert_eq!(format::bytes_speicher_paar(2_576_980_378, 16_642_998_272), "2.4 / 15.5 GB");
        assert_eq!(format::rate(1_200_000.0), "1.2 MB/s");
        assert_eq!(format::rate(-5.0), "0 B/s");
        assert_eq!(format::dauer_mmss(1728), "28:48");
        assert_eq!(format::dauer_mmss(7), "0:07");
    }

    #[test]
    fn bezier_endpoints_und_monotonie() {
        for curve in [motion::STANDARD, motion::EMPH_DECEL, motion::EXPRESSIVE_SPATIAL] {
            assert_eq!(curve.eval(0.0), 0.0);
            assert_eq!(curve.eval(1.0), 1.0);
        }
        // Überschwinger: expressive darf > 1 gehen
        let mut max = 0.0f32;
        for i in 0..=100 {
            max = max.max(motion::EXPRESSIVE_SPATIAL.eval(i as f32 / 100.0));
        }
        assert!(max > 1.0, "expressive Kurve muss federn (max war {max})");
    }

    #[test]
    fn feder_kommt_an_und_ist_unterbrechbar() {
        let mut s = motion::Spring::new(0.0);
        s.retarget(100.0);
        for _ in 0..60 { s.tick(1.0 / 60.0); }
        s.retarget(50.0); // Unterbrechung mitten in der Bewegung
        let mut steps = 0;
        while s.tick(1.0 / 60.0) && steps < 600 { steps += 1; }
        assert!((s.value - 50.0).abs() < 0.01);
    }

    #[test]
    fn feder_presets_wie_leitbild() {
        // glatt (smooth, sprung 0) = kritisch gedämpft: nie über das Ziel hinaus
        let laufen = |mut s: motion::Spring| {
            s.retarget(1.0);
            let mut max = 0.0f32;
            for _ in 0..600 {
                s.tick(1.0 / 60.0);
                max = max.max(s.value);
            }
            assert!((s.value - 1.0).abs() < 0.01, "Feder muss ankommen");
            max
        };
        let glatt = laufen(motion::Spring::glatt(0.0));
        let zackig = laufen(motion::Spring::zackig(0.0));
        let federnd = laufen(motion::Spring::federnd(0.0));
        assert!(glatt <= 1.0001, "glatt darf nicht überschwingen (war {glatt})");
        assert!(federnd > 1.02, "federnd muss sichtbar überschwingen (war {federnd})");
        assert!(
            glatt < zackig && zackig < federnd,
            "Nachschwung muss mit dem sprung-Wert wachsen ({glatt} / {zackig} / {federnd})"
        );
    }

    #[test]
    fn radius_konzentrizitaet() {
        use radius::*;
        // Der Basis-Radius ist der Anker.
        assert_eq!(NORMAL, CORNER_RADIUS);
        // Innenradius = außen − Abstand (konzentrische Ecken).
        assert_eq!(innen(NORMAL, spacing::XS), NORMAL - spacing::XS);
        assert_eq!(innen(16.0, 4.0), 12.0);
        // Nie unter MINI, auch bei großem Abstand (kein kantiger Kollaps).
        assert_eq!(innen(12.0, 20.0), MINI);
        // Kapsel rundet eine Fläche voll.
        assert_eq!(kapsel(12.0), 6.0);
        assert_eq!(kapsel(24.0), 12.0);
        // Die Skala ist geordnet.
        assert!(MINI < KLEIN && KLEIN < NORMAL && NORMAL < GROSS);
    }

    #[test]
    fn typo_hierarchie_ist_monoton() {
        use typo::*;
        // Die Rollen bilden eine absteigende Größen-Treppe (wie Leitbild- Skala).
        let treppe = [GROSSTITEL, TITEL, UNTERTITEL, KOPF, FLIESS, HINWEIS, KLEIN];
        for paar in treppe.windows(2) {
            assert!(
                paar[0].groesse >= paar[1].groesse,
                "Typo-Rollen müssen von groß nach klein geordnet sein"
            );
        }
        // Betonte Rollen tragen mehr Gewicht als Fließtext gleicher Größe.
        assert_eq!(KOPF.groesse, FLIESS.groesse);
        assert_eq!(KOPF.gewicht, Gewicht::Halbfett);
        assert_eq!(FLIESS.gewicht, Gewicht::Normal);
        // Etikett ist winzig, aber leicht betont (bleibt als Struktur lesbar).
        assert_eq!(ETIKETT.gewicht, Gewicht::Medium);
    }
}

/// Systemweite Einstellung „Bewegung reduzieren" (Barrierefreiheit, wie
/// im Leitbild): ~/.config/matrix/bewegung mit Inhalt "reduziert" schaltet
/// Federn/Übergänge auf Sofort-Zustände um.
/// Relative Zeit (Foundation RelativeDateTimeFormatter, Runde 14):
/// „gerade eben", „vor 5 Min.", „vor 2 Std.", „gestern", „vor 5 Tagen".
/// Reine Funktion (testbar); ab 14 Tagen formatiert die App absolut.
pub mod zeit {
    pub fn relativ(unix: i64, jetzt: i64) -> String {
        let d = jetzt - unix;
        if d < 90 {
            return String::from("gerade eben");
        }
        let min = d / 60;
        if min < 60 {
            return format!("vor {min} Min.");
        }
        let std = d / 3600;
        if std < 24 {
            return format!("vor {std} Std.");
        }
        let tage = d / 86400;
        match tage {
            1 => String::from("gestern"),
            2 => String::from("vorgestern"),
            _ => format!("vor {tage} Tagen"),
        }
    }

    pub fn relativ_jetzt(unix: i64) -> String {
        let jetzt = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(unix);
        relativ(unix, jetzt)
    }
}

/// Rückgängig-Kultur (Leitbild UndoManager, Runde 13): ein kleiner
/// benannter Undo-Stapel. Die App merkt VOR jeder Änderung den alten
/// Wert; Strg+Z (mkw::Taste::Rueckgaengig) holt ihn zurück.
pub mod rueckgaengig {
    pub struct Stapel<T> {
        eintraege: Vec<(String, T)>,
    }

    impl<T> Default for Stapel<T> {
        fn default() -> Self {
            Self::neu()
        }
    }

    impl<T> Stapel<T> {
        pub fn neu() -> Self {
            Self { eintraege: Vec::new() }
        }

        /// Alten Wert VOR der Änderung merken (behält die letzten 20).
        pub fn merken(&mut self, name: impl Into<String>, wert: T) {
            self.eintraege.push((name.into(), wert));
            if self.eintraege.len() > 20 {
                self.eintraege.remove(0);
            }
        }

        /// Letzte Änderung zurücknehmen: (Name, alter Wert).
        pub fn zurueck(&mut self) -> Option<(String, T)> {
            self.eintraege.pop()
        }

        pub fn leer(&self) -> bool {
            self.eintraege.is_empty()
        }
    }
}

/// accessibilityReduceTransparency-Extrakt (Leitbild-Runde 18):
/// ~/.config/matrix/transparenz = "reduziert" macht alle Leisten-Flächen
/// und Schleier deckend — Lesbarkeit vor Glas-Optik.
pub fn transparenz_reduziert() -> bool {
    einstellung::lesen("transparenz").map(|s| s == "reduziert").unwrap_or(false)
}

/// ColorSchemeContrast-Extrakt: ~/.config/matrix/kontrast = "hoch"
/// verstärkt die Palette systemweit (Sekundärtöne rücken an die
/// Primärtöne, Konturen werden kräftiger) — via Palette::load überall.
pub fn kontrast_hoch() -> bool {
    einstellung::lesen("kontrast").map(|s| s == "hoch").unwrap_or(false)
}

pub fn bewegung_reduziert() -> bool {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    std::fs::read_to_string(format!("{home}/.config/matrix/bewegung"))
        .map(|s| s.trim() == "reduziert")
        .unwrap_or(false)
}

/// App-Berechtigungen — die BINDENDE Rechteverwaltung von MatrixKit.
///
/// Jede MatrixKit-App bezieht Systemzugriffe ausschliesslich ueber den
/// Rahmen; der Rahmen prueft VOR jedem Zugriff diese Rechte. Ein
/// abgeschalteter Schalter bedeutet: die App KANN nicht zugreifen —
/// nicht "soll nicht". Ablage: ~/.config/matrix/berechtigungen/<app>.conf
pub mod rechte {
    use std::path::PathBuf;

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Recht {
        Netzwerk,
        Zwischenablage,
        Kamera,
        Mikrofon,
        DateienLesen,
        DateienSchreiben,
    }

    impl Recht {
        pub fn schluessel(self) -> &'static str {
            match self {
                Recht::Netzwerk => "netzwerk",
                Recht::Zwischenablage => "zwischenablage",
                Recht::Kamera => "kamera",
                Recht::Mikrofon => "mikrofon",
                Recht::DateienLesen => "dateien-lesen",
                Recht::DateienSchreiben => "dateien-schreiben",
            }
        }
        pub fn anzeige(self) -> &'static str {
            match self {
                Recht::Netzwerk => "Netzwerk",
                Recht::Zwischenablage => "Zwischenablage",
                Recht::Kamera => "Kamera",
                Recht::Mikrofon => "Mikrofon",
                Recht::DateienLesen => "Dateien lesen",
                Recht::DateienSchreiben => "Dateien schreiben",
            }
        }
    }

    /// Rechtestand einer App. Fehlender Eintrag = erlaubt (Opt-out-Modell;
    /// die App fragt ohnehin nur Rechte ab, die sie wirklich nutzt).
    #[derive(Clone, Debug)]
    pub struct Berechtigungen {
        app_id: String,
        verweigert: Vec<Recht>,
    }

    fn datei(app_id: &str) -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        PathBuf::from(home).join(format!(".config/matrix/berechtigungen/{app_id}.conf"))
    }

    impl Berechtigungen {
        pub fn laden(app_id: &str) -> Self {
            let mut verweigert = Vec::new();
            if let Ok(inhalt) = std::fs::read_to_string(datei(app_id)) {
                for zeile in inhalt.lines() {
                    if let Some((k, w)) = zeile.split_once('=') {
                        if w.trim() == "aus" {
                            for r in ALLE {
                                if r.schluessel() == k.trim() {
                                    verweigert.push(*r);
                                }
                            }
                        }
                    }
                }
            }
            Self { app_id: app_id.into(), verweigert }
        }

        /// DIE Pruefstelle: der Rahmen ruft sie vor jedem Zugriff auf.
        pub fn erlaubt(&self, r: Recht) -> bool {
            !self.verweigert.contains(&r)
        }

        /// Recht setzen und sofort atomar persistieren.
        pub fn setzen(&mut self, r: Recht, erlaubt: bool) {
            self.verweigert.retain(|v| *v != r);
            if !erlaubt {
                self.verweigert.push(r);
            }
            let pfad = datei(&self.app_id);
            if let Some(dir) = pfad.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let mut inhalt = String::from("# MatrixKit-Berechtigungen — verwaltet ueber den App-Namen im Titelbalken\n");
            for v in &self.verweigert {
                inhalt.push_str(&format!("{}=aus\n", v.schluessel()));
            }
            let tmp = pfad.with_extension("conf.neu");
            if std::fs::write(&tmp, inhalt).is_ok() {
                let _ = std::fs::rename(&tmp, &pfad);
            }
        }
    }

    /// Bestätigung mit dem Account-Passwort (PAM über `sudo -Skv`).
    /// Blockiert bis zur Antwort (bei falschem Passwort verzögert PAM ~2 s) —
    /// Apps rufen das über Task::perform im Hintergrund auf. Der
    /// sudo-Zeitstempel wird danach sofort verworfen (-K): die Prüfung
    /// hinterlässt KEINE erhöhten Rechte.
    pub fn passwort_pruefen(passwort: &str) -> bool {
        use std::io::Write;
        let Ok(mut kind) = std::process::Command::new("sudo")
            .args(["-S", "-k", "-v"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        else {
            return false;
        };
        if let Some(mut stdin) = kind.stdin.take() {
            let _ = writeln!(stdin, "{passwort}");
        }
        let ok = kind.wait().map(|s| s.success()).unwrap_or(false);
        let _ = std::process::Command::new("sudo")
            .arg("-K")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        ok
    }

    pub const ALLE: &[Recht] = &[
        Recht::Netzwerk,
        Recht::Zwischenablage,
        Recht::Kamera,
        Recht::Mikrofon,
        Recht::DateienLesen,
        Recht::DateienSchreiben,
    ];
}

/// Die Leinwand-Fassade (R45): EINE Stelle im ganzen Kit kennt den
/// Compositor. Heute ist das darunter unser niri-Fork „Leinwand" —
/// Apps und Dienste sprechen nur dieses Matrix-Vokabular. Das ist
/// Entkopplungs-Schritt 1: Wechselt der Unterbau (harter Fork,
/// Smithay-Eigenbau), ändert sich genau diese Datei.
pub mod leinwand {
    use std::process::{Child, Command, Output, Stdio};

    /// Der Compositor-Binary-Name — NUR hier und im Greeter-Kommando.
    pub const BINARY: &str = "niri";

    fn laufzeit_dir() -> String {
        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into())
    }

    /// Steckt dieser Prozess in einer Leinwand-Session? (Anker für
    /// Dienste wie den Wachdienst.)
    pub fn anker() -> Option<String> {
        std::env::var("NIRI_SOCKET").ok()
    }

    /// Socket der laufenden Leinwand: Umgebung zuerst, sonst Suche im
    /// Laufzeit-Verzeichnis (Deploy-Wege ohne Session-Umgebung).
    pub fn socket() -> Option<String> {
        if let Some(s) = anker() {
            return Some(s);
        }
        std::fs::read_dir(laufzeit_dir()).ok()?.filter_map(Result::ok).find_map(|e| {
            let n = e.file_name().to_str()?.to_string();
            (n.starts_with("niri.") && n.ends_with(".sock"))
                .then(|| e.path().to_string_lossy().into_owned())
        })
    }

    fn kommando(args: &[&str]) -> Command {
        let mut c = Command::new(BINARY);
        if let Some(s) = socket() {
            c.env("NIRI_SOCKET", s);
        }
        c.args(args);
        c
    }

    /// Roher Compositor-Ruf mit Ausgabe — fürs Kit selbst und die
    /// wenigen Spezialfälle (Wächter-Choreografien).
    pub fn roh(args: &[&str]) -> Option<Output> {
        kommando(args).output().ok()
    }

    /// Feuer-und-vergessen-Aktion.
    pub fn aktion(args: &[&str]) -> bool {
        kommando(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    // ------------------------------------------------- das Vokabular

    /// Session beenden → zurück zum Greeter. `rueckfrage` lässt dem
    /// Compositor seine eigene Bestätigung.
    pub fn abmelden(rueckfrage: bool) {
        if rueckfrage {
            aktion(&["msg", "action", "quit"]);
        } else {
            aktion(&["msg", "action", "quit", "--skip-confirmation"]);
        }
    }

    /// Fenster fokussieren — die Leinwand fährt es ganz ins Bild.
    pub fn fenster_fokussieren(id: u64) {
        aktion(&["msg", "action", "focus-window", "--id", &id.to_string()]);
    }

    pub fn fenster_schliessen(id: u64) {
        aktion(&["msg", "action", "close-window", "--id", &id.to_string()]);
    }

    /// Schwebendes Fenster relativ verschieben (Web-Andock-Kopplung).
    pub fn fenster_bewegen(id: u64, dx: i64, dy: i64) {
        aktion(&[
            "msg", "action", "move-floating-window", "--id", &id.to_string(),
            "-x", &format!("{dx:+}"), "-y", &format!("{dy:+}"),
        ]);
    }

    pub fn fenster_breite(id: u64, w: i64) {
        aktion(&["msg", "action", "set-window-width", "--id", &id.to_string(), &w.to_string()]);
    }

    pub fn fenster_hoehe(id: u64, h: i64) {
        aktion(&["msg", "action", "set-window-height", "--id", &id.to_string(), &h.to_string()]);
    }

    /// Das fokussierte Fenster in die Ablage legen (Minimieren-Ersatz).
    pub fn fenster_zur_ablage() {
        aktion(&["msg", "action", "move-window-to-workspace", "ablage"]);
    }

    /// Ereignis-Strom des Compositors als Kindprozess (stdout zeilenweise,
    /// `json` = maschinenlesbar). Rufer liest und verbindet selbst neu.
    pub fn ereignis_strom(json: bool) -> std::io::Result<Child> {
        let args: &[&str] = if json { &["msg", "-j", "event-stream"] } else { &["msg", "event-stream"] };
        kommando(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
    }
}

/// Fenster-Zivilisation: Einzelinstanz, Ablage (Minimieren-Ersatz),
/// Größengedächtnis — die Leitbild-Selbstverständlichkeiten.
pub mod fenster {
    use std::path::PathBuf;

    fn home() -> String {
        std::env::var("HOME").unwrap_or_else(|_| "/root".into())
    }

    fn laufzeit_dir() -> String {
        std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into())
    }

    // Compositor-Zugriff läuft seit R45 über die Leinwand-Fassade.
    use super::leinwand;

    /// Einzelinstanz wie im Leitbild: Läuft die App schon, wird ihr Fenster
    /// fokussiert und false geliefert — der neue Prozess beendet sich.
    /// Der Sperr-Socket lebt so lange wie der Prozess; Absturz-Leichen
    /// werden erkannt (niemand lauscht) und aufgeräumt.
    pub fn einzelinstanz(app_id: &str) -> bool {
        use std::os::unix::net::{UnixListener, UnixStream};
        let pfad = format!("{}/matrixkit-{app_id}.sock", laufzeit_dir());
        match UnixListener::bind(&pfad) {
            Ok(l) => {
                std::mem::forget(l);
                true
            }
            Err(_) => {
                if UnixStream::connect(&pfad).is_err() {
                    // Leiche eines Absturzes — aufräumen und selbst antreten
                    let _ = std::fs::remove_file(&pfad);
                    if let Ok(l) = UnixListener::bind(&pfad) {
                        std::mem::forget(l);
                        return true;
                    }
                }
                fokussieren(app_id);
                false
            }
        }
    }

    /// Das (erste) Fenster einer App-ID in den Fokus holen. Liegt es in
    /// der Ablage (minimiert), wird es erst zur AKTIVEN Arbeitsfläche
    /// geholt — der Leitbild-Restore über Launcher/Neustart der App.
    pub fn fokussieren(app_id: &str) {
        let Some(out) = leinwand::roh(&["msg", "-j", "windows"]) else { return };
        let Ok(fenster) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) else {
            return;
        };
        let Some(f) = fenster
            .iter()
            .find(|f| f.get("app_id").and_then(|a| a.as_str()) == Some(app_id))
        else {
            return;
        };
        let Some(id) = f.get("id").and_then(|i| i.as_u64()) else { return };

        // Ablage-Check: Workspaces holen, aktive + ablage bestimmen
        if let Some(wout) = leinwand::roh(&["msg", "-j", "workspaces"]) {
            if let Ok(ws) = serde_json::from_slice::<Vec<serde_json::Value>>(&wout.stdout) {
                let ablage = ws.iter().find(|w| w.get("name").and_then(|n| n.as_str()) == Some("ablage"));
                let aktive = ws.iter().find(|w| w.get("is_focused").and_then(|b| b.as_bool()) == Some(true));
                if let (Some(ab), Some(ak)) = (ablage, aktive) {
                    let in_ablage = f.get("workspace_id").and_then(|v| v.as_u64())
                        == ab.get("id").and_then(|v| v.as_u64());
                    let ak_idx = ak.get("idx").and_then(|v| v.as_u64());
                    if in_ablage && ab.get("id") != ak.get("id") {
                        if let Some(idx) = ak_idx {
                            let _ = leinwand::roh(&[
                                "msg", "action", "move-window-to-workspace",
                                "--window-id", &id.to_string(), &idx.to_string(),
                            ]);
                        }
                    }
                }
            }
        }
        let _ = leinwand::roh(&["msg", "action", "focus-window", "--id", &id.to_string()]);
    }

    /// „Minimieren“ auf Matrix: das Fenster in die Ablage-Arbeitsfläche legen.
    pub fn ablage() {
        let _ = leinwand::roh(&[
            "msg",
            "action",
            "move-window-to-workspace",
            "--focus",
            "false",
            "ablage",
        ]);
    }

    fn groessen_pfad(app_id: &str) -> PathBuf {
        PathBuf::from(home()).join(format!(".config/matrix/fenster/{app_id}"))
    }

    /// Gemerkte Fenstergröße (Breite, Höhe) — None beim ersten Start.
    pub fn groesse_lesen(app_id: &str) -> Option<(f32, f32)> {
        let inhalt = std::fs::read_to_string(groessen_pfad(app_id)).ok()?;
        let (w, h) = inhalt.trim().split_once('x')?;
        Some((w.parse().ok()?, h.parse().ok()?))
    }

    /// Größe fürs nächste Öffnen merken (Leitbild-Fenstergedächtnis).
    pub fn groesse_merken(app_id: &str, breite: f32, hoehe: f32) {
        let pfad = groessen_pfad(app_id);
        if let Some(dir) = pfad.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(pfad, format!("{breite:.0}x{hoehe:.0}"));
    }
}

/// Der OSD-Kanal der Leisten-Familie: `matrix-osd` (Tasten-Binary)
/// schreibt den Stand hierher, das DOCK liest ihn und morpht kurz zum
/// OSD („Dynamic Dock", Nutzer-Entwurf 7.7.2026). Eine einfache
/// Laufzeit-Datei, kein DBus — die mtime ist die Lebensuhr.
pub mod osd {
    /// Was zuletzt am System gestellt wurde.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Stand {
        pub typ: Typ,
        /// 0–100.
        pub prozent: f32,
        pub stumm: bool,
        /// Kam die Änderung von einem Pegel-Schritt (lauter/leiser/
        /// heller/dunkler) oder einem Schalter (stumm/mikro)?
        pub schritt: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum Typ {
        Ton,
        Mikro,
        Licht,
    }

    impl Stand {
        /// Zeile 2 des Dynamic Dock: WAS wurde geändert, in Worten.
        pub fn text(&self) -> String {
            match (self.typ, self.schritt, self.stumm) {
                (Typ::Ton, true, _) => format!("Lautstärke {:.0} %", self.prozent),
                (Typ::Ton, false, true) => String::from("Ton aus"),
                (Typ::Ton, false, false) => format!("Ton an — {:.0} %", self.prozent),
                (Typ::Mikro, _, true) => String::from("Mikrofon aus"),
                (Typ::Mikro, _, false) => String::from("Mikrofon an"),
                (Typ::Licht, _, _) => format!("Helligkeit {:.0} %", self.prozent),
            }
        }
    }

    fn pfad() -> String {
        let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| String::from("/tmp"));
        format!("{dir}/matrix-osd-zustand")
    }

    /// Eine Zeile: `ton 75 1 schritt|schalter`.
    pub fn schreiben(stand: Stand) {
        let typ = match stand.typ {
            Typ::Ton => "ton",
            Typ::Mikro => "mikro",
            Typ::Licht => "licht",
        };
        let art = if stand.schritt { "schritt" } else { "schalter" };
        let _ = std::fs::write(
            pfad(),
            format!("{typ} {:.0} {} {art}", stand.prozent, stand.stumm as u8),
        );
    }

    /// Stand + Zeitpunkt des letzten Tastendrucks (Datei-mtime).
    pub fn lesen() -> Option<(Stand, std::time::SystemTime)> {
        let p = pfad();
        let mtime = std::fs::metadata(&p).ok()?.modified().ok()?;
        let inhalt = std::fs::read_to_string(&p).ok()?;
        let mut teile = inhalt.split_whitespace();
        let typ = match teile.next()? {
            "ton" => Typ::Ton,
            "mikro" => Typ::Mikro,
            "licht" => Typ::Licht,
            _ => return None,
        };
        let prozent: f32 = teile.next()?.parse().ok()?;
        let stumm = teile.next() == Some("1");
        let schritt = teile.next() == Some("schritt");
        Some((Stand { typ, prozent, stumm, schritt }, mtime))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn rundreise_und_texte() {
            std::env::set_var("XDG_RUNTIME_DIR", std::env::temp_dir());
            let s = Stand { typ: Typ::Ton, prozent: 75.0, stumm: false, schritt: true };
            schreiben(s);
            let (gelesen, _) = lesen().unwrap();
            assert_eq!(gelesen, s);
            assert_eq!(gelesen.text(), "Lautstärke 75 %");
            let stumm = Stand { typ: Typ::Ton, prozent: 75.0, stumm: true, schritt: false };
            assert_eq!(stumm.text(), "Ton aus");
            let licht = Stand { typ: Typ::Licht, prozent: 40.0, stumm: false, schritt: true };
            assert_eq!(licht.text(), "Helligkeit 40 %");
        }
    }
}

/// Befehls-Brücken der Leisten-Familie: JEDE App spricht Systemwerkzeuge
/// (wpctl, nmcli, brightnessctl, niri …) über diese zwei Wege — nie über
/// eigene Command-Schnipsel. Wohnt im Theme, damit auch iced-freie
/// Werkzeuge (matrix-osd) sie nutzen.
pub mod befehl {
    /// Ausführen, Erfolg melden — Ausgabe interessiert nicht.
    pub fn still(cmd: &str, args: &[&str]) -> bool {
        std::process::Command::new(cmd)
            .args(args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Erste Ausgabezeile (getrimmt) oder None bei Fehlern/Leere.
    pub fn erste_zeile(cmd: &str, args: &[&str]) -> Option<String> {
        let out = std::process::Command::new(cmd).args(args).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        let erste = s.lines().next()?.trim();
        (!erste.is_empty()).then(|| erste.to_string())
    }

    /// Die vollständige Standardausgabe eines Befehls (None bei Fehlstatus).
    pub fn text_von(cmd: &str, args: &[&str]) -> Option<String> {
        let out = std::process::Command::new(cmd).args(args).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn brueckenverhalten() {
            assert!(still("true", &[]));
            assert!(!still("false", &[]));
            assert_eq!(erste_zeile("echo", &["hallo", "welt"]).as_deref(), Some("hallo welt"));
            assert_eq!(erste_zeile("false", &[]), None);
        }
    }
}

/// Dock-Abzeichen — das NSDockTile.badgeLabel-Extrakt (Leitbild-Runde 17):
/// Eine App hängt ihrem Dock-Icon eine kleine Zahl/Notiz an („1 Update").
/// Kanal: eine Datei je app_id im Laufzeit-Verzeichnis — leicht zu
/// setzen, leicht zu löschen, nichts überlebt den Neustart.
pub mod abzeichen {
    fn dir() -> std::path::PathBuf {
        let basis = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| String::from("/tmp"));
        std::path::PathBuf::from(basis).join("matrix-abzeichen")
    }

    pub fn setzen(app_id: &str, text: &str) {
        let d = dir();
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join(app_id), text);
    }

    pub fn loeschen(app_id: &str) {
        let _ = std::fs::remove_file(dir().join(app_id));
    }

    pub fn lesen(app_id: &str) -> Option<String> {
        let t = std::fs::read_to_string(dir().join(app_id)).ok()?;
        let t = t.trim().to_string();
        (!t.is_empty()).then_some(t)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn rundreise() {
            std::env::set_var("XDG_RUNTIME_DIR", std::env::temp_dir());
            setzen("test-app", "3");
            assert_eq!(lesen("test-app").as_deref(), Some("3"));
            loeschen("test-app");
            assert_eq!(lesen("test-app"), None);
        }
    }
}

/// Das sensoryFeedback-Extrakt (Leitbild-Runde 20): SEMANTISCHE
/// Feedback-Rollen statt Datei-Pfaden — Apps sagen WAS geschah
/// (Erfolg, Fehler, Hinweis), das System wählt den Matrix-Klang und
/// respektiert die Klänge-Kultur (~/.config/matrix/klaenge.conf:
/// Master „alle" und der Benachrichtigungs-Schalter).
pub mod feedback {
    fn aus(k: &str) -> bool {
        let home = std::env::var("HOME").unwrap_or_default();
        let conf = std::fs::read_to_string(format!("{home}/.config/matrix/klaenge.conf"))
            .unwrap_or_default();
        conf.lines()
            .any(|z| z.split_once('=').map(|(a, b)| (a.trim(), b.trim())) == Some((k, "aus")))
    }

    /// Die Klänge-Kultur: Master „alle" plus ein eigener Schlüssel je
    /// Klang-Familie (fehlender Schlüssel = an).
    fn erlaubt_fuer(schluessel: &str) -> bool {
        !aus("alle") && !aus(schluessel)
    }

    fn erlaubt() -> bool {
        erlaubt_fuer("benachrichtigungen")
    }

    /// Klangdatei auflösen: der Dev-Stand im Home GEWINNT (12-fehler
    /// als Anker), sonst das Image — dieselbe Regel wie der
    /// klangordner() der Klänge-App. Vorher spielte feedback stur aus
    /// /usr/share und ließ frisch gerenderte Klänge ungehört
    /// (Nutzer-Fund 15.7.).
    fn klangpfad(datei: &str) -> String {
        if let Ok(home) = std::env::var("HOME") {
            let dev = format!("{home}/.local/share/matrix/klaenge");
            if std::path::Path::new(&format!("{dev}/12-fehler.wav")).exists() {
                return format!("{dev}/{datei}");
            }
        }
        format!("/usr/share/matrix/klaenge/{datei}")
    }

    fn spielen(datei: &'static str) {
        if !erlaubt() {
            return;
        }
        std::thread::spawn(move || {
            let pfad = klangpfad(datei);
            for spieler in ["pw-play", "paplay"] {
                if super::befehl::still(spieler, &[&pfad]) {
                    return;
                }
            }
        });
    }

    /// SYNCHRON abspielen — für kurzlebige Werkzeuge (matrix-osd) und für
    /// Momente, in denen der Klang FERTIG sein muss, bevor es weitergeht
    /// (Abmelde-Klang vor poweroff). Blockiert für die Dauer der Datei.
    pub fn jetzt(schluessel: &str, datei: &str) -> bool {
        if !erlaubt_fuer(schluessel) {
            return false;
        }
        let pfad = klangpfad(datei);
        for spieler in ["pw-play", "paplay"] {
            if super::befehl::still(spieler, &[&pfad]) {
                return true;
            }
        }
        false
    }

    /// „fertig" — etwas ist gelungen (11).
    pub fn erfolg() {
        spielen("11-fertig.wav");
    }
    /// „fehler" — etwas ist schiefgegangen (12).
    pub fn fehler() {
        spielen("12-fehler.wav");
    }
    /// Ein Hinweis will Aufmerksamkeit (03).
    pub fn hinweis() {
        spielen("03-benachrichtigung.wav");
    }
}
