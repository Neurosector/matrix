//! MatrixKit Lebende Icons 2.0 — App-Icons, die mit dem Wallpaper atmen.
//!
//! Nach dem Leitbild-Prinzip sind Icons hier GETRENNTE EBENEN statt fertiger
//! Bilder: eine Kachel (Squircle mit Tiefe) und eine Glyphe (das Motiv).
//! Daraus entstehen Varianten — Standard (farbig) und Getönt (monochrome
//! Glyphe, wie Leitbild- Tinted Mode), wählbar über ~/.config/matrix/icon-stil.
//!
//! Die Kachel ist eine echte Superellipse (kontinuierliche Krümmung — der
//! unterschwellige Leitbild-Look, den Rundrechtecke nie treffen) mit feinem
//! Vertikalverlauf und Lichtkante; die Glyphe wirft einen weichen Kernschatten.

use matrixkit_theme as mk;
use std::path::PathBuf;
use std::time::Duration;
use tiny_skia::{Pixmap, Transform};

/// Master-Leinwand der App-Icons; kleinere hicolor-Größen werden
/// hochwertig herunterskaliert (Launcher/Dock/Leisten nutzen verschiedene).
const SIZE: u32 = 256;
const ALLE_GROESSEN: &[u32] = &[48, 64, 128, 256];

use matrixkit_icons::*;

fn main() {
    // System-Modus (Image-Build): alle Icons mit der Standard-Palette in
    // einen Icon-Root rendern — stabile Namen, keine versionierten Kopien,
    // keine Desktop-/Avatar-Nebenwirkungen. So bringt das Image für JEDES
    // (auch neue) Konto ein statisches Fallback-Icon aller Apps mit; die
    // lebende Umfärbung übernimmt später der Farb-Sync pro Nutzer.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--system") {
        let root = PathBuf::from(args.get(i + 1).cloned().unwrap_or_else(|| "/usr/share/icons".into()));
        system_rendern(&root);
        return;
    }

    // Plymouth-Modus (Image-Build): Boot-Zeichen + Puls-Punkt für das
    // Matrix-Boot-Theme rendern — palettenfrei (siehe lib::plymouth_logo_png).
    if let Some(i) = args.iter().position(|a| a == "--plymouth") {
        let dir = PathBuf::from(
            args.get(i + 1).cloned().unwrap_or_else(|| "/usr/share/plymouth/themes/matrix".into()),
        );
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("matrixkit-icons: Plymouth-Verzeichnis nicht anlegbar: {e}");
            std::process::exit(1);
        }
        let Some(logo) = plymouth_logo_png() else {
            eprintln!("matrixkit-icons: Plymouth-Zeichen nicht renderbar");
            std::process::exit(1);
        };
        let _ = std::fs::write(dir.join("watermark.png"), logo);
        // two-step-Konvention (siehe spinner-Theme): throbber-* ist die
        // LAUFENDE Puls-Animation, animation-* die Endanimation. Ohne
        // throbber malt das Plugin nur ein Standbild oben links.
        const FRAMES: u32 = 30;
        for f in 0..FRAMES {
            if let Some(png) = plymouth_animation_png(f, FRAMES) {
                let _ = std::fs::write(dir.join(format!("throbber-{:04}.png", f + 1)), &png);
                let _ = std::fs::write(dir.join(format!("animation-{:04}.png", f + 1)), &png);
            }
        }
        println!("{}", dir.join("watermark.png").display());
        return;
    }

    // Wettlauf-Schutz: der Sync feuert, während matugen die Farbdatei noch
    // schreibt. Notfalls (Datei fehlt ganz) mit der Fallback-Palette zeichnen.
    let palette = mk::Palette::load_settled(Duration::from_secs(3)).unwrap_or_default();

    let dir = icon_dir(SIZE);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("matrixkit-icons: Icon-Verzeichnis nicht anlegbar: {e}");
        std::process::exit(1);
    }

    let glyph = match stil() {
        Stil::Standard => Glyph {
            a: palette.primary,
            b: palette.secondary,
            c: palette.tertiary,
        },
        // Getönt: monochrome Glyphe in Abstufungen der Textfarbe
        Stil::Getoent => Glyph {
            a: mit_alpha(palette.on_surface, 1.0),
            b: mit_alpha(palette.on_surface, 0.72),
            c: mit_alpha(palette.on_surface, 0.5),
        },
    };

    for spec in ICONS {
        let mut pixmap = Pixmap::new(SIZE, SIZE).expect("Pixmap");
        kachel(&palette, &mut pixmap);
        // Ebenen-Schatten + Material macht der Zeichner selbst.
        (spec.glyphe)(&glyph, &mut Zeichner::neu(&mut pixmap));

        let png = match pixmap.encode_png() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("matrixkit-icons: {} nicht kodierbar: {e}", spec.name);
                continue;
            }
        };
        // Stabiler Name in ALLEN hicolor-Größen — für klassische Theme-Lookups.
        for &g in ALLE_GROESSEN {
            let gdir = icon_dir(g);
            let _ = std::fs::create_dir_all(&gdir);
            let stable = gdir.join(format!("{}.png", spec.name));
            let bytes = if g == SIZE {
                png.clone()
            } else {
                let mut klein = Pixmap::new(g, g).expect("Pixmap");
                let s = g as f32 / SIZE as f32;
                klein.draw_pixmap(
                    0,
                    0,
                    pixmap.as_ref(),
                    &tiny_skia::PixmapPaint {
                        quality: tiny_skia::FilterQuality::Bilinear,
                        ..Default::default()
                    },
                    Transform::from_scale(s, s),
                    None,
                );
                match klein.encode_png() {
                    Ok(b) => b,
                    Err(_) => continue,
                }
            };
            if let Err(e) = std::fs::write(&stable, &bytes) {
                eprintln!("matrixkit-icons: {} nicht schreibbar: {e}", stable.display());
            }
        }
        // Versionierter Name — Cache-Buster für laufende Shells: Quickshell/Qt
        // cachen Bilder PRO PFAD; ein Farb-Hash im Dateinamen macht jeden
        // Palettenstand zu einem frischen Pfad.
        let versioned_name = format!("{}-{:08x}.png", spec.name, fnv1a(&png));
        let versioned = dir.join(&versioned_name);
        if !versioned.exists() {
            let _ = std::fs::write(&versioned, &png);
        }
        prune_old_versions(&dir, spec.name, &versioned_name);
        // Icon= im Nutzer-Desktop-Eintrag auf den frischen Pfad drehen —
        // das stößt zugleich DMS' Desktop-Entry-Watcher an (Neuauflösung).
        update_desktop_icon(spec.name, &versioned);
    }

    // Lebender Avatar (Desktop, Lockscreen, Greeter): nur wenn der Nutzer
    // ihn per Marker aktiviert hat — ein eigenes Foto in ~/.face wird nie
    // ueberschrieben (Marker loeschen = eigenes Bild verwenden).
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    if std::path::Path::new(&format!("{home}/.config/matrix/avatar-lebendig")).exists() {
        if let Some(png) = avatar_png(&palette) {
            let _ = std::fs::write(format!("{home}/.face"), png);
        }
    }

    // Icon-Theme-Cache auffrischen (best effort).
    if let Some(icons_root) = dir.ancestors().nth(2) {
        let _ = std::process::Command::new("gtk-update-icon-cache")
            .arg(icons_root)
            .output();
    }
}

