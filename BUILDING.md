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

## Qualitätsmessung

`npm run quality:java` führt einen sauberen Maven-Build mit JaCoCo aus und prüft mindestens 85 % Zeilen- und 65 % Zweigabdeckung im vollständigen Kern. Routingsteuerung, HTTP-Antwortparser und Signaturprüfung benötigen jeweils mindestens 95 % Zeilen- und 80 % Zweigabdeckung. Der Bericht liegt unter `engine/target/site/jacoco/`.

`npm run quality:ui` führt dieselben Browserprüfungen mit V8-Instrumentierung aus. Source Maps ordnen die Ergebnisse Vue und TypeScript zu. Alle ausführbaren Module müssen im Bericht vorkommen; reine Typmodule werden nicht als ungetesteter Laufzeitcode gezählt. Mindestwerte sind 90 % ausführbare Zeilen und 80 % Zweige. Die V8-eigene Quellenzeilenzählung unterscheidet sich von der ausführbaren Zeilenzählung des Grenzwertprüfers. Berichte liegen unter `.local/quality/browser-coverage/`.

`npm run quality:duplicates` prüft Produktquellen auf gleiche Blöcke ab fünf Zeilen und 70 Tokens, ohne Kommentare. Der Höchstwert beträgt 0,5 % der vom Werkzeug gezählten Zeilen. Eingebettete Vue-Sprachen werden getrennt tokenisiert. Die Messung ersetzt weder fachliche Tests noch eine Prüfung semantisch ähnlicher Implementierungen.

Alle drei Prüfungen laufen im bestehenden Linux-CI-Job; ihre Berichte werden als Prüfartefakt bereitgestellt. Windows prüft weiterhin den tatsächlichen Installer, WebView2 und NVDA. Die öffentliche Zielprüfung und TLS-Verifikation bleiben im produktiven `PublicHttp`/`PinnedHttp`-Pfad. Tests ersetzen DNS und Abrufantworten nur über interne Schnittstellen; der reine Antwortparser benötigt keine Netzwerkfreigabe.

## V-PKI-Truststore aktualisieren

`engine/src/main/resources/de/fbforge/inspect/vpki/manifest.json` bindet alle gebündelten DER-Dateien an Herkunft und SHA-256. Neue Wurzeln ausschließlich aus dem offiziellen BSI-HTTPS-Angebot beziehen und gegen die separat veröffentlichten BSI-Fingerabdrücke prüfen. Untergeordnete CAs aus dem offiziellen x500-Verzeichnis erst nach erfolgreicher Kettenprüfung zu diesen Wurzeln als Kettenmaterial aufnehmen. Eigenständige, nicht unabhängig verankerte Wurzeln bleiben ausgeschlossen. Manifest und Dateien gemeinsam aktualisieren und `VpkiTrustStoreTest` sowie den vollständigen Java-Testlauf ausführen. Die App lädt keine neuen Vertrauensanker automatisch aus dem Netz; abgelaufene Einträge werden nicht verwendet.

## Windows-Paket und native Prüfung

Zuerst den Java-Kern mit `mvn -f engine/pom.xml package` bauen. Anschließend in Windows PowerShell:

```powershell
.\scripts\build-windows.ps1 -InspectJavaHome 'C:\Toolchains\temurin-25'
```

Das Skript verwendet ein neues temporäres Buildverzeichnis, installiert festgeschriebene npm-Pakete ohne Installationsskripte, prüft TypeScript, sammelt Lizenzen, bündelt Java und baut EXE sowie unsignierten NSIS-Installer. Es prüft den Kern mit der gebündelten Laufzeit und verifiziert deren Paketbeilage erneut. Artefakte und vollständige SHA-256-Summen liegen unter `.local/artifacts/windows-x64/`. Die EXE allein ist kein portables Paket und benötigt ihr Ressourcenverzeichnis.

