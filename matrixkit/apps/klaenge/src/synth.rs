//! Matrix-Systemklänge — von Grund auf synthetisiert, ohne Fremdbibliotheken.
//!
//! Philosophie: Klänge sind Code. Jeder Systemklang ist eine kleine,
//! deterministische Rezeptur (Töne, Hüllkurven, gefiltertes Rauschen) —
//! reproduzierbar, versionierbar, palettenhaft aufeinander abgestimmt.
//!
//! Klangsprache (v4 „Gong-Universum", Nutzer 15.7.): ALLE Klänge sind
//! Verwandte des F♯-Dur-Gongs — dasselbe Instrument, dieselbe Wärme,
//! dazu die Gong-Signatur: ein leiser Oktav-Schimmer über dem Hauptton.
//! Bedeutung entsteht aus Richtung, Rhythmus und Intervall (aufwärts =
//! ankommen, abwärts = gehen, Tritonus = Spannung), nie aus Härte.
//! Pegel v4: hörbar statt schüchtern (Nutzer-Befund: zu leise).
//!
//! Als Modul der App „Matrix Klänge" eingebaut; der Generator-Modus
//! (`matrix-klaenge --generieren <ordner>`) nutzt dieselben Rezepturen.

const SR: f32 = 48_000.0;

/// Ein Mono-Klang in 32-Bit-Float, am Ende als 16-Bit-WAV geschrieben.
struct Klang {
    samples: Vec<f32>,
}

impl Klang {
    fn neu(dauer_s: f32) -> Self {
        Self { samples: vec![0.0; (dauer_s * SR) as usize] }
    }

    /// Runder Ton: Sinus-Grundton mit sanft verstimmtem Zwilling (leise
    /// Schwebung), warmer Sub-Oktave und einem Hauch 2. Teilton. Weicher
    /// exponentieller Attack, langes natürliches Ausklingen. Additiv —
    /// Töne dürfen sich überlagern (Akkorde, Arpeggien).
    fn ton(&mut self, start_s: f32, freq: f32, amp: f32, attack_s: f32, tau_s: f32) {
        let start = (start_s * SR) as usize;
        let n = self.samples.len();
        for i in start..n {
            let t = (i - start) as f32 / SR;
            // Weicher Einsatz: exponentielle Annäherung statt harter Rampe
            let env_a = 1.0 - (-t / attack_s.max(0.002)).exp();
            let env_d = (-t / tau_s).exp();
            if env_d < 0.0005 {
                break;
            }
            let w = std::f32::consts::TAU * freq * t;
            let w2 = std::f32::consts::TAU * freq * 1.0025 * t; // Schwebung
            let s = 0.65 * w.sin()
                + 0.35 * w2.sin()
                + 0.20 * (0.5 * w).sin() // Sub-Oktave: Waerme
                + 0.06 * (2.0 * w).sin() * (-t / (tau_s * 0.4)).exp();
            self.samples[i] += amp * env_a * env_d * s;
        }
    }

