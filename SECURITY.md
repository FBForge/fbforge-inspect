# Sicherheit

Inspect verarbeitet ausschließlich öffentliche Daten. Bitte keine Zugangsdaten, privaten Schlüssel oder Fachdaten in Diagnosefelder eingeben. Private und symmetrische Schlüssel werden abgewiesen. Die App speichert Eingaben und Ergebnisse nicht automatisch und sendet keine Telemetrie.

Relevante Sicherheitsgrenzen sind UI → native Kommandos, Host → Java-IPC, öffentliche URL → Netzwerk, untrusted JSON/XML/JOSE/X.509 → Parser und Bericht → lokales HTML. Die Darstellung behandelt externe Texte als Daten. HTML-Exporte maskieren Markup und blockieren Skripte und externe Ressourcen per CSP.

Der Transport erlaubt nur öffentliche IP-Ziele und Standardports. HTTPS ist der Standard, HTTP und OCSP-POST sind auf ausdrücklich gestartete PKI-Prüfungen beschränkt. DNS-geprüfte IP-Adressen werden beim Verbindungsaufbau festgehalten. Keine automatische Fremdreferenzauflösung, keine DTD/XXE, keine automatische Auswahl eines Vertrauensankers aus empfangenen Daten. Kryptographische Einzelergebnisse ersetzen keine Aussage über die Gesamtintegration.

Fehlende Sperrnachweise bleiben unbekannt. OCSP-Signatur, autorisierter Responder, Zertifikatszuordnung und aktuelle Zeitangaben sind erforderlich. Bei delegierten Signierern muss zusätzlich eine gültige CA-signierte nocheck-Ausnahme oder ein aktueller direkter vollständiger CRL-Nachweis vorliegen. CRLs müssen direkt, vollständig, passend signiert und aktuell sein. Delta- und indirekte CRLs werden nicht als Good-Nachweis akzeptiert.

Melde vermutete Schwachstellen privat über den Sicherheitskontakt im GitHub-Projekt oder an kontakt@fbforge.de. Keine sensiblen Eingaben in öffentliche Issues stellen. Füge Version, Betriebssystem, einen synthetischen Reproduktionsfall und die erwartete Sicherheitsgrenze bei. Es gibt noch keine zugesagte Supportfrist oder Sicherheitszertifizierung.