Der Installer richtet WebView2 bei Bedarf über den offiziellen Bootstrapper ein, wofür Internet erforderlich ist. Java wird bereits mitgeliefert. Die unterstützten Buildbefehle verwenden `--no-binary-patching`, damit lose und installierte EXE bytegleich bleiben. Inspect benötigt keine Updater-Kennung. Der Test vergleicht die vollständige installierte Datei per SHA-256, ohne Byteausnahmen.

`node tests/native-windows.mjs <Pfad-zur-EXE> <Belegordner>` startet nur die gewählte App mit temporärem WebView2-Profil und lokalem Debug-Port. Die Probe prüft den UI-/Rust-/Java-Weg, getrennte Arbeitsstände, Scrollpositionen, Schema-Rückweg, Struktursuche, HTML-Vorschau und private Schlüsselabwehr. Sie bedient den echten Windows-Speicherdialog im eigenen App-Prozess und prüft Abbruch sowie JSON/HTML mit und ohne Rohantworten gegen die gespeicherten Dateien. Danach beendet sie die App. Lokale Proben unter einem gewöhnlichen, nicht erhöhten Windows-Benutzer starten.

## CI und Abnahmegrenzen

[Verify 0.1.0 candidate](.github/workflows/verify.yml) läuft bei Pushes nach `main` und `release/0.1.0`, bei Pull Requests und manuell. Linux prüft Build, Java-/Node-/Rust-Tests, Clippy, Format, Browserabläufe und Hilfe. Windows baut den unsignierten Kandidaten und führt native Prüfungen aus. Mit `INSPECT_LIVE_READONLY=1` prüft die lose App zusätzlich TEST-Gebietssuche, Destination mit JWK/x5c und unabhängige Limits. Der öffentliche Nienburger PROD-Zustellpunkt wird in v2 und v3 über die Destination geprüft: Schlüsselkennung mit Leerzeichen, tatsächlicher Schlüsselabruf, gebündeltes V-PKI-Vertrauen sowie OCSP und beide CRLs. Die Abfragen bleiben öffentlich und lesend; OCSP verwendet dafür POST. Konkrete Live-Abnahmen weiterer Fälle sind gesonderte Momentaufnahmen.

Die gehosteten Verify-Jobs nutzen nach Lockfile beziehungsweise POM getrennte npm- und Maven-Download-Caches. `npm ci`, Java-Tests und alle Paketprüfungen laufen weiterhin bei jedem Durchlauf. Push und Pull Request desselben Quellbranches teilen eine Concurrency-Gruppe, damit nur der neueste Lauf weiterarbeitet. Das Quellrepository ist Teil dieser Gruppe, sodass gleichnamige Fork-Branches getrennt bleiben. Windows baut das Frontend nur im frischen Paketverzeichnis, zusammen mit der dortigen Typprüfung. Die NVDA-Vorbereitung unterdrückt die langsame Download-Fortschrittsanzeige von Windows PowerShell und prüft weiterhin die festgeschriebene SHA-256. Der Signierjob übernimmt diese Caches nicht und baut weiterhin aus einem frischen Verzeichnis. Laufzeitgewinne anhand des jeweiligen CI-Laufs bewerten.

`scripts/windows-accessibility-smoke.ps1` installiert genau diesen Kandidaten, vergleicht die EXE-Prüfsumme und prüft Tastaturnavigation sowie elf von NVDA tatsächlich erzeugte Sprachlabels. Der offizielle NVDA-Download wird gegen eine feste SHA-256 geprüft. Der stille Synthesizer liefert keine hörende Usability-Abnahme. Nach der Sprachprüfung starten die nativen Kern-/Exportprüfungen die installierte EXE mit bereinigtem Java-/PATH-Umfeld. Die Belege liegen im Actions-Artefakt unter `.local/native-evidence/` und `.local/installed-native-evidence/`. Erst ein erfolgreicher Lauf und die Artefakte des jeweiligen Commits belegen dessen Prüfung.

