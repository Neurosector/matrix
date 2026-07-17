//! Zeichenkern der Lebenden Icons — als Bibliothek, damit Apps ihr
//! eigenes Icon live rendern können (z. B. im „Über"-Panel der Root-Ebene).

use matrixkit_theme as mk;
use tiny_skia::{
    BlendMode,
    Color, FillRule, GradientStop, LinearGradient, Paint, Path, PathBuilder, Pixmap, Point,
    Shader, SpreadMode, Stroke, Transform,
};

/// Master-Leinwand der App-Icons.
pub const SIZE: u32 = 256;

/// Die Motivfarben einer Glyphe — im Standard-Stil primary/secondary/tertiary,
/// im getönten Stil Abstufungen der Textfarbe (Leitbild- "Tinted").
pub struct Glyph {
    pub a: mk::Rgba,
    pub b: mk::Rgba,
    pub c: mk::Rgba,
}

/// Zeichenfläche für Glyphen: kapselt Pixmap + Versatz, damit derselbe
/// Glyphen-Code erst den Schatten (versetzt) und dann das Motiv malt.
pub struct Zeichner<'a> {
    pub pixmap: &'a mut Pixmap,
    pub dy: f32,
    /// Ebenen-Tiefe (Nutzer, 8.7.): jede Form wirft einen weichen
    /// Schatten auf das, was UNTER ihr liegt — nicht nur die Glyphe
    /// als Ganzes auf die Kachel. Aus für Roh-Zeichnungen (Plymouth).
    pub ebenen_schatten: bool,
    /// Material-Standard (8.7., „die bestehenden Icons besser machen"):
    /// fill() veredelt automatisch — dezente Transluzenz (0.92) und
    /// Glanzkante auf JEDER Form. Die eingebauten Glyphen erben den
    /// Composer-Look, ohne dass eine einzige umgeschrieben wird.
    pub material_standard: bool,
}

impl<'a> Zeichner<'a> {
    /// Der Standard für App-Icons: Tiefe + Material an.
    pub fn neu(pixmap: &'a mut Pixmap) -> Self {
        Zeichner { pixmap, dy: 0.0, ebenen_schatten: true, material_standard: true }
    }
    /// Roh (Plymouth/Sonderfälle): flach, pur.
    pub fn roh(pixmap: &'a mut Pixmap) -> Self {
        Zeichner { pixmap, dy: 0.0, ebenen_schatten: false, material_standard: false }
    }
}

impl Zeichner<'_> {
    fn roh_fill(&mut self, path: &Path, c: mk::Rgba, dy: f32) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba(c.r, c.g, c.b, c.a).unwrap_or(Color::BLACK));
        paint.anti_alias = true;
        self.pixmap.fill_path(
            path,
            &paint,
            FillRule::Winding,
            Transform::from_translate(0.0, self.dy + dy),
            None,
        );
    }

    pub fn fill(&mut self, path: &Path, c: mk::Rgba) {
        if self.material_standard {
            let veredelt = mk::Rgba { a: c.a * 0.92, ..c };
            self.fill_ex(path, veredelt, BlendMode::SourceOver, true);
        } else {
            self.fill_ex(path, c, BlendMode::SourceOver, false);
        }
    }

    /// Material-Füllung (Icon Composer): Mischmodus + Glanzlicht.
    pub fn fill_ex(&mut self, path: &Path, c: mk::Rgba, blend: BlendMode, glanz: bool) {
        if self.ebenen_schatten {
            // Fake-Blur: drei versetzte Alphalagen = weicher Wurf.
            for (d, a) in [(2.0, 0.10), (4.0, 0.08), (6.0, 0.05)] {
                self.roh_fill(path, mk::Rgba { r: 0.0, g: 0.0, b: 0.0, a }, d);
            }
        }
        if glanz {
            // Specular-Fake: helle Lage minimal nach oben — der oben
            // überstehende Saum bleibt als Glanzkante sichtbar.
            self.roh_fill(path, mk::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 0.55 }, -2.5);
        }
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba(c.r, c.g, c.b, c.a).unwrap_or(Color::BLACK));
        paint.anti_alias = true;
        paint.blend_mode = blend;
        self.pixmap.fill_path(
            path,
            &paint,
            FillRule::Winding,
            Transform::from_translate(0.0, self.dy),
            None,
        );
    }
}

