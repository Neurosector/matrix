# Matrix

**Ein personalisiertes Niri-Desktop-OS** — ein bootc-Image auf Basis von
[Zirconium](https://github.com/zirconium-dev/zirconium) (NVIDIA-Variante) mit allen
Stabilitäts-Fixes und der kompletten Personalisierung, erarbeitet und getestet
auf einem realen NVIDIA-System.

## Aufbau

| Ebene | Inhalt | Mechanismus |
|---|---|---|
| **Image** ([Containerfile](Containerfile)) | Systemfixes: render-node-Rechte (schwarzer Bildschirm), greeter-Home, polkit-uupd-Reparatur, GPU-USB-C-i2c-Blacklist, tmpfiles, subuid, Dienste-Preset | `bootc switch ghcr.io/<owner>/matrix:latest` |
| **Dotfiles** ([dotfiles/](dotfiles/)) | Benutzer-Ebene: Niri-Overrides (Titelleisten, Ablage-Workspace, Mausrad-Tausch), Theme-Automatik (07:00/19:00), Farb-Sync (kdeglobals/Qt5/GTK), Fonts/Themes/Cursor, Firefox-Matugen, Flatpak-Overrides | `./dotfiles/install.sh` |
| **MatrixKit** ([matrixkit/](matrixkit/)) | Die eigene, SYSTEMWEITE UI in Rust (Iced 0.14): `matrixkit-theme` (Tokens, Live-Palette, Feder-Engine, Einstellungs-Kultur, Befehls-Brücken, OSD-Kanal, bindende Berechtigungen + PAM-Schloss) · `matrixkit-widgets` (Ampel-Header, Formular-Grammatik, Fokusmodell, Symbole, Leisten-Familie: Flächen/Knopf-Stil/niri-Brücke/Toggle/OSD-Anzeige) · `matrixkit-icons` (lebende Squircle-Icons) · **~20 Apps**: elf Fenster-Apps (`matrix-sysmon`, `matrix-farben`, `matrix-klaenge`, `matrix-hilfe`, `matrix-codes` = 2FA, `matrix-schluessel-app` = USB-Login-Schlüssel, `matrix-wiederherstellung` = Wächter-Recovery, `matrix-einstellungen`, `matrix-updater`, `matrix-leinwand`, `matrix-web` = Browser mit angedocktem WebKit-Träger) und die **Matrix-Shell** (`matrix-bar` = Topbar mit Widgets + Sitzungsmenü, `matrix-dock` = zweizeiliges Dock mit Pins + Rechtsklick-Pinnen + Dynamic-Dock-OSD, `matrix-zentrale` = Kontrollzentrum + Bedienungshilfen, `matrix-start` = App-Launcher, `matrix-osd` = Tasten-CLI, `matrix-greeter` = Login-Screen, `matrix-kontext` = Leinwand-Rechtsklickmenü, `matrix-mitteilungen` = Benachrichtigungs-Daemon) — DMS-Leisten/-OSD/-Notifications sind abgelöst | `cargo build --release` im Workspace |

## Neuinstallation eines PCs — Komplettablauf

1. Zirconium-ISO (NVIDIA) installieren und einmal booten **oder** bestehendes Fedora-Atomic nutzen
2. `sudo bootc switch ghcr.io/<owner>/matrix:latest && sudo systemctl reboot`
3. Anmelden, dann: `git clone https://github.com/<owner>/matrix && ./matrix/dotfiles/install.sh`
4. Ab- und wieder anmelden. Fertig.

## Updates

Der GitHub-Actions-Workflow baut **wöchentlich** (und bei jedem Push) neu auf dem
aktuellen Zirconium auf — Upstream-Updates werden geerbt, die Fixes bleiben.
Der PC holt sich das Ergebnis über den normalen nächtlichen `uupd`-Lauf.

## Enthaltene Fixes (Kurzliste)

1. `renderD*` udev-Regel + greetd-Drop-in → behebt schwarzen Bildschirm (NVIDIA)
2. greeter-User: Home `/var/lib/greeter` + subuid/subgid → behebt wireplumber-Crash & uupd-Distrobox-Fehler
3. polkit `uupd.rules` entnestet → Auto-Updates funktionieren passwortlos
4. `i2c_nvidia_gpu`/`ucsi_ccg` geblacklistet → keine i2c-Fehlerflut (GPU-USB-C war defekt)
5. tmpfiles-Korrektur für `/home`,`/srv`-Symlinks; `systemd-remount-fs` maskiert (composefs)
6. `NetworkManager-wait-online` + `systemd-homed` deaktiviert, `input-remapper` + `sshd` aktiviert
7. Dotfiles: DMS-Frame deaktiviert (verdeckte alle Fenster — Upstream-Bug)

Vollständige Historie und Begründungen: private Projektdoku.

## Hinweise

- **sshd ist im Image bewusst NICHT aktiviert** (öffentliches Image). Bei Bedarf lokal:
  `sudo systemctl enable --now sshd` — die Einstellung überlebt Image-Updates.
- Ersetze `<owner>` durch den GitHub-Benutzernamen; ghcr.io-Paket auf „public" stellen,
  damit `bootc switch` ohne Anmeldung zieht.
