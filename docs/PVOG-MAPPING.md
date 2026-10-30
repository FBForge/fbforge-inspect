# PVOG- und XZuFi-Feldzuordnung

Geprüft am 30.10.2026 gegen die tatsächlich heruntergeladenen offiziellen
XRepository-Pakete XZuFi 2.2.0 und 2.3.1 und die öffentliche PVOG-OpenAPI.
URLs, Dateigrößen und SHA-256 stehen in [source-artifacts.json](source-artifacts.json).
Der Laufzeitabruf verwendet ausdrücklich JZuFi 2.3.1; 2.2.0 ist Vergleichsquelle,
kein stiller Fallback. Die OpenAPI enthält ein älteres Rollen-Beispiel als Objekt;
das 2.3.1-XSD und die beobachtete Live-Antwort liefern eine Liste.

## Erhaltene Quellen und Kontexte

Die vollständige, maschinenlesbare Deklarationszuordnung liegt in
[pvog-field-mapping.json](pvog-field-mapping.json). Sie enthält die transitiv
referenzierten lokalen XZuFi-Typen ab Leistungsbeschreibung, erweitertem
Onlinedienst und Zuständigkeitstransfer einschließlich der Ableitungen für
Organisationseinheit, Onlinedienst und Person. Je Typ bleiben Quelldatei,
Basistypen und sämtliche Element-/Attributdeklarationen mit Typen und
Kardinalitäten erhalten. Externe Basistypen werden benannt, nicht nachgeladen.
Dies ist ein Vertragsinventar, keine XML-Instanzvalidierung.

Die relevanten offiziellen Codelisten sind mit Version, Originalcode und
Kurzbezeichnung in [pvog-codelists.json](pvog-codelists.json) erfasst.
Die EfA-Liste des geprüften Pakets enthält beim Accessibility-URL-Code ein
unsichtbares U+200B nach `efa.`. Inspect verändert solche gelieferten Codes nicht
stillschweigend und behauptet keine Gleichheit mit einem anders geschriebenen
Code aus einer anderen Quelle.

Die fachlichen JSON-Objekte werden vollständig in `report.data` erhalten. Unbekannte Felder,
mehrsprachige Werte, Reihenfolgen und Wiederholungen werden nicht entfernt.
Die Tabellen verdichten diese Quellen; aufklappbare Quellansichten und die
JSON-/HTML-Berichte enthalten weiterhin sämtliche erhaltenen Felder.
`rawResponses` mit den ursprünglichen Antworttexten bleibt eine gesonderte,
standardmäßig ausgeschlossene Exportoption.

