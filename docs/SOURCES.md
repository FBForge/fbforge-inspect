# Öffentliche Implementierungsquellen

Vertragsstand geprüft am 28.10.2026. Die App verwendet keine nicht öffentlichen Implementierungen. Versionen der Schnittstellen und einzelner Implementierungen sind unterschiedliche Angaben.

| Thema | Primärquelle | Verwendung |
| --- | --- | --- |
| Submission v3 | https://schema.fitko.de/fit-connect/submission-api/3.0.0/submission-api.yaml | Öffentliche Destination, Prozesssuche, Limits und Schlüssel |
| Submission v2 | https://schema.fitko.de/fit-connect/submission-api/2.0.0/submission-api.yaml | Öffentliche Destination und Schlüssel |
| Routing-Vertrag 2.1 | https://schema.fitko.de/fit-connect/routing-api/2.1.0/routing-api.yaml | Routen, Gebiete und Paginierung |
| Routing und Signaturen | https://docs.fitko.de/fit-connect/docs/sending/get-destination/ | Laufzeitpfad /v2, feste JWKS-Quellen, PS512 und signierte Zuständigkeiten |
| Betriebsumgebungen | https://docs.fitko.de/fit-connect/docs/getting-started/environments/ | TEST/STAGE/PROD, fehlende STAGE-Routing-API, PKI-Profile |
| Kryptographische Vorgaben | https://docs.fitko.de/fit-connect/docs/details/crypto/ | JWK, RSA ab 3072 Bit, key_ops, x5c und Zertifikatsprofil |
| v3-Funktionen | https://docs.fitko.de/fit-connect/docs/major-release/fit-connect_3/ | Öffentliche Lesefunktionen und Versionsabgrenzung |
| PVOG-Suchdienst | https://pvog.fitko.net/suchdienst/api/v3/api-docs | Nicht veralteter GET /v1/relations/jzufi-2-3, Pflichtparameter und RelationPage23 |
| PVOG-Dokumentation | https://pvog.fitko.de/docs/suchdienst/doku/ | Öffentlicher Zugriff ohne Authentifizierung |
| PVOG-Umgebungen | https://pvog.fitko.de/docs/umgebungen/umgebungen-ueberblick/ | STAGE und PROD |
| XZuFi 2.3.1 | https://www.xrepository.de/details/urn:xoev-de:fim:standard:xzufi_2.3.1 | Offizielles XML-Schema, Spezifikation und Codelisten; Rollen, Kommunikationssysteme, EfA-Parameter |
| XZuFi 2.2.0 | https://www.xrepository.de/details/urn:xoev-de:fim:standard:xzufi_2.2.0 | Offizielles Vergleichspaket; keine automatische Konvertierung |
| OCSP | https://www.rfc-editor.org/rfc/rfc6960 | Signatur, autorisierter Antwortsender, CertID und Zeiten |
| X.509 / CRL | https://www.rfc-editor.org/rfc/rfc5280 | Zertifizierungspfade und direkte vollständige CRLs |
| JWK | https://www.rfc-editor.org/rfc/rfc7517 | Öffentliche Schlüsselrepräsentation |
| Tauri 2 | https://v2.tauri.app/ | Ressourcen, Kommandos und Sicherheitsmodell |

Die Serversammlung des Routing-OpenAPI-Artefakts enthält noch /v1-Beispieladressen. Für den Laufzeitpfad /v2 folgt Inspect der aktuellen öffentlichen Anleitung zum Ermitteln eines Zustellpunkts. Eine Routing API v3 wird nicht erfunden.

Die 3072-Bit-Vorgabe gilt für Destination-Schlüssel. Die öffentliche Anleitung fordert für Adressierungssignaturen einen 4096-Bit-Schlüssel, diese Regel wird separat angewendet. Die App verwendet feste umgebungsbezogene JWKS-URLs und eine enge Liste bekannter Aussteller. Tokenfelder wie jku steuern keinen Netzwerkabruf.

Am 30.10.2026 wurden die offiziellen XZuFi-Pakete 2.2.0/2.3.1 und die PVOG-OpenAPI erneut tatsächlich abgerufen. [Feldzuordnung und Beweisgrenzen](PVOG-MAPPING.md) dokumentieren die vollständige Quellenerhaltung, Codelistenbindung und Parameterzuordnung. [Artefaktprüfsummen](source-artifacts.json) binden die lokale Paketprüfung an diese Quellen. Das Vertragsinventar ersetzt keine vollständige XSD- oder Instanzvalidierung.

Die gemeinsamen öffentlichen Theme-Tokens, Desktopmaße und das Darstellungsmenü folgen [FBForge Connect 0.3.0](https://github.com/FBForge/fbforge-connect/tree/fa80c1cfdccb07709de0525fb59b2f19a9c4298d/frontend/src) unter EUPL-1.2. Inspect verwendet die Original-Theme-Tokens einschließlich der Hoverfarben.
