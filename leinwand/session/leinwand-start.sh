#!/bin/sh
# Die Leinwand-Session hält den Desktop-Modus-Vermerk synchron.
mkdir -p "$HOME/.config/matrix" && printf leinwand > "$HOME/.config/matrix/desktop-modus"
# Matrix-Leinwand-Sitzung: Shell + Dienste anwerfen. Bei direktem
# niri-Start (nicht über niri.service) wird graphical-session.target nicht
# automatisch aktiv — daher die Shell hier direkt und die Dienste per
# systemctl. Idempotent. Nach ~/.config/niri/leinwand-start.sh legen.
systemctl --user start --no-block graphical-session.target 2>/dev/null
# Matrix statt DMS (8.7.2026): Wallpaper + Palette macht matrix-hintergrund
# (spawn-at-startup in local.kdl); Quickshell startet nicht mehr.
# Rückweg bei Not: die alte Zeile war
#   pgrep -u "$(id -u)" -f "dms run" || setsid dms run --session &
systemctl --user start matrix-dock.service matrix-klang-hooks.service iio-niri.service 2>/dev/null
