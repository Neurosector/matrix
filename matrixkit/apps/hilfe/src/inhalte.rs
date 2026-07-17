//! Die Wissensbasis der Hilfe-App — Kategorien, Artikel, Texte.
//!
//! Inhalte fuer beide Zielgruppen: Nutzer (Bedienung, Farben, Rechte)
//! und Entwickler (Architektur, Regeln, App-Bau). Neue Artikel: einfach
//! hier ergaenzen — die App rendert Sidebar, Karten und Leseansicht daraus.

pub struct Artikel {
    pub titel: &'static str,
    pub teaser: &'static str,
    pub inhalt: &'static str,
}

pub struct Kategorie {
    pub name: &'static str,
    pub artikel: &'static [Artikel],
}

pub const KATEGORIEN: &[Kategorie] = &[
    Kategorie {
        name: "Erste Schritte",
        artikel: &[
            Artikel {
                titel: "Was ist MatrixKit?",
                teaser: "Das UI-Fundament von Matrix — eine Sprache, ein Look, ein Gefühl.",
                inhalt: "MatrixKit ist das App-Fundament des Matrix-Systems: eine Sammlung von Bausteinen, mit denen alle eigenen Apps gebaut werden — vollständig in Rust.\n\nDie Idee dahinter: Auf einem Mac fühlt sich jede gute App wie ein Teil des Ganzen an, weil alle dieselbe Basis nutzen (SwiftUI). MatrixKit übernimmt dieses Prinzip: Fenster, Kopfzeile, Schalter, Farben, Bewegung und Berechtigungen kommen aus dem Rahmen — die App selbst ist nur noch Inhalt.\n\nDeshalb sehen Matrix-Apps nicht nur gleich aus, sie verhalten sich auch gleich: gleiche Gesten, gleiche Abstände, gleiche Reaktionen. Und weil die Farben aus derselben Quelle stammen wie die des restlichen Systems, ist eine MatrixKit-App von der Shell visuell nicht zu unterscheiden.",
            },
            Artikel {
                titel: "Die Matrix-Apps",
                teaser: "Monitor, Farben, Hilfe — und wie sie in Launcher und Dock leben.",
                inhalt: "Aktuell gehören acht Apps zur Familie:\n\nMatrix Einstellungen — die Systemeinstellungen im Matrix-Stil: Erscheinungsbild (Icon-Stil, Bewegung), Anzeige und mehr. Jede Änderung wirkt sofort und landet als einfache Datei unter ~/.config/matrix/.\n\nMatrix Monitor — Prozessor, Arbeitsspeicher, Datenträger und Netzwerk auf einen Blick, mit federnden Balken und einer Verlaufskurve der letzten zwei Minuten.\n\nMatrix Farben — die lebende System-Palette. Jede Farbrolle mit Hex-Wert; ein Klick kopiert ihn in die Zwischenablage.\n\nMatrix Klänge — die Systemklänge: anhören, einzeln oder komplett abschalten. Die Klänge selbst sind Code: von Grund auf in Rust synthetisiert, alle aus einer Familie tiefer, ruhiger Pulse.\n\nMatrix Codes — ein 2FA-Authenticator: zeigt deine Zwei-Faktor-Codes (TOTP) mit ablaufendem Ring, ein Klick kopiert. Die Codes werden von Grund auf in Rust berechnet (mit den RFC-6238-Testvektoren belegt); Konten liegen hinter dem Passwort-Schloss.\n\nMatrix Schlüssel — richtet einen USB-Stick als Login-Schlüssel ein: danach meldest du dich mit dem Stick ODER dem Passwort an, beide gleichwertig. Das Passwort bleibt immer gültig, du kannst dich nicht aussperren.\n\nMatrix Hilfe — diese App. Mit Volltextsuche oben rechts: tippen zeigt Treffer aus allen Kapiteln, Esc räumt die Suche wieder weg.\n\nUnd eine achte, besondere: Matrix Wiederherstellung — die geführte Recovery des Wächters. Sie wohnt nicht im Launcher, sondern erscheint am Login-Bildschirm, wenn jemand sein Passwort vergessen hat (siehe Kapitel „Der Wächter“).\n\nAlle Apps findest du im Launcher (kurz die Super-Taste drücken und den Namen tippen). Laufende Apps erscheinen im Dock — mit ihrem lebenden Icon, das die aktuellen Systemfarben trägt.",
            },
            Artikel {
                titel: "Lebende Farben",
                teaser: "Warum jede App sofort mitfärbt, wenn das Hintergrundbild wechselt.",
                inhalt: "Matrix hat eine einzige Farbquelle: dein Hintergrundbild. Daraus errechnet das System eine vollständige Material-Palette — und diese eine Wahrheit fließt überall hin: Shell, Fenster, Terminal, Firefox, Login-Bildschirm und eben auch in jede MatrixKit-App.\n\nMatrixKit-Apps beobachten die Palette live: Wechselst du das Wallpaper, färben laufende Apps binnen zwei Sekunden um — ohne Neustart. Sogar die App-Icons in Launcher und Dock werden neu gezeichnet, denn sie sind keine statischen Bilder, sondern werden aus der aktuellen Palette generiert.\n\nDazu kommt der Sonnen-Rhythmus: Bei Sonnenaufgang wird das System hell, bei Sonnenuntergang dunkel — täglich neu berechnet für deinen Standort.",
            },
        ],
    },
    Kategorie {
        name: "Bedienung",
        artikel: &[
            Artikel {
                titel: "Fenster & Gesten",
                teaser: "Ziehen, Größe ändern, Schließen — ohne klassische Fensterdeko.",
                inhalt: "MatrixKit-Fenster haben eine eigene Kopfzeile im System-Look statt einer Fremd-Dekoration.\n\nVerschieben: die Kopfzeile mit der Maus greifen und ziehen — das ist dieselbe native Compositor-Geste wie bei den System-Fenstern.\n\nGröße ändern: an der rechten Kante, der Unterkante oder der Ecke unten rechts ziehen. Jede App hat eine sinnvolle Mindestgröße; kleiner geht es bewusst nicht.\n\nSchließen: der ×-Knopf rechts in der Kopfzeile.\n\nAlles Interaktive in der Kopfzeile zeigt sich beim Überfahren mit der Maus (Hover-Aufhellung) — was keinen Hover-Effekt hat, ist auch nicht klickbar.

Tastatur: Tab wandert durch die Elemente (ein farbiger Ring zeigt den Fokus), Enter aktiviert, Esc schließt Ebenen oder führt zurück. Shift+Tab wandert rückwärts.",
            },
            Artikel {
                titel: "Die Root-Ebene",
                teaser: "Ein Klick auf den App-Namen — Über, Einstellungen, Berechtigungen.",
                inhalt: "Jede MatrixKit-App hat eine Verwaltungsebene, die Root-Ebene. Du erreichst sie mit einem Klick auf den App-Namen in der Kopfzeile.\n\nDie App dimmt dabei aus — wie eine angehaltene virtuelle Maschine — und darüber erscheint ein Panel mit zwei Dingen: „Über“ (Name, Version, Beschreibung, wie man es von macOS kennt) und den Berechtigungen der App.\n\nSchließen: auf „Fertig“ klicken, auf den abgedunkelten Bereich — oder einfach Esc drücken. Die App läuft dabei ungestört weiter — pausiert wird nur die Optik.

Die Ebene gleitet federnd herein. Gefährliche Aktionen (etwa das Löschen eines Sticks) fragen grundsätzlich noch einmal nach — Abbrechen ist immer der nächstliegende Weg. Wer weniger Bewegung möchte: eine Datei ~/.config/matrix/bewegung mit dem Inhalt „reduziert“ schaltet alle Übergänge auf sofortige Zustände um.",
            },
            Artikel {
                titel: "Berechtigungen",
                teaser: "Bindend, nicht symbolisch: Ohne Recht kein Zugriff — wortwörtlich.",
                inhalt: "Die Berechtigungen in der Root-Ebene sind keine Deko. Der MatrixKit-Rahmen prüft sie, BEVOR die App auf etwas zugreift: Ist ein Schalter aus, wird der Zugriff gar nicht erst ausgeführt.\n\nÄnderungen sind durch dein Account-Passwort geschützt (das Schloss, wie bei macOS): Solange die Sektion verriegelt ist, sind die Schalter stumpf — erst die Passwort-Bestätigung entsperrt sie, und beim Schließen der Root-Ebene verriegelt sie wieder.\n\nBeispiel: Schaltest du beim Systemmonitor „Netzwerk“ aus, fragt die App die Netzwerk-Schnittstellen nicht mehr ab — die Zeile zeigt „Berechtigung aus“. Schaltest du bei Matrix Farben die Zwischenablage aus, kopiert der Klick nichts und die Fußzeile erklärt warum.\n\nIm Katalog stehen außerdem Kamera, Mikrofon und Datei-Zugriffe bereit: Sobald eine künftige App sie nutzt, laufen sie durch dieselbe Prüfstelle und erscheinen automatisch in ihrer Root-Ebene.\n\nDeine Einstellungen überleben App-Neustarts — gespeichert wird pro App unter ~/.config/matrix/berechtigungen/.",
            },
            Artikel {
                titel: "Die unendliche Leinwand",
                teaser: "Der neu erfundene Desktop: Fenster nebeneinander, nie minimiert.",
                inhalt: "Die Leinwand ist Matrix' eigene Antwort auf den Schreibtisch (Einstellungen → Schreibtisch → Desktop-Modus): Fenster überlappen nicht — sie leben nebeneinander auf einer Fläche, die in alle Richtungen wächst.\n\nNavigieren heute: Super + Mausrad wandert durch die Spalten (links/rechts), Super + Bild-Tasten durch die Arbeitsflächen (oben/unten) — zusammen ist das ein Raster in zwei Richtungen. Die Übersicht (Super + O) zoomt heraus und zeigt die ganze Fläche als Karte: Fenster greifen, verschieben, hinspringen — wie die App-Wolke einer Apple Watch.\n\nMinimieren gibt es auf der Leinwand nicht mehr. Die \u{2212}-Ampel legt stattdessen den PRIVATSCHLEIER über das Fenster: Es bleibt an seinem Ort und in seiner Größe, nur der Inhalt ist verdeckt — ein Klick auf die Fläche öffnet es wieder. So verliert kein Fenster je seinen Platz, und neugierige Blicke sehen trotzdem nichts.\n\nAusblick: Das freie Ziehen der Sicht mit gedrückter Maustaste auf dem Leerraum — in jede Richtung, wie auf einer echten Leinwand — braucht einen Eingriff in den Compositor und steht als nächster großer Schritt auf der Roadmap.",
            },
            Artikel {
                titel: "Matrix Einstellungen",
                teaser: "Die Einstellungs-Zentrale: Regler, Schalter, Stufen — alles wirkt sofort.",
                inhalt: "Matrix Einstellungen ist die Einstellungs-Zentrale des Systems — du findest sie an erster Stelle im Dock oder tippst „Einstellungen“ im Launcher.\n\nErscheinungsbild: Der Icon-Stil schaltet zwischen farbigen und auf die Palette getönten App-Symbolen — die Icons in Dock und Launcher werden sofort neu gezeichnet. „Bewegung reduzieren“ beschränkt Federn und Übergänge auf das Nötigste.\n\nAnzeige: Anzeigegröße (Schieberegler mit Prozentwert) und Eckenradius (±-Stufen) sind Vormerkungen für den Shell-Ausbau — sie werden gespeichert und künftig systemweit gelesen.\n\nDie Bedienelemente folgen einer festen Grammatik: Ein Schieberegler wächst unter der Maus, an einer Grenze dimmt die ±-Taste und reagiert nicht, die aktive Auswahl trägt die Akzentfarbe. Diese Grammatik gilt in allen Matrix-Apps gleich.\n\nDas Prinzip dahinter: Jede Einstellung ist eine einfache, lesbare Datei unter ~/.config/matrix/ — die App ist nur die Oberfläche darüber. Nichts ist versteckt, alles lässt sich auch von Hand ändern oder sichern.",
            },
        ],
    },
    Kategorie {
        name: "Der Wächter",
        artikel: &[
            Artikel {
                titel: "Was ist der Wächter?",
                teaser: "Die Instanz, die über das ganze System wacht — auch vor dem Login.",
                inhalt: "Der Wächter ist die Schutz-Instanz von Matrix. Er gehört zu keiner App und zu keiner Sitzung — er wacht über das GESAMTE System, auch dort, wo du noch gar nicht angemeldet bist: auf dem Login-Bildschirm.\n\nDu erkennst ihn an seiner Stimme: tiefe, gedämpfte Klänge, die bewusst auffallen. Wird dein USB-Login-Schlüssel erkannt, spielt der Wächter seine Melodie — hörbar für den ganzen Raum. Das ist Absicht: Sollte jemand deinen Schlüssel unbefugt benutzen, bekommt es die Umgebung wenigstens mit. Sicherheit, die man hören kann.\n\nDer Wächter hat zwei Aufgaben: Er macht Sicherheits-Ereignisse hörbar (der Schlüssel-Klang am Login-Screen lässt sich deshalb nicht abschalten) — und er hilft dir, wenn du ausgesperrt bist (siehe „Passwort vergessen“).",
            },
            Artikel {
                titel: "Passwort vergessen",
                teaser: "Kein Gerät wird unbrauchbar — der Wächter führt in die Wiederherstellung.",
                inhalt: "Hast du dein Passwort vergessen (und keinen USB-Schlüssel), erscheint der Wächter auf dem Login-Bildschirm — mit einem eigenen, unüberhörbaren Ruf.\n\nEr bietet dir die Wiederherstellung an: eine geführte Umgebung wie die macOS-Recovery. Dort wird Schritt für Schritt erklärt, was passiert:\n\n1. Du wählst, was zurückgesetzt wird — nur dein Konto (wenn mehrere Personen den PC nutzen, bleiben die anderen unberührt) oder das ganze System (Werkseinstellung).\n\n2. VOR der Löschung zeigt dir der Wächter ein Speicher-Diagramm: wie viele GB Bilder, Musik, Dokumente, Videos und Sonstiges zu diesem Konto gehören und gelöscht werden. Du weißt genau, was du aufgibst.\n\n3. Erst nach deiner ausdrücklichen Bestätigung wird gelöscht — und der Wächter legt eine Sicherheits-Wartezeit von 30 Minuten ein (dazu gleich mehr). Danach wird vollständig und sauber gelöscht.\n\n4. Danach geht das Setup direkt weiter: Du (oder die Person, die den PC übernimmt) legst ein frisches Konto an und meldest dich neu an.\n\nWichtig zu wissen: Die Wiederherstellung kann Daten LÖSCHEN, aber niemals lesen oder retten. Ein vergessenes Passwort öffnet keine Hintertür zu deinen Dateien.",
            },
            Artikel {
                titel: "Schutz vor fremdem Zugriff",
                teaser: "Eine Wartezeit hält Spaßvögel ab — dein Schlüssel lässt dich sofort durch.",
                inhalt: "Damit nicht einfach ein Freund (oder wer auch immer kurz allein am PC ist) über \u{201e}Passwort vergessen\u{201c} dein Konto löscht, legt der Wächter vor jeder Löschung eine Sicherheits-Wartezeit von 30 Minuten ein.\n\nWährend dieser Zeit läuft ein sichtbarer Countdown — und der Wächter ruft regelmäßig laut. Niemand harrt eine halbe Stunde neben einem rufenden Rechner aus, nur um Unfug zu treiben. Die Löschung wird also nicht verhindert (kein Gerät wird unbrauchbar), sondern nur verzögert und laut angekündigt.\n\nUnd du als Besitzer? Steck deinen USB-Login-Schlüssel ein. Der Wächter prüft sein Geheimnis, erkennt dich als rechtmäßigen Besitzer und löscht sofort — ganz ohne Wartezeit. So bist du in Sekunden durch, während ein Fremder die volle halbe Stunde mit lautem Ruf überstehen müsste.\n\nKurz: Die Wartezeit kostet dich nichts (du hast ja den Schlüssel), hält aber Gelegenheits-Unfug wirksam ab. Und weil die Frist im geschützten System-Werkzeug sitzt, lässt sie sich nicht mit einem Neustart oder Uhr-Umstellen austricksen.",
            },
            Artikel {
                titel: "Die Philosophie dahinter",
                teaser: "Geräte werden nicht unbrauchbar — Daten werden korrekt gelöscht.",
                inhalt: "Manche Systeme machen ein Gerät praktisch wertlos, sobald ein Passwort verloren geht. Das ist nicht unsere Vorstellung von Sicherheit.\n\nDie Matrix-Philosophie: Ein vergessenes Passwort darf niemals ein Gerät unbrauchbar machen. Der PC gehört dem Menschen davor — nicht dem Passwort.\n\nGleichzeitig gilt: Deine persönlichen Daten gehören DIR. Deshalb ist der einzige Weg zurück ins System die KORREKTE, vollständige Löschung der personenbezogenen Daten, die dieses Konto erstellt hat — Bilder, Musik, Dokumente, Einstellungen, Schlüssel. Wer den PC ohne Passwort übernimmt, bekommt ein frisches System, aber niemals deine Dateien.\n\nDass Matrix das kann, liegt an seiner Bauart: Das Betriebssystem selbst ist ein unveränderliches, versioniertes Abbild — es muss nicht neu installiert werden, es IST nach der Löschung der Nutzerdaten wieder wie am ersten Tag. Werkseinstellung heißt bei Matrix: dieselbe frische Installation, drei Minuten statt drei Stunden.\n\nUnd damit Übernahme nie heimlich passiert, ist der Wächter hörbar: Wer die Wiederherstellung startet oder einen Login-Schlüssel benutzt, tut das mit Ansage — für alle im Raum.",
            },
        ],
    },
    Kategorie {
        name: "Design",
        artikel: &[
            Artikel {
                titel: "Das Designsystem",
                teaser: "Tokens statt Werte: Abstände, Radius, Schrift — alles aus einer Quelle.",
                inhalt: "Kein Wert in einer MatrixKit-App ist erfunden — alles verweist auf Tokens, die aus der System-Shell extrahiert wurden:\n\nAbstände: 2 / 4 / 8 / 12 / 16 / 24 — alles im 4er-Raster. Die Fensterlücken des Desktops (16) liegen auf demselben Raster.\n\nRundung: EIN Eckenradius (12) für Karten und Knöpfe; Kreise sind halbe Höhe.\n\nSchrift: Inter Variable in vier Größen (12/14/16/20). Hierarchie entsteht durch Größe und Farbrolle, nicht durch Fettdruck.\n\nFarben: nur Material-Rollen (primary, surface, outline …), niemals Hex-Werte im Code — sonst bräche der Wallpaper-Wechsel.\n\nZustände: Interaktive Flächen legen beim Hover 12 % und beim Drücken 20 % der Textfarbe über ihren Grund — dadurch funktioniert Feedback auf jeder Fläche, hell wie dunkel.",
            },
            Artikel {
                titel: "Bewegung",
                teaser: "Federn statt Timer: Warum sich Matrix-Apps lebendig anfühlen.",
                inhalt: "Das Apple-Gefühl steckt in der Bewegung, nicht in der Optik — deshalb war die Animations-Engine das erste Fundament von MatrixKit.\n\nWerte springen nie: Die Balken im Systemmonitor FEDERN zu ihrem neuen Ziel und dürfen dabei leicht überschwingen. Die Federn sprechen Apples Sprache — drei Charaktere aus dem echten macOS-SDK: glatt (kein Nachschwung), zackig (ein Hauch — der Standard für Panels und Dialoge) und federnd (deutlich, für Akzente). Kommt ein neuer Messwert, während die Feder noch schwingt, nimmt sie einfach das neue Ziel — unterbrechbar, wie bei iOS.\n\nDie Kurven und Dauern (200/450/600 ms, sanftes Aus- und Einklingen, Überschwinger für räumliche Bewegung) stammen aus dem Material-3-Bewegungssystem der Shell — Apps und System bewegen sich dadurch im selben Takt.\n\nUnd im Ruhezustand? Kostet eine MatrixKit-App null Animations-Zyklen — getickt wird nur, solange sich etwas bewegt.",
            },
            Artikel {
                titel: "Lebende Icons",
                teaser: "App-Icons, die aus deiner Palette gerendert werden — bei jedem Wechsel neu.",
                inhalt: "MatrixKit-Apps liefern keine statischen Icon-Bilder aus. Ihre Icons werden vom Werkzeug matrixkit-icons aus der aktuellen Palette GEZEICHNET — nach dem Apple-Prinzip als getrennte Ebenen: eine gemeinsame Kachel (echte Superellipse mit Lichtkante und Tiefenverlauf) und eine Glyphe pro App mit weichem Schatten.\n\nBei jedem Wallpaper- oder Hell/Dunkel-Wechsel entstehen alle Icons neu — in wenigen Sekunden tragen Launcher und Dock die neuen Farben, auch bei laufenden Apps.\n\nAus der Ebenen-Trennung entstehen Stile: Standard (farbig) und Getönt (monochrom, wie bei Apple). Umschalten: Datei ~/.config/matrix/icon-stil mit Inhalt „getoent“ anlegen; löschen bringt die Farben zurück.\n\nFür Entwickler: Ein neues App-Icon ist eine Zeile in der ICONS-Liste plus eine kleine Glyphen-Funktion — Kachel, Schatten, Größen, Stile und das Umfärben zur Laufzeit übernimmt der Rahmen.",
            },
        ],
    },
    Kategorie {
        name: "Entwickler",
        artikel: &[
            Artikel {
                titel: "Architektur",
                teaser: "Vier Crates, klare Rollen: theme, widgets, icons, apps.",
                inhalt: "Der MatrixKit-Workspace besteht aus vier Bausteinen:\n\nmatrixkit-theme — die Wahrheit: Design-Tokens, die Live-Palette (mit Watcher für Wallpaper-Wechsel), die Feder/Bezier-Bewegungs-Engine und die bindende Rechteverwaltung.\n\nmatrixkit-widgets — das Vokabular: Kopfzeile, Fensteraufbau mit nativen Resize-Griffen, Schalter, Fußzeile, Root-Ebene, Zeitgeber. Ein Widget trifft keine eigenen Designentscheidungen — es referenziert Tokens.\n\nmatrixkit-icons — die lebenden Icons: rendert alle App-Icons aus der Palette (tiny-skia, pures Rust).\n\napps/ — die Apps selbst: Monitor, Farben, Klänge, Codes, Schlüssel, Hilfe. Eine App ist im Idealfall nur noch Zustand, Update-Logik und Inhalt.\n\nAlles ist Rust (iced 0.14) — dieselbe Sprache wie der Compositor. Gebaut wird mit cargo build --release im Workspace; die CI testet und baut bei jedem Push.",
            },
            Artikel {
                titel: "Eine neue App bauen",
                teaser: "Vom leeren Ordner zur App in Launcher und Dock — die Schritte.",
                inhalt: "1. Crate anlegen unter apps/<name> und im Workspace-Cargo.toml als Mitglied eintragen. Abhängigkeiten: iced, matrixkit-theme, matrixkit-widgets.\n\n2. Fenster bauen: iced::application + mkw::fenster_settings(app_id, breite, hoehe) — die App-ID MUSS dem Namen der .desktop-Datei entsprechen, sonst zeigt das Dock ein Fremd-Icon. Die Ansicht liefert mkw::app_fenster(titel, palette, inhalt, …) — Kopfzeile, Griffe und Root-Ebene kommen mit.\n\n3. Live-Farben: Palette::load() beim Start, PaletteWatcher + mkw::tick (2 s) im Update — niemals iced::time::every (braucht Tokio, das wir nicht fahren).\n\n4. Rechte deklarieren: nur die, die die App wirklich nutzt — und JEDEN Zugriff vorher mit rechte.erlaubt(...) prüfen. Bindend heißt bindend.\n\n5. Icon-Rezept in matrixkit-icons ergänzen (eine Zeile + Zeichen-Funktion), Desktop-Eintrag schreiben, App ins Containerfile aufnehmen.\n\n6. Nach dem Anlegen des Desktop-Eintrags einmal die Shell neu starten (systemctl --user restart dms), damit der Launcher die neue App einliest.",
            },
            Artikel {
                titel: "Regeln & Lektionen",
                teaser: "Die hart verdienten Erkenntnisse — damit sie niemand neu bezahlen muss.",
                inhalt: "App-ID = Name der .desktop-Datei. Ohne sie kann das Dock das Fenster keinem Eintrag zuordnen.\n\nNiemals Hex-Farben im App-Code — nur Palette-Rollen. Sonst bricht der Wallpaper-Wechsel.\n\nZeit über mkw::tick (Thread + Kanal), nicht über iced::time::every — funktioniert in Fenster- UND Layershell-Laufzeit.\n\nIcons brauchen versionierte Dateinamen: Die Shell cached Bilder pro Pfad; gleicher Name = altes Bild bis zum Neustart. matrixkit-icons erledigt das automatisch.\n\nWer dms-colors.json liest, muss Schreib-Wettläufe abfangen — der Farb-Sync feuert, während matugen noch schreibt (Palette::load_settled nutzen).\n\nJedes Diagramm trägt eine Beschriftung — räumliche Nähe erzeugt sonst falsche Zuordnung.\n\nAlles Interaktive in Titel/Navigation zeigt Hover-Feedback; Statusmeldungen gehören in die Rahmen-Fußzeile.\n\nUnd die wichtigste: Neue Erkenntnisse gehören in Doku und Rahmen — nicht nur in den Kopf.",
            },
        ],
    },
];
