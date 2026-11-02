# Bauen und prüfen

Benötigt werden Node ab 22.13, npm, OpenJDK 25 mit Maven und Rust 1.96.0 mit rustfmt und Clippy. Die CI verwendet Node 24. Für Windows sind der MSVC-Toolchain und die C++-Buildwerkzeuge von Visual Studio erforderlich. Lokale Linux-Entwicklung benötigt GTK 3, WebKitGTK 4.1, libsoup 3 und deren Entwicklungspakete. Releaseziel von 0.1.0 ist Windows x64.

## Entwicklung

```sh
npm ci
cargo fetch --locked --manifest-path desktop/Cargo.toml
npm run licenses:collect
npm run desktop:dev
```

`desktop:dev` baut den Java-Kern, stellt ihn mit einer lokalen Java-Laufzeit bereit und startet Tauri mit Vite. `npm run dev` allein zeigt die Oberfläche im Browser und führt keine Live-Diagnose aus.

Für das Staging `JAVA_HOME` auf ein OpenJDK 25 der Zielplattform mit GPL 2 und Classpath Exception setzen, etwa Eclipse Temurin. `npm run stage:engine` erzeugt daraus eine reduzierte Laufzeit und übernimmt die Lizenztexte. Windows- und Linux-Laufzeiten dürfen nicht ausgetauscht werden. Bei einem JDK-Wechsel ein frisches Buildverzeichnis verwenden.

Das Staging vergleicht vollständige Version, Hersteller und Plattform der gebündelten Laufzeit mit dem gewählten JDK. `engine/runtime-provenance.json` im Paket hält diese Angaben, Quellenreferenzen und Prüfsummen fest. Die Paketbeilage `THIRD_PARTY_NOTICES.md` wird daraus ergänzt. Unterstützte JDK-Metadaten liefern die genaue Quellrevision und gegebenenfalls die zugehörigen Buildskripte. Fehlt eine passende Quellenreferenz, muss `INSPECT_JAVA_SOURCE_URL` ausdrücklich auf die entsprechenden Quellen dieser Distribution zeigen. Der Build prüft die Laufzeitidentität, lädt aber keine Quellenarchive herunter. Deren Verfügbarkeit und Bereitstellung vor einer Binärverteilung prüfen.

## Aufbau und Erweiterung

Die Oberfläche trennt Arbeitsstände (`useWorkspace`), native Diagnoseläufe (`useDiagnosis`) und den Exportdialog. Formulare unter `frontend/src/forms/` und Ergebnisansichten unter `frontend/src/results/` gehören jeweils zu einer Aufgabe. Gemeinsame Berichtstypen stehen in `contracts.ts` und `reportData.ts`; reine Anzeigehelfer in `diagnosis.ts` und `resultValues.ts`. Die CSS-Module werden bewusst in fester Reihenfolge importiert, damit die bestehende Kaskade erhalten bleibt.

Der Java-Kern verwendet `Api` als schmale Fassade. `PublicJsonClient` besitzt JSON-Abrufe und Paginierung; Destination-, Routing- und Prozessdiagnosen haben eigene Klassen. `Certificates` prüft Kette und Vertrauen, `CertificateRevocation` die unabhängigen OCSP-/CRL-Nachweise. Neue Netzwerkfunktionen müssen weiterhin den gemeinsamen `PublicHttp`-Transport verwenden.

Wenn die Oberfläche einen Befund besonders aufbereitet, erhält er eine stabile `FindingCode`-Kennung in Java und TypeScript. Anzeigetexte dienen nicht als Kennung. Änderungen am Berichtsvertrag durch Kern- und Oberflächentests gemeinsam absichern; zusätzliche Felder dürfen bestehende Protokoll-v1-Verbraucher nicht brechen.

## Automatisierte Prüfungen

```sh
npm run build
npm test
npm run format:check
cargo fmt --manifest-path desktop/Cargo.toml --check
cargo test --locked --manifest-path desktop/Cargo.toml
cargo clippy --locked --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
npm run test:ui
npm run docs:build
```

`build` umfasst Typecheck, Frontend und Maven `package` einschließlich Kerntests. Die Browserprüfungen benötigen einen zu Playwright passenden Chromium (`npx playwright install chromium`) oder einen ausdrücklich über `INSPECT_CHROMIUM_EXECUTABLE` gewählten Browser. Für Kern- und UI-Tests dasselbe `JAVA_HOME` verwenden. Node-Tests laufen mit `--test-isolation=none`.

UI-Tests verwenden isolierte Browserkontexte, synthetische IPC-Antworten und bei lokalen Parser-/Schlüsselproben den echten Java-Kern. Java-Tests prüfen unter anderem synthetische Zertifikate und signierte OCSP-/CRL-Fixtures ohne externe Dienste. Paketprüfungen erkennen abweichende Java-Versionen, veränderte Laufzeiten und unpassende Paketbeilagen. Diese Tests ersetzen keine native Windows-Abnahme.

