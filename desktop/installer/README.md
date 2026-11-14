# NSIS-Integration: gesperrter Entwicklungsschritt

Diese Vorlage ist **kein veröffentlichbarer Installer**. `FBForgeRequireMutationEvidence`
bricht mit Exitcode70 ab, weil der notwendige native Bestands-/Prozessnachweis noch
fehlt. Es gibt keinen Erfolgs-Return, Compile-Define, Kommandozeilenschalter oder
Rendererwert zum Umgehen dieses Gates. Das gilt auch für vermeintliche
Neuinstallationen, Silent-/Passive-/Update-Modus und den neuen Uninstaller.
Eine vorhandene Lease bedeutet weder `Stopped` noch eine Installationsfreigabe.

## Lebensdauer und unveränderte ABI

`.onInit` und `un.onInit` beginnen mit Acquire der fest einkompilierten AppID und
dem fehlenden nativen Mutationsnachweis, vor Sprache/Registry/Produkt-/Nutzdaten
oder WebView-Aktionen. Eigene DLL-/NSIS-Temp-Extraktion vor dem Guard ist kein
versprochener Null-Schreibvorgang. Abandoned/Busy/Unavailable, NSIS-Fehler,
fehlende/frühere/abweichende Antworten oder verlorene Sentinel führen zum Abbruch.
Kein Wiedererwerbsversuch und kein Fremdprozess wird beendet.

Der residente Unicode-x86-Guard verwendet genau AcquireV1/StatusV1/ReleaseV1.
Vor jeder geschützten Phase und jedem Hook wird erneut Status/Missing-Evidence
geprüft. Die Lease bleibt bis zum Ende aller Mutationen gehalten. Der
Install-Finalizer verarbeitet die Desktopauswahl zuerst und bestätigt danach
Release. GUI-Finish-Leave, ShowReadme- und Run-Callback konvergieren idempotent.
Spätere Shortcutcallbacks schreiben nichts. Silent/Passive erledigen Shortcuts
im Installabschnitt und finalisieren in `.onInstSuccess` vor optionalem RunAsUser.
Der neue Uninstaller löst erst nach Post-Uninstall-Hook und letzter Mutation.
Fehler/Cancel verlassen den Prozess ohne Start der Anwendung. Unload/GUI-Ende
geben die Lease nicht aktiv frei. Ein fehlendes Release-ACK erlaubt keinen Start.

Alle auf Registry-UninstallString basierenden NSIS-/WiX-Aufrufe und ihre
anschließende Cleanupdelegation sind entfernt. Historische Uninstaller werden
niemals unter der Elternlease gestartet. Force-Shutdown-Makros werden nicht
eingebunden oder aufgerufen. `safe-utils.nsh` ist das unveränderte Tauri2.12.3-
Hilfsmakropaket **ohne** dessen `CheckIfAppIsRunning`/RestartManager-Killmakro.
Die Registry darf keinen beliebigen Dateinamen für ausführbare Cleanupziele liefern.
Neustart-queued Verzeichnislöschungen wurden entfernt. Vorhandene In-use-Dateien
müssen in einer später freigegebenen Implementierung zu Fehlern führen.

## Nächster erforderlicher nativer Eingang

Das fehlende Gate darf erst durch einen echten nativen Vorflug ersetzt werden,
der an diese feste AppID und dieselbe noch gehaltene Lease gebunden ist:

1. Der **endgültige** native Ziel-/Bestandsplan enthält ausschließlich validierte,
   begrenzte lokale Dateiziele und aktuelle vertrauenswürdige Tool-/Policydaten.
   Registry, CLI, Renderer und Marker können keine Befehle oder Freigaben liefern.
   Ein nachträglicher Wechsel des Installationsziels invalidiert den Vorflug.
