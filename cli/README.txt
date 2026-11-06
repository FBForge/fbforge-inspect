FBForge Inspect CLI 0.2.0

Voraussetzung: Java 25 oder neuer, Windows oder Linux, keine grafische Oberfläche erforderlich.

Windows PowerShell:
.\inspect.ps1 --job auftrag.json --output ergebnis.inspect.json

Linux:
sh ./inspect.sh --job auftrag.json --output ergebnis.inspect.json

Direkt auf beiden Plattformen:
java -jar inspect-engine.jar --job auftrag.json --output ergebnis.inspect.json

Optionen: java -jar inspect-engine.jar --help

Ergebnisdateien werden nicht überschrieben. Für jeden Lauf einen neuen Dateinamen wählen.
Die Ergebnisdatei in Inspect unter Serverdiagnose öffnen. Die Datei ist kein Herkunftsnachweis.
Öffentliche Quellen und Konfiguration werden geprüft. Keine authentifizierte Zustellprobe.
Auf dem tatsächlichen System mit dem passenden Benutzerkonto und Netzwerkweg ausführen.
Proxy und Unternehmens-CA ausdrücklich konfigurieren. Systemproxy wird nicht automatisch übernommen.
Keine Zugangsdaten oder privaten Schlüssel in Aufträge aufnehmen.

Exitcodes: 0 abgeschlossen ohne Fehler/ungeklärte Punkte, 2 Fehlerbefund, 3 unvollständig,
64 ungültige Eingabe, 70 interner Fehler, 74 Dateifehler oder Größenlimit.

https://fbforge.de/inspect/hilfe/0.2/
https://github.com/FBForge/fbforge-inspect
EUPL-1.2. Lizenz und Drittanbieterhinweise siehe Releasepaket.
