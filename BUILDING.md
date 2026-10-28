# Bauen und prüfen

Benötigt werden Node ab 22.13, npm, Java/JDK 25 mit Maven sowie Rust ab 1.90. Die aktuell festgelegten Tauri-Abhängigkeiten können neuere Rust-Versionen benötigen. Der jeweils erfolgreich geprüfte Compiler wird im lokalen Abnahmeprotokoll festgehalten. Verwende für Windows den MSVC-Toolchain und die C++-Buildwerkzeuge von Visual Studio. Für lokale Linux-Entwicklung benötigt Tauri GTK 3, WebKitGTK 4.1, libsoup 3 und deren Entwicklungspakete.

Es gibt keinen Baseline-Build vor der Projekterstellung, da das Repository leer angelegt wurde.

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

Die UI-Tests benötigen einen zu Playwright passenden Chromium. Ein bereits installierter Browser kann ausdrücklich über `INSPECT_CHROMIUM_EXECUTABLE` gewählt werden. Die Tests verwenden isolierte Kontexte und synthetische IPC-Antworten. Sie ersetzen keinen nativen Windows-/WebView2-Test. Die Java-Tests verwenden synthetische Zertifikate und signierte OCSP-/CRL-Fixtures, ohne externe Dienste.

Node-Tests laufen mit `--test-isolation=none`, damit die hier verwendete Node-24-Laufzeit die einzelnen exportbezogenen Tests zuverlässig ausführt. Maven `package` führt die Kerntests aus. Es gibt keine Git-Hooks und keinen automatischen Veröffentlichungsweg.

## Windows x64

Zuerst den plattformunabhängigen Java-Kern mit `mvn -f engine/pom.xml package` bauen. Anschließend in Windows PowerShell:

```powershell
.\scripts\build-windows.ps1 -InspectJavaHome 'C:\Toolchains\temurin-25'
```

Das Skript erstellt ein neues temporäres Windows-Buildverzeichnis, installiert die festgeschriebenen npm-Pakete ohne Installationsskripte, prüft den TypeScript-Code, sammelt Lizenztexte, bündelt eine Windows-Java-Laufzeit, führt eine lokale Kernprobe aus und baut EXE und NSIS-Installer. Artefakte und SHA-256-Summen werden unter `.local/artifacts/windows-x64/` abgelegt. Dieser lokale Ordner wird nicht versioniert.

Der Installer richtet WebView2 bei Bedarf über den offiziellen Bootstrapper ein, wofür Internetzugang erforderlich ist. Die gebündelte Java-Laufzeit benötigt keinen zusätzlichen Download beim App-Start. Die EXE allein ist kein portables Distributionspaket, sie benötigt das zugehörige Ressourcenverzeichnis.

Eine echte native Probe kann unter Windows mit `node tests/native-windows.mjs <Pfad-zur-EXE> <Belegordner>` gestartet werden. Sie startet nur die angegebene App mit einem temporären WebView2-Profil, prüft den vollständigen IPC-Weg und beendet den Prozess danach. Der lokale Debug-Port gilt ausschließlich für diesen Testprozess.

## Vor der Veröffentlichung

Windows-Neuinstallation in einem separaten Testbenutzer, Start ohne installiertes Java, Deinstallation, WebView2-Verhalten, Tastatur/NVDA und Netzwerkfehler prüfen. Danach signierte EXE und signierten Installer mit Zeitstempel erzeugen und deren Signaturen sowie SHA-256-Summen kontrollieren. Ein Signierprovider oder ein privater Schlüssel ist nicht im Repository konfiguriert. Signierung und Veröffentlichung erfordern einen gesondert autorisierten Schritt. Ein erfolgreicher Build wird nicht als signiertes Release ausgegeben.
