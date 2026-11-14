# Windows-Updater nach regulärem Lease-Drop

Freigegebene Reihenfolge:
1. Bestehenden Consent- und Idlepfad beibehalten. Verifiziertes Update samt Bytes
   genau einmal nativ vormerken. Das Updatinggate bleibt nach erfolgreichem
   Vormerken bis zum Eventloop-Ende gesetzt.
2. Nur `app.exit(0)` anfordern. Nach `run_return` auf demselben Startthread die
   Lease freigeben und erst danach den vorgemerkten offiziellen Installer starten.
   Sein Windows-`on_before_exit`-Callback ist bewusst leer, weil Tauri schon
   bereinigt wurde. Ein Fehler zeigt einen festen nativen Hinweis und beendet
   ausschließlich die eigene Anwendung mit Exitcode 1.
3. Einmaligkeit, Gate und Reihenfolge über denselben Produktionshelper prüfen.
   Native Verträge und Windows-GNU-Build werden isoliert geprüft. Ein
   Compile-only-Javaressourcenplatzhalter beweist keinen Engine-/GUI-Betrieb.

Der Lease-Drop reserviert keine Installerübernahme. Eine neue Anwendung kann
vor dem Installer gewinnen. Das ist normale Konkurrenz und erfordert künftig
eine Busy-Ablehnung durch dessen Guard. Diese Änderung ersetzt weder den Guard
noch behebt sie ForceShutdown-/Legacy-NSIS-Sicherheit oder einen Elternhandoff.
Schlägt Tauri `request_exit` fehl, kann `AppHandle::exit` direkt `process::exit`
aufrufen. Dann wird kein Installer gestartet, aber der reguläre Lease-Drop kann
ausfallen. OS-Recovery/Abandoned ist kein bestätigter Wiederherstellungsnachweis.

## Nativer Übergabevertrag

`InstallerHandoff` besitzt genau einen der Zustände `Empty`, `Pending` oder
`Closed`. Ein erfolgreicher Submit verbraucht Update und Bytes. Doppelte
Übergaben, vergiftete Locks und verspätete Übergaben nach dem Eventloop-Ende
werden abgewiesen. Auch ein normaler Exit ohne Update schließt die Queue.
Ein von null verschiedener Eventloop-Exitcode startet keinen Installer.

Main behält einen eigenen `Arc` außerhalb des Tauri-State. Es übergibt die
wirkliche, auf diesem Startthread erworbene Lease an `finish_after_exit`.
Dieser gemeinsame Produktionshelper droppt zuerst die Lease und nimmt erst
danach das Pending-Element. Nur dann ruft er einmal `Update::install(bytes)`
auf. Rückgabefehler und Rustpaniken führen zum festen nativen Fehlerhinweis und
Exitcode 1. Es wird kein Eventloop wieder geöffnet. Ein OS-Crash bleibt außerhalb
dieser Fehlerbehandlung und ist keine bestätigte Wiederherstellung.

Im Worker bleibt das Updatinggate nach erfolgreichem Queuecommit gesetzt.
Der Status wird vorher kopiert. Danach gibt es keine fallible Stateoperation
mehr vor dem Exitwunsch und keinen Tauri-Statezugriff nach `app.exit(0)`.
Fehler vor dem Commit behalten den bisherigen Rückweg. Der bisherige
nicht-Windows-Installationspfad bleibt erhalten.

## Gebundener SDK-Vertrag und Nachweisgrenzen

Die tatsächlich gebundenen Quellen sind `tauri-plugin-updater` 2.14.1 und
Tauri 2.12.4. `UpdaterExt::updater_builder` setzt zunächst einen Callback mit
`AppHandle::cleanup_before_exit`. Nur beim Erzeugen des Windows-Updateobjekts
wird er durch `on_before_exit(|| {})` ersetzt. `run_return` hat die Bereinigung
schon durchgeführt. Der spätere Installationspfad verwendet deshalb keine
Tauri-Runtimeoperation über den voreingestellten Callback.

Die öffentliche Methode `Update::install` prüft die Paketbytes erneut gegen
die Signatur, bevor sie den Windows-Installer extrahiert und startet. Bei
erfolgreichem Start ruft das SDK `process::exit(0)` auf. Genau deshalb muss der
reguläre Lease-Drop bereits vorher stattgefunden haben. Signaturprüfung,
Installermodus, Argumente, Pakete und Neustartoption werden nicht verändert.

Quellen:
[UpdaterExt und Cleanup-Callback](https://github.com/tauri-apps/plugins-workspace/blob/updater-v2.14.1/plugins/updater/src/lib.rs),
[UpdaterBuilder, Update::install und Windows-Start](https://github.com/tauri-apps/plugins-workspace/blob/updater-v2.14.1/plugins/updater/src/updater.rs),
[Tauri AppHandle::exit und run_return](https://github.com/tauri-apps/tauri/blob/tauri-v2.12.4/crates/tauri/src/app.rs).

Die portablen Tests prüfen Queue-Einmaligkeit, normales und abnormales Ende,
Gatezustand, vergiftete Queue und Fehler-/Panikreihenfolge. Der Windows-Test
ruft denselben Produktionshelper mit einer echten Lease auf. Ein anderer
Thread muss anschließend sauber erwerben können, damit rekursive Übernahme
auf demselben Thread keinen vergessenen Windows-Mutex verdeckt.
Wine ist dabei kein nativer GUI- oder Installerabnahmenachweis. Es werden keine
signierten Installer gestartet und keine Java-Engine mit einem Platzhalter
simuliert. Vollständige Appbuilds verwenden nur einen ausdrücklich markierten
Compile-only-Ressourcentext in Scratch. Native Windows-/GUI-/Engine- und
signierte Updateabnahme bleiben offen. Exakte Prüfbefehle, Grenzen, bestehende
Lintfehler und Quellhashes stehen im externen Patch-Handoff.
