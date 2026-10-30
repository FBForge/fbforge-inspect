# Änderungen

## 0.1.0 – noch nicht veröffentlicht

- Connect-0.3-Darstellung mit identischen Schrift-, Farb- und Hover-Tokens, gemeinsamer Kopfzeile und Darstellungsmenü. Zusätzliche Aktionspfeile entfernt.
- Eingaben, Fehler und abgeschlossene Diagnosen bleiben je Bereich, Umgebung und API-Version während der Sitzung erhalten.
- Leistungs-/Schemalisten sind kompakt, suchbar und aufklappbar. Öffentliche JWK-Felder, x5c, PEM und Zertifikatsverwendungen sind direkt einsehbar.
- Fehlende X.509-Key-Usage bei TEST-Zertifikaten wird getrennt von einer ausdrücklich unpassenden Zertifikatsverwendung bewertet. JWK key_ops bleibt eine eigene Prüfung.
- Info nennt bei CI-Builds den genauen Quellcommit.

Erste Desktop-Version von FBForge Inspect zum Prüfen öffentlicher FIT-Connect-Zustellpunkte, Routing-Ergebnisse und PVOG-Adressierungsdaten.

- Destination v2/v3 und expliziter Versionsvergleich.
- Routing mit Gebietssuche, Kontexten und optionaler PVOG-Evidenz.
- Vollständige PVOG-Quellansichten, mehrere Zuständigkeitsrollen und zugeordnete
  Onlinedienstparameter mit Sprachangaben; Leistungs- und Antwort-IDs werden verglichen.
- Öffentliche v3-Prozesssuche und paarbezogene Limits.
- Öffentliche JWK/JWKS, Zertifikate, PKIX, OCSP/CRL und Adressierungssignaturen.
- Fehlende oder mehrdeutige Signaturschlüssel bleiben „nicht nachgewiesen“;
  daraus wird keine ungültige Signatur abgeleitet.
- JSON-Schema-/XSD-Strukturansicht mit sicheren Referenzgrenzen.
- Offizielle XZuFi-Paketprüfung; XSD-Schemaversion und Zielnamensraum getrennt,
  Dokumentationsannotationen außerhalb des Strukturknotenbudgets.
- XSD-Dateien mit UTF-8-BOM und eine exakte Grenze von 4.000 Strukturknoten.
- Strikte JSON-Syntax beim Schemaeinlesen. Kommentare, einfache Anführungszeichen
  und unzitierte Werte werden als ungültiges JSON zurückgewiesen.
- Vergrößerte Texte und schmale Arbeitsflächen mit umbrochener Werkzeugleiste,
  erreichbarer Navigation, Tastaturzugang zu Tabellen/Quelltexten und Fokusführung
  in der Exportvorschau. Lesbarer Marken- und Platzhaltertext in Hell/Dunkel.
- Lokale JSON-/HTML-Berichte mit Vorschau und optionalen Rohantworten.
- Connect-Familienstil, neues Inspect-Logo, helle/dunkle Oberfläche und Offlinehilfe.

Zielplattform: Windows x64. Signierung und Release-Veröffentlichung erfolgen erst nach gesonderter Abnahme.