/// Ein MatrixKit-App-Icon: Name = .desktop-/Icon-Name + Glyphen-Rezept.
/// Die Kachel kommt für alle Apps aus demselben Rezept (Familienzugehörigkeit).
pub struct IconSpec {
    pub name: &'static str,
    pub glyphe: fn(&Glyph, &mut Zeichner),
}

/// Alle MatrixKit-Apps. Neue Apps ergänzen hier genau EINE Zeile + Zeichen-Fn.
pub const ICONS: &[IconSpec] = &[
    IconSpec { name: "matrix-sysmon", glyphe: glyphe_sysmon },
    IconSpec { name: "matrix-farben", glyphe: glyphe_farben },
    IconSpec { name: "matrix-hilfe", glyphe: glyphe_hilfe },
    IconSpec { name: "matrix-klaenge", glyphe: glyphe_klaenge },
    IconSpec { name: "matrix-codes", glyphe: glyphe_codes },
    IconSpec { name: "matrix-schluessel-app", glyphe: glyphe_schluessel_app },
    IconSpec { name: "matrix-wiederherstellung", glyphe: glyphe_wache },
    IconSpec { name: "matrix-einstellungen", glyphe: glyphe_einstellungen },
    IconSpec { name: "matrix-updater", glyphe: glyphe_updater },
    IconSpec { name: "matrix-entwicklerzugang", glyphe: glyphe_entwicklerzugang },
    IconSpec { name: "matrix-leinwand", glyphe: glyphe_leinwand },
    IconSpec { name: "matrix-web", glyphe: glyphe_web },
    IconSpec { name: "matrix-dateien", glyphe: glyphe_dateien },
    IconSpec { name: "matrix-morpheus", glyphe: glyphe_installer },
    IconSpec { name: "matrix-icons", glyphe: glyphe_icons },
    IconSpec { name: "matrix-tastatur", glyphe: glyphe_tastatur },
    IconSpec { name: "matrix-terminal", glyphe: glyphe_terminal },
    IconSpec { name: "matrix-aufnahme", glyphe: glyphe_aufnahme },
    IconSpec { name: "matrix-player", glyphe: glyphe_player },
];

#[derive(PartialEq)]
pub enum Stil {
    Standard,
    Getoent,
}

pub fn stil() -> Stil {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    match std::fs::read_to_string(format!("{home}/.config/matrix/icon-stil")) {
        Ok(s) if s.trim() == "getoent" => Stil::Getoent,
        _ => Stil::Standard,
    }
}

// --- Die Kachel: Squircle mit Tiefe ------------------------------------------

/// Superellipse (|x/r|^n + |y/r|^n = 1) — Leitbild- kontinuierliche Krümmung.
pub fn squircle(cx: f32, cy: f32, r: f32, n: f32) -> Path {
    let mut pb = PathBuilder::new();
    let steps = 256;
    let exp = 2.0 / n;
    for i in 0..steps {
        let t = (i as f32 / steps as f32) * std::f32::consts::TAU;
        let (s, c) = t.sin_cos();
        let x = cx + r * c.abs().powf(exp) * c.signum();
        let y = cy + r * s.abs().powf(exp) * s.signum();
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();
    pb.finish().expect("squircle")
}

fn hell(c: mk::Rgba, f: f32) -> mk::Rgba {
    mk::Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }.over(c, f)
}

fn dunkel(c: mk::Rgba, f: f32) -> mk::Rgba {
    mk::Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }.over(c, f)
}

fn ts(c: mk::Rgba) -> Color {
    Color::from_rgba(c.r, c.g, c.b, c.a).unwrap_or(Color::BLACK)
}

