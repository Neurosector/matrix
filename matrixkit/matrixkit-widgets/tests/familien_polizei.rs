//! Die Familien-Polizei (R47): Konsistenz ist eine Eigenschaft des
//! Builds, keine Tugend. Dieser Test durchsucht die App-Quelltexte
//! nach Mustern, die an den MatrixKit-Familien vorbeibauen — und wird
//! rot, wenn jemand (auch die KI) es wieder tut.
//!
//! Ausnahmen sind erlaubt, aber nur DEKLARIERT: eine Zeile
//! `// familien-ausnahme: <Grund>` direkt über dem Fund lässt ihn
//! passieren. So bleibt Abweichung eine bewusste Entscheidung mit
//! Begründung im Code — nie wieder stiller Wildwuchs.

use std::path::{Path, PathBuf};

struct Regel {
    name: &'static str,
    /// Substring ODER (bei `regex_zahl`) Muster-Erkennung per Hand.
    muster: &'static str,
    familie: &'static str,
}

const REGELN: &[Regel] = &[
    Regel {
        name: "rohes text_input",
        muster: "text_input(",
        familie: "mkw::eingabefeld / mkw::suchfeld",
    },
    Regel {
        name: "handgerollter Knopf-Stil",
        muster: "button::Style {",
        familie: "mkw::knopf (Stil x Rolle x Groesse) / mkw::ui::werkzeug_knopf",
    },
    Regel {
        name: "Mono-Schrift-Literal",
        muster: "Font::with_name(\"Maple Mono",
        familie: "mkw::mono() / mkw::mono_font_laden()",
    },
    Regel {
        name: "999er-Pseudo-Kapsel",
        muster: "radius: 999",
        familie: "mk::radius::kapsel(hoehe)",
    },
];

fn apps_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../apps")
}

fn rs_dateien(dir: &Path, sammel: &mut Vec<PathBuf>) {
    let Ok(eintraege) = std::fs::read_dir(dir) else {
        return;
    };
    for e in eintraege.flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_dateien(&p, sammel);
        } else if p.extension().is_some_and(|e| e == "rs") {
            sammel.push(p);
        }
    }
}

/// Zeile `i` ist begnadigt, wenn direkt darüber eine deklarierte
/// Ausnahme steht (Leerzeilen dazwischen zählen nicht).
fn begnadigt(zeilen: &[&str], i: usize) -> bool {
    i > 0 && zeilen[i - 1].contains("familien-ausnahme:")
}

/// Nacktes Zahlen-Literal als Symbolgröße? (mkw::symbol(X, 18.0, ...))
fn symbol_literal(zeile: &str) -> bool {
    let Some(pos) = zeile.find("mkw::symbol") else {
        return false;
    };
    let rest = &zeile[pos..];
    // erstes Komma nach der öffnenden Klammer, dann Ziffer + Punkt?
    let Some(klammer) = rest.find('(') else {
        return false;
    };
    let Some(komma) = rest[klammer..].find(',') else {
        return false;
    };
    let nach = rest[klammer + komma + 1..].trim_start();
    nach.chars().next().is_some_and(|c| c.is_ascii_digit())
        && nach.contains('.')
}

#[test]
fn familien_polizei() {
    let mut dateien = Vec::new();
    rs_dateien(&apps_dir(), &mut dateien);
    assert!(
        dateien.len() > 10,
        "Apps-Verzeichnis nicht gefunden — Polizei kann nicht patrouillieren"
    );

    let mut funde: Vec<String> = Vec::new();
    for pfad in &dateien {
        let Ok(inhalt) = std::fs::read_to_string(pfad) else {
            continue;
        };
        let zeilen: Vec<&str> = inhalt.lines().collect();
        for (i, zeile) in zeilen.iter().enumerate() {
            if zeile.trim_start().starts_with("//") {
                continue;
            }
            for regel in REGELN {
                if zeile.contains(regel.muster) && !begnadigt(&zeilen, i) {
                    funde.push(format!(
                        "{}:{} — {} (nimm {})",
                        pfad.display(),
                        i + 1,
                        regel.name,
                        regel.familie
                    ));
                }
            }
            if symbol_literal(zeile) && !begnadigt(&zeilen, i) {
                funde.push(format!(
                    "{}:{} — nacktes Symbolgrößen-Literal (nimm mk::icon_size oder eine benannte Konstante)",
                    pfad.display(),
                    i + 1
                ));
            }
        }
    }

    assert!(
        funde.is_empty(),
        "\n\nDie Familien-Polizei hat {} Verstoß/Verstöße gefunden:\n\n{}\n\n\
         Entweder auf die Familie umziehen oder direkt darüber deklarieren:\n\
         // familien-ausnahme: <Grund>\n",
        funde.len(),
        funde.join("\n")
    );
}
