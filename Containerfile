# Matrix — personalisiertes Niri-Desktop-OS auf Zirconium-Basis
# Baut Zirconium-NVIDIA nach und brennt alle lokal erarbeiteten Fixes ein.
# Doku der einzelnen Fixes: https://github.com/<OWNER>/matrix (README)

# --- Stufe 1: MatrixKit-Apps bauen (Rust) ---
FROM docker.io/library/rust:1 AS matrixkit-builder
# Wayland-Build-Abhaengigkeiten: seit dem Layershell-Widget verlangt
# smithay-client-toolkit xkbcommon.pc/wayland-client.pc zur Bauzeit.
RUN apt-get update && apt-get install -y --no-install-recommends         pkg-config libxkbcommon-dev libwayland-dev libpam0g-dev     && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY matrixkit/ .
RUN cargo build --release -j 2 -p matrix-sysmon -p matrix-farben -p matrix-hilfe -p matrix-klaenge -p matrix-codes -p matrix-schluessel -p matrix-schluessel-app -p matrix-wachter -p matrixkit-icons -p matrix-wache -p matrix-wiederherstellung -p matrix-einstellungen -p matrix-updater -p matrix-dock -p matrix-bar -p matrix-zentrale -p matrix-start -p matrix-osd -p matrix-greeter -p matrix-web -p matrix-kontext -p matrix-mitteilungen -p matrix-sperre -p matrix-hintergrund -p matrix-icons-app -p matrix-wachdienst -p matrix-dateien -p matrix-morpheus -p matrix-tastatur -p matrix-terminal -p matrix-aufnahme -p matrix-player

# --- Stufe 1b: Der Leinwand-Compositor (gepatchtes niri) ---
# Nutzer-unendlicher 2D-Desktop. Basis = exakt der ausgelieferte
# niri-Commit; unser Patch legt das 2D-Pan auf Leerraum-Drag + Kollisions-
# auflösung. Reproduzierbar an den Commit-Hash gebunden.
# WICHTIG: FEDORA-Basis, nicht Debian! Ein Debian-gebautes niri linkt
# gegen fremde sonames (libdisplay-info.so.2 statt .so.3) und stirbt auf
# dem Zielsystem beim Laden — das Surface hat es aufgedeckt (7.7.2026).
FROM registry.fedoraproject.org/fedora:44 AS niri-builder
RUN dnf -y install rust cargo git clang gcc pkgconf-pkg-config \
        systemd-devel mesa-libgbm-devel libxkbcommon-devel mesa-libEGL-devel \
        wayland-devel libinput-devel dbus-devel libseat-devel \
        pipewire-devel pango-devel cairo-devel cairo-gobject-devel gtk4-devel webkitgtk6.0-devel libdisplay-info-devel \
    && dnf clean all
ARG NIRI_COMMIT=a30ca7983b2f1fc3ddeb209b3fe18fa78e0dbd25
RUN git clone https://github.com/YaLTeR/niri /niri \
    && cd /niri && git checkout "$NIRI_COMMIT"
# Smithay mit Matrix-Fix: Keymap-Dedup pro wl_keyboard statt seat-global —
# sonst deuten Fenster, die NACH dem Keymap-Upload der Bildschirmtastatur
# ihr wl_keyboard binden, vk-Tasten mit der falschen Keymap („." → q).
# SMITHAY_COMMIT MUSS der in niris Cargo.toml gepinnten Revision entsprechen
# (bei NIRI_COMMIT-Bump mitprüfen!).
ARG SMITHAY_COMMIT=ff5fa7df392cecfba049ffed55cdaa4e98a8e7ef
RUN git clone https://github.com/Smithay/smithay /smithay \
    && cd /smithay && git checkout "$SMITHAY_COMMIT"
COPY leinwand/0002-smithay-keymap-pro-wl_keyboard.patch /tmp/smithay-keymap.patch
RUN cd /smithay && git apply /tmp/smithay-keymap.patch
COPY leinwand/0001-leinwand-drag.patch /tmp/leinwand.patch
# Der [patch]-Eintrag wird erst im Build angehängt: der niri-Patch lässt
# Cargo.toml/Cargo.lock bewusst unangetastet (Patch-Regen bleibt konfliktfrei).
RUN cd /niri && git apply /tmp/leinwand.patch \
    && printf '\n[patch."https://github.com/Smithay/smithay.git"]\nsmithay = { path = "/smithay" }\n' >> Cargo.toml \
    && cargo build --release -j 2