    /// Gefiltertes Rauschen (Ein-Pol-Tiefpass): für Luft, Klicks, Wischen.
    /// `tiefpass` 0..1 — klein = dumpf, groß = hell. `sweep` verschiebt die
    /// Filteröffnung über die Dauer (positiv = öffnet, negativ = schließt).
    fn rauschen(
        &mut self,
        start_s: f32,
        dauer_s: f32,
        amp: f32,
        attack_s: f32,
        tau_s: f32,
        tiefpass: f32,
        sweep: f32,
    ) {
        let start = (start_s * SR) as usize;
        let ende = ((start_s + dauer_s) * SR) as usize;
        let mut zufall = 0x9e37_79b9_u32; // deterministisch — Klänge als Code
        let mut y = 0.0f32;
        for i in start..ende.min(self.samples.len()) {
            let t = (i - start) as f32 / SR;
            zufall ^= zufall << 13;
            zufall ^= zufall >> 17;
            zufall ^= zufall << 5;
            let x = (zufall as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let a = (tiefpass + sweep * (t / dauer_s)).clamp(0.02, 0.95);
            y += a * (x - y);
            let env = (t / attack_s.max(0.001)).min(1.0) * (-t / tau_s).exp();
            self.samples[i] += amp * env * y;
        }
    }

    /// Auf Zielpegel normalisieren + kurze Schutz-Blende am Ende.
    fn abschliessen(&mut self, pegel: f32) {
        let max = self.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        if max > 0.0 {
            let f = pegel / max;
            for s in &mut self.samples {
                *s *= f;
            }
        }
        let blende = (0.04 * SR) as usize;
        let n = self.samples.len();
        for i in 0..blende.min(n) {
            self.samples[n - 1 - i] *= i as f32 / blende as f32;
        }
    }

    /// WAV (PCM 16-bit mono) — der Container, den alles abspielen kann.
    fn schreibe(&self, pfad: &std::path::Path) -> std::io::Result<()> {
        let daten_bytes = (self.samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + daten_bytes as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + daten_bytes).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes()); // fmt-Länge
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&(SR as u32).to_le_bytes());
        out.extend_from_slice(&((SR as u32) * 2).to_le_bytes()); // Byterate
        out.extend_from_slice(&2u16.to_le_bytes()); // Blockgröße
        out.extend_from_slice(&16u16.to_le_bytes()); // Bittiefe
        out.extend_from_slice(b"data");
        out.extend_from_slice(&daten_bytes.to_le_bytes());
        for s in &self.samples {
            out.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
        }
        std::fs::write(pfad, out)
    }
}

