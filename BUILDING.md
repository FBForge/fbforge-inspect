# Bauen und prüfen

Benötigt werden Node ab 22.13, npm, Java/JDK 25 mit Maven sowie Rust. Die CI verwendet Node 24 und Rust 1.96.0 mit rustfmt und Clippy. Verwenden Sie diese Rust-Version für den festgeschriebenen Abhängigkeitsbaum. Verwende für Windows den MSVC-Toolchain und die C++-Buildwerkzeuge von Visual Studio. Für lokale Linux-Entwicklung benötigt Tauri GTK 3, WebKitGTK 4.1, libsoup 3 und deren Entwicklungspakete.

## Entwicklung

```sh
npm ci --registry=https://registry.npmjs.org
cargo fetch --locked --manifest-path desktop/Cargo.toml
npm run build
npm run licenses:collect
npm run desktop:dev
```

`desktop:dev` baut den Java-Kern, stellt ihn samt lokaler Java-Laufzeit bereit und startet Tauri mit Vite. `npm run dev` allein zeigt die Oberfläche im Browser. Der Browser führt keine Live-Diagnose aus.

`npm run stage:engine` erzeugt mit einem OpenJDK 25 der aktuellen Plattform eine reduzierte Laufzeit. Verwende eine Distribution mit GPL 2 und Classpath Exception, etwa Eclipse Temurin, und setze `JAVA_HOME`. Das Staging prüft die Lizenztexte und lehnt abweichend lizenzierte Laufzeiten ab. Die Laufzeit darf nicht zwischen Windows und Linux kopiert werden. Beim Wechsel der JDK-Version die ausschließlich generierte Ressource `desktop/resources/engine/runtime` in einem frischen Buildverzeichnis neu erzeugen. Der Java-Kern und die gesammelten Lizenzverzeichnisse müssen vorher vorhanden sein.

## Automatisierte Prüfungen

```sh
npm run test
npm run typecheck
npm run build:frontend
npm run format:check
cargo fmt --manifest-path desktop/Cargo.toml --check
cargo check --locked --manifest-path desktop/Cargo.toml
cargo test --locked --manifest-path desktop/Cargo.toml
cargo clippy --locked --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
npm run test:ui
npm run docs:build
```

Die UI-Tests benötigen einen zu Playwright passenden Chromium. Ein bereits installierter Browser kann ausdrücklich über `INSPECT_CHROMIUM_EXECUTABLE` gewählt werden. Die Tests verwenden isolierte Kontexte. Sie prüfen Diagnoseabläufe mit synthetischen IPC-Antworten, lokale Datei- und Parsergrenzen sowie öffentliche RSA-Schlüssel über den echten Java-Kern, Systemdarstellung, Präferenzspeicherung, Toolbar und Exportdialoge. Deshalb zuerst `npm run build:engine` ausführen und für die Tests dasselbe `JAVA_HOME` verwenden. Sie ersetzen keinen nativen Windows-/WebView2-Test. Die Java-Tests verwenden synthetische Zertifikate und signierte OCSP-/CRL-Fixtures ohne externe Dienste.

Node-Tests laufen mit `--test-isolation=none`, damit die hier verwendete Node-24-Laufzeit die einzelnen exportbezogenen Tests zuverlässig ausführt. Maven `package` führt die Kerntests aus. Es gibt keine Git-Hooks und keinen automatischen Veröffentlichungsweg.

## Windows x64

Zuerst den plattformunabhängigen Java-Kern mit `mvn -f engine/pom.xml package` bauen. Anschließend in Windows PowerShell:

```powershell
.\scripts\build-windows.ps1 -InspectJavaHome 'C:\Toolchains\temurin-25'
```

Das Skript erstellt ein neues temporäres Windows-Buildverzeichnis, installiert die festgeschriebenen npm-Pakete ohne Installationsskripte, prüft den TypeScript-Code, sammelt Lizenztexte, bündelt eine Windows-Java-Laufzeit, führt eine lokale Kernprobe aus und baut EXE und NSIS-Installer. Artefakte und SHA-256-Summen werden unter `.local/artifacts/windows-x64/` abgelegt. Dieser lokale Ordner wird nicht versioniert.

Der Installer richtet WebView2 bei Bedarf über den offiziellen Bootstrapper ein, wofür Internetzugang erforderlich ist. Die gebündelte Java-Laufzeit benötigt keinen zusätzlichen Download beim App-Start. Die EXE allein ist kein portables Distributionspaket, sie benötigt das zugehörige Ressourcenverzeichnis.

Die unterstützten Buildbefehle verwenden `--no-binary-patching`. Tauri würde
sonst vor der NSIS-Bündelung die interne Bundle-Kennung von `UNK` auf `NSS`
ändern und die lose EXE anschließend zurücksetzen. Inspect verwendet keinen
Updater und benötigt diese Kennzeichnung nicht. Der Installer enthält dadurch
dieselbe EXE, die der native Test verwendet. Die installierte Datei wird weiterhin
vollständig per SHA-256 verglichen, ohne Ausnahmen für einzelne Bytes.


Eine echte native Probe kann unter Windows mit `node tests/native-windows.mjs <Pfad-zur-EXE> <Belegordner>` gestartet werden. Sie startet nur die angegebene App mit einem temporären WebView2-Profil, prüft den vollständigen IPC-Weg und beendet den Prozess danach. Der lokale Debug-Port gilt ausschließlich für diesen Testprozess.

## Vor der Veröffentlichung

