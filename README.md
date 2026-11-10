# FBForge Inspect

<img src="desktop/icons/128x128@2x.png" alt="" width="80" height="80">

Desktop-Werkzeug für FIT-Connect-Zustellpunkte, Routing und PVOG-Adressierung. Für Entwickler und Integratoren.

- Zustellpunkte über API v2/v3 abrufen und Konfiguration, Leistungen, Regionen und öffentliche Schlüssel prüfen.
- Routing-Ziele und Gebiete suchen, PVOG-Leistungsbeschreibungen mit OE-Verweisen vorschlagen und Adressierungssignaturen abgleichen.
- V3-Prozessnachrichten und zusammengehörige Konfigurationserwartungen prüfen.
- Öffentliche JWK/JWKS, Zertifikatsketten und Sperrstatus untersuchen.
- JSON Schema und XSD lokal einlesen, Struktur und Referenzen nachvollziehen.
- Befunde als JSON oder HTML speichern. Rohantworten nur nach ausdrücklicher Auswahl einschließen.

Für **Windows x64 und FIT-Connect TEST, STAGE und PROD**. Java wird mitgeliefert. Lokale Schlüssel- und Schemainspektion funktionieren offline. Für FIT-Connect werden keine Zugangsdaten benötigt. Ein manueller Proxy kann eigene Zugangsdaten erfordern. Die Anwendung führt keine Verwaltungsoperationen oder Zustellungen aus.

## Version 0.2.0

Windows-Pakete und Prüfsummen finden Sie im [Release 0.2.0](https://github.com/FBForge/fbforge-inspect/releases/tag/0.2.0). Verwenden Sie den Installer aus dem veröffentlichten Release. Die EXE allein ist kein portables Paket und benötigt die mitgelieferte Laufzeit und Ressourcen.

Die [CI-Prüfung](https://github.com/FBForge/fbforge-inspect/actions/workflows/verify.yml) baut den Windows-Kandidaten und prüft den nativen WebView2-/Java-Durchlauf, die Berichtsdateien über den Windows-Speicherdialog sowie NVDA-Sprachsequenzen. Der manuelle Signierlauf prüft anschließend die Signaturen und die Installation desselben Pakets auf einem unabhängigen Windows-Runner. Diese Einzelprüfungen bestätigen keine vollständige Barrierefreiheit oder Gesamtintegration.

Für lokale Authenticode-Builds und den manuellen CI-Signierweg siehe [Signierung unter Windows](BUILDING.md#lokal-signiert-unter-windows-bauen). Beide verwenden denselben PowerShell-Builder und den lokalen Zertifikatsspeicher beziehungsweise Hardwaretoken. Nur das unabhängig geprüfte Artefakt `windows-signed-verified` dient als Ausgangspunkt der Veröffentlichung. Unsignierte Actions-Testkandidaten sind keine Release-Pakete.

## Einstieg

1. Den Windows-Installer aus dem [Release 0.2.0](https://github.com/FBForge/fbforge-inspect/releases/tag/0.2.0) installieren oder mit der [Bauanleitung](BUILDING.md) lokal bauen.
2. FBForge Inspect starten und Umgebung sowie API-Version wählen.
3. Einen Zustellpunkt prüfen oder eine lokale Schlüssel- beziehungsweise Schemadatei einlesen.
4. Befunde und Quellen prüfen und bei Bedarf einen Bericht lokal speichern.

Die [Anleitung mit Bildern und Beispielen](https://fbforge.de/inspect/hilfe/0.2/) beschreibt die Abläufe. Eine durchsuchbare Anleitung ist offline in der App verfügbar. Die bebilderte Websitehilfe wird ausdrücklich im Browser geöffnet. Helle, dunkle und automatische Darstellung sind verfügbar.

## Quellen und Grenzen

Fachdatenschemata sind bei der jeweiligen Leistung im FIT-Connect-Zustellpunkt hinterlegt. PVOG liefert Adressierungssignaturen (Destination Signatures) und gegebenenfalls EfA-Parameter.

STAGE besitzt keine eigene Routing-API. Die Anwendung kennzeichnet dort die verwendete PROD-Routingquelle. Fehlende Vertrauensanker, ungeklärte Sperrstatus und unvollständige Ergebnisse bleiben ausdrücklich ungeklärt. Eine Diagnose ist keine Freigabe der gesamten Integration.

[Architektur](https://github.com/FBForge/fbforge-inspect/blob/main/docs/ARCHITECTURE.md) · [Lokal bauen](https://github.com/FBForge/fbforge-inspect/blob/main/BUILDING.md) · [Mitwirken](https://github.com/FBForge/fbforge-inspect/blob/main/CONTRIBUTING.md) · [Issues](https://github.com/FBForge/fbforge-inspect/issues) · [Sicherheit](https://github.com/FBForge/fbforge-inspect/blob/main/SECURITY.md)

## Lizenz

[EUPL-1.2](LICENSE). Siehe auch [Drittanbieter-Lizenzen](https://github.com/FBForge/fbforge-inspect/blob/main/THIRD_PARTY_NOTICES.md).

## Analysefunktionen in 0.2.0

- Sammelprüfung mit bis zu 32 Einträgen, Pause, Abbruch und gezielter Wiederholung.
- Netzwerkphasen, konkrete TLS-Ursachen und ausdrücklich ergänzbare HTTPS-Unternehmens-CAs.
- Rückwärtsrouting vom bekannten Zustellpunkt und nachvollziehbare öffentliche Adressierungskontexte.
- Direkte öffentliche Zertifikats- und Adressierungssignaturprüfung sowie öffentliche Schlüsselwechselvorbereitung.
- Zertifikatsinventar mit betroffenen untersuchten Zustellpunkten und Kalenderexport.
- Portables Java-CLI für Windows und Linux mit versionierten Aufträgen, Ergebnisimport und stabilen Exitcodes.
- Kontextübernahme, Vorschau erkannter Kennungen, Supporttexte und integrierte Windows-Aktualisierung.

Das [CLI-Handbuch](docs/CLI.md) erläutert Serverbetrieb und Austauschformate. Die Desktop-App bleibt für Windows x64 vorgesehen. Das plattformneutrale CLI benötigt Java 25 oder neuer und enthält im eigenen Archiv keine Java-Laufzeit. Es prüft öffentliche Konfiguration und Erreichbarkeit. Authentifizierte Zustellproben und Fachdatenprüfung gehören zu FBForge Connect.