| PVOG-/XZuFi-Quelle | Berichtspfad / Anzeige | Auswertung und Grenze |
| --- | --- | --- |
| RelationPage23 `content[]` | `$.pvogRelations[]` | Jeder Transferkontext bleibt erhalten, auch bei wiederholten Objekt- oder Destination-IDs. |
| `page.number`, `size`, `totalElements`, `totalPages` | `$.pvogPages[].page`, jeweils mit `source` | Typ, Seitennummer, Kapazität, Gesamtumfang und Konsistenz zwischen Seiten werden geprüft. Zehn Seiten maximal. Fehler und Teilbestände bleiben unbekannt. |
| `uebergeordnetesObjektID` | Kontextzeile und vollständige Quellansicht | `value` und sämtliche Scheme-Angaben bleiben erhalten. Identifikatoren verschiedener Stellen/Schemata werden nicht gleichgesetzt. |
| `uebergeordnetesObjektKlasse` | Kontextzeile / Quellansicht | `code`, `name`, `listURI`, `listVersionID`. Nur Code `003` der offiziellen Transfer-Codeliste erlaubt einen Onlinedienstabruf. Namen werden nicht als Typkennung interpretiert. |
| `zustaendigkeit[]` | Eigene Zeile mit `$.pvogRelations[i].zustaendigkeit[j]` | Keine globale Deduplizierung. Alle nachfolgenden Felder gehören genau zu dieser Zuständigkeit. |
| `id`, `leistungID[]`, `gebietID[]` | Zeile und Quellansicht | Werte einschließlich Herausgeber, Schema und Schemaversion; Leistungsbeschreibung und LeiKa bleiben unterschiedliche Kennungen. ARS wird nicht aus AGS oder Gebiets-ID geraten. |
| `idSekundaer[]` | Quellansicht, ggf. `$.addressingSignatures[]` | Nur das dokumentierte `schemeID=urn:de:fitko:fit-connect:xzufi:destination` wird als Signatur gelesen. Prüfung weiterhin PS512/4096 Bit, unabhängige JWKS und Leistung/Gebiet im selben signierten Kontext. Andere IDs bleiben Quelldaten. |
| `kommunikationssystem[]` | Quellansicht der Zuständigkeit | `kanal`, `kennung`, `kennungzusatz`, `kennzeichenOeffentlichAnzeigen`, `gueltigkeit[]`. Kanal `004` bezeichnet FIT-Connect, eine Kennung ist deshalb noch keine verifizierte Signatur. Keine automatischen URL-Aufrufe. |
| `weitereInformation[]` | Quellansicht | Alle lokalisierten Werte einschließlich Sprachcode; keine Interpretation als HTML. |
| `positionDarstellung` | Quellansicht | Übernommene Positionsangabe; Quellreihenfolge wird nicht umsortiert. |
| `herausgeber` | Quellansicht | Vollständiges Herausgeberobjekt mit Identifikatoren, Bezeichnungen, Beschreibung und Standardgebieten. |
| `gueltigkeit[]` | Quellansicht | `beginn`, `ende`, `zusatz[]` bleiben unverändert; Datums-/Zeitzonenangaben werden nicht zu einer automatischen Zustellfreigabe verdichtet. |
| OE-Ableitung `rolle[]` | Alle Rollen in der Kontextzeile | Mehrere Rollen bleiben sichtbar; `code`, `name` und Codelistenmetadaten bleiben erhalten. |
| OE-Ableitung `kriterium[]`, `kennzeichenTerminvereinbarungVerpflichtend` | Quellansicht | Vollständige Kriterien und Terminpflicht; keine automatische Rekonstruktion interner Auswahlregeln. |
| OD-Ableitung `organisationseinheit[]` | Quellansicht | Organisationseinheit-ID und deren eigene `rolle[]` bleiben innerhalb dieser Zuständigkeit. |
| Personen-Ableitung | Quellansicht / Vertragsinventar | Geerbte Zuständigkeitsfelder bleiben erhalten; kein zusätzlicher Personenabruf. |
| GET Leistungsbeschreibung, `referenzLeistungsschluessel[]` | `$.pvogService`, `$.pvogServiceSource` | Explizit angefragte LB-ID; publizierte Schlüssel werden mit dem eingegebenen LeiKa verglichen. Fehlender Schlüssel ist unbekannt, Abweichung ein Fehlerbefund. Alle weiteren Leistungsfelder bleiben Quelldaten. |
| GET Onlinedienst | `$.pvogOnlineServices[]` mit `context`, `id`, `source`, `matchesParent`, `sourceIndex` | PVOG-ID aus `schemeAgencyID + '.OD.' + value`, unabhängig für jede Transferzuordnung dokumentiert; `sourceIndex` verweist auf `$.pvogOnlineServiceSources[]` mit der einmal erhaltenen vollständigen Antwort in `data`. Die Antwort muss dieselbe Stelle und denselben Wert nennen. Höchstens zehn verschiedene Abrufe; Caching entfernt keine Kontexte. |
| Onlinedienst `parameterEfA[]` | Parametertabelle und `$.pvogOnlineServiceSources[s].data.parameterEfA[]` | `parameterEfA.{code,name,listURI,listVersionID}`, optionaler `parameterWert`, alle `languageCode[]`. Keine Normalisierung von Codes oder URLs und keine Behauptung einer EfA-Validierung. |
| Onlinedienst `parameterIndividuell[]` | Parametertabelle und `$.pvogOnlineServiceSources[s].data.parameterIndividuell[]` | `parameterName`, optionaler `parameterWert`, alle `languageCode[]`; fehlender Wert wird von leerem Wert unterschieden. Wiederholungen bleiben erhalten. |
| Weitere Onlinedienstfelder | Vollständige Quellansicht `$.pvogOnlineServiceSources[s].data` | Unter anderem ID, Sekundär-IDs, globale ID, Bezeichnungen, Beschreibung, Status, Links, dynamische URL-Parameter, Kategorien, Typ, Reifegrad, Identifizierung, Vertrauen, Zahlungsweisen, Hilfe, Sprachen, Herausgeber, Logo, Sichtbarkeit, Versionsinformation und Zuständigkeiten. Keine Fremdreferenzen werden ausgeführt. |

Wenn `matchesParent` falsch ist, zeigt die Oberfläche die Antwort als Beleg,
ordnet deren Parameter jedoch nicht dem angefragten Kontext zu.
Konfigurationswerte und Kommunikationskennungen sind weder verifizierte
Adressierung noch Fachdatenschemata. Letztere stammen weiterhin aus
`publicServices[].submissionSchemas` des FIT-Connect-Zustellpunkts.
Fehlt die Signaturkennung `kid` im unabhängig abgerufenen JWKS oder ist sie
mehrdeutig, bleibt die Signatur nicht nachgewiesen. Das beweist weder ihre
Gültigkeit noch ihre Ungültigkeit; es wird kein Ersatzschlüssel geraten.

## Reproduzierbare Paketprüfung

Die ZIP-Dateien werden lokal aus den angegebenen offiziellen URLs bereitgestellt.
Das Prüfhilfsmittel lädt nichts herunter und entpackt nichts in den Arbeitsbaum:

```sh
python3 scripts/verify-xzufi.py \
  --package 2.2.0=/pfad/xzufi-2.2.0.zip \
  --package 2.3.1=/pfad/xzufi-2.3.1.zip \
  --codelists /pfad/xzufi-2.3.1-genericode.zip \
  --engine engine/target/inspect-engine-0.1.0.jar \
  --java "$JAVA_HOME/bin/java"
```

Geprüft werden SHA-256, Deklarationsinventar und jede XSD-Datei über das echte
Java-IPC-Protokoll. Schemaversion und Zielnamensraum müssen passen; Import- und
Include-Referenzen dürfen keinen Netzabruf auslösen. Annotations-/Appinfo-Inhalte
zählen nicht als Strukturknoten. Das Struktur-, Tiefen- und Eingabebudget bleibt
begrenzt. `--write-mapping` regeneriert das Inventar nur ausdrücklich für eine
erneute fachliche Prüfung; es ist kein automatisches Akzeptieren neuer Quellen.

## Grenzen der Abnahme

Die Prüfung belegt öffentliche Verträge, Paketlesbarkeit und konkrete
Momentaufnahmen. Sie belegt keine Vollständigkeit aller Redaktionsdaten,
keine interne Routingentscheidung und keine erfolgreiche Zustellung.
Menschliche Windows-/NVDA-Bedienung, frischer Windows-Benutzer,
WebView2-Erstinstallation und native Skalierung bleiben eigene Abnahmen.
