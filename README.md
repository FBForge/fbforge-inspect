# FBForge Inspect

<img src="assets/brand/inspect-source.png" alt="FBForge Inspect" width="112" />

Öffentliche FIT-Connect- und PVOG-Schnittstellen verstehen, Konfiguration prüfen und Befunde lokal weitergeben. Eine eigenständige Desktop-App ohne Anmeldung, Backendserver oder Telemetrie.

FBForge Inspect ergänzt Integrationstests um nachvollziehbare Diagnosen. Es nutzt ausschließlich öffentliche Schnittstellen und implementiert deren dokumentierte Verträge unabhängig. Kein offizielles Produkt der FITKO.

## Funktionen in 0.1.0

- **Destination v2/v3:** Status, Typ, Metadaten, Leistungen, Prozessnachrichten, Regionen, Fachschemareferenzen und öffentliche Verschlüsselungsschlüssel. Optionaler Versionsvergleich und zusammengehörige Konfigurationserwartungen.
- **Routing:** LeiKa mit AGS, ARS oder Gebiets-ID, öffentliche Gebietssuche, begrenzte vollständige Paginierung und erhaltene Ergebniskontexte. Optionale PVOG-JZuFi-Evidenz mit eigener Quellenangabe.
- **Adressierungssignaturen:** Dekodierung und Verifikation getrennt, PS512 und fest gewählte öffentliche FIT-Connect-Schlüsselquellen. Leistung und ARS werden im gemeinsamen signierten Kontext verglichen.
- **Prozessnachrichten:** Öffentliche v3-Suche anhand von Modell, Nachricht und Region. Optionale paarbezogene Limits zwischen zwei Destinationen.
- **Öffentliche Schlüssel:** JWK/JWKS aus Datei, Zwischenablage oder HTTPS. RSA, Algorithmen, Schlüsseloperationen, x5c-Zuordnung, Laufzeit und PKIX gegen ausdrücklich ausgewählte öffentliche CA-Anker. Optionale signaturgeprüfte OCSP-/CRL-Diagnose.
- **Fachschemata:** Struktur und Referenzen von JSON Schema und XSD, sichere XML-Verarbeitung. Keine Fachdaten erforderlich.
- **Berichte:** Lokales JSON oder eigenständiges HTML mit Exportvorschau. Rohantworten werden nur nach ausdrücklicher Auswahl exportiert.
- Helles und dunkles Design sowie laufend aktualisierte Systemeinstellung, Tastaturbedienung und vollständig gebündelte Hilfe.

## Starten

Die erste Zielplattform ist **Windows x64** mit WebView2. Der Installer bündelt eine reduzierte Java-25-Laufzeit. Ein separat installiertes Java ist für die App nicht erforderlich. Internet wird ausschließlich für ausdrücklich gestartete öffentliche Abfragen benötigt. Lokale JWK- und Schema-Inspektion funktionieren offline.

Installer werden erst nach Abnahme veröffentlicht. Solange noch kein Release vorhanden ist, sind die [Bauanleitung](BUILDING.md) und der Quellcode der verfügbare Einstieg. Hier gibt es keinen Platzhalter-Download.

1. Umgebung und API-Version wählen. Standard ist TEST / v3.
2. **Destination prüfen** öffnen und die UUID eingeben.
3. Prüfung starten und Befunde samt Quellen lesen.
4. Bei Bedarf erwartete Leistung, Region oder Schema ergänzen.
5. Berichtsvorschau prüfen und lokal speichern.

## Ergebnisse richtig einordnen

Ein bestätigter Einzelbefund ist keine Freigabe der gesamten Integration. Unbekannt bleibt unbekannt, beispielsweise bei fehlendem Vertrauensanker, nicht verifizierbarem Sperrstatus, Ratenlimit oder unvollständiger Paginierung. Öffentliche Lesbarkeit beweist keine Sende- oder Empfangsfähigkeit.

V2 und V3 werden ausdrücklich ausgewählt. Routing verwendet die öffentliche Routing API v2. STAGE besitzt keine eigene Routing-API, FBForge Inspect kennzeichnet dort die verwendete PROD-Routingquelle. PVOG STAGE wird mit TEST, PVOG PROD mit dem produktiven Routing verglichen.

Kein Senden/Empfangen, keine Konten, keine Verwaltungsoperationen, kein BPMN-Import, keine Schema-Instanzvalidierung und keine interne Routing-Rekonstruktion. Authentifizierte Unternehmensproxies und private TLS-CAs werden in 0.1.0 nicht eingerichtet.

Die [Hilfequelle](docs/help.json), [Architektur](docs/ARCHITECTURE.md), [öffentlichen Quellen](docs/SOURCES.md) und [Sicherheitsgrenzen](SECURITY.md) erklären Details und Grenzen.

## Entwickeln

Tauri 2 · Vue 3 · TypeScript · Vite · Rust · Java 25. Versionen sind in package-lock.json, desktop/Cargo.lock und engine/pom.xml festgelegt.

```sh
npm ci
npm run build
npm run test
npm run licenses:collect
npm run desktop:dev
```

Für Rust-Abhängigkeiten muss zuvor `cargo fetch --locked --manifest-path desktop/Cargo.toml` erfolgreich gewesen sein. Details zur Zielplattform und den Prüfungen stehen in [BUILDING.md](BUILDING.md).

Lizenz: [EUPL-1.2](LICENSE). Fremdkomponenten: [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) und `licenses/`.
