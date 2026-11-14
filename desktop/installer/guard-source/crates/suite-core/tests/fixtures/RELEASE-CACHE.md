# Öffentliche Release-Cache-Testvektoren

`release-cache-v2.json` enthält ausschließlich einen öffentlichen Dummy-Schlüssel
und vier tatsächlich mit Tauri CLI 2.12.3 signierte v2-Hüllen. Der vorhandene
Offline-npm-Cache wurde ohne Lifecycle-Skripte in einer netzlosen Bubblewrap-
Sitzung verwendet. Es wurden keine Abhängigkeiten ergänzt oder heruntergeladen.

Für jede Hülle wurden genau die Bytes `UTF-8("FBForge Suite metadata v2")`, ein
NUL-Byte und danach der UTF-8-Text aus `signed` ohne weiteren Zeilenumbruch an
`tauri signer sign` übergeben. Die Ausgabe ist eine Tauri-Base64-Minisignbox mit
`ED`-Prehashsignatur, authentifiziertem Trusted Comment und globaler Signatur.

Der zufällige private Dummy-Schlüssel und alle Zwischendateien blieben nur im
Scratchbereich und wurden mit der Sandbox verworfen. Übernommen wurden der
öffentliche Schlüssel und die benannten Hüllen: Katalog, Available-Folge 1,
Available-Folge 2 mit neuer Version und Withdrawn-Folge 3. Es wurden keine
Produktionsschlüssel oder Releasefreigaben verwendet.
