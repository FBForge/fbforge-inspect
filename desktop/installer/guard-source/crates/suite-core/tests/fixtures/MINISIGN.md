# Öffentliche Metadaten-Testvektoren

`minisign-v2.json` enthält ausschließlich einen öffentlichen Dummy-Schlüssel und
elf öffentliche Signaturhüllen. Ein neuer zufälliger Testschlüssel wurde nur in
einer netzlosen Bubblewrap-Sandbox erzeugt. Der private Dummy-Schlüssel wurde
mit deren Ende verworfen und ist kein Bestandteil des Repositorys.

Generator: vorhandene Tauri CLI 2.12.3, offline mit `npm ci --ignore-scripts` aus
dem bestehenden Cache geladen. `tauri signer generate --ci --password ''
--write-keys /scratch/dummy.key` und danach `tauri signer sign
--private-key-path /scratch/dummy.key --password '' --app-version 0.1.0` signierten
jeweils die exakten Bytes `FBForge Suite metadata v2\0` plus `signed`-Text.
`wrongDomain` wurde absichtlich mit dem v1-Präfix signiert.

Die privaten Dummy-Dateien und nativen CLI-Dateien blieben im wegwerfbaren
Sandboxbereich. Übernommen wurden nur der öffentliche Schlüssel und die genau
benannten JSON-/Signaturwerte. Es wurden keine Produktionsschlüssel verwendet.
Die Signaturen sind Tauri-Base64-Minisignboxen mit Prehash und authentifiziertem
Trusted Comment. Die signierten Inhalte sind Testdaten und keine Releasefreigabe.
