# FBForge Inspect

Ein privat entwickeltes Open-Source-Werkzeug zur Diagnose öffentlich erreichbarer
FIT-Connect-Endpunkte und ihrer Konfigurationsdaten. Ergänzend unterstützt die
Anwendung die lokale Prüfung von Schlüsseln, Zertifikaten und Fachdatenschemata.

**In Entwicklung. Version 0.1.0 ist in Arbeit.** Es gibt noch keinen freigegebenen
Installer. Der aktuelle Implementierungsstand liegt im Branch
[`release/0.1.0`](https://github.com/FBForge/fbforge-inspect/tree/release/0.1.0).
Dieser Hauptbranch enthält zunächst die Projektvorstellung und die Lizenz.

## Geplanter und implementierter Umfang

- Konfiguration von FIT-Connect-Zustellpunkten und Ergebnisse öffentlicher Routing-Abfragen untersuchen.
- Adressierungssignaturen aus PVOG nachvollziehen; dort können außerdem EfA-Parameter hinterlegt sein.
- Öffentliche Schlüssel und Zertifikate sowie die Struktur von JSON Schema und XSD lokal prüfen.
- Befunde lokal anzeigen und bei Bedarf als JSON- oder HTML-Bericht speichern.

Fachdatenschemata sind bei der jeweiligen Leistung im FIT-Connect-Zustellpunkt
hinterlegt. Die genannten Funktionen werden im Entwicklungsbranch dokumentiert;
die abschließende Windows- und NVDA-Abnahme steht noch aus.

## Dokumentation und Projektstand

- [Projektvorstellung auf fbforge.de](https://fbforge.de/inspect/)
- [Anleitung zum Entwicklungsstand](https://fbforge.de/inspect/hilfe/0.1/)
- [Entwicklungsbranch mit Quellcode und Prüfhinweisen](https://github.com/FBForge/fbforge-inspect/tree/release/0.1.0)

## Lizenz

[European Union Public Licence 1.2 (EUPL-1.2)](LICENSE).

FBForge ist ein privates Open-Source-Projekt von Fabian Braun.
