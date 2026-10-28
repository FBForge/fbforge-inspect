# Website-Quellen

`inspect/` enthält die vorbereitete Produktseite. `npm run docs:build` erzeugt die Anleitung aus `docs/help.json` unter `dist-help/inspect/hilfe/0.1/`. Diese fachliche Quelle wird auch von der Offline-Hilfe der Desktop-App gelesen.

Produktseite und Anleitung verwenden die gemeinsame FBForge-Website-Gestaltung: `fbforge-v8.css`, `brand-v1.css`, `theme-v8.js`, Favicon und lokal bereitgestellte Inter-Schrift. Die Hilfe verwendet zusätzlich `guide-v1.css`. Kopfzeile und Footer der generierten Hilfe stammen direkt aus der Produktseite, damit keine zweite abweichende Fassung entsteht. Diese gemeinsamen Assets werden vom Website-Repository bereitgestellt, nicht von der Desktop-App gebündelt. Die Anleitung bleibt ohne JavaScript lesbar; die explizite Hell-/Dunkel-/System-Auswahl benötigt das gemeinsame Theme-Skript.

Die Produktseite nach `nginx/inspect/` und die vollständige erzeugte Hilfe samt Beispielbild nach `nginx/inspect/hilfe/0.1/` übernehmen. Anschließend die Website-Prüfungen einschließlich 320 px, 200 % Textvergrößerung, Hell/Dunkel/System und der tatsächlichen HTTP-Routen ausführen. Der bestehende Website-Branding-Importer verarbeitet bislang die FBForge-Connect-Hilfe; eine zusätzliche Inspect-Integration darf nicht als bereits vorhanden angenommen werden.

`homepage.patch` ist eine vorbereitete Ergänzung der Startseite um FBForge Inspect, ohne Änderungen an bestehenden Release- und Downloadinformationen. Vor Anwendung mit `git apply --check` im Website-Repository prüfen. Die vorbereitete Seite enthält keinen Installer-Download. Erst ein tatsächlich veröffentlichtes und abgenommenes Release darf verlinkt werden. Dateiübernahme ist kein Deployment.
