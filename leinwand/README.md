# Der Leinwand-Compositor (matrix-leinwand)

Nutzer-Desktop-Neuerfindung: der Scrollbalken-lose, unendliche 2D-Desktop.
Vollständiges Projekt-Journal: zirconium-pc/LEINWAND.md (lokal).

## Patches gegen niri (Basis a30ca798 = ausgelieferte Version)

- `0001-leinwand-drag.patch`: Config-Flag `gestures { leinwand-drag }` +
  Linksklick-Drag auf dem Leerraum startet den vorhandenen
  SpatialMovementGrab (2D-Pan über Spalten ↔ und Arbeitsflächen ↕).
- `0002-smithay-keymap-pro-wl_keyboard.patch`: gilt NICHT gegen niri,
  sondern gegen **Smithay** @ff5fa7df (niris gepinnte Revision — bei
  NIRI_COMMIT-Bump neu prüfen). Fix für den Tastatur-Bug („." tippte „q",
  R59b): send_keymap dedupliziert seat-global per SHA-256, aber frisch
  gebundene wl_keyboards bekommen nur die Seat-Keymap — Fenster, die nach
  dem vk-Upload der Bildschirmtastatur binden, deuten vk-Tasten falsch.
  Der Patch trackt die zuletzt gesendete Keymap PRO wl_keyboard.
  Einbindung im niri-builder: /smithay klonen+patchen, `[patch]`-Eintrag
  wird im Build an niris Cargo.toml angehängt (der 0001-Patch lässt
  Cargo.toml/Lock bewusst aus). Nach dem Deploy kann der client-seitige
  Workaround in matrixkit/apps/tastatur/src/einspeisung.rs (alternierende
  F19/F20-Keymaps, Commit 75a7c51) zurückgebaut werden.
  Beweis nested: /tmp/kbtest.sh auf dem Surface (alt: „." → Escape,
  neu: keymap-Event vor dem Key → period). Upstream-Entwurf: Nr. 11 in
  zirconium-pc/UPSTREAM-REPORTS.md.

## Bauen (auf dem PC)

```
distrobox enter leinwand -- bash -lc "cd ~/leinwand/niri && cargo build --release"
```

## Nested testen (gefahrlos)

```
~/leinwand/niri/target/release/niri -c /tmp/leinwand-test.kdl
# Apps hinein: WAYLAND_DISPLAY=wayland-2 <app>
```
