# Änderungen

## 0.2.0

- Lokale automatische Strukturzuordnung für XSD, JSON Schema und XDF mit optionalen Excel-Anforderungen. Eindeutige Treffer, gezielte Entscheidungen bei Unklarheiten und nachvollziehbare Fundstellen.
- Zuordnungsbericht und MapForce-2025-Entwurf für geklärte direkte XSD-Zuordnungen. Offene Transformationsregeln bleiben ausdrücklich dokumentiert.

- Navigation mit ausreichender Breite, ganzen Wörtern und kompakteren Zeilen.
- Gemeinsame Diagnosezusammenfassung für Einzel-, Sammel- und Serverprüfungen. Bestätigte Punkte und technische Belege werden gezielt über Buttons geöffnet.
- Sammelprüfung mit Filterbuttons und zusammengehörigen Aktionen pro Eintrag. Serverdiagnose mit drei geführten Schritten und einheitlicher Dateiauswahl.

- Einzelne Checks samt zugehörigem Ergebnis aus Sammelprüfungen entfernen. Übrige Einträge bleiben erhalten und der Tastaturfokus bleibt im Ablauf.
- Laufende Werkzeugprüfungen zeigen Fortschritt, Abbrechen und Fehlermeldungen. Nach dem Abbruch ist die Rückkehr zur Startseite wieder möglich.

- Geführte Prüfaufträge, sichtbare Sammelergebnisse mit Handlungsbedarf und Filter. Bereits abgeschlossene Ergebnisse bleiben beim Ergänzen erhalten.
- CLI und vorbereiteter öffentlicher Auftrag werden gemeinsam exportiert. Serverergebnisse lassen sich anhand der exakten Auftragsbytes zuordnen.
- Fehlgeschlagene Verbindungsphasen erklären nachfolgende ungeprüfte Schritte und führen bei lokalen Prüfungen direkt zu den Netzwerkeinstellungen.

- Portables Java-CLI für Windows und Linux mit vorbereitbaren Prüfaufträgen, versionierten Ergebnisdateien und stabilen Exitcodes.
- Serverergebnisse lokal öffnen, Transportphasen auswerten und öffentliche Zertifikate ins Inventar übernehmen.
- Sammelprüfung mit begrenzter Warteschlange, Pause, Abbruch und gezielter Wiederholung. Rohantworten bleiben außerhalb der Sammelergebnisse.
- Netzwerkphasen für Ziel-DNS, Proxy-DNS, TCP, Proxy-TLS, CONNECT, Ziel-TLS und HTTP. Konkrete Zertifikatsursachen und unerwartete HTML-Antworten werden erklärt.
- Öffentliche Unternehmens-CAs für HTTPS ausdrücklich laden, getrennt vom V-PKI-Vertrauen. Hostnamenprüfung bleibt aktiv.
- Rückwärtsrouting vom bekannten Zustellpunkt mit ausgewählten Leistungs-/Gebietspaaren und ausdrücklicher Vollständigkeit.
- Öffentliche Zertifikatsdateien und Adressierungssignaturen separat prüfen. Keine Speicherung fremder Zugangstoken als Signaturbefund.
- Öffentlichen Schlüsselwechsel anhand aktiver und neuer Kennung, tatsächlichem Schlüsselinhalt und Zertifikatsüberlappung vorbereiten.
- Zertifikatsinventar mit betroffenen untersuchten Zustellpunkten, Umgebungen, API-Versionen und Kalenderexport.
- Konkrete Adressierungszusammenhänge aus Zuständigkeit, Signatur und Routingantwort. Gleiche Destination-IDs verschmelzen keine Behördenkontexte.
- Intelligente Übernahme öffentlicher Kennungen und URLs mit Vorschau und erhaltenem Kontext.
- Supporttexte mit Vorschau und durchsuchbare Offline-Hilfe.
- Integrierter Windows-Updater mit nativer Bestätigung und Signaturbindung an Inspect-Produktdateiname und Version.
- Unabhängige Updateprüfung, CLI-Prüfung auf Windows und Linux, Installationsupgrade von 0.1.0 und erweiterte NVDA-Abnahme.

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
