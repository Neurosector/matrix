#!/bin/bash
# Matrix-Dotfiles: Benutzer-Ebene der Matrix-Personalisierung.
# Idempotent — kann jederzeit erneut ausgefuehrt werden.
set -euo pipefail
cd "$(dirname "$0")"

echo "==> Configs kopieren"
mkdir -p ~/.config ~/.local/bin
cp -r config/* ~/.config/ 2>/dev/null
# environment.d/fontconfig/matugen/niri/systemd/qt5ct/qt6ct landen damit an Ort und Stelle
cp bin/* ~/.local/bin/ && chmod +x ~/.local/bin/*
# Qt-Configs: Home-Platzhalter aufloesen
sed -i "s|__HOME__|$HOME|g" ~/.config/qt6ct/qt6ct.conf ~/.config/qt5ct/qt5ct.conf

echo "==> GTK an live gepflegte DMS-Farben haengen"
mkdir -p ~/.config/gtk-3.0 ~/.config/gtk-4.0
printf '@import "dank-colors.css";\n' > ~/.config/gtk-3.0/gtk.css
printf '@import "dank-colors.css";\n' > ~/.config/gtk-4.0/gtk.css

echo "==> X11-only nvidia-settings-Autostart verstecken"
mkdir -p ~/.config/autostart
printf '[Desktop Entry]\nType=Application\nName=nvidia-settings-load\nHidden=true\n' > ~/.config/autostart/nvidia-settings-load.desktop

echo "==> Benutzer in input-Gruppe (DMS-Empfehlung, braucht sudo)"
sudo usermod -aG input "$USER" || true

echo "==> Greeter-Slot: Login-Screen folgt dem Desktop-Theme (braucht sudo)"
sudo usermod -aG greeter "$USER" || true
sudo mkdir -p "/var/cache/dms-greeter/users/$USER" /var/cache/dms-greeter/.local/share
sudo chown "$USER:greeter" "/var/cache/dms-greeter/users/$USER"
sudo chmod 2750 "/var/cache/dms-greeter/users/$USER"
for f in settings.json session.json dms-colors.json; do
    sudo ln -sfn "users/$USER/$f" "/var/cache/dms-greeter/$f"
done
sudo ln -sfn "users/$USER/dms-colors.json" /var/cache/dms-greeter/colors.json
# Erstbefuellung passiert durch sync-kdeglobals.sh (laeuft bei jedem Theme-Wechsel)
~/.local/bin/sync-kdeglobals.sh || true

echo "==> niri config.kdl: Include auf Fensterdeko-Variante umbiegen (idempotent)"
CFG=~/.config/niri/config.kdl
if [ -f "$CFG" ] && grep -q 'zdots/system/niri/zirconium.kdl' "$CFG"; then
    sed -i 's|include "/usr/share/zirconium/zdots/system/niri/zirconium.kdl"|include "zirconium-mit-fensterdeko.kdl"|' "$CFG"
fi

echo "==> matugen: Firefox-Profil erkennen und Template-Ziel setzen"
PROFDIR=""
for INI in ~/.config/mozilla/firefox/profiles.ini ~/.mozilla/firefox/profiles.ini; do
    [ -f "$INI" ] && PROFDIR="$(dirname "$INI")/$(awk -F= '/^\[Install/{f=1} f&&/^Default=/{print $2; exit}' "$INI")" && break
done
if [ -n "$PROFDIR" ] && [ -d "$PROFDIR" ]; then
    mkdir -p "$PROFDIR/chrome"
    sed -i "s|output_path = .*userChrome.css.*|output_path = \"$PROFDIR/chrome/userChrome.css\"|" ~/.config/matugen/config.toml
    cat > "$PROFDIR/user.js" <<'EOF'
user_pref("toolkit.legacyUserProfileCustomizations.stylesheets", true);
user_pref("userChrome.theme-material", true);
user_pref("svg.context-properties.content.enabled", true);
EOF
else
    echo "    (kein Firefox-Profil gefunden — Schritt uebersprungen)"
fi

echo "==> Fonts & Themes herunterladen (falls fehlend)"
if [ ! -d ~/.local/share/fonts/inter ]; then
    IURL=$(curl -s https://api.github.com/repos/rsms/inter/releases/latest | jq -r ".assets[0].browser_download_url")
    curl -sL -o /tmp/inter.zip "$IURL" && mkdir -p ~/.local/share/fonts/inter /tmp/inter_x
    unzip -oq /tmp/inter.zip -d /tmp/inter_x && find /tmp/inter_x -name "InterVariable*.ttf" -exec cp {} ~/.local/share/fonts/inter/ \;
    rm -rf /tmp/inter.zip /tmp/inter_x; fc-cache -f >/dev/null
fi
if [ ! -d ~/.local/share/themes/adw-gtk3-dark ]; then
    AURL=$(curl -s https://api.github.com/repos/lassekongo83/adw-gtk3/releases/latest | jq -r '.assets[] | select(.name | endswith(".tar.xz")) | .browser_download_url' | head -1)
    mkdir -p ~/.local/share/themes && curl -sL "$AURL" | tar xJ -C ~/.local/share/themes/
fi
if [ ! -d ~/.local/share/icons/WhiteSur-cursors ]; then
    curl -sL -o /tmp/wsc.tar.gz https://github.com/vinceliuice/WhiteSur-cursors/archive/refs/heads/master.tar.gz
    tar xzf /tmp/wsc.tar.gz -C /tmp && cp -r /tmp/WhiteSur-cursors-master/dist ~/.local/share/icons/WhiteSur-cursors
    rm -rf /tmp/wsc.tar.gz /tmp/WhiteSur-cursors-master
fi

echo "==> GNOME/GTK-Einstellungen"
gsettings set org.gnome.desktop.interface font-name "Inter 10" || true
gsettings set org.gnome.desktop.interface document-font-name "Inter 10" || true
gsettings set org.gnome.desktop.interface gtk-theme "adw-gtk3-dark" || true
gsettings set org.gnome.desktop.interface cursor-theme "WhiteSur-cursors" || true
gsettings set org.gnome.desktop.interface cursor-size 24 || true

echo "==> Flatpak: Themes + Overrides (braucht sudo)"
sudo flatpak install -y --system --noninteractive flathub \
    org.gtk.Gtk3theme.adw-gtk3 org.gtk.Gtk3theme.adw-gtk3-dark io.github.kolunmi.Bazaar || true
sudo flatpak override \
    --filesystem=xdg-data/fonts:ro --filesystem=xdg-data/icons:ro --filesystem=xdg-data/themes:ro \
    --filesystem=xdg-config/gtk-3.0:ro --filesystem=xdg-config/gtk-4.0:ro --filesystem=xdg-config/fontconfig:ro \
    --env=QT_QPA_PLATFORMTHEME=kde

echo "==> systemd-User-Units aktivieren"
systemctl --user daemon-reload
systemctl --user enable --now theme-sonne.timer matrix-gesundheit.timer sync-kdeglobals.path
systemctl --user enable theme-sonne-login.service
~/.local/bin/matrix-sonne || true

echo "==> DMS-Einstellungen (Frame aus = Workaround fuer Fenster-Verdeckung)"
dms ipc call settings set frameEnabled false 2>/dev/null || true
dms ipc call settings set dankLauncherV2IncludeFilesInAll true 2>/dev/null || true
dms ipc call settings set dankLauncherV2IncludeFoldersInAll true 2>/dev/null || true

echo "==> Launcher-Rechner-Plugin"
dms plugins install calculator 2>/dev/null || true

# Optional fuer Steam auf NVIDIA (32-Bit-Treiber; Version an Image-Treiber anpassen):
# sudo flatpak install -y flathub org.freedesktop.Platform.Compat.i386 org.freedesktop.Platform.GL32.nvidia-<VERSION>

echo "==> Pywalfox-Brücke (Firefox-Live-Farben, portal-unabhängig)"
bash "$(dirname "$0")/bin/pywalfox-bruecke.sh" || true

echo "==> MatrixKit-Apps (falls Rust-Toolchain vorhanden)"
if command -v cargo >/dev/null 2>&1 || [ -x "$HOME/.cargo/bin/cargo" ]; then
    CARGO="${CARGO:-$HOME/.cargo/bin/cargo}"; command -v cargo >/dev/null 2>&1 && CARGO=cargo
    ( cd ../matrixkit && "$CARGO" build --release -p matrix-sysmon -p matrixkit-icons ) && {
        cp ../matrixkit/target/release/matrix-sysmon ~/.local/bin/matrix-sysmon
        cp ../matrixkit/target/release/matrixkit-icons ~/.local/bin/matrixkit-icons
        mkdir -p ~/.local/share/icons/hicolor/256x256/apps ~/.local/share/applications
        # Lebende Icons: aus der aktuellen System-Palette generiert
        ~/.local/bin/matrixkit-icons || true
        cat > ~/.local/share/applications/matrix-sysmon.desktop <<DESK
[Desktop Entry]
Type=Application
Name=Matrix Monitor
GenericName=Systemmonitor
Comment=CPU, Arbeitsspeicher und Datentraeger im Blick — die erste MatrixKit-App
Exec=$HOME/.local/bin/matrix-sysmon
Icon=matrix-sysmon
Terminal=false
Categories=System;Monitor;
DESK
        update-desktop-database ~/.local/share/applications 2>/dev/null || true
    }
else
    echo "    (kein cargo — MatrixKit-Apps uebersprungen; rustup installieren und erneut ausfuehren)"
fi

echo "==> Fertig. Einmal ab- und wieder anmelden fuer volle Wirkung."
