# Änderungen

## 0.1.0

- Connect-0.3-Darstellung mit identischen Schrift-, Farb- und Hover-Tokens, gemeinsamer Kopfzeile und Darstellungsmenü. Zusätzliche Aktionspfeile entfernt.
- Eingaben, Fehler und abgeschlossene Diagnosen bleiben je Bereich, Umgebung und API-Version während der Sitzung erhalten.
- Leistungs-/Schemalisten sind kompakt, suchbar und aufklappbar. Öffentliche JWK-Felder, x5c und PEM sind direkt einsehbar.
- Die Verwendungsdiagnose bewertet die JWK-Felder key_ops und use zusammen mit alg. X.509-Key-Usage wird nicht mehr als JWK-Anforderung bewertet.
- Info nennt bei CI-Builds den genauen Quellcommit.

Erste Desktop-Version von FBForge Inspect zum Prüfen öffentlicher FIT-Connect-Zustellpunkte, Routing-Ergebnisse und PVOG-Adressierungsdaten.

- Destination v2/v3 und expliziter Versionsvergleich.
- Routing mit Gebietssuche, Kontexten und standardmäßig aktivem PVOG-Abgleich.
- Vollständige PVOG-Quellansichten, mehrere Zuständigkeitsrollen und zugeordnete
  Onlinedienstparameter mit Sprachangaben. Leistungs- und Antwort-IDs werden verglichen.
- Öffentliche v3-Prozesssuche und paarbezogene Limits. Die Prozesssuche bleibt eingeschränkt, wenn die Schnittstelle eine Anmeldung verlangt. Der unabhängige Limits-Abruf ist verfügbar.
- Öffentliche JWK/JWKS, Zertifikate, PKIX, OCSP/CRL und Adressierungssignaturen. Geprüfter V-PKI-Truststore, standardmäßig aktive Online-Sperrprüfung und erlaubte TEST-Zertifikatsausnahmen als Hinweise.
- Manueller HTTP(S)-Proxy mit optionaler Basic-Anmeldung und Netzwerkprüfung der benötigten FIT-Connect-Endpunkte.
- Fehlende oder mehrdeutige Signaturschlüssel bleiben „nicht nachgewiesen“;
  daraus wird keine ungültige Signatur abgeleitet.
- JSON-Schema-/XSD-Strukturansicht mit sicheren Referenzgrenzen.
- Offizielle XZuFi-Paketprüfung. XSD-Schemaversion und Zielnamensraum getrennt,
  Dokumentationsannotationen außerhalb des Strukturknotenbudgets.
- XSD-Dateien mit UTF-8-BOM und eine exakte Grenze von 4.000 Strukturknoten.
- Strikte JSON-Syntax beim Schemaeinlesen. Kommentare, einfache Anführungszeichen
  und unzitierte Werte werden als ungültiges JSON zurückgewiesen.
- Vergrößerte Texte und schmale Arbeitsflächen mit umbrochener Werkzeugleiste,
  erreichbarer Navigation, Tastaturzugang zu Tabellen/Quelltexten und Fokusführung
  in der Exportvorschau. Lesbarer Marken- und Platzhaltertext in Hell/Dunkel.
- Lokale JSON-/HTML-Berichte mit Vorschau und optionalen Rohantworten.
- Connect-Familienstil, neues Inspect-Logo, helle/dunkle Oberfläche und bebilderte Website-Anleitung.

Zielplattform: Windows x64. Java wird mitgeliefert. Die Veröffentlichung verwendet den signierten und unabhängig geprüften Windows-Installer.

### UX-Korrekturen nach dem Flächendurchgang

- Bereichsbezogene Scrollpositionen und navigierbare Schema-Arbeitsstände erhalten Eingaben und Ergebnisse auch beim Verfolgen von Referenzen.
- Kurzes Ergebnisfazit, priorisierte offene Befunde und kompakte bestätigte Einzelprüfungen. Struktur- und Schemasuche grenzen große Listen gezielt ein.
- Kompakte alternative Eingaben, vorab erklärte PVOG-ARS-Voraussetzung und getrennte Prozess-/Limits-Aufgaben mit lesbarer Limits-Tabelle.
- Formatierte, skriptfreie HTML-Vorschau; breite Zertifikats-/PVOG-Detailflächen, veröffentlichte Onlinedienstnamen und konkretere Syntax-/HTTP-Meldungen.
- Aufgabenorientierte Hilfe mit erhaltenen Grundlagen und fest hinterlegten Quellenlinks für den Systembrowser.