/// Alle Klänge in den Zielordner rendern. Deterministisch — zweimal
/// generieren ergibt bitidentische Dateien.
pub fn generieren(ziel: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(ziel)?;

    // Tonvorrat: F♯-Dur über vier Oktaven — der Akkord des Gongs ist
    // das Universum, alle Klänge sind seine Kinder.
    let (fis2, dis3, fis3, gis3, ais3) = (92.50, 155.56, 185.0, 207.65, 233.08);
    let (cis4, fis4, cis5) = (277.18, 369.99, 554.37);
    // Die Gong-Signatur: Hauptton + leiser Schimmer eine Oktave darüber.
    fn glocke(k: &mut Klang, start: f32, freq: f32, amp: f32, tau: f32) {
        k.ton(start, freq, amp, 0.010, tau);
        k.ton(start + 0.004, freq * 2.0, amp * 0.28, 0.012, tau * 0.7);
    }
    let klaenge: Vec<(&str, Klang)> = vec![
        ("00-gong", {
            // Der Matrix-Gong (Leitbild-Extrakt, R31) — die Referenz des
            // Universums: F♯-Dur über drei Oktaven, EIN Schlag, minimal
            // versetzt wie ein echter Anschlag, würdevoll ausklingend.
            let mut k = Klang::neu(3.6);
            k.ton(0.000, fis2, 0.70, 0.018, 1.90);
            k.ton(0.004, fis3, 0.85, 0.012, 1.50);
            k.ton(0.008, ais3, 0.70, 0.012, 1.40);
            k.ton(0.012, cis4, 0.65, 0.012, 1.30);
            k.ton(0.016, fis4, 0.50, 0.014, 1.10);
            k
        }),
        ("01-anmeldung", {
            // Ankommen: das Gong-Arpeggio aufwärts — der Akkord baut
            // sich Ton für Ton auf, das Fundament trägt.
            let mut k = Klang::neu(2.6);
            k.ton(0.00, fis2, 0.45, 0.014, 1.40);
            glocke(&mut k, 0.00, fis3, 0.80, 0.55);
            glocke(&mut k, 0.22, ais3, 0.75, 0.65);
            glocke(&mut k, 0.44, cis4, 0.70, 1.10);
            k
        }),
        ("02-abmeldung", {
            // Gehen: dasselbe Arpeggio rückwärts, das Fundament bleibt
            // als Letztes im Raum.
            let mut k = Klang::neu(2.6);
            glocke(&mut k, 0.00, cis4, 0.70, 0.45);
            glocke(&mut k, 0.22, ais3, 0.75, 0.55);
            glocke(&mut k, 0.44, fis3, 0.80, 1.00);
            k.ton(0.46, fis2, 0.50, 0.016, 1.30);
            k
        }),
        ("03-benachrichtigung", {
            // Gute Nachricht: Quinte aufwärts, hell und kurz — zwei
            // Gong-Kinder.
            let mut k = Klang::neu(1.6);
            glocke(&mut k, 0.00, cis4, 0.75, 0.28);
            glocke(&mut k, 0.16, fis4, 0.80, 0.70);
            k
        }),
        ("04-benachrichtigung-leise", {
            // Der kleine Bruder: ein einzelner cis4-Anschlag mit
            // fis3-Wärme darunter.
            let mut k = Klang::neu(1.3);
            k.ton(0.00, fis3, 0.35, 0.012, 0.50);
            glocke(&mut k, 0.00, cis4, 0.80, 0.55);
            k
        }),
        ("05-lautstaerke", {
            // Der Pegel-Tupfer: ein kurzer fis4-Punkt — hell genug zum
            // Hören, kurz genug zum Wiederholen.
            let mut k = Klang::neu(0.5);
            k.ton(0.0, fis3, 0.30, 0.006, 0.10);
            k.ton(0.0, fis4, 0.85, 0.005, 0.12);
            k
        }),
        ("06-geraet-verbunden", {
            // Angesteckt: Quinte aufwärts im Fundament-Register.
            let mut k = Klang::neu(1.5);
            glocke(&mut k, 0.00, fis3, 0.75, 0.28);
            glocke(&mut k, 0.16, cis4, 0.80, 0.60);
            k
        }),
        ("07-geraet-getrennt", {
            // Abgezogen: dieselbe Quinte, rückwärts.
            let mut k = Klang::neu(1.5);
            glocke(&mut k, 0.00, cis4, 0.75, 0.28);
            glocke(&mut k, 0.16, fis3, 0.80, 0.65);
            k
        }),
        ("08-arbeitsflaeche", {
            // Arbeitsflächen-Wechsel: ein weicher Terz-Hauch — der
            // leiseste Verwandte, kaum mehr als ein Atem des Gongs.
            let mut k = Klang::neu(0.9);
            k.ton(0.00, fis3, 0.60, 0.020, 0.30);
            k.ton(0.03, ais3, 0.45, 0.025, 0.35);
            k
        }),
        ("09-screenshot", {
            // Der Auslöser: zwei sehr kurze helle Anschläge — die
            // Kamera-Mechanik, übersetzt in Glockensprache.
            let mut k = Klang::neu(0.7);
            k.rauschen(0.0, 0.025, 0.35, 0.002, 0.012, 0.45, 0.0);
            k.ton(0.000, fis4, 0.85, 0.003, 0.07);
            k.ton(0.045, cis5, 0.60, 0.003, 0.14);
            k
        }),
        ("10-papierkorb", {
            // Leeren: zwei dunkle Stufen abwärts, die im Fundament
            // landen — endgültig, ohne Drama.
            let mut k = Klang::neu(1.4);
            glocke(&mut k, 0.00, dis3, 0.70, 0.25);
            glocke(&mut k, 0.20, fis2, 0.85, 0.70);
            k
        }),
        ("15-wurf", {
            // Der Dateimanager-Referenz-Plopp (R37): EIN dumpfer fis2-Punkt, kurz.
            let mut k = Klang::neu(0.5);
            k.rauschen(0.0, 0.04, 0.30, 0.003, 0.018, 0.20, 0.0);
            k.ton(0.01, fis2, 0.90, 0.005, 0.14);
            k
        }),
        ("11-fertig", {
            // Erledigt: die Dur-Auflösung — Terz und Quinte finden
            // aufwärts zusammen.
            let mut k = Klang::neu(1.7);
            glocke(&mut k, 0.00, ais3, 0.75, 0.22);
            glocke(&mut k, 0.16, cis4, 0.70, 0.30);
            glocke(&mut k, 0.32, fis4, 0.80, 0.80);
            k
        }),
        ("12-fehler", {
            // Ernstfall: zwei gedämpfte Schritte ABWÄRTS aus dem
            // Universum (gis→fis) — geerdet, nie schrill; die Moll-
            // Färbung kommt aus der Richtung, nicht aus Härte.
            let mut k = Klang::neu(1.4);
            k.ton(0.00, gis3, 0.80, 0.008, 0.18);
            k.ton(0.20, fis3, 0.85, 0.010, 0.32);
            k.ton(0.22, fis2, 0.40, 0.014, 0.50);
            k
        }),
        ("13-schluessel-erkannt", {
            // Der Wächter erkennt den Stick — die Sicherheits-Signatur
            // (Tritonus spannt, Quinte löst), transponiert ins
            // F♯-Universum. Bewusst auffällig: die Umgebung soll eine
            // Schlüssel-Nutzung hören.
            let c4_tritonus = 261.63; // Tritonus zu fis3 — Spannung von außen
            let mut k = Klang::neu(4.0);
            k.ton(0.00, fis2, 0.55, 0.020, 1.60);
            glocke(&mut k, 0.08, fis3, 0.80, 0.30);
            glocke(&mut k, 0.33, c4_tritonus, 0.80, 0.30);
            glocke(&mut k, 0.58, cis4, 0.85, 0.70);
            glocke(&mut k, 1.20, fis4, 0.80, 0.40);
            glocke(&mut k, 1.45, cis5, 0.85, 1.00);
            glocke(&mut k, 2.25, fis3, 0.80, 0.22);
            glocke(&mut k, 2.50, fis3, 0.85, 0.90);
            k
        }),
        ("14-waechter-ruf", {
            // Der Ruf der Wiederherstellung: zweifacher Ruf mit
            // Tritonus-Spannung (gis → d → gis'), die sich erst im
            // zweiten Anlauf löst — unüberhörbar, würdevoll.
            let d4_tritonus = 293.66; // Tritonus zu gis3
            let gis4 = 415.30;
            let mut k = Klang::neu(4.0);
            glocke(&mut k, 0.00, gis3, 0.80, 0.25);
            glocke(&mut k, 0.25, d4_tritonus, 0.80, 0.25);
            glocke(&mut k, 0.50, gis4, 0.85, 0.60);
            glocke(&mut k, 1.20, gis3, 0.80, 0.25);
            glocke(&mut k, 1.45, d4_tritonus, 0.80, 0.25);
            glocke(&mut k, 1.70, gis4, 0.85, 1.00);
            k.ton(2.50, fis2, 0.60, 0.015, 1.20);
            k
        }),
    ];

    for (name, mut klang) in klaenge {
        // Pegel v4 (Nutzer: „alle so leise") — hörbar statt schüchtern.
        // Hierarchie bleibt: Gong füllt den Raum, der Wächter darf rufen,
        // der Pegel-Tupfer wiederholt sich und bleibt eine Stufe zurück.
        let pegel = match name {
            "13-schluessel-erkannt" => 0.78,
            "14-waechter-ruf" => 0.82,
            "00-gong" => 0.72,
            "05-lautstaerke" => 0.45,
            "08-arbeitsflaeche" => 0.42,
            _ => 0.58,
        };
        klang.abschliessen(pegel);
        klang.schreibe(&ziel.join(format!("{name}.wav")))?;
    }
    Ok(())
}
