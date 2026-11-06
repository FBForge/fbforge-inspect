# Website-Quellen

`inspect/` enthält die Produktseite für FBForge Inspect 0.2.0. `npm run docs:build` erzeugt die bebilderte Anleitung aus `docs/help.json` unter `dist-help/inspect/hilfe/0.2/`. Dieselbe Quelle wird in der App als durchsuchbare Offline-Hilfe gebündelt.

Produktseite und Anleitung verwenden die gemeinsame FBForge-Website-Gestaltung mit lokal bereitgestellter Inter-Schrift. Kopfzeile und Footer der generierten Hilfe stammen aus der Produktseite. Die gemeinsamen Website-Assets werden vom Website-Repository bereitgestellt. Die Anleitung bleibt ohne JavaScript lesbar.

Nach den Prüfungen Änderungen committen und bei sauberem Arbeitsbaum `npm run website:export` ausführen. `dist-website/manifest.json` nennt Quellcommit, Version, Arbeitsbaumstatus und SHA-256 jeder öffentlichen Datei. Der Import im Website-Repository mit `python3 scripts/import-inspect.py <Pfad-zu-dist-website>` prüft Quelle, Prüfsummen und zulässige Pfade. Anschließend `python3 scripts/brand-guide.py` und die Website-Prüfungen einschließlich 320 px, 200 % Textvergrößerung und Hell/Dunkel/System ausführen. Die Anleitung 0.1 bleibt im Website-Repository für Bestandsinstallationen erhalten.

Downloadlinks auf 0.2.0 erst veröffentlichen, wenn die geprüften Releasepakete tatsächlich verfügbar sind. Der Import ist noch kein Deployment. `homepage.patch` dient nur als Einfügungsbeispiel für eine bisher fehlende Projektkarte. Die bestehende Website verwendet ihren gemeinsamen Projektrenderer und ihre verifizierten Releasedaten.

Die Bilder stammen aus der aktuellen Browseroberfläche mit ausdrücklich fiktiven Beispieldaten. Schemaaufnahmen verwenden den echten Java-Kern über eine Testbrücke. Browserbilder belegen keine native Windows- oder NVDA-Abnahme. Native Abnahmebelege gehören in die Releaseprüfung. Die Bildquelle bleibt `docs/images/`. Generierte Website-Kopien nicht separat bearbeiten.