/// Die gemeinsame Kachel aller MatrixKit-Icons: Squircle, sanfter
/// Vertikalverlauf (oben heller), Lichtkante an der Oberkante.
pub fn kachel(p: &mk::Palette, pixmap: &mut Pixmap) {
    let form = squircle(128.0, 128.0, 120.0, 4.6);

    // Vertikalverlauf: Licht faellt von oben
    let oben = hell(p.surface_container, 0.10);
    let unten = dunkel(p.surface_container, 0.08);
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = LinearGradient::new(
        Point::from_xy(128.0, 8.0),
        Point::from_xy(128.0, 248.0),
        vec![
            GradientStop::new(0.0, ts(oben)),
            GradientStop::new(1.0, ts(unten)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(ts(p.surface_container)));
    pixmap.fill_path(&form, &paint, FillRule::Winding, Transform::identity(), None);

    // Lichtkante: feiner Randverlauf, oben am staerksten, unten verschwunden
    let rand = squircle(128.0, 128.0, 118.5, 4.6);
    let mut licht = Paint::default();
    licht.anti_alias = true;
    licht.shader = LinearGradient::new(
        Point::from_xy(128.0, 8.0),
        Point::from_xy(128.0, 160.0),
        vec![
            GradientStop::new(0.0, Color::from_rgba(1.0, 1.0, 1.0, 0.22).unwrap()),
            GradientStop::new(1.0, Color::from_rgba(1.0, 1.0, 1.0, 0.0).unwrap()),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::TRANSPARENT));
    let stroke = Stroke { width: 3.0, ..Default::default() };
    pixmap.stroke_path(&rand, &licht, &stroke, Transform::identity(), None);
}

// --- Die Glyphen ---------------------------------------------------------------

/// matrix-sysmon: drei Balken — die Miniatur der App selbst.
pub fn glyphe_sysmon(g: &Glyph, z: &mut Zeichner) {
    z.fill(&round_rect(48.0, 76.0, 148.0, 100.0, 12.0), g.a);
    z.fill(&round_rect(48.0, 116.0, 196.0, 140.0, 12.0), g.b);
    z.fill(&round_rect(48.0, 156.0, 120.0, 180.0, 12.0), g.c);
}

/// matrix-farben: drei überlappende Farbkreise — die Palette als Bild.
pub fn glyphe_farben(g: &Glyph, z: &mut Zeichner) {
    // Der Photos-Gruß als Bestands-Icon: transluzente Kreise, deren
    // Überlappungen multiplikativ satter werden (Material-Engine).
    let kreis = |cx: f32, cy: f32| squircle(cx, cy, 48.0, 2.0);
    let blatt = |z: &mut Zeichner, cx: f32, cy: f32, f: mk::Rgba| {
        z.fill_ex(
            &kreis(cx, cy),
            mk::Rgba { a: f.a * 0.78, ..f },
            BlendMode::Multiply,
            true,
        );
    };
    blatt(z, 102.0, 106.0, g.c);
    blatt(z, 154.0, 106.0, g.b);
    blatt(z, 128.0, 152.0, g.a);
}

/// matrix-klaenge: drei vertikale Klangbalken (Equalizer) — der Klang als Bild.
pub fn glyphe_klaenge(g: &Glyph, z: &mut Zeichner) {
    z.fill(&round_rect(64.0, 116.0, 92.0, 188.0, 14.0), g.b);
    z.fill(&round_rect(114.0, 76.0, 142.0, 188.0, 14.0), g.a);
    z.fill(&round_rect(164.0, 140.0, 192.0, 188.0, 14.0), g.c);
}

/// matrix-schluessel-app: ein Schild (Setup/Sicherheit) mit Kern-Punkt —
/// klar abgesetzt vom Codes-Schlüssel.
pub fn glyphe_schluessel_app(g: &Glyph, z: &mut Zeichner) {
    // Schild: oben breit, unten spitz — als Pfad
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(128.0, 46.0);
    pb.line_to(196.0, 74.0);
    pb.line_to(196.0, 128.0);
    pb.cubic_to(196.0, 174.0, 166.0, 200.0, 128.0, 214.0);
    pb.cubic_to(90.0, 200.0, 60.0, 174.0, 60.0, 128.0);
    pb.line_to(60.0, 74.0);
    pb.close();
    if let Some(pfad) = pb.finish() {
        z.fill(&pfad, g.a);
    }
    // Kern-Punkt
    z.fill(&round_rect(110.0, 104.0, 146.0, 140.0, 18.0), g.b);
    z.fill(&round_rect(120.0, 132.0, 136.0, 168.0, 8.0), g.b);
}

/// matrix-codes: ein Schlüssel (Bogen + Schaft + zwei Zähne) — 2FA/Sicherheit.
pub fn glyphe_codes(g: &Glyph, z: &mut Zeichner) {
    // Bogen (runder Kopf)
    z.fill(&round_rect(60.0, 60.0, 140.0, 140.0, 40.0), g.a);
    // Schaft nach unten
    z.fill(&round_rect(90.0, 128.0, 110.0, 200.0, 10.0), g.a);
    // Zwei Zähne rechts
    z.fill(&round_rect(110.0, 150.0, 150.0, 166.0, 6.0), g.b);
    z.fill(&round_rect(110.0, 176.0, 138.0, 192.0, 6.0), g.c);
}

/// matrix-wiederherstellung: der Wächter — ein Schild mit wachsamem Auge.
/// Schutz (Schild) + Aufmerksamkeit (Auge): „bewacht das ganze System".
pub fn glyphe_wache(g: &Glyph, z: &mut Zeichner) {
    // Schild (wie Schlüssel-App, gemeinsame Sicherheits-Formensprache)
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(128.0, 44.0);
    pb.line_to(198.0, 74.0);
    pb.line_to(198.0, 130.0);
    pb.cubic_to(198.0, 176.0, 167.0, 202.0, 128.0, 216.0);
    pb.cubic_to(89.0, 202.0, 58.0, 176.0, 58.0, 130.0);
    pb.line_to(58.0, 74.0);
    pb.close();
    if let Some(pfad) = pb.finish() {
        z.fill(&pfad, g.a);
    }
    // Auge: Mandel-Form (zwei gespiegelte Bögen) + Pupille
    let mut lid = tiny_skia::PathBuilder::new();
    lid.move_to(90.0, 128.0);
    lid.cubic_to(108.0, 106.0, 148.0, 106.0, 166.0, 128.0);
    lid.cubic_to(148.0, 150.0, 108.0, 150.0, 90.0, 128.0);
    lid.close();
    if let Some(pfad) = lid.finish() {
        z.fill(&pfad, g.b);
    }
    z.fill(&round_rect(118.0, 118.0, 138.0, 138.0, 10.0), g.c);
}

/// matrix-einstellungen: drei horizontale Schieberegler mit Griff —
/// die Einstellungen als Bild, im Einklang mit dem Inhalt der App.
pub fn glyphe_einstellungen(g: &Glyph, z: &mut Zeichner) {
    let knopf = |cx: f32, cy: f32| round_rect(cx - 17.0, cy - 17.0, cx + 17.0, cy + 17.0, 17.0);
    // Obere Spur: Griff rechts
    z.fill(&round_rect(60.0, 79.0, 196.0, 93.0, 7.0), g.b);
    z.fill(&knopf(162.0, 86.0), g.a);
    // Mittlere Spur: Griff links
    z.fill(&round_rect(60.0, 121.0, 196.0, 135.0, 7.0), g.b);
    z.fill(&knopf(94.0, 128.0), g.a);
    // Untere Spur: Griff Mitte
    z.fill(&round_rect(60.0, 163.0, 196.0, 177.0, 7.0), g.b);
    z.fill(&knopf(130.0, 170.0), g.c);
}

/// matrix-updater: Pfeil landet in der Schale — Software kommt an.
/// Stamm + Rauten-Spitze (Squircle n=1.2) + Auffang-Schale.
pub fn glyphe_updater(g: &Glyph, z: &mut Zeichner) {
    z.fill(&round_rect(115.0, 52.0, 141.0, 128.0, 13.0), g.a);
    z.fill(&squircle(128.0, 148.0, 30.0, 1.2), g.c);
    z.fill(&round_rect(58.0, 176.0, 72.0, 204.0, 7.0), g.b);
    z.fill(&round_rect(58.0, 190.0, 198.0, 204.0, 7.0), g.b);
    z.fill(&round_rect(184.0, 176.0, 198.0, 204.0, 7.0), g.b);
}

/// matrix-leinwand: zwei freie Fenster auf weiter Flaeche — der
/// unendliche Desktop als Bild.
pub fn glyphe_leinwand(g: &Glyph, z: &mut Zeichner) {
    z.fill(&round_rect(46.0, 66.0, 210.0, 190.0, 16.0), g.b);
    z.fill(&round_rect(66.0, 88.0, 132.0, 142.0, 10.0), g.a);
    z.fill(&round_rect(146.0, 120.0, 192.0, 168.0, 10.0), g.c);
}

/// matrix-hilfe: abstrahiertes "i" (Punkt + Stamm) — Hilfe/Info.
/// Matrix Web: eine runde Welt mit Äquator und Meridian — der Browser.
pub fn glyphe_web(g: &Glyph, z: &mut Zeichner) {
    // Weltkugel (Kapsel-Squircle = Kreisfläche)
    z.fill(&squircle(128.0, 128.0, 78.0, 2.0), g.b);
    // Innenwelt einen Ton ruhiger
    z.fill(&squircle(128.0, 128.0, 62.0, 2.0), g.a);
    // Äquator + Meridian
    z.fill(&round_rect(58.0, 120.0, 198.0, 136.0, 8.0), g.c);
    z.fill(&round_rect(120.0, 58.0, 136.0, 198.0, 8.0), g.c);
}

pub fn glyphe_hilfe(g: &Glyph, z: &mut Zeichner) {
    z.fill(&round_rect(108.0, 56.0, 148.0, 96.0, 20.0), g.c);
    z.fill(&round_rect(108.0, 116.0, 148.0, 200.0, 20.0), g.a);
}


/// matrix-entwicklerzugang: ein Schlüssel — Ring + Bart — für Zugang.
pub fn glyphe_entwicklerzugang(g: &Glyph, z: &mut Zeichner) {
    // Schlüsselring (offener Kreis via zwei Squircles)
    z.fill(&squircle(104.0, 104.0, 52.0, 2.0), g.c);
    z.fill(&squircle(104.0, 104.0, 30.0, 2.0), g.a);
    // Schaft diagonal zum Bart
    z.fill(&round_rect(120.0, 120.0, 200.0, 148.0, 12.0), g.c);
    // zwei Bartzaehne
    z.fill(&round_rect(168.0, 148.0, 184.0, 186.0, 5.0), g.b);
    z.fill(&round_rect(196.0, 148.0, 212.0, 178.0, 5.0), g.b);
}

/// matrix-dateien: ein Ordner — Reiter + zweitoniger Korpus mit
/// Lichtkante, die Dateimanager-Referenz-Silhouette in der Matrix-Familie.
pub fn glyphe_dateien(g: &Glyph, z: &mut Zeichner) {
    // Reiter oben links
    z.fill(&round_rect(52.0, 70.0, 134.0, 110.0, 14.0), g.b);
    // Rückwand
    z.fill(&round_rect(48.0, 90.0, 208.0, 186.0, 18.0), g.b);
    // Front, leicht abgesetzt (der „geöffnete" Ordner)
    z.fill(&round_rect(48.0, 104.0, 208.0, 186.0, 18.0), g.a);
    // Lichtkante unter dem Deckelrand
    z.fill(&round_rect(60.0, 112.0, 196.0, 121.0, 4.5), g.c);
}

/// matrix-installer: ein Pfeil senkt sich in eine Platte — Installation.
pub fn glyphe_installer(g: &Glyph, z: &mut Zeichner) {
    // Zielplatte unten
    z.fill(&round_rect(60.0, 156.0, 196.0, 190.0, 14.0), g.b);
    // Pfeilschaft
    z.fill(&round_rect(116.0, 62.0, 140.0, 128.0, 10.0), g.a);
    // Pfeilspitze (drei gestufte Balken als Dreieck-Näherung)
    z.fill(&round_rect(96.0, 118.0, 160.0, 136.0, 8.0), g.a);
    z.fill(&round_rect(108.0, 132.0, 148.0, 146.0, 7.0), g.a);
    z.fill(&round_rect(119.0, 142.0, 137.0, 154.0, 6.0), g.c);
}

/// matrix-tastatur: die Bildschirmtastatur — Platte mit zwei Tasten-
/// zeilen und breiter Leertaste, die Tablet-Leitbild-Silhouette (R58).
pub fn glyphe_tastatur(g: &Glyph, z: &mut Zeichner) {
    // Tastatur-Platte
    z.fill(&round_rect(44.0, 82.0, 212.0, 174.0, 18.0), g.b);
    // Zeile 1: vier Tasten
    for i in 0..4 {
        let x = 58.0 + i as f32 * 37.0;
        z.fill(&round_rect(x, 96.0, x + 27.0, 118.0, 6.0), g.a);
    }
    // Zeile 2: vier Tasten
    for i in 0..4 {
        let x = 58.0 + i as f32 * 37.0;
        z.fill(&round_rect(x, 126.0, x + 27.0, 148.0, 6.0), g.a);
    }
    // Leertaste — der Akzent
    z.fill(&round_rect(84.0, 154.0, 172.0, 166.0, 6.0), g.c);
}

/// matrix-terminal: dunkle Scheibe, Prompt-Winkel und Cursor-Block —
/// die klassische Terminal-Silhouette in der Matrix-Familie (R61).
pub fn glyphe_terminal(g: &Glyph, z: &mut Zeichner) {
    // Scheibe
    z.fill(&round_rect(48.0, 70.0, 208.0, 186.0, 18.0), g.b);
    // Prompt-Winkel ❯: volles Dreieck, Kehle in Scheibenfarbe ausgespart
    let mut pb = PathBuilder::new();
    pb.move_to(74.0, 98.0);
    pb.line_to(124.0, 126.0);
    pb.line_to(74.0, 154.0);
    pb.close();
    z.fill(&pb.finish().expect("prompt"), g.a);
    let mut pb = PathBuilder::new();
    pb.move_to(74.0, 116.0);
    pb.line_to(92.0, 126.0);
    pb.line_to(74.0, 136.0);
    pb.close();
    z.fill(&pb.finish().expect("kehle"), g.b);
    // Cursor-Block daneben — der Akzent
    z.fill(&round_rect(136.0, 138.0, 172.0, 156.0, 5.0), g.c);
}

/// matrix-aufnahme: Kamera-Silhouette mit Aufnahme-Punkt (R69).
pub fn glyphe_aufnahme(g: &Glyph, z: &mut Zeichner) {
    // Gehaeuse mit Sucher-Buckel
    z.fill(&round_rect(52.0, 96.0, 204.0, 178.0, 20.0), g.b);
    z.fill(&round_rect(96.0, 78.0, 160.0, 104.0, 12.0), g.b);
    // Objektiv-Ring + Linse
    z.fill(&squircle(128.0, 136.0, 34.0, 2.0), g.a);
    z.fill(&squircle(128.0, 136.0, 20.0, 2.0), g.b);
    // Aufnahme-Punkt — der Akzent
    z.fill(&squircle(184.0, 116.0, 9.0, 2.0), g.c);
}

/// matrix-player: Bühne mit Wiedergabe-Dreieck und Zeitleiste (R71).
pub fn glyphe_player(g: &Glyph, z: &mut Zeichner) {
    // Die Bühne
    z.fill(&round_rect(52.0, 78.0, 204.0, 178.0, 20.0), g.b);
    // Play-Dreieck
    let mut pb = PathBuilder::new();
    pb.move_to(106.0, 100.0);
    pb.line_to(166.0, 126.0);
    pb.line_to(106.0, 152.0);
    pb.close();
    z.fill(&pb.finish().expect("play"), g.a);
    // Zeitleiste mit Puck — der Akzent
    z.fill(&round_rect(72.0, 160.0, 184.0, 166.0, 3.0), g.a);
    z.fill(&squircle(118.0, 163.0, 8.0, 2.0), g.c);
}

// --- Zeichen-Helfer ----------------------------------------------------------

/// Abgerundetes Rechteck (x1,y1)-(x2,y2) mit Radius r — Ecken als
/// Kubik-Bögen (Kreis-Approximation, kappa ≈ 0.5523).
pub fn round_rect(x1: f32, y1: f32, x2: f32, y2: f32, r: f32) -> Path {
    let r = r.min((x2 - x1) / 2.0).min((y2 - y1) / 2.0);
    let k = 0.552_285 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x1 + r, y1);
    pb.line_to(x2 - r, y1);
    pb.cubic_to(x2 - r + k, y1, x2, y1 + r - k, x2, y1 + r);
    pb.line_to(x2, y2 - r);
    pb.cubic_to(x2, y2 - r + k, x2 - r + k, y2, x2 - r, y2);
    pb.line_to(x1 + r, y2);
    pb.cubic_to(x1 + r - k, y2, x1, y2 - r + k, x1, y2 - r);
    pb.line_to(x1, y1 + r);
    pb.cubic_to(x1, y1 + r - k, x1 + r - k, y1, x1 + r, y1);
    pb.close();
    pb.finish().expect("round_rect")
}


/// Ein komplettes App-Icon als PNG rendern — dieselbe Pipeline wie der
/// Generator (Kachel + Schatten + Glyphe im aktuellen Stil).
pub fn render_png(name: &str, palette: &mk::Palette) -> Option<Vec<u8>> {
    // Composer-Rezepte des Nutzers übersteuern die eingebauten Glyphen.
    if let Some(rezept) = rezept_laden(name) {
        return rezept_png(&rezept, palette);
    }
    let spec = ICONS.iter().find(|s| s.name == name)?;
    let glyph = match stil() {
        Stil::Standard => Glyph {
            a: palette.primary,
            b: palette.secondary,
            c: palette.tertiary,
        },
        Stil::Getoent => Glyph {
            a: mit_alpha(palette.on_surface, 1.0),
            b: mit_alpha(palette.on_surface, 0.72),
            c: mit_alpha(palette.on_surface, 0.5),
        },
    };
    let mut pixmap = Pixmap::new(SIZE, SIZE)?;
    kachel(palette, &mut pixmap);
    // Ebenen-Schatten übernimmt der Zeichner form-für-form.
    (spec.glyphe)(&glyph, &mut Zeichner::neu(&mut pixmap));
    pixmap.encode_png().ok()
}

pub fn glyphe_icons(g: &Glyph, z: &mut Zeichner) {
    // Die Kompositions-Metapher: drei Ebenen, gestaffelt.
    z.fill(&squircle(104.0, 104.0, 52.0, 5.0), g.b);
    z.fill(&round_rect(96.0, 96.0, 200.0, 168.0, 20.0), g.a);
    z.fill(&squircle(168.0, 168.0, 34.0, 2.0), g.c);
}

pub fn mit_alpha(c: mk::Rgba, a: f32) -> mk::Rgba {
    c.mit_alpha(a)
}

/// Der lebende Avatar: Kreisfläche in primary_container mit einem
/// 3×3-Punktraster (das Matrix-Zeichen) — kein Foto, kein Buchstabe,
/// dafür bei jedem Wallpaper-Wechsel frisch aus der Palette.
/// Boot-Zeichen für Plymouth: das Matrix-Punktraster auf transparentem
/// Grund in ruhigem Warmweiß — bewusst palettenfrei, denn beim Einschalten
/// gibt es noch keinen Nutzer und kein Wallpaper.
pub fn plymouth_logo_png() -> Option<Vec<u8>> {
    let mut pixmap = Pixmap::new(SIZE, SIZE)?;
    let warmweiss = mk::Rgba { r: 0.902, g: 0.890, b: 0.851, a: 1.0 };
    let halb = mit_alpha(warmweiss, 0.45);
    let mut z = Zeichner::roh(&mut pixmap);
    let d = 30.0;
    for reihe in 0..3 {
        for spalte in 0..3 {
            let x = 128.0 - d / 2.0 + (spalte as f32 - 1.0) * 52.0;
            let y = 128.0 - d / 2.0 + (reihe as f32 - 1.0) * 52.0;
            let farbe = if (reihe + spalte) % 2 == 0 { warmweiss } else { halb };
            z.fill(&round_rect(x, y, x + d, y + d, d / 2.0), farbe);
        }
    }
    pixmap.encode_png().ok()
}

/// Ein Einzelbild der Plymouth-Ladeanimation (two-step spielt die Bilder
/// im Kreis ab): drei Punkte, deren Deckkraft sinusförmig wandert — die
/// Boot-Fassung der ruhigen Matrix-Pulse.
pub fn plymouth_animation_png(frame: u32, frames: u32) -> Option<Vec<u8>> {
    let mut pixmap = Pixmap::new(96, 20)?;
    let warmweiss = mk::Rgba { r: 0.902, g: 0.890, b: 0.851, a: 1.0 };
    let mut z = Zeichner::neu(&mut pixmap);
    let d = 12.0;
    for i in 0..3 {
        let phase = (frame as f32 / frames as f32) * std::f32::consts::TAU - i as f32 * 0.9;
        let deck = 0.30 + 0.70 * (0.5 + 0.5 * phase.sin());
        let x = 12.0 + i as f32 * 30.0;
        z.fill(&round_rect(x, 4.0, x + d, 4.0 + d, d / 2.0), mit_alpha(warmweiss, deck));
    }
    pixmap.encode_png().ok()
}

pub fn avatar_png(p: &mk::Palette) -> Option<Vec<u8>> {
    let mut pixmap = Pixmap::new(SIZE, SIZE)?;
    // Kreis = Superellipse mit Exponent 2
    let kreis = squircle(128.0, 128.0, 120.0, 2.0);
    let oben = hell(p.primary_container, 0.10);
    let unten = dunkel(p.primary_container, 0.08);
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = LinearGradient::new(
        Point::from_xy(128.0, 8.0),
        Point::from_xy(128.0, 248.0),
        vec![
            GradientStop::new(0.0, ts(oben)),
            GradientStop::new(1.0, ts(unten)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(ts(p.primary_container)));
    pixmap.fill_path(&kreis, &paint, FillRule::Winding, Transform::identity(), None);

    let voll = p.on_primary_container;
    let halb = mit_alpha(p.on_primary_container, 0.45);
    let mut z = Zeichner::roh(&mut pixmap);
    let d = 30.0;
    for reihe in 0..3 {
        for spalte in 0..3 {
            let x = 128.0 - d / 2.0 + (spalte as f32 - 1.0) * 52.0;
            let y = 128.0 - d / 2.0 + (reihe as f32 - 1.0) * 52.0;
            let farbe = if (reihe + spalte) % 2 == 0 { voll } else { halb };
            z.fill(&round_rect(x, y, x + d, y + d, d / 2.0), farbe);
        }
    }
    pixmap.encode_png().ok()
}

// ===========================================================================
// Das Rezept-System (Matrix Icon Composer, 8.7.2026): Icons als DATEN.
// Der Composer (App #22) speichert Kompositionen als JSON-Rezepte nach
// ~/.config/matrix/icons/<name>.json; render_png lädt Nutzer-Rezepte
// VOR den eingebauten Glyphen — komponierte Icons wirken sofort
// systemweit (Dock, Launcher, Mitteilungen) und leben mit der Palette,
// weil sie auf den Glyph-Tönen a/b/c aufbauen (Standard/Getönt und der
// Paletten-Fade kommen gratis).
// ===========================================================================

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "form", rename_all = "kebab-case")]
pub enum RezeptForm {
    /// Superellipse um (cx, cy): r = halbe Kante, n = Eckigkeit
    /// (2 = Kreis, 5 = Squircle-Kachel).
    Squircle { cx: f32, cy: f32, r: f32, n: f32 },
    /// Rechteck x1,y1 → x2,y2 mit Eckradius.
    RoundRect { x1: f32, y1: f32, x2: f32, y2: f32, r: f32 },
}

fn eins() -> f32 { 1.0 }
fn normal() -> String { String::from("normal") }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RezeptEbene {
    #[serde(flatten)]
    pub form: RezeptForm,
    /// Palette-Slot: "a" (primär), "b" (sekundär), "c" (tertiär).
    pub slot: String,
    /// Deckkraft 0.15–1.0 — Leitbild- Photos-Look lebt von Transluzenz.
    #[serde(default = "eins")]
    pub deckkraft: f32,
    /// "normal" | "multiplizieren" — Überlappungen werden satter.
    #[serde(default = "normal")]
    pub mischung: String,
    /// Glanzlicht an der Oberkante (Specular-Fake: helle Lage dy −2).
    #[serde(default)]
    pub glanz: bool,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Rezept {
    pub ebenen: Vec<RezeptEbene>,
}

pub fn rezept_pfad(name: &str) -> std::path::PathBuf {
    let heim = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(format!("{heim}/.config/matrix/icons/{name}.json"))
}

pub fn rezept_laden(name: &str) -> Option<Rezept> {
    let raw = std::fs::read_to_string(rezept_pfad(name)).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn rezept_speichern(name: &str, rezept: &Rezept) -> std::io::Result<()> {
    let pfad = rezept_pfad(name);
    if let Some(eltern) = pfad.parent() {
        std::fs::create_dir_all(eltern)?;
    }
    std::fs::write(pfad, serde_json::to_string_pretty(rezept).unwrap_or_default())
}

fn rezept_zeichnen(rezept: &Rezept, g: &Glyph, z: &mut Zeichner) {
    for ebene in &rezept.ebenen {
        let basis = match ebene.slot.as_str() {
            "b" => g.b,
            "c" => g.c,
            _ => g.a,
        };
        let farbe = mk::Rgba {
            a: basis.a * ebene.deckkraft.clamp(0.15, 1.0),
            ..basis
        };
        let blend = if ebene.mischung == "multiplizieren" {
            BlendMode::Multiply
        } else {
            BlendMode::SourceOver
        };
        let pfad = match &ebene.form {
            RezeptForm::Squircle { cx, cy, r, n } => squircle(*cx, *cy, *r, *n),
            RezeptForm::RoundRect { x1, y1, x2, y2, r } => round_rect(*x1, *y1, *x2, *y2, *r),
        };
        z.fill_ex(&pfad, farbe, blend, ebene.glanz);
    }
}

/// Ein Rezept vollwertig rendern (Kachel + Schattenlage + Glyphe) —
/// exakt der Weg der eingebauten Icons; auch die Composer-Vorschau.
pub fn rezept_png(rezept: &Rezept, palette: &mk::Palette) -> Option<Vec<u8>> {
    let glyph = match stil() {
        Stil::Standard => Glyph {
            a: palette.primary,
            b: palette.secondary,
            c: palette.tertiary,
        },
        Stil::Getoent => Glyph {
            a: mit_alpha(palette.on_surface, 1.0),
            b: mit_alpha(palette.on_surface, 0.72),
            c: mit_alpha(palette.on_surface, 0.5),
        },
    };
    let mut pixmap = Pixmap::new(SIZE, SIZE)?;
    kachel(palette, &mut pixmap);
    rezept_zeichnen(rezept, &glyph, &mut Zeichner::neu(&mut pixmap));
    pixmap.encode_png().ok()
}