/// Ein Icon (Kachel + Schatten + Glyphe) mit gegebener Palette rendern.
fn icon_pixmap(palette: &mk::Palette, spec: &IconSpec) -> Pixmap {
    let glyph = Glyph { a: palette.primary, b: palette.secondary, c: palette.tertiary };
    let mut pixmap = Pixmap::new(SIZE, SIZE).expect("Pixmap");
    kachel(palette, &mut pixmap);
    (spec.glyphe)(&glyph, &mut Zeichner::neu(&mut pixmap));
    pixmap
}

/// Alle App-Icons in `<root>/hicolor/<g>x<g>/apps/<name>.png` schreiben.
/// Für den Image-Build (Standard-Palette, stabile Namen, sonst nichts).
fn system_rendern(root: &std::path::Path) {
    let palette = mk::Palette::default();
    for spec in ICONS {
        let pixmap = icon_pixmap(&palette, spec);
        let png_gross = match pixmap.encode_png() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("matrixkit-icons: {} nicht kodierbar: {e}", spec.name);
                continue;
            }
        };
        for &g in ALLE_GROESSEN {
            let gdir = root.join(format!("hicolor/{g}x{g}/apps"));
            if std::fs::create_dir_all(&gdir).is_err() {
                continue;
            }
            let ziel = gdir.join(format!("{}.png", spec.name));
            let bytes = if g == SIZE {
                png_gross.clone()
            } else {
                let mut klein = Pixmap::new(g, g).expect("Pixmap");
                let s = g as f32 / SIZE as f32;
                klein.draw_pixmap(
                    0,
                    0,
                    pixmap.as_ref(),
                    &tiny_skia::PixmapPaint {
                        quality: tiny_skia::FilterQuality::Bilinear,
                        ..Default::default()
                    },
                    Transform::from_scale(s, s),
                    None,
                );
                match klein.encode_png() {
                    Ok(b) => b,
                    Err(_) => continue,
                }
            };
            if let Err(e) = std::fs::write(&ziel, &bytes) {
                eprintln!("matrixkit-icons: {} nicht schreibbar: {e}", ziel.display());
            }
        }
    }
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .arg(root.join("hicolor"))
        .output();
}

