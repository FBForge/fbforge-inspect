# FBForge Inspect

<img src="assets/brand/inspect-source.png" alt="" width="80" height="80">

Desktop-Werkzeug für FIT-Connect-Zustellpunkte, Routing und PVOG-Adressierung. Für Entwickler und Integratoren.

- Zustellpunkte über API v2/v3 abrufen und Konfiguration, Leistungen, Regionen und öffentliche Schlüssel prüfen.
- Routing-Ziele und Gebiete suchen, Ergebniskontexte erhalten und PVOG-Adressierungssignaturen ergänzen.
- V3-Prozessnachrichten und zusammengehörige Konfigurationserwartungen prüfen.
- Öffentliche JWK/JWKS, Zertifikatsketten und Sperrstatus untersuchen.
- JSON Schema und XSD lokal einlesen, Struktur und Referenzen nachvollziehen.
- Befunde als JSON oder HTML speichern. Rohantworten nur nach ausdrücklicher Auswahl einschließen.

Für **Windows x64 und FIT-Connect TEST, STAGE und PROD**. Java wird mitgeliefert. Lokale Schlüssel- und Schemainspektion funktionieren offline. Die Anwendung benötigt keine Zugangsdaten und führt keine Verwaltungsoperationen oder Zustellungen aus.

## Entwicklungsstand

**0.1.0 wird für die Abnahme vorbereitet.** Der vollständige Quellstand liegt auf [release/0.1.0](https://github.com/FBForge/fbforge-inspect/tree/release/0.1.0). `main` enthält derzeit den Projekteinstieg. Ein freigegebener Installer ist noch nicht veröffentlicht.

Die [CI-Prüfung](https://github.com/FBForge/fbforge-inspect/actions/workflows/verify.yml) baut den Windows-Kandidaten und prüft den nativen WebView2-/Java-Durchlauf, die Berichtsdateien über den Windows-Speicherdialog sowie NVDA-Sprachsequenzen. Ein erfolgreicher Build ersetzt keine vollständige Funktions- und Installationsabnahme. Ein unsignierter Testkandidat ist kein signierter Release.

Zum Testen öffnen Sie einen erfolgreichen CI-Lauf für `release/0.1.0` und laden unter **Artifacts** das Paket `windows-unsigned-candidate` herunter. Nach dem Entpacken den `*-setup.exe`-Installer verwenden. `SHA256SUMS.txt` enthält die Prüfsummen. Das Paket ist unsigniert und noch kein veröffentlichter Release. Der Download der Actions-Artefakte benötigt eine GitHub-Anmeldung.

## Einstieg

1. Den Windows-Testkandidaten installieren oder den Quellstand von `release/0.1.0` mit der [Bauanleitung](https://github.com/FBForge/fbforge-inspect/blob/release/0.1.0/BUILDING.md) bauen.
2. FBForge Inspect starten und Umgebung sowie API-Version wählen.
3. Einen Zustellpunkt prüfen oder eine lokale Schlüssel- beziehungsweise Schemadatei einlesen.
4. Befunde und Quellen prüfen und bei Bedarf einen Bericht lokal speichern.

Die [Anleitung mit Bildern und Beispielen](https://fbforge.de/inspect/hilfe/0.1/) und die gebündelte Offline-Hilfe beschreiben dieselben Abläufe. Helle, dunkle und automatische Darstellung sind verfügbar.

## Quellen und Grenzen

Fachdatenschemata sind bei der jeweiligen Leistung im FIT-Connect-Zustellpunkt hinterlegt. PVOG liefert Adressierungssignaturen (Destination Signatures) und gegebenenfalls EfA-Parameter.

STAGE besitzt keine eigene Routing-API. Die Anwendung kennzeichnet dort die verwendete PROD-Routingquelle. Fehlende Vertrauensanker, ungeklärte Sperrstatus und unvollständige Ergebnisse bleiben ausdrücklich ungeklärt. Eine Diagnose ist keine Freigabe der gesamten Integration.

[Architektur](https://github.com/FBForge/fbforge-inspect/blob/release/0.1.0/docs/ARCHITECTURE.md) · [Lokal bauen](https://github.com/FBForge/fbforge-inspect/blob/release/0.1.0/BUILDING.md) · [Mitwirken](https://github.com/FBForge/fbforge-inspect/blob/release/0.1.0/CONTRIBUTING.md) · [Issues](https://github.com/FBForge/fbforge-inspect/issues) · [Sicherheit](https://github.com/FBForge/fbforge-inspect/blob/release/0.1.0/SECURITY.md)

## Lizenz

[EUPL-1.2](LICENSE). Siehe auch [Drittanbieter-Lizenzen](https://github.com/FBForge/fbforge-inspect/blob/release/0.1.0/THIRD_PARTY_NOTICES.md).