# Matrix Web baut HIER (Fedora): WebKitGTK-ABI muss zur Laufzeit passen
# (Debian-Builder wäre die libdisplay-info-Falle in Grün).
COPY matrixkit/ /matrixkit/
RUN cd /matrixkit && cargo build --release -j 2 -p matrix-web-inhalt

# --- Stufe 2: Das Betriebssystem ---
FROM ghcr.io/zirconium-dev/zirconium-nvidia:latest

LABEL org.opencontainers.image.title="Matrix"
LABEL org.opencontainers.image.description="Zirconium-NVIDIA + Stabilitäts-Fixes (render-node, greeter, polkit, updates)"

# Surface-Variante: mit `--build-arg SURFACE=1` wird der linux-surface-Kernel
# eingebaut (Touch/Stift/Kameras/Sensoren am Surface Pro 4). Standard 0 =
# Zirconium-Kernel (der PC ist kein Surface). Secure Boot muss aus sein
# (sonst MOK-Enrollment am Gerät nötig).
ARG SURFACE=0
# f43-gepinnt: linux-surface hat (Stand 7/2026) noch kein f44 — die
# f43-Pakete laufen auf f44 (Kernel ist eigenstaendig). Nutzer-Wahl 10.7.
# Schritt 1: NVIDIA-dracut-Zwang ZUERST auf i915 setzen (Surface = Intel), DANN
# den Kernel tauschen — die initramfs-Erzeugung des Kernel-Pakets selbst (dnf5-
# %posttrans, fatal bei Fehler) laeuft sonst noch mit nvidia-Zwang und findet
# keine nvidia-Module fuer den Surface-Kernel. (teurer Download, cached).
RUN if [ "$SURFACE" = "1" ]; then set -eux; \
        printf '# Matrix-Surface: nur Intel i915 (keine nvidia-Module fuer den Surface-Kernel)\nforce_drivers+=" i915 "\n' \
            > /usr/lib/dracut/dracut.conf.d/99-nvidia.conf; \
        mkdir -p /etc/kernel/install.d; \
        ln -sf /dev/null /etc/kernel/install.d/50-dracut.install; \
        printf '[linux-surface]\nname=linux-surface\nbaseurl=https://pkg.surfacelinux.com/fedora/f43/\nenabled=1\ngpgcheck=1\ngpgkey=https://raw.githubusercontent.com/linux-surface/linux-surface/master/pkg/keys/surface.asc\n' \
            > /etc/yum.repos.d/linux-surface.repo; \
        rpm --import https://raw.githubusercontent.com/linux-surface/linux-surface/master/pkg/keys/surface.asc; \
        dnf -y --nobest swap kernel kernel-surface; \
        dnf clean all; \
    fi