Erhöhte WebView2-Hosts auf GitHub-Hosted-Runnern benötigen kurzzeitige, anwendungsspezifische Testparameter. Der Helfer prüft Repository, Runnerkontext und Pfade, bindet den Debug-Port nur an Loopback und entfernt eigene Einträge danach. Vorhandene Überschreibungen werden nicht ersetzt. Der ausgelieferte Installer richtet diese Testparameter nicht ein. Eine fehlende interaktive Desktop-Sitzung lässt die native Prüfung fehlschlagen.

Vor Veröffentlichung bleiben separat zu prüfen: frischer Windows-Benutzer, Start ohne systemweites Java, WebView2-Erstinstallation, Deinstallation, Tastatur und hörende NVDA-Bedienung, Hell/Dunkel und 200-%-Skalierung sowie repräsentative positive und negative Destination-, Routing-, Prozess- und PKI-Fälle. Ungeklärte Ergebnisse bleiben offen. Schemainspektion ist keine vollständige Schema-/Instanzvalidierung.

Abhängigkeiten mit `npm audit`, `cargo audit --file desktop/Cargo.lock` und `mvn -f engine/pom.xml dependency:list -DincludeScope=runtime` gegen aktuelle Advisories prüfen. Datenbankstand, Koordinaten und Grenzen protokollieren. Die bisherigen Linux-Hinweise zu `glib 0.18.5` und `proc-macro-error 1.0.4` waren nicht im ermittelten Windows-x64-Baum enthalten. Nach Lockfile-Änderungen diese Einordnung erneuern. Ein leerer Trefferbestand ist keine Sicherheitsgarantie und bewertet nicht automatisch die gebündelte JDK-Distribution.

## Lokal signiert unter Windows bauen

Der gemeinsame Einstieg für lokale und CI-Builds ist `scripts/build-signed-windows.ps1`. In einer normalen interaktiven Windows-PowerShell ausführen. Windows-JDK 25, Node, Git, Rust/MSVC und Visual Studio C++ Build Tools mit Windows SDK werden benötigt. Maven wird bei Bedarf als geprüfter Download im lokalen Buildtools-Verzeichnis bereitgestellt.

Das Code-Signing-Zertifikat muss im Speicher `Cert:\CurrentUser\My` des ausführenden Windows-Benutzers liegen und Zugriff auf seinen privaten Schlüssel haben. Beim Hardwaretoken dieselbe Middleware wie für FBForge Connect verwenden. Die PIN wird ausschließlich im Dialog des Providers eingegeben; Skripte und GitHub speichern sie nicht.

```powershell
Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
    Select-Object Subject, Thumbprint, NotAfter, HasPrivateKey

.\scripts\build-signed-windows.ps1 -Thumbprint 'THUMBPRINT-DES-CODE-SIGNING-ZERTIFIKATS'
```

Der Builder erwartet einen sauberen Checkout und baut in einem frischen temporären Verzeichnis. Er prüft JDK-Quellenzuordnung und Laufzeit, führt die Java-Kerntests und die Typprüfung aus und erzeugt ein Paket mit signierter EXE, signiertem NSIS-Installer, Laufzeit und Lizenzbeilagen. Tauri sichert beim Signieren die tatsächlich in NSIS verpackte Hauptdatei, da die lose Build-EXE nach dem Bundling wiederhergestellt wird. Beide ausgelieferten Dateien werden mit Authenticode und `signtool verify /pa /all /tw` auf erwarteten Signierer und Zeitstempel geprüft. SHA-256-Prüfsummen und `build-info.json` entstehen danach. Der Standardzeitstempeldienst ist `http://timestamp.digicert.com` mit RFC 3161 und SHA-256; `-TimestampUrl` erlaubt einen anderen passenden Dienst. Der ausgegebene Paketpfad kann mit `-InspectOutputDirectory` auf ein neues lokales Verzeichnis festgelegt werden.

Die privaten Komfortskripte zum frischen Windows-Checkout und zur Einrichtung der GitHub-Variablen bleiben außerhalb des Repos. Für einen wiederholten Build ist weder ein Reset des WSL-Checkouts noch dessen Verwendung erforderlich.

