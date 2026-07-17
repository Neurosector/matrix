# Matrix-Leinwand-Sitzung (Session-Kandidat)

Additive greetd-Sitzung, die das gepatchte niri (`niri-leinwand`) mit der
normalen Umgebung + `gestures { leinwand-drag }` startet. Die reguläre
Niri-Sitzung bleibt völlig unberührt (Rollback = normale Sitzung wählen).

## Live-Installation (auf dem PC, sudo)
```
sudo cp ~/leinwand/niri/target/release/niri /usr/local/bin/niri-leinwand
sudo install -m755 niri-leinwand-session /usr/local/bin/
sudo mkdir -p /usr/local/share/wayland-sessions
sudo cp matrix-leinwand.desktop /usr/local/share/wayland-sessions/
cp leinwand-session.kdl ~/.config/niri/leinwand-session.kdl
cp leinwand-start.sh ~/.config/niri/leinwand-start.sh   # Shell+Dienste (graphical-session-Fix)
chmod +x ~/.config/niri/leinwand-start.sh
niri-leinwand validate -c ~/.config/niri/leinwand-session.kdl   # muss "valid" sagen
```
**Grauer Bildschirm nach Login?** Der direkte niri-Start aktiviert
graphical-session.target nicht → DMS/Wallpaper/Leiste starten nicht. Fix:
leinwand-start.sh (oben) wird per spawn-at-startup von der Config gerufen
und wirft Shell + Dienste an. (Sauberere Lösung — eigene niri-leinwand.service
mit graphical-session-Bindung — ist TODO.)

Beim nächsten Login im Greeter die Sitzung **„Matrix Leinwand"** wählen
(der Greeter liest XDG_DATA_DIRS, das /usr/local/share enthält).

## Image-Integration (ERLEDIGT 7.7.)
Im Containerfile: Stage `niri-builder` klont niri @a30ca798, wendet
0001-leinwand-drag.patch an, baut es → /usr/bin/niri-leinwand. Wrapper +
.desktop nach /usr/bin bzw. /usr/share/wayland-sessions. Leinwand-Config
+ Startskript in /etc/skel/.config/niri (neue Konten). Nach Image-Update
ist die Sitzung „Matrix Leinwand" dauerhaft ohne /usr/local-Nachsetzen da.

### Alt (vor Image-Integration)
Das gepatchte niri im Containerfile bauen (CI) → /usr/bin/niri-leinwand,
Wrapper + .desktop nach /usr/share. Bis dahin lebt es in /usr/local
(bootc-beschreibbar, überlebt Updates NICHT — nach Image-Update neu setzen).
