# Serverdiagnose mit FBForge Inspect 0.2.0

Das portable CLI führt denselben öffentlichen Java-Diagnosekern wie die Desktop-App aus. Es benötigt Windows oder Linux und Java 25 oder neuer. Eine grafische Oberfläche ist auf dem Zielsystem nicht erforderlich. Das eigenständige CLI-Archiv enthält keine Java-Laufzeit. Eine passende Laufzeit kann separat entpackt und direkt aufgerufen werden, ohne die Laufzeit der Fachanwendung zu ersetzen.

## Ablauf

1. In Inspect unter **Sammelprüfung** die Einträge vorbereiten und den Auftrag speichern.
2. Unter **Serverdiagnose** das portable CLI exportieren oder das Java-CLI-Archiv des Releases herunterladen.
3. Auftrag und CLI auf das tatsächliche Anwendungssystem übertragen.
4. Den Auftrag dort unter dem passenden Benutzerkonto und mit dem passenden Netzwerkweg ausführen.
5. Die Ergebnisdatei zurückübertragen und unter **Serverdiagnose → Ergebnis auswerten → Serverergebnis öffnen** importieren.
6. Die Zusammenfassung lesen. Bestätigte Einzelprüfungen und **Beleg und Feldpfad** über die jeweiligen Buttons öffnen. Offene Befunde bleiben direkt sichtbar. **Aufrufe und Rückgabecodes** zeigt die Startbefehle für den Serverlauf.

Inspect zeigt Betriebssystem, Architektur, Java-Version, Exitcode und die Einzelbefunde. Teilnehmerzertifikate aus diesem Ergebnis stehen auch im Zertifikatsinventar zur Verfügung. Der Import startet keine Verbindung und führt keine Programme aus.

## Aufrufe

Windows PowerShell:

```powershell
.\inspect.ps1 --job auftrag.json --output ergebnis.inspect.json
$LASTEXITCODE
```

Linux:

```sh
sh ./inspect.sh --job auftrag.json --output ergebnis.inspect.json
echo $?
```

Direkter Aufruf auf beiden Plattformen:

```text
java -Xmx256m -jar inspect-engine.jar --job auftrag.json --output ergebnis.inspect.json
java -jar inspect-engine.jar --help
java -jar inspect-engine.jar --version
```

Für jeden Lauf einen neuen Ergebnisdateinamen wählen. Vorhandene Ergebnisdateien werden nicht überschrieben.

## Netzwerkweg

Proxy und zusätzliche HTTPS-CAs werden ausdrücklich angegeben. Windows-Systemproxy, PAC und Proxy-Umgebungsvariablen werden vom Diagnosekern nicht automatisch übernommen. Ein Browser oder ein anderer Java-Prozess kann andere Einstellungen verwenden.

| Option | Bedeutung |
| --- | --- |
| `--proxy-url` | HTTP(S)-Proxy ohne eingebettete Zugangsdaten |
| `--proxy-user-env` | Name einer Umgebungsvariablen mit dem Proxy-Benutzernamen |
| `--proxy-password-env` | Name einer Umgebungsvariablen mit dem Proxy-Passwort |
| `--ca-file` | Öffentliche zusätzliche HTTPS-CA-Zertifikate als PEM |

Beispiel mit bereits auf dem Zielsystem gesetzten Variablen:

```text
java -jar inspect-engine.jar --job auftrag.json --output ergebnis.inspect.json --proxy-url https://proxy.beispiel.de:8080 --proxy-user-env INSPECT_PROXY_USER --proxy-password-env INSPECT_PROXY_PASSWORD --ca-file unternehmens-ca.pem
```

Die Optionen nennen Variablen, keine Passwortwerte. Zugangsdaten gehören weder in Aufträge noch in Berichte oder Shell-Historien. Die zusätzliche CA ergänzt nur das HTTPS-Vertrauen des jeweiligen Laufs. Hostnamen- und Zertifikatsprüfung bleiben aktiv. Der unabhängige V-PKI-Truststore wird dadurch nicht verändert.

## Exitcodes

| Code | Bedeutung |
| --- | --- |
| `0` | Abgeschlossen ohne Fehlerbefund oder ungeklärten Punkt. Hinweise können vorhanden sein. |
| `2` | Mindestens ein Fehlerbefund und kein ungeklärter Prüfpunkt |
| `3` | Mindestens ein Eintrag fehlgeschlagen, unvollständig oder nicht nachgewiesen |
| `64` | Ungültiger Auftrag oder ungültige Optionen |
| `70` | Interner Fehler |
| `74` | Datei nicht lesbar, Größenlimit, Schreibfehler oder Ergebnisdatei bereits vorhanden |