# Schritt 1b: Surface-Userland — NUR iptsd (Touchscreen). libwacom-surface
# ist bewusst NICHT drin: es kollidiert mit dem Basis-libwacom, und
# --allowerasing riss dabei niri (Greeter-Compositor, haengt an libwacom)
# mit raus -> Login kaputt (10.7.). iptsd allein hat keinen Konflikt; der
# Surface-Stift laeuft ueber den Kernel-HID-Treiber auch ohne libwacom-surface.
RUN if [ "$SURFACE" = "1" ]; then set -eux; \
        dnf -y install iptsd; \
        dnf clean all; \
        test -x /usr/bin/niri; \
        # Display-Sparfunktionen von i915 AUS: PSR, FBC (Framebuffer-
        # Kompression) und DC-States sind auf dem Skylake-Panel des Pro 4 die
        # klassischen Flickergate-Ausloeser — flackern auch bei kuehlem Panel
        # (12.7. bestaetigt: PSR allein reichte nicht, Restflackern bei 41°C).
        # kargs.d wirkt bei bootc/rpm-ostree-Deployments automatisch.
        mkdir -p /usr/lib/bootc/kargs.d; \
        printf 'kargs = ["i915.enable_psr=0", "i915.enable_fbc=0", "i915.enable_dc=0"]\n' > /usr/lib/bootc/kargs.d/50-matrix-surface.toml; \
        # Flickergate-Linderung (Hardware-Defekt Pro 4: Anzeige flackert bei
        # Waerme, 10.7. bestaetigt): Hitze unter der Flacker-Schwelle halten.
        # Turbo AUS + RAPL-Leistungslimit (PL1 8W statt 15W, PL2 12W). Boot-
        # Dienst, weil sysfs/RAPL fluechtig sind. Nur Surface-Variante.
        printf '#!/bin/bash\nset -e\necho 1 > /sys/devices/system/cpu/intel_pstate/no_turbo 2>/dev/null || true\nR=/sys/class/powercap/intel-rapl:0\nif [ -d "$R" ]; then\n  echo 8000000  > "$R/constraint_0_power_limit_uw" 2>/dev/null || true\n  echo 12000000 > "$R/constraint_1_power_limit_uw" 2>/dev/null || true\nfi\n' > /usr/bin/matrix-surface-kuehl; \
        chmod +x /usr/bin/matrix-surface-kuehl; \
        printf '[Unit]\nDescription=Matrix Surface: Flickergate-Hitzelinderung (Turbo aus + RAPL-Limit)\nAfter=multi-user.target\n\n[Service]\nType=oneshot\nExecStart=/usr/bin/matrix-surface-kuehl\n\n[Install]\nWantedBy=multi-user.target\n' > /usr/lib/systemd/system/matrix-surface-kuehl.service; \
        # Haertung (13.7., Task 69): thermald setzt no_turbo WIEDERHOLT auf 0
        # zurueck (nicht nur nach dem Boot) — ein Timer erzwingt die Linderung
        # alle 2 Minuten neu. RemainAfterExit musste dafuer weg, sonst feuert
        # der Timer eine "aktive" oneshot-Unit nie wieder.
        printf '[Unit]\nDescription=Matrix Surface: Kuehl-Linderung periodisch erzwingen (thermald-Watchdog)\n\n[Timer]\nOnBootSec=30s\nOnUnitActiveSec=2min\nUnit=matrix-surface-kuehl.service\n\n[Install]\nWantedBy=timers.target\n' > /usr/lib/systemd/system/matrix-surface-kuehl.timer; \
        systemctl enable matrix-surface-kuehl.service matrix-surface-kuehl.timer; \
    fi
