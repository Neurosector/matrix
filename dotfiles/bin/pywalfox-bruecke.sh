#!/bin/bash
# Pywalfox-Brücke für Flatpak-Firefox — idempotent.
#
# Hintergrund: Bis xdg-desktop-portal 1.20 lief Native Messaging über das
# WebExtensions-Portal (Pref = 1). Seit 1.22 fehlt dieses Interface im
# Portal — Firefox scheitert dann hart. Der robuste Weg ist klassisches
# Native Messaging: Manifest im Sandbox-Profil + Wrapper, der den Host
# per flatpak-spawn erreicht (gleiche Technik wie Plasma-Integration).
set -euo pipefail

APP="$HOME/.var/app/org.mozilla.firefox"
[ -d "$APP" ] || { echo "Firefox-Flatpak fehlt — nichts zu tun."; exit 0; }

# 1) Wrapper: läuft IN der Sandbox, ruft den Host-Pywalfox
cat > "$APP/pywalfox-host" <<EOF
#!/bin/sh
exec flatpak-spawn --host $HOME/.local/bin/pywalfox "\$@"
EOF
chmod +x "$APP/pywalfox-host"

# 2) Manifest im Sandbox-Sichtbereich (~/.mozilla ist per --persist gemappt)
mkdir -p "$APP/.mozilla/native-messaging-hosts"
cat > "$APP/.mozilla/native-messaging-hosts/pywalfox.json" <<EOF
{
  "name": "pywalfox",
  "description": "Pywalfox ueber flatpak-spawn-Bruecke",
  "path": "$APP/pywalfox-host",
  "type": "stdio",
  "allowed_extensions": [ "pywalfox@frewacom.org" ]
}
EOF

# 3) D-Bus-Erlaubnis für flatpak-spawn
flatpak override --user --talk-name=org.freedesktop.Flatpak org.mozilla.firefox

# 4) Portal-Weg abschalten (0 = nie Portal, immer klassisch)
PROFIL=$(ls -d "$APP"/.mozilla/firefox/*.default-release 2>/dev/null | head -1)
if [ -n "$PROFIL" ]; then
    UJ="$PROFIL/user.js"
    touch "$UJ"
    grep -v 'use-xdg-desktop-portal.native-messaging' "$UJ" > "$UJ.neu" || true
    printf 'user_pref("widget.use-xdg-desktop-portal.native-messaging", 0);\n' >> "$UJ.neu"
    mv "$UJ.neu" "$UJ"
fi

echo "Pywalfox-Brücke eingerichtet — Firefox neu starten."
