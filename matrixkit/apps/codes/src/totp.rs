//! TOTP-Kern (RFC 6238) — von Grund auf in Rust, ohne Krypto-Fremdcode.
//!
//! Wie „Klänge als Code": SHA-1, HMAC und Base32 sind offene, exakt
//! spezifizierte Standards — und mit den offiziellen RFC-Testvektoren
//! (unten als #[test]) beweisbar korrekt. So bleibt der Authenticator
//! transparent und abhängigkeitsfrei.

/// SHA-1 (RFC 3174).
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let ml = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&ml.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, hi) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&hi.to_be_bytes());
    }
    out
}

/// HMAC-SHA1 (RFC 2104), Blockgröße 64.
fn hmac_sha1(key: &[u8], msg: &[u8]) -> [u8; 20] {
    let mut k = if key.len() > 64 {
        sha1(key).to_vec()
    } else {
        key.to_vec()
    };
    k.resize(64, 0);
    let mut inner: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    inner.extend_from_slice(msg);
    let ih = sha1(&inner);
    let mut outer: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();
    outer.extend_from_slice(&ih);
    sha1(&outer)
}

/// Base32-Dekodierung (RFC 4648) — case-insensitiv, Padding/Leerzeichen egal.
pub fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let mut bits = 0u32;
    let mut val = 0u32;
    let mut out = Vec::new();
    for c in s.chars() {
        if c == '=' || c.is_whitespace() {
            continue;
        }
        let d = match c.to_ascii_uppercase() {
            up @ 'A'..='Z' => up as u32 - 'A' as u32,
            dg @ '2'..='7' => dg as u32 - '2' as u32 + 26,
            _ => return None,
        };
        val = (val << 5) | d;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((val >> bits) as u8);
        }
    }
    (!out.is_empty()).then_some(out)
}

/// TOTP-Code (RFC 6238) für einen Zeitpunkt.
pub fn totp(secret: &[u8], zeit_s: u64, periode: u64, stellen: u32) -> String {
    let counter = zeit_s / periode.max(1);
    let hash = hmac_sha1(secret, &counter.to_be_bytes());
    let offset = (hash[19] & 0x0f) as usize;
    let bin = ((hash[offset] as u32 & 0x7f) << 24)
        | ((hash[offset + 1] as u32) << 16)
        | ((hash[offset + 2] as u32) << 8)
        | (hash[offset + 3] as u32);
    let code = bin % 10u32.pow(stellen);
    format!("{code:0width$}", width = stellen as usize)
}

/// Ein Konto: Anzeige-Daten + dekodiertes Geheimnis.
#[derive(Clone)]
pub struct Konto {
    pub name: String,
    pub issuer: String,
    pub secret_b32: String,
    pub secret: Vec<u8>,
    pub stellen: u32,
    pub periode: u64,
}

impl Konto {
    /// Aus einem base32-Geheimnis + Namen bauen (None bei ungültigem Secret).
    pub fn neu(name: &str, issuer: &str, secret_b32: &str, stellen: u32, periode: u64) -> Option<Self> {
        let secret = base32_decode(secret_b32)?;
        Some(Self {
            name: name.trim().to_string(),
            issuer: issuer.trim().to_string(),
            secret_b32: secret_b32.split_whitespace().collect(),
            secret,
            stellen,
            periode,
        })
    }

    /// otpauth://totp/LABEL?secret=…&issuer=…&digits=…&period=… ODER ein
    /// nacktes base32-Geheimnis (dann Name „Konto“).
    pub fn aus_eingabe(text: &str) -> Option<Self> {
        let text = text.trim();
        if let Some(rest) = text.strip_prefix("otpauth://totp/") {
            let (label, query) = rest.split_once('?').unwrap_or((rest, ""));
            let label = pct_decode(label);
            let mut secret = String::new();
            let mut issuer = String::new();
            let mut stellen = 6u32;
            let mut periode = 30u64;
            for paar in query.split('&') {
                let Some((k, v)) = paar.split_once('=') else { continue };
                match k {
                    "secret" => secret = v.to_string(),
                    "issuer" => issuer = pct_decode(v),
                    "digits" => stellen = v.parse().unwrap_or(6),
                    "period" => periode = v.parse().unwrap_or(30),
                    _ => {}
                }
            }
            // Label hat oft die Form „Issuer:Konto“
            let (li, ln) = label.split_once(':').unwrap_or(("", label.as_str()));
            if issuer.is_empty() && !li.is_empty() {
                issuer = li.to_string();
            }
            Konto::neu(ln.trim(), issuer.trim(), &secret, stellen, periode)
        } else {
            Konto::neu("Konto", "", text, 6, 30)
        }
    }

    /// Anzeigename: Issuer bevorzugt, sonst Kontoname.
    pub fn issuer_oder_name(&self) -> String {
        if self.issuer.is_empty() {
            self.name.clone()
        } else if self.name.is_empty() || self.name == "Konto" {
            self.issuer.clone()
        } else {
            format!("{} · {}", self.issuer, self.name)
        }
    }

    /// Aktueller Code (aus der Systemzeit).
    pub fn code(&self, jetzt_s: u64) -> String {
        totp(&self.secret, jetzt_s, self.periode, self.stellen)
    }

    /// Verbleibender Anteil des aktuellen Fensters (1.0 → 0.0).
    pub fn rest_anteil(&self, jetzt_s: u64, sub_s: f32) -> f32 {
        let p = self.periode.max(1) as f32;
        let verbraucht = (jetzt_s % self.periode.max(1)) as f32 + sub_s;
        (1.0 - verbraucht / p).clamp(0.0, 1.0)
    }
}

/// Minimales Prozent-Decoding für otpauth-Labels.
fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 6238 Appendix B: Secret ASCII "12345678901234567890" (SHA1).
    fn rfc_secret() -> Vec<u8> {
        b"12345678901234567890".to_vec()
    }

    #[test]
    fn rfc6238_testvektoren() {
        // T=59 → 94287082, T=1111111109 → 07081804, T=1234567890 → 89005924
        assert_eq!(totp(&rfc_secret(), 59, 30, 8), "94287082");
        assert_eq!(totp(&rfc_secret(), 1_111_111_109, 30, 8), "07081804");
        assert_eq!(totp(&rfc_secret(), 1_234_567_890, 30, 8), "89005924");
    }

    #[test]
    fn base32_rfc4648() {
        // "MFRGGZDFMZTWQ2LK" == "abcdefghij"? Prüfe bekannten Wert:
        // ASCII "12345678901234567890" base32 = GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ
        let dek = base32_decode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        assert_eq!(dek, rfc_secret());
        assert!(base32_decode("8901").is_none()); // 8,9,0,1 sind kein Base32
    }

    #[test]
    fn konto_aus_base32_liefert_rfc_code() {
        let k = Konto::neu("Test", "RFC", "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 8, 30).unwrap();
        assert_eq!(k.code(59), "94287082");
    }

    #[test]
    fn otpauth_uri_wird_geparst() {
        let k = Konto::aus_eingabe(
            "otpauth://totp/ACME:alice?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=ACME&digits=8",
        )
        .unwrap();
        assert_eq!(k.issuer, "ACME");
        assert_eq!(k.name, "alice");
        assert_eq!(k.code(59), "94287082");
    }
}