Ein ungeklärter Punkt hat bei der Zusammenfassung Vorrang vor einem Fehlerbefund. Die Einzelbefunde bleiben erhalten. Exitcode 0 ist keine Gesamtfreigabe eines Zustellpunkts und keine authentifizierte Zustellprobe.

## Austauschformat 1

Aufträge sind UTF-8-JSON mit `format: "fbforge-inspect-job"` und `formatVersion: 1`.

```json
{
  "format": "fbforge-inspect-job",
  "formatVersion": 1,
  "environment": "TEST",
  "apiVersion": "v3",
  "checks": [
    { "id": "netzwerk", "operation": "network", "input": {} },
    {
      "id": "zustellpunkt",
      "operation": "destination",
      "input": { "destinationId": "00000000-0000-4000-8000-000000000001" }
    }
  ]
}
```

Die UUID ist fiktiv. Öffentliche Funktionen sind `network`, `destination`, `routing`, `reverse`, `areas`, `process`, `limits`, `keys`, `certificate`, `signature`, `rollover` und die bestehende Schema-Strukturansicht `schema`.

Prüfkennungen sind eindeutig und bestehen aus 1 bis 64 Buchstaben, Ziffern, Bindestrichen oder Unterstrichen. Ein Auftrag enthält 1 bis 32 Einträge. Umgebung und API-Version gelten für alle Einträge. Leistungs-/Gebietspaare werden nicht frei miteinander kombiniert.

Optional enthält `network` die Felder `mode`, `proxyUrl` und `httpsCaPem`. Proxy-Anmeldedaten sind darin nicht zulässig. CLI-Optionen können den öffentlichen Netzwerkweg ausdrücklich überschreiben. Beim Desktopimport werden abweichende Netzwerkeinstellungen vor einer gesonderten Übernahme angezeigt. Dabei werden bisherige Proxy-Anmeldedaten entfernt.

Ergebnisse verwenden `format: "fbforge-inspect-result"` und `formatVersion: 1`.

| Feld | Inhalt |
| --- | --- |
| `appVersion` | Version des ausführenden Diagnosekerns |
| `createdAt` | Prüfzeit in UTC |
| `jobSha256` | SHA-256 der ursprünglichen Auftragsbytes beim CLI-Lauf |
| `runtime` | Betriebssystem, Architektur und Java-Version |
| `checks` | Eintrag mit derselben Kennung und `report` oder festem `error`-Code |
| `exitCode` | Zusammengefasster Prüfstatus |

Der Bericht folgt dem bestehenden Vertrag mit Befunden, öffentlichen Daten und Abfragebelegen. Rohantworten werden nicht aufgenommen. Feste Laufcodes sind `CHECK_FAILED`, `CHECK_TIME_LIMIT`, `JOB_TIME_LIMIT` und `RESULT_SIZE_LIMIT`. Desktop-Sammelläufe kennzeichnen noch nicht ausgeführte Einträge mit `NOT_EXECUTED`.

Ein SHA-256-Fingerabdruck erlaubt den Abgleich mit einer bekannten Originaldatei. Er beglaubigt weder Absender noch Echtheit des Ergebnisses. Inspect bewertet die Datei als übermittelten Beleg.

## Grenzen

- Höchstens 2 MiB Auftrag und 16 MiB Ergebnis.
- Höchstens 32 Einträge, nacheinander mit getrennten Netzwerkbudgets.
- Zehn Minuten Gesamtzeit und 130 Sekunden pro CLI-Einzelprüfung.
- Die bestehenden Grenzen für öffentliche Ziele, Standardports, Umleitungen, Antwortgrößen, Abfragezahl und Zertifikatsketten gelten auch im CLI.
- Rechnername, Benutzername und private Schlüsselbestände gehören nicht zum Laufzeitkontext.

Öffentliche Kennungen, Organisationsnamen und ausdrücklich gewählte Proxyziele können vorkommen. Vor Weitergabe den Inhalt prüfen. Authentifizierung, Einreichungen, Empfang und Fachdatenprüfung erfolgen in Connect oder der angebundenen Anwendung.

Der Desktopexport unter Serverdiagnose nimmt einen vorbereiteten öffentlichen Auftrag als `auftrag.json` in den neuen CLI-Ordner auf. Zugangsdaten bleiben ausgeschlossen. Beim Import ordnet der SHA-256-Fingerabdruck das Ergebnis den genauen Auftragsbytes zu. Er ist kein Nachweis der Serveridentität. Vor der Ausführung `java -version` prüfen. Java 25 oder neuer kann separat portabel entpackt werden.
