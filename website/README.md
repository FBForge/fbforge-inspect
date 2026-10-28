# Website-Quellen

`inspect/` enthält die vorbereitete Produktseite. `npm run docs:build` erzeugt die Anleitung aus `docs/help.json` unter `dist-help/inspect/hilfe/0.1/`. Diese fachliche Quelle wird auch von der Offline-Hilfe der Desktop-App gelesen.

Produktseite und Anleitung verwenden die gemeinsame FBForge-Website-Gestaltung: `fbforge-v8.css`, `brand-v1.css`, `theme-v8.js`, Favicon und lokal bereitgestellte Inter-Schrift. Die Hilfe verwendet zusätzlich `guide-v1.css`. Kopfzeile und Footer der generierten Hilfe stammen direkt aus der Produktseite, damit keine zweite abweichende Fassung entsteht. Diese gemeinsamen Assets werden vom Website-Repository bereitgestellt, nicht von der Desktop-App gebündelt. Die Anleitung bleibt ohne JavaScript lesbar; die explizite Hell-/Dunkel-/System-Auswahl benötigt das gemeinsame Theme-Skript.

Die Produktseite nach `nginx/inspect/` und die vollständige erzeugte Hilfe samt Beispielbild nach `nginx/inspect/hilfe/0.1/` übernehmen. Anschließend die Website-Prüfungen einschließlich 320 px, 200 % Textvergrößerung, Hell/Dunkel/System und der tatsächlichen HTTP-Routen ausführen. Der bestehende Website-Branding-Importer verarbeitet bislang die FBForge-Connect-Hilfe; eine zusätzliche Inspect-Integration darf nicht als bereits vorhanden angenommen werden.

`homepage.patch` ist eine vorbereitete Ergänzung der Startseite um FBForge Inspect, ohne Änderungen an bestehenden Release- und Downloadinformationen. Vor Anwendung mit `git apply --check` im Website-Repository prüfen. Die vorbereitete Seite enthält keinen Installer-Download. Erst ein tatsächlich veröffentlichtes und abgenommenes Release darf verlinkt werden. Dateiübernahme ist kein Deployment.

## Geprüften Entwicklungsstand exportieren

Nach den Prüfungen Änderungen committen und bei sauberem Arbeitsbaum `npm run website:export` ausführen. `dist-website/manifest.json` nennt Quellcommit, Version, Arbeitsbaumstatus und SHA-256 jeder öffentlichen Datei. Der Import im Website-Repository mit `python3 scripts/import-inspect.py <Pfad-zu-dist-website>` lehnt einen schmutzigen Quellstand, beschädigte Dateien und unzulässige Pfade ab. Anschließend `python3 scripts/brand-guide.py` und die Website-Prüfungen ausführen.

Die Projektseite kennzeichnet FBForge Inspect als **in Entwicklung**, zeigt Logo und Bilder sowie die noch offene Abnahme und bietet keinen Installer-Download. Die Bilder stammen aus der Browseroberfläche mit fiktiven lokalen Daten; beim Schema-Ergebnis lief der echte Java-Kern über eine reine Testbrücke. Daraus wird keine native Windows-/NVDA-Abnahme abgeleitet. Die Quelle für alle Bilder bleibt `docs/images/`; generierte Website-Kopien nicht separat bearbeiten.