Windows-Neuinstallation in einem separaten Testbenutzer, Start ohne installiertes Java, Deinstallation, WebView2-Verhalten, Tastatur/NVDA und Netzwerkfehler prüfen. Danach signierte EXE und signierten Installer mit Zeitstempel erzeugen und deren Signaturen sowie SHA-256-Summen kontrollieren. Ein Signierprovider oder ein privater Schlüssel ist nicht im Repository konfiguriert. Signierung und Veröffentlichung erfordern einen gesondert autorisierten Schritt. Ein erfolgreicher Build wird nicht als signiertes Release ausgegeben.

## CI und Windows-Abnahme

[Verify 0.1.0 candidate](.github/workflows/verify.yml) läuft bei Pushes nach `main` und `release/0.1.0`, bei Pull Requests sowie manuell. Der Linux-Job prüft Build, Java-/Node-/Rust-Tests, Clippy, Formatierung, Browser-Workflows und Hilfegenerierung. Der Windows-Job baut einen **unsignierten** NSIS-Kandidaten mit gebündeltem Java und bewahrt Installer, SHA-256-Summen und native Testbelege als Actions-Artefakt auf. Es gibt keine automatische Veröffentlichung und keine Signierschlüssel.

Die native Probe `tests/native-windows.mjs` prüft WebView2 und den tatsächlichen IPC-Weg. Danach installiert `scripts/windows-accessibility-smoke.ps1` genau den gebauten Installer in ein temporäres Verzeichnis, gleicht die EXE-Prüfsumme ab und prüft Tastaturnavigation und von **NVDA 2026.2 tatsächlich erzeugte Sprachsequenzen**. Der stille Synthesizer vermeidet Audioausgabe; dies ist keine hörende Usability-Abnahme. Die NVDA-Vorbereitung prüft den Download gegen eine feste SHA-256-Summe. Die Testhilfen folgen dem EUPL-lizenzierten Connect-Vorbild (Referenz `6708441`). Sie werden nicht in der App mitgeliefert.

Erhöht gestartete WebView2-Hosts ignorieren Umgebungsüberschreibungen. Auf dem
wegwerfbaren GitHub-Hosted-Runner richtet die Probe deshalb nach dem Connect-
Vorbild kurzzeitig anwendungsspezifische WebView2-Testparameter für
`fbforge-inspect.exe` und `de.fbforge.inspect` ein. Der Debug-Port bindet nur an
Loopback, das Profil liegt unter `RUNNER_TEMP`. Vorhandene Überschreibungen
werden nicht ersetzt. Der Helfer prüft Repository, Runnerkontext und Pfade und
entfernt die eigenen Einträge im Abschlussweg. Es gibt keine Wildcard-Regel
und keine solche Änderung durch den ausgelieferten Installer. Lokale native
Proben unter einem gewöhnlichen, nicht erhöhten Windows-Benutzer starten.

Der Windows-NVDA-Job benötigt eine interaktive Desktop-Sitzung des Runners. Fehlt sie, schlägt der Job fehl; das gilt nicht als bestandene Barrierefreiheitsprüfung. Ein angelegter Workflow ist noch kein erfolgreicher CI-Lauf. Erst die Artefakte des aktuellen Commits belegen Windows-Build und native Prüfungen.

Die Windows-CI aktiviert zusätzlich `INSPECT_LIVE_READONLY=1`: In der echten
installierbaren Oberfläche wird die öffentliche TEST-Gebietssuche nach Bonn
gestartet. Der Test verlangt mindestens ein Ergebnis, HTTP 200 und ausschließlich
GET-Anfragen an den TEST-Routinghost. Er speichert den Befund ohne Rohantworten
als Beleg. Dies führt keine Zustellung oder Verwaltungsoperation aus und ersetzt
nicht die Abnahme repräsentativer eigener Destinationen, PVOG-Kontexte oder PKI.

Für die abschließende menschliche Abnahme: frisch installieren, ohne systemweit installiertes Java starten, alle Diagnosearten mit repräsentativen öffentlichen Daten prüfen, JSON/HTML mit und ohne Rohantworten speichern, den nativen Speicherdialog abbrechen und bestätigen, alle Formulare mit Tastatur und NVDA bedienen, Hell/Dunkel und 200-%-Skalierung prüfen und deinstallieren. Reale PKI-/Routing-/Destination-Daten und Netzwerkausfälle bleiben eigenständige Integrationsprüfungen.

## Abhängigkeiten vor einer Abnahme prüfen

Für npm `npm audit` und für den festgeschriebenen Rust-Baum `cargo audit --file desktop/Cargo.lock` mit einer aktuellen RustSec-Datenbank ausführen. Informative Meldungen nicht als „keine Befunde“ ausgeben: Der Linux-GTK-Pfad enthält derzeit Hinweise zu `glib 0.18.5` und `proc-macro-error 1.0.4`. Beide sind im ermittelten Windows-x64-Abhängigkeitsbaum nicht enthalten. Linux bleibt Entwicklungsplattform, kein Releaseziel von 0.1.0. Bei Änderungen an Tauri und dem Lockfile diese Einordnung erneut prüfen.

Java-Laufzeitabhängigkeiten mit `mvn -f engine/pom.xml dependency:list -DincludeScope=runtime` ermitteln und gegen aktuelle Advisories mit Maven-Versionssemantik abgleichen. Datenbankstand, geprüfte Koordinaten und Grenzen des Abgleichs protokollieren. Ein leerer Trefferbestand ist keine allgemeine Sicherheitsgarantie; die gebündelte JDK-Distribution und das konkrete Windows-Paket bleiben eigene Prüfgegenstände.