Die [offizielle XZuFi-Paketprüfung](docs/PVOG-MAPPING.md#reproduzierbare-paketprüfung) benötigt zuvor lokal bereitgestellte ZIPs. `scripts/verify-xzufi.py` prüft festgehaltene SHA-256, Deklarationsinventar und alle 15 XSD-Dateien über den gebauten Java-Kern. Es lädt keine Quellen oder Fremdreferenzen nach. Nach Änderungen an PVOG oder der Schemaansicht diese Prüfung und die nativen Windows-Prüfungen wiederholen.

## Windows-Paket und native Prüfung

Zuerst den Java-Kern mit `mvn -f engine/pom.xml package` bauen. Anschließend in Windows PowerShell:

```powershell
.\scripts\build-windows.ps1 -InspectJavaHome 'C:\Toolchains\temurin-25'
```

Das Skript verwendet ein neues temporäres Buildverzeichnis, installiert festgeschriebene npm-Pakete ohne Installationsskripte, prüft TypeScript, sammelt Lizenzen, bündelt Java und baut EXE sowie unsignierten NSIS-Installer. Es prüft den Kern mit der gebündelten Laufzeit und verifiziert deren Paketbeilage erneut. Artefakte und vollständige SHA-256-Summen liegen unter `.local/artifacts/windows-x64/`. Die EXE allein ist kein portables Paket und benötigt ihr Ressourcenverzeichnis.

Der Installer richtet WebView2 bei Bedarf über den offiziellen Bootstrapper ein, wofür Internet erforderlich ist. Java wird bereits mitgeliefert. Die unterstützten Buildbefehle verwenden `--no-binary-patching`, damit lose und installierte EXE bytegleich bleiben. Inspect benötigt keine Updater-Kennung. Der Test vergleicht die vollständige installierte Datei per SHA-256, ohne Byteausnahmen.

`node tests/native-windows.mjs <Pfad-zur-EXE> <Belegordner>` startet nur die gewählte App mit temporärem WebView2-Profil und lokalem Debug-Port. Die Probe prüft den UI-/Rust-/Java-Weg, getrennte Arbeitsstände, Scrollpositionen, Schema-Rückweg, Struktursuche, HTML-Vorschau und private Schlüsselabwehr. Sie bedient den echten Windows-Speicherdialog im eigenen App-Prozess und prüft Abbruch sowie JSON/HTML mit und ohne Rohantworten gegen die gespeicherten Dateien. Danach beendet sie die App. Lokale Proben unter einem gewöhnlichen, nicht erhöhten Windows-Benutzer starten.

## CI und Abnahmegrenzen

[Verify 0.1.0 candidate](.github/workflows/verify.yml) läuft bei Pushes nach `main` und `release/0.1.0`, bei Pull Requests und manuell. Linux prüft Build, Java-/Node-/Rust-Tests, Clippy, Format, Browserabläufe und Hilfe. Windows baut den unsignierten Kandidaten und führt native Prüfungen aus. Mit `INSPECT_LIVE_READONLY=1` prüft die lose App zusätzlich TEST-Gebietssuche, Destination mit JWK/x5c und unabhängige Limits über öffentliche GETs. Konkrete Live-Abnahmen weiterer Fälle sind gesonderte Momentaufnahmen.

`scripts/windows-accessibility-smoke.ps1` installiert genau diesen Kandidaten, vergleicht die EXE-Prüfsumme und prüft Tastaturnavigation sowie elf von NVDA tatsächlich erzeugte Sprachlabels. Der offizielle NVDA-Download wird gegen eine feste SHA-256 geprüft. Der stille Synthesizer liefert keine hörende Usability-Abnahme. Nach der Sprachprüfung starten die nativen Kern-/Exportprüfungen die installierte EXE mit bereinigtem Java-/PATH-Umfeld. Die Belege liegen im Actions-Artefakt unter `.local/native-evidence/` und `.local/installed-native-evidence/`. Erst ein erfolgreicher Lauf und die Artefakte des jeweiligen Commits belegen dessen Prüfung.

Erhöhte WebView2-Hosts auf GitHub-Hosted-Runnern benötigen kurzzeitige, anwendungsspezifische Testparameter. Der Helfer prüft Repository, Runnerkontext und Pfade, bindet den Debug-Port nur an Loopback und entfernt eigene Einträge danach. Vorhandene Überschreibungen werden nicht ersetzt. Der ausgelieferte Installer richtet diese Testparameter nicht ein. Eine fehlende interaktive Desktop-Sitzung lässt die native Prüfung fehlschlagen.

Vor Veröffentlichung bleiben separat zu prüfen: frischer Windows-Benutzer, Start ohne systemweites Java, WebView2-Erstinstallation, Deinstallation, Tastatur und hörende NVDA-Bedienung, Hell/Dunkel und 200-%-Skalierung sowie repräsentative positive und negative Destination-, Routing-, Prozess- und PKI-Fälle. Ungeklärte Ergebnisse bleiben offen. Schemainspektion ist keine vollständige Schema-/Instanzvalidierung.

Abhängigkeiten mit `npm audit`, `cargo audit --file desktop/Cargo.lock` und `mvn -f engine/pom.xml dependency:list -DincludeScope=runtime` gegen aktuelle Advisories prüfen. Datenbankstand, Koordinaten und Grenzen protokollieren. Die bisherigen Linux-Hinweise zu `glib 0.18.5` und `proc-macro-error 1.0.4` waren nicht im ermittelten Windows-x64-Baum enthalten. Nach Lockfile-Änderungen diese Einordnung erneuern. Ein leerer Trefferbestand ist keine Sicherheitsgarantie und bewertet nicht automatisch die gebündelte JDK-Distribution.

Signierung von EXE und Installer mit Zeitstempel sowie Veröffentlichung benötigen eine gesonderte Freigabe. Signierprovider und private Schlüssel sind nicht konfiguriert. Es gibt keine automatische Veröffentlichung oder Git-Hooks. Ein erfolgreicher unsignierter Kandidat wird nicht als signierter Release ausgegeben.
