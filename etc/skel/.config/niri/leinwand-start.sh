#!/bin/sh
# Matrix-Leinwand-Sitzung: Dienste-Kette anwerfen. Bei direktem
# niri-Start (nicht über niri.service) wird graphical-session.target nicht
# automatisch aktiv. Die Matrix-Shell (Bar/Dock/Mitteilungen/Hintergrund/
# Wachdienst) startet über local.kdl — hier nur noch die systemd-Seite.
# (DMS ist seit dem großen Schnitt abgelöst und wird NICHT mehr gestartet.)
systemctl --user start --no-block graphical-session.target 2>/dev/null
systemctl --user start matrix-klang-hooks.service iio-niri.service 2>/dev/null
