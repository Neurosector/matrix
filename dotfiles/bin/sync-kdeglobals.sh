#!/bin/bash
# Uebernimmt das von DMS/matugen generierte KDE-Farbschema in kdeglobals,
# damit KDE-/Qt-Flatpaks dieselben Farben zeigen wie der Rest des Systems.
# Beruecksichtigt den aktuellen Hell/Dunkel-Modus.
set -e
MODE=$(dms ipc call theme getMode 2>/dev/null || echo dark)
# GTK3-Theme dem Modus anpassen (sonst bleiben GTK3-Apps im Hellmodus dunkel)
export DBUS_SESSION_BUS_ADDRESS=${DBUS_SESSION_BUS_ADDRESS:-unix:path=$XDG_RUNTIME_DIR/bus}
if [ "$MODE" = "light" ]; then GTKT=adw-gtk3; else GTKT=adw-gtk3-dark; fi
gsettings set org.gnome.desktop.interface gtk-theme "$GTKT" 2>/dev/null || true
if [ "$MODE" = "light" ]; then N="DankMatugenLight"; else N="DankMatugenDark"; fi
SCHEME="$HOME/.local/share/color-schemes/$N.colors"
KDEG="$HOME/.config/kdeglobals"
[ -f "$SCHEME" ] || exit 0
ACCENT=$(awk -F= "/^\[Colors:Selection\]/{s=1} s&&/^BackgroundNormal=/{print \$2; exit}" "$SCHEME")
{
  echo "[General]"
  echo "ColorScheme=$N"
  [ -n "$ACCENT" ] && echo "AccentColor=$ACCENT"
  echo
  echo "[Icons]"
  echo "Theme=WhiteSur"
  echo
  awk "/^\[Colors:/{c=1} /^\[/&&!/^\[Colors:/{c=0} c" "$SCHEME"
} > "$KDEG.new"
mv "$KDEG.new" "$KDEG"
# Qt5-Palette mitziehen (kein eigenes matugen-Template vorhanden)
cp -f "$HOME/.config/qt6ct/colors/matugen.conf" "$HOME/.config/qt5ct/colors/matugen.conf" 2>/dev/null || true

# Greeter-Slot aktualisieren: Login-Screen bekommt dieselben Farben + Wallpaper
GSLOT="/var/cache/dms-greeter/users/$USER"
if [ -d "$GSLOT" ] && [ -w "$GSLOT" ]; then
    SESS="$HOME/.local/state/DankMaterialShell/session.json"
    LIGHTW=$(jq -r ".wallpaperPathLight // empty" "$SESS" 2>/dev/null)
    DARKW=$(jq -r ".wallpaperPathDark // empty" "$SESS" 2>/dev/null)
    CURW=$(dms ipc call wallpaper get 2>/dev/null || true)
    GL=""; GD=""; GC=""
    if [ -f "$LIGHTW" ]; then GL="$GSLOT/wallpaper-light.${LIGHTW##*.}"; cp -f "$LIGHTW" "$GL"; fi
    if [ -f "$DARKW" ];  then GD="$GSLOT/wallpaper-dark.${DARKW##*.}";  cp -f "$DARKW" "$GD"; fi
    if [ -f "$CURW" ];   then GC="$GSLOT/wallpaper.${CURW##*.}";        cp -f "$CURW" "$GC"; fi
    [ -z "$GC" ] && GC="$GD"; [ -z "$GC" ] && GC="$GL"
    # Session-Kopie: alle Pfade auf Slot-Kopien umbiegen
    jq --arg c "$GC" --arg l "${GL:-$GC}" --arg d "${GD:-$GC}"        ".wallpaperPath=\$c | .wallpaperPathLight=\$l | .wallpaperPathDark=\$d"        "$SESS" > "$GSLOT/session.json" 2>/dev/null || cp -f "$SESS" "$GSLOT/session.json"
    cp -f "$HOME/.cache/DankMaterialShell/dms-colors.json" "$GSLOT/dms-colors.json" 2>/dev/null || true
    [ -f "$HOME/.face" ] && cp -f "$HOME/.face" "$GSLOT/profile.png" 
    jq --arg w "$GC" --arg a "$GSLOT/profile.png" '.greeterWallpaperPath=$w | .greeterWallpaperFillMode="Fill" | .profileImage=$a' "$HOME/.config/DankMaterialShell/settings.json" > "$GSLOT/settings.json" 2>/dev/null || cp -f "$HOME/.config/DankMaterialShell/settings.json" "$GSLOT/settings.json" 
    chmod g+r "$GSLOT"/* 2>/dev/null || true
fi

# MatrixKit-App-Icons an die aktuelle Palette anpassen (lebende Icons)
"$HOME/.local/bin/matrixkit-icons" 2>/dev/null || true

# Firefox-Chrome live umfaerben (Pywalfox-Daemon; still, falls Firefox zu)
"$HOME/.local/bin/pywalfox" update >/dev/null 2>&1 || true

# Niri-Backdrop im Theme: nimmt Login-Kaltstart und Übersicht den grauen
# Roh-Moment (Ebene hinter Wallpaper/Fenstern) — Farbe = surface.
NIRIFARBEN="$HOME/.config/niri/farben.kdl"
MODE_NIRI=dark
[ "$(jq -r .isLightMode "$HOME/.local/state/DankMaterialShell/session.json" 2>/dev/null)" = "true" ] && MODE_NIRI=light
SURF=$(jq -r ".colors.$MODE_NIRI.surface // empty" "$HOME/.cache/DankMaterialShell/dms-colors.json" 2>/dev/null)
if [ -n "$SURF" ]; then
    printf '// VON sync-kdeglobals.sh GENERIERT — nicht von Hand editieren.\noutput "HDMI-A-1" {\n    background-color "%s"\n}\noverview {\n    backdrop-color "%s"\n}\n' "$SURF" "$SURF" > "$NIRIFARBEN"
    niri msg action load-config-file >/dev/null 2>&1 || true
fi

# avatar-nachziehen: matrixkit-icons hat den lebenden Avatar ggf. frisch
# gerendert — Greeter-Slot und AccountsService aktualisieren
if [ -f "$HOME/.face" ] && [ -d "$GSLOT" ]; then
    cp -f "$HOME/.face" "$GSLOT/profile.png" 2>/dev/null || true
fi
