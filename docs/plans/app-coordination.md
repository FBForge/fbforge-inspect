# Native Appstart-Koordination

FBForge Inspect erwirbt unter Windows vor dem Tauri-Builder und damit vor
Java-/Arbeitsstart eine native Lease für `de.fbforge.inspect`. Der Startthread behält sie
bis zum Ende des Eventloops und der vorhandenen Exit-Callbacks. `run_return`
ermöglicht die Freigabe auf demselben Thread; anschließend endet der Prozess
mit dem vom Eventloop gelieferten Exitcode. Die Lease ist weder Send noch Sync.
Sie wird nicht in Tauri-State, einen Tokio-Task oder einen anderen Prozess verschoben.

Bei Busy oder nicht verfügbarer Koordination erscheint ein nativer Hinweis und
nur der neue Einstieg endet. Verlassener Besitz wird gehalten übernommen, für
diesen Start abgewiesen und auf dem Besitzerthread freigegeben. Ein späterer
expliziter Start kann neu erwerben. Ein solcher Erwerb beweist keine frühere
Java-/Datenwiederherstellung: Drop verbraucht das Abandoned-Signal; nach dem
letzten Handle-Close kann das OS die Historie vollständig verlieren. Es gibt
keinen dauerhaften Crashmarker. Die bestehende Ressourceninitialisierung bleibt
verpflichtend und wird nicht umgangen. Es wird kein ungeprüfter Health-Gate simuliert.
Die Hinweise enthalten keine SID, Pfade oder rohen Windowsfehler. Linux-Entwicklung
verwendet die Windows-Startlease nicht.

Die vorhandenen Close-/Shutdownpfade bleiben bestehen. Der Starteinstieg beendet
keine laufende Anwendung oder fremde Arbeit. Inspect besitzt kein Single-Instance-Plugin. Ein Windows-Zweiteinstieg wird
nun vor dem Builder mit einem Busy-Hinweis beendet.

## Bewusste Grenzen

Dieser Schritt bindet weder NSIS noch den appinternen Updater ein. Deren bisherige
Start-/Exitlogik bleibt unverändert. Insbesondere umgeht der Windows-Updater mit
`process::exit` einen regulären Rust-Drop. Daraus folgt keine sichere oder
unterbrechungsfreie Installerübergabe. Die vorhandene NSIS-Prüfung kann weiterhin
laufende Prozesse schließen; dieser Schritt gibt Installer oder Updates nicht frei.

Nur kooperierende Versionen derselben Windows-Benutzer-SID verwenden diesen
Ausschluss. Alte Apps, Java-Kinder nach einem Absturz, andere Benutzer-SIDs und
maschinenweite Installationen sind damit nicht als beendet oder sicher bewiesen.
Ein gestartetes Prozessimage kann bereits vor `main` abgebildet sein. Native
Prozess-/Paket-/Healthprüfungen vor Änderungen bleiben getrennt erforderlich.

## Herkunft und Prüfung

Die lokale Abhängigkeit `desktop/vendor/suite-coordination` enthält unveränderte
öffentliche Quellen und Tests aus Suite-Commit
`6db3fc7ff7ec9fe132ad8285d34757de4936d1ec`. Standalone-Manifest, EUPL-1.2-Lizenzwahl
und genaue Quellhashes stehen in dessen `PROVENANCE.md`.

Die portable Bibliothek verwendet weiterhin Rust 1.90. Ihre Mindestversion wird
mit einer getrennt heruntergeladenen, per offiziellem SHA256 geprüften Rust-1.90-
Toolchain getestet. Die Anwendung verwendet ihren bisherigen Buildvertrag
(Rust 1.96.0); ihre `rust-version = "1.90"` wird nicht angehoben.

Native Vertragsprüfungen:

```sh
cargo test --manifest-path desktop/native-contracts/Cargo.toml --locked --offline
cargo build --manifest-path desktop/native-contracts/Cargo.toml --locked --offline
cargo clippy --manifest-path desktop/native-contracts/Cargo.toml --all-targets --locked --offline -- -D warnings
cargo fmt --manifest-path desktop/Cargo.toml --all -- --check
cargo fmt --manifest-path desktop/native-contracts/Cargo.toml --all -- --check
```