2. Jede betroffene vorhandene Installation wird am gehaltenen unveränderten
   Programmhandle auf x64-PE, aktive Publisherpolicy, Zeitstempel/Revocation und
   exakten AppID-/Protokoll1-Marker geprüft. Alle relevanten Prozesse brauchen
   positive native Zuordnung und vollständige Abdeckung. Fehlender Marker,
   Altprogramm, blockierter Zugriff, andere/unklare Installation oder unbekannte
   Prozesslage bleibt **Denied/Unknown**. Registry-/Dateiabwesenheit und reine
   Prozesslisten sind kein allgemeiner Abwesenheitsbeweis.
3. Lease, geschützte Vorfahren, Prozessnachweise und der geprüfte Plan bleiben
   innerhalb der nativen Sitzung bis nach der letzten Mutation erhalten. Alte
   Image-Proofs bleiben bis zum kontrollierten nativen Übergang an den Mutator
   gehalten. Ihr aktueller Read-only-Handle ohne Write/Delete-Sharing würde die
   eigene EXE-Ersetzung sperren. Es fehlt daher auch eine besitzgebundene
   Übergabeschnittstelle mit erneuter Identitätsprüfung am Mutationshandle.
   Bloßes Proof-Drop und anschließendes ungeprüftes Pfad-Reopen reicht nicht.
   Keine Umwandlung in einen dauerhaften NSIS-Boolean/String oder eine
   gespeicherte Ausführungsfreigabe. Lange GUI-Pausen/Phasenwechsel benötigen
   aktuelle Policy-/Uhr-/Prozessnachweise. Zwischen Beobachtung und OS-Image-Mapping
   vor App-main wird keine globale Atomizität behauptet. In-use-/Dateischreibfehler
   müssen Änderungen abbrechen, ohne Force/Ignore als Erfolg auszulegen.

Die bestehende Drei-Export-ABI stellt diesen Eingang **noch nicht bereit**.
Dieser Slice erweitert sie nicht. Der folgende native Vorflug muss auch die
verbleibenden WebView-Installer-/Registry-Updatepfade, sichere Ziel-/Cleanuppläne
und harte Schreibfehlerbehandlung separat positiv absichern, bevor das Gate
jemals Erfolg zurückgeben darf. Kein alter Uninstaller wird dabei legitimiert.

## Herkunft und Prüfung

`upstream.json` pinnt Tauri-CLI2.12.3 und dessen Original-SHA256. Die unveränderte
Vorlage liegt nur als **nicht konfigurierte Textreferenz** in
`upstream-installer.nsi.txt`. Die aktive Vorlage ist `installer.nsi`. Keine breiten
Ignorierbereiche verstecken die Integrationsänderungen. NSIS3.13 und die
MUI2-Finishvariablen/-Callbacks sind die Compilerbasis. Der eigene Guard-DLL-Pfad
ist fest `installer/guard/fbforge_suite_nsis_guard.dll`. Fehlendes Artefakt führt
zum Compilerfehler. Vor Distribution sind dessen native Provenienz, Signierung
und Paketbindung noch erforderlich. Die Compilerfixtures verwenden ausschließlich
inerten Anwendungsinhalt und eine separat gebaute Entwicklungs-DLL.

Tests prüfen den frühen Abbruch, aktive Aufrufpfade, Finalisierungsreihenfolge und
exakte Scanner-/Identitätserhaltung. Compilerprüfung beweist Syntax, Include- und
Exportauflösung, keine Installation. Native Windows-UI-/Silent-/Uninstall- und
Appstart-Konkurrenztests, DACL-/Sitzungstests, Tempbereinigung und vollständige
Vorflugabnahme bleiben offen. Java-/Profil-/Verschlüsselungsformate sind unverändert.

Quellen: [Tauri-Vorlage](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.3/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi),
[Tauri-Makros](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.3/crates/tauri-bundler/src/bundle/windows/nsis/utils.nsh),
[NSIS3.13 MUI-Finish](https://github.com/NSIS-Dev/nsis/blob/v313/Contrib/Modern%20UI%202/Pages/Finish.nsh).
Copyright Tauri Apps Contributors, MIT/Apache-2.0. Zugehörige Lizenztexte liegen
unter `licenses/tauri-MIT.txt` und `licenses/Apache-2.0.txt`.