## Signierung in GitHub Actions

`Sign Windows candidate` wird ausschließlich manuell auf `main` gestartet. Vor dem Signierrechner prüft ein gehosteter Job, dass die eingegebene Verify-Run-ID zu genau diesem Commit im eigenen Repository gehört und beide Jobs `verify` und `windows` erfolgreich abgeschlossen sind. Ein fehlgeschlagener, noch laufender oder fremder Lauf wird abgelehnt. Der Workflow veröffentlicht keinen Release und führt kein Deployment aus.

Einmalig durch den Repository-Eigentümer einzurichten:

1. Environment `windows-signing` mit genau `FBForge` als erforderlichem Reviewer; Eigenfreigabe zulassen und Deployments auf `main` beschränken.
2. Repository-Variable `WINDOWS_SIGNING_THUMBPRINT` auf das verwendete Zertifikat setzen. Optional `WINDOWS_TIMESTAMP_URL` für den Zeitstempeldienst setzen. Private Schlüssel und PIN gehören nicht in GitHub-Variablen oder Secrets.
3. Vor der Runner-Registrierung für das öffentliche Repository die Workflow-Freigabe für **alle externen Fork-Beitragenden** verlangen. Nur geprüfte Fork-Workflows freigeben; alle Schreibberechtigten des Repos müssen für den Signierrechner vertrauenswürdig sein. Ein Repo-Runner kann auch von anderen Workflows desselben Repos angesprochen werden; der Main- und Environment-Schutz dieses Signierworkflows allein ersetzt diese Prüfung nicht. Wo verfügbar, die Runner-Zulassung auf den Signierworkflow auf `main` beschränken.
4. Einen für Inspect zugelassenen Windows-x64-Self-hosted-Runner mit dem zusätzlichen Label `fbforge-signing` registrieren. Für einen separaten Repo-Runner ein eigenes Verzeichnis verwenden. Er muss unter demselben angemeldeten Windows-Benutzer mit Zertifikat und Token laufen. Wegen Token-PIN-Dialogen `run.cmd` in der interaktiven Sitzung verwenden, keinen Windows-Dienst. Die Runner-Zulassung und der Schutz von `main` müssen in GitHub geprüft werden.

### Inspect-Runner neben Connect einrichten

Das offizielle Windows-x64-Runnerpaket aus den GitHub-Repositoryeinstellungen nach `C:\actions-runner-inspect` entpacken. Den vorhandenen Connect-Runner und dessen Ordner beibehalten. Den Inspect-Runner unter dem angemeldeten Benutzer mit Zertifikat und Token registrieren:

```powershell
Set-Location 'C:\actions-runner-inspect'

.\config.cmd `
    --url 'https://github.com/FBForge/fbforge-inspect' `
    --name 'fbforge-signing' `
    --labels 'fbforge-signing' `
    --work '_work'
```

Den kurzlebigen Registration Token ausschließlich im lokalen Dialog eingeben. Bei einer angebotenen Runnergruppe genügt für diesen Repo-Runner `Default`. Bei **Run as a service?** mit **N** antworten. Anschließend `.\run.cmd` starten und das Fenster während der Nutzung offen lassen. In GitHub muss der Runner die Labels `self-hosted`, `Windows`, `X64` und `fbforge-signing` besitzen. Der Name dient der Anzeige, das Label wählt den Runner im Workflow aus.

Meldet die Einrichtung **already configured**, zuerst den laufenden Inspect-Runner mit Strg+C stoppen. Beim bisherigen Inspect-Runner in GitHub **Remove** öffnen und den dort bereitgestellten Befehl mit Removal Token lokal ausführen. Danach mit einem neuen Registration Token erneut konfigurieren. Keine Konfigurationsdateien manuell löschen und den Connect-Runner nicht entfernen.