// --- Verwaltung (Cache-Buster, Desktop-Einträge) -----------------------------

/// FNV-1a über die PNG-Bytes — gleiche Palette ergibt denselben Namen,
/// also kein Dateien-Churn bei unverändertem Theme.
fn fnv1a(data: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in data {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Ältere versionierte Icon-Stände desselben Namens entfernen.
fn prune_old_versions(dir: &std::path::Path, name: &str, keep: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let prefix = format!("{name}-");
    for e in entries.flatten() {
        let f = e.file_name();
        let Some(f) = f.to_str() else { continue };
        if f.starts_with(&prefix) && f.ends_with(".png") && f != keep {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// `Icon=` des Nutzer-Desktop-Eintrags auf den versionierten Pfad setzen.
/// Fehlt der Nutzer-Eintrag, wird der System-Eintrag als Vorlage kopiert.
/// Atomar geschrieben (tmp + rename) — halbe .desktop-Dateien würden den
/// Desktop-Entry-Watcher der Shell mit Parse-Müll füttern.
fn update_desktop_icon(name: &str, icon_path: &std::path::Path) {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    let user_dir = PathBuf::from(&home).join(".local/share/applications");
    let user_entry = user_dir.join(format!("{name}.desktop"));
    let template = if user_entry.exists() {
        user_entry.clone()
    } else {
        let sys = PathBuf::from(format!("/usr/share/applications/{name}.desktop"));
        if !sys.exists() {
            return; // App (noch) nicht installiert — nichts zu tun
        }
        sys
    };
    let Ok(content) = std::fs::read_to_string(&template) else { return };
    let new_line = format!("Icon={}", icon_path.display());
    let mut replaced = false;
    let mut out: Vec<String> = content
        .lines()
        .map(|l| {
            if l.starts_with("Icon=") {
                replaced = true;
                new_line.clone()
            } else {
                l.to_string()
            }
        })
        .collect();
    if !replaced {
        out.push(new_line);
    }
    let new_content = out.join("\n") + "\n";
    if new_content == content {
        return; // schon aktuell — Watcher nicht unnötig wecken
    }
    if std::fs::create_dir_all(&user_dir).is_err() {
        return;
    }
    let tmp = user_dir.join(format!(".{name}.desktop.neu"));
    if std::fs::write(&tmp, new_content).is_ok() {
        let _ = std::fs::rename(&tmp, &user_entry);
    }
}

fn icon_dir(size: u32) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(format!(".local/share/icons/hicolor/{size}x{size}/apps"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sysmon_icon_traegt_palettenfarben() {
        let p = mk::Palette::default();
        let g = Glyph { a: p.primary, b: p.secondary, c: p.tertiary };
        let mut pixmap = Pixmap::new(SIZE, SIZE).unwrap();
        kachel(&p, &mut pixmap);
        glyphe_sysmon(&g, &mut Zeichner::neu(&mut pixmap));
        // Balken 1 (Mitte) trägt primary — mit Rundungstoleranz, denn
        // Premultiplied-Alpha + Antialiasing dürfen ±4 kosten.
        let px = pixmap.pixel(90, 88).unwrap();
        let soll = (p.primary.r * 255.0).round() as i32;
        assert!((px.red() as i32 - soll).abs() <= 4, "rot {} vs. soll {soll}", px.red());
        // Ecken bleiben transparent (Squircle-Kachel)
        assert_eq!(pixmap.pixel(2, 2).unwrap().alpha(), 0);
        // Kachelgrund ist ein Verlauf: oben heller als unten
        let oben = pixmap.pixel(128, 20).unwrap();
        let unten = pixmap.pixel(128, 236).unwrap();
        assert!(oben.red() > unten.red() || oben.green() > unten.green());
    }
}