# Schritt 2: initramfs selbst bauen — explizit --no-hostonly (die Basis-
# 01-dist.conf setzt hostonly=yes, das scheitert im Container an /root).
# ostree-Modul fuer bootc. Danach kernel-install-dracut wieder freigeben.
RUN if [ "$SURFACE" = "1" ]; then set -eux; \
        KVER="$(basename "$(ls -d /usr/lib/modules/*surface* | head -1)")"; \
        find /usr/lib/modules -mindepth 1 -maxdepth 1 -type d ! -name '*surface*' -exec rm -rf {} + ; \
        dracut --no-hostonly --reproducible --add ostree --force \
            "/usr/lib/modules/$KVER/initramfs.img" "$KVER"; \
        rm -f /etc/kernel/install.d/50-dracut.install; \
        test -s "/usr/lib/modules/$KVER/initramfs.img"; \
        test "$(ls -1 /usr/lib/modules | wc -l)" = "1"; \
        echo "Surface-Kernel eingebaut: $KVER"; \
    fi

# --- Systemfixes als Dateien (udev, greetd, modprobe, tmpfiles) ---
COPY etc/ /etc/

# Inter systemweit (Familien-Look bis vor das Login) + dbus-tools:
# dms-greeter laedt den Avatar via dbus-send (fehlte -> leerer Kreis),
# und die Matrix-Klang-Hooks nutzen busctl/dbus-Monitoring.
# wf-recorder + ffmpeg-free: Matrix Aufnahme (Film) und Matrix Player
# (Decoder außer Prozess) — Runtime-Pakete gehören in DIESE Stage.
RUN dnf -y install rsms-inter-vf-fonts dbus-tools wf-recorder ffmpeg-free && dnf clean all

RUN set -eux; \
    # Fix: polkit-Regel liegt im Image als Verzeichnis-in-Verzeichnis -> entnesten
    if [ -d /etc/polkit-1/rules.d/uupd.rules ]; then \
        mv /etc/polkit-1/rules.d/uupd.rules/uupd.rules /etc/polkit-1/rules.d/uupd.rules.f; \
        rmdir /etc/polkit-1/rules.d/uupd.rules; \
        mv /etc/polkit-1/rules.d/uupd.rules.f /etc/polkit-1/rules.d/uupd.rules; \
    fi; \
    # Fix: greeter-User braucht ein schreibbares Home (sonst wireplumber-Crash im Greeter)
    usermod -d /var/lib/greeter greeter; \
    printf 'd /var/lib/greeter 0700 greeter greeter -\n' > /etc/tmpfiles.d/greeter-home.conf; \
    # Fix: subuid/subgid fuer greeter (sonst scheitert uupd/distrobox)
    grep -q '^greeter:' /etc/subuid || echo 'greeter:262144:65536' >> /etc/subuid; \
    grep -q '^greeter:' /etc/subgid || echo 'greeter:262144:65536' >> /etc/subgid; \
    # Dauerwarnung "include not found" abstellen
    mkdir -p /etc/niri && touch /etc/niri/local.kdl; \
    # Dienste: kaputte/unnoetige aus, nuetzliche an
    systemctl mask systemd-remount-fs.service; \
    systemctl disable NetworkManager-wait-online.service || true; \
    systemctl disable systemd-homed.service || true; \
    systemctl enable input-remapper.service; \
    # Der grosse Schnitt: DMS ist durch die MatrixKit-Shell abgeloest.
    # Die Basis aktiviert dms.service global (Nutzer-Scope) -> stilllegen,
    # sonst startet auf frischen Installationen DMS NEBEN der Matrix-Shell
    # (9.7., Laptop: doppelte Leisten).
    systemctl --global disable dms.service || true; \
    systemctl --global mask dms.service || true
    # Hinweis: sshd wird bewusst NICHT im Image aktiviert (oeffentliches Image).
    # Bei Bedarf lokal: sudo systemctl enable --now sshd

# --- MatrixKit: eigene Apps als OS-Bestandteil ---
COPY --from=matrixkit-builder /src/target/release/matrix-sysmon /usr/bin/matrix-sysmon
COPY --from=matrixkit-builder /src/target/release/matrix-farben /usr/bin/matrix-farben
COPY --from=matrixkit-builder /src/target/release/matrix-hilfe /usr/bin/matrix-hilfe
COPY --from=matrixkit-builder /src/target/release/matrix-klaenge /usr/bin/matrix-klaenge
COPY --from=matrixkit-builder /src/target/release/matrix-codes /usr/bin/matrix-codes
# Systemklaenge zur BAUZEIT rendern — Klaenge als Code, deterministisch
RUN /usr/bin/matrix-klaenge --generieren /usr/share/matrix/klaenge
COPY --from=matrixkit-builder /src/target/release/matrix-wachter /usr/bin/matrix-wachter
COPY --from=matrixkit-builder /src/target/release/matrix-schluessel /usr/bin/matrix-schluessel
COPY --from=matrixkit-builder /src/target/release/matrix-schluessel-app /usr/bin/matrix-schluessel-app
COPY --from=matrixkit-builder /src/target/release/matrixkit-icons /usr/bin/matrixkit-icons
# Der Waechter: Recovery-Werkzeug (root) + gefuehrte Wiederherstellungs-App
COPY --from=matrixkit-builder /src/target/release/matrix-wache /usr/bin/matrix-wache
COPY --from=matrixkit-builder /src/target/release/matrix-wiederherstellung /usr/bin/matrix-wiederherstellung
# Matrix Einstellungen: die Systemeinstellungen im MatrixKit-Stil
COPY --from=matrixkit-builder /src/target/release/matrix-einstellungen /usr/bin/matrix-einstellungen
# Matrix Updater: Softwareupdate mit einem Klick (App #9)
COPY --from=matrixkit-builder /src/target/release/matrix-updater /usr/bin/matrix-updater
# Matrix Leinwand: Einstellungen des unendlichen Desktops (App #10)
COPY --from=matrixkit-builder /src/target/release/matrix-dock /usr/bin/matrix-dock
COPY --from=matrixkit-builder /src/target/release/matrix-bar /usr/bin/matrix-bar
COPY --from=matrixkit-builder /src/target/release/matrix-zentrale /usr/bin/matrix-zentrale
COPY --from=matrixkit-builder /src/target/release/matrix-start /usr/bin/matrix-start
COPY --from=matrixkit-builder /src/target/release/matrix-osd /usr/bin/matrix-osd
COPY --from=matrixkit-builder /src/target/release/matrix-greeter /usr/bin/matrix-greeter
COPY --from=matrixkit-builder /src/target/release/matrix-web /usr/bin/matrix-web
COPY --from=matrixkit-builder /src/target/release/matrix-kontext /usr/bin/matrix-kontext
COPY --from=matrixkit-builder /src/target/release/matrix-mitteilungen /usr/bin/matrix-mitteilungen
COPY --from=matrixkit-builder /src/target/release/matrix-sperre /usr/bin/matrix-sperre
COPY --from=matrixkit-builder /src/target/release/matrix-hintergrund /usr/bin/matrix-hintergrund
COPY --from=matrixkit-builder /src/target/release/matrix-icons /usr/bin/matrix-icons
COPY --from=matrixkit-builder /src/target/release/matrix-wachdienst /usr/bin/matrix-wachdienst
COPY --from=matrixkit-builder /src/target/release/matrix-dateien /usr/bin/matrix-dateien
COPY --from=matrixkit-builder /src/target/release/matrix-morpheus /usr/bin/matrix-morpheus
COPY --from=matrixkit-builder /src/target/release/matrix-tastatur /usr/bin/matrix-tastatur
COPY --from=matrixkit-builder /src/target/release/matrix-terminal /usr/bin/matrix-terminal
COPY --from=matrixkit-builder /src/target/release/matrix-aufnahme /usr/bin/matrix-aufnahme
COPY --from=matrixkit-builder /src/target/release/matrix-player /usr/bin/matrix-player
COPY --from=niri-builder /matrixkit/target/release/matrix-web-inhalt /usr/bin/matrix-web-inhalt
# Der Leinwand-Compositor + Sitzungs-Startskript (Sitzungs-Auswahl im Greeter)
COPY --from=niri-builder /niri/target/release/niri /usr/bin/niri-leinwand
COPY leinwand/session/niri-leinwand-session /usr/bin/niri-leinwand-session
COPY leinwand/session/matrix-leinwand.desktop /usr/share/wayland-sessions/matrix-leinwand.desktop
COPY usr/ /usr/
# Ausfuehrbar-Bit der Root-Helfer sichern (COPY traegt es zwar, aber
# der Installer-Helfer MUSS laufen — lieber explizit).
RUN chmod 0755 /usr/libexec/matrix-installiere

# --- Der Wächter: Recovery-Benutzer + passwortlose Anmeldung (nur `wache`) ---
# `wache` besitzt keine Daten und dient allein der gefuehrten
# Wiederherstellung. Er darf sich passwortlos am Greeter anmelden (bewusst:
# wer physisch am Login-Screen steht, soll ohne Passwort ins Recovery
# koennen — es ist laut und loescht nur korrekt) und `matrix-wache`
# passwortlos als root aufrufen.
RUN set -eux; \
    getent passwd wache >/dev/null || useradd --system --create-home \
        --home-dir /var/lib/matrix-wache --shell /bin/bash \
        --comment "Matrix Waechter (Wiederherstellung)" wache; \
    usermod -aG video,input wache; \
    # Passwortlos am Greeter — NUR fuer den Benutzer wache (pam_succeed_if
    # springt ueber die normale Passwortpruefung; alle anderen Nutzer
    # brauchen weiterhin ihr Passwort):
    if ! grep -q 'pam_succeed_if.so user = wache' /etc/pam.d/greetd; then \
        printf 'auth\t[success=done default=ignore]\tpam_succeed_if.so user = wache quiet\n' > /tmp/wache-pam; \
        cat /etc/pam.d/greetd >> /tmp/wache-pam; \
        cp /tmp/wache-pam /etc/pam.d/greetd; \
    fi; \
    # sudo: wache darf NUR das Recovery-Werkzeug als root aufrufen:
    printf 'wache ALL=(root) NOPASSWD: /usr/bin/matrix-wache\n' > /etc/sudoers.d/matrix-wache; \
    chmod 440 /etc/sudoers.d/matrix-wache; \
    visudo -cf /etc/sudoers.d/matrix-wache; \
    # Matrix Updater: wheel-Nutzer duerfen NUR den Update-Helfer als root
    # aufrufen (er kennt genau zwei Wege: bootc upgrade oder der einmalige
    # switch von Stick-Origin auf die Matrix-Quelle — keine Argumente):
    printf '%%wheel ALL=(root) NOPASSWD: /usr/bin/matrix-update-helfer\n' > /etc/sudoers.d/matrix-updater; \
    chmod 440 /etc/sudoers.d/matrix-updater; \
    visudo -cf /etc/sudoers.d/matrix-updater; \
    # chezmoi (Zirconium-Erbe) wuerde Matrix' Farbdateien zurueckdrehen —
    # Timer + Dienste global stilllegen (Fund am Surface, 7.7.):
    systemctl --global disable chezmoi-update.timer 2>/dev/null || true; \
    systemctl --global mask chezmoi-update.service chezmoi-init.service 2>/dev/null || true

# --- Neue Konten sind MatrixKit-zentriert ---
# Statische App-Icons zur BAUZEIT rendern (Standard-Palette, stabile Namen) —
# so hat JEDES Konto sofort ein Icon aller Apps, auch vor dem ersten
# Farb-Sync. Die Desktop-Eintraege (COPY usr/) + diese Icons machen die
# Matrix-Apps fuer jeden Nutzer sichtbar; /etc/skel pinnt sie ins Dock
# (Matrix-Apps zuerst — "neue Konten bestehen hauptsaechlich aus Matrix-Apps").
RUN /usr/bin/matrixkit-icons --system /usr/share/icons

# --- Boot-Auftritt: Matrix statt Hersteller-Logo ---
# Eigenes Plymouth-Theme (two-step: Punktraster-Watermark + Puls-Frames,
# beides von matrixkit-icons gerendert). Das Theme muss in die Initramfs —
# Plymouth laeuft, bevor / gemountet ist — daher Initramfs-Neubau.
RUN /usr/bin/matrixkit-icons --plymouth /usr/share/plymouth/themes/matrix && \
    plymouth-set-default-theme matrix && \
    KVER=$(basename /usr/lib/modules/*) && \
    dracut --no-hostonly --reproducible --add ostree \
        -f /usr/lib/modules/$KVER/initramfs.img "$KVER"

# --- Branding: Das System stellt sich als "Matrix" vor ---
# (Boot-Menue, About-Dialoge, fastfetch etc. lesen os-release.
#  ID/CPE/VERSION bleiben unveraendert, damit Tooling nicht bricht.)
RUN sed -i \
    -e 's/^NAME="Zirconium"/NAME="Matrix"/' \
    -e 's/^PRETTY_NAME="Zirconium"/PRETTY_NAME="Matrix"/' \
    -e 's/^DEFAULT_HOSTNAME="zirconium"/DEFAULT_HOSTNAME="matrix"/' \
    -e 's|^HOME_URL=.*|HOME_URL="https://github.com/Neurosector/matrix"|' \
    /usr/lib/os-release

# --- Zeitmaschinen-Leitbild: täglicher btrfs-Home-Snapshot (SSD-Vorsorge) ---
RUN systemctl enable matrix-home-snapshot.timer matrix-sicherung.timer matrix-gong.service

# Abschliessender Selbsttest des Images.
# --skip var-tmpfiles: das Zirconium-BASIS-Image enthaelt eine Nicht-UTF-8-Datei
# in /var, an der dieser eine Check scheitert (Altlast upstream, nicht von uns).
RUN bootc container lint --skip var-tmpfiles