Connect und Inspect dürfen denselben Rechner mit getrennten Runnerordnern verwenden. Signierläufe am gemeinsamen Hardwaretoken nacheinander starten. Die Concurrency-Regel des Inspect-Workflows serialisiert keine Jobs aus dem Connect-Repository. Vor dem Start müssen die oben genannten Freigabe- und Runner-Zulassungsregeln geprüft sein.

Der Signierjob verwendet denselben PowerShell-Builder wie lokal. Ein unabhängiger gehosteter Windows-Job lädt ausschließlich den Kandidaten aus diesem Signierlauf, prüft vollständige Dateiprüfsummen, Commit-/Run-Zuordnung, Signaturen und Laufzeitprovenienz und führt Installation, nativen Kern-/Exportdurchlauf und NVDA-Prüfungen aus. Erst danach entsteht das Artefakt `windows-signed-verified`. `windows-signed-candidate` ist der noch nicht unabhängig abgenommene Zwischenstand.

Der Workflow wird in Actions verfügbar, sobald seine Definition auf dem Standardbranch liegt. Das Vorbereiten auf `release/0.1.0` aktiviert keine Signierung und ersetzt keinen Main-Merge. Ein aktueller fehlgeschlagener Verify-Lauf bleibt ein Abnahmehindernis und wird durch Signieren nicht aufgehoben. Ein erfolgreicher unsignierter Kandidat ist kein signierter Release.

### Vom geprüften Kandidaten zur Veröffentlichung

`Publish verified Windows release` veröffentlicht einen vorhandenen Versions-Tag aus einem erfolgreich abgeschlossenen Signierlauf. Der manuelle Workflow läuft ausschließlich auf `main` im eigenen Repository. Er prüft den Tag-Commit gegen den Signierlauf und dessen erfolgreichen Main-Verify. Er lädt ausschließlich `windows-signed-verified`, prüft dessen unveränderlichen Artefakt-Digest, Signaturprovenienz und sämtliche Dateiprüfsummen und erzeugt Installer, vollständiges Paketarchiv, Release-Provenienz und Prüfsummen. Der Installer wird beim Übernehmen nur umbenannt, seine Bytes bleiben unverändert. Die hochgeladenen Release-Dateien müssen dieselben Größen und Digests haben, bevor der vorhandene Entwurf veröffentlicht wird. Der Publishjob benötigt keinen Zugriff auf den Hardwaretoken.

Der Versions-Tag muss bereits auf den tatsächlich signierten und geprüften Quellcommit zeigen. Ein späterer Main-Commit mit ergänzter Release-Automatisierung wird nicht nachträglich zum Quellstand dieses Pakets erklärt. Bei fehlenden oder abweichenden Nachweisen bleibt der Release unveröffentlicht.

1. Den Release-PR nach `main` prüfen und übernehmen. Für den daraus entstandenen Main-Commit müssen beide Verify-Jobs erfolgreich sein. Einen älteren grünen Release-Branch-Lauf nicht als Ersatz verwenden.
2. Den Inspect-Runner unter dem angemeldeten Windows-Benutzer mit Zertifikat und Token über `run.cmd` starten. Einen parallelen Connect-Signierlauf am selben Token vermeiden.
3. In Actions **Sign Windows candidate** auf `main` starten. Bei `validation_run` die numerische Run-ID des erfolgreichen Main-Verify-Laufs eintragen und die Freigabe für `windows-signing` erteilen. Die PIN ausschließlich im lokalen Providerdialog eingeben.
4. Den erfolgreichen unabhängigen Job `verify-package` abwarten. Für die manuelle Windows-Abnahme das Artefakt `windows-signed-verified` aus genau diesem Lauf verwenden. `windows-signed-candidate` ist noch nicht abgenommen.
5. Nach der dokumentierten manuellen Abnahme den geprüften Commit und die SHA-256 des tatsächlich geprüften Installers für Tag, Release und Downloadzuordnung festhalten. Website und Download erst im ausdrücklich freigegebenen Veröffentlichungsschritt aktualisieren.