Standalone-Mindestversionsprüfung mit Rust 1.90.0:

```sh
cargo test --manifest-path desktop/vendor/suite-coordination/Cargo.toml --locked --offline
cargo build --manifest-path desktop/vendor/suite-coordination/Cargo.toml --target x86_64-pc-windows-gnu --locked --offline
cargo test --manifest-path desktop/vendor/suite-coordination/Cargo.toml --target x86_64-pc-windows-gnu --no-run --locked --offline
```

Alle ausgeführten Prüfungen laufen in netzloser Bubblewrap-Sandbox mit leerem,
explizit gesetztem Environment, schreibgeschützten Quellen/Toolchains/Caches und
einmaliger vollständiger Scratchquellkopie. Grenzen: 2 GiB Scratch, 4 GiB
Adressraum, 120 CPU-Sekunden je Prozess, 128 Prozesse, 256 MiB je Datei,
256 Dateideskriptoren, 600 Sekunden je Sitzung und zwei Cargo-Buildjobs.

Die unveränderte vollständige Windows-GNU-Buildbaseline scheitert hier am nicht
eingecheckten Java-Ressourcenstaging (`resources/engine`). Ein getrennter
Compile-only-Lauf darf ausschließlich in Scratch eine markierte Textdatei in
diesem Ordner verwenden, um den tatsächlichen Windows-Appcode einschließlich
`run_return` und Dialog zu bauen/linten. Dies stellt keine Engine bereit und
belegt weder Paketinhalt noch Java-/WebView-/Windows-Laufzeitverhalten.
Reale Windows-, RDP-/UAC-, Installer- und Zugänglichkeitsabnahme bleiben offen.

## Erfasster Stand dieser Integration

Die unveränderte Bibliothek bestand unter Rust 1.90.0 drei portable Verhaltenstests
und zwei Compile-fail-Doctests. Windows-GNU-Bibliothek und alle drei nativen
Testprogramme wurden mit demselben Compiler gelinkt. Der Library-/Testquellcode
ist in beiden Repositories identisch zum genannten Suite-Commit.

Der tatsächliche neue Appstartadapter bestand in einer separaten kleinen
Prüfcrate drei portable Fälle und Windows-GNU-Clippy mit `-D warnings`. Unter
Wine 10 bestanden alle fünf Windowsfälle einschließlich gehaltenem Besitz,
Freigabe auf dem Besitzerthread und abgewiesenem verlassenem Besitz. Kein
Appfenster, kein MessageBox-Aufruf und kein Tauri-Eventloop wurden dabei gestartet.
Die Tests belegen keine frühere Java-/Datenrecovery und keine Paket-/Workerfreiheit.
Wine meldete außerdem bekannte Fontconfig-/OLE-Fehler bei seinen Hilfsprozessen;
diese Emulatorprüfung ersetzt keine echte Windowsabnahme.

Die tatsächliche Windows-App wurde im beschriebenen Compile-only-Scratchlauf
mit dem dokumentierten Compiler gelinkt. Der unveränderte Connect-Exitcallback
und `run_return` sind damit tatsächlich kompiliert. Der nachfolgende vollständige
Desktop-Clippy-Befehl

```sh
cargo clippy --manifest-path desktop/Cargo.toml --target x86_64-pc-windows-gnu --all-targets --locked --offline -- -D warnings
```

blieb innerhalb des 600-Sekunden-Sitzungslimits unvollständig. Der erfolgreiche
Adapter-Clippy ist ausdrücklich kein vollständiger Desktop-Clippy. Die spätere
Präzisierung des Abandoned-Warntexts und Testnamens wird separat am vollständigen
Adapter erneut kompiliert/geprüft; die Eventloopintegration bleibt unverändert.

Inspect bestand alle 14 portablen nativen Vertragstests, nativen Build,
NativeContracts-Clippy mit `-D warnings` und die Rustformatprüfungen unter
Rust 1.96.0.
