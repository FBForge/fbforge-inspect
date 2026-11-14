# Nativer Installer-Guard

Der Tauri-`beforeBundleCommand` baut und stellt `fbforge_suite_nsis_guard.dll`
(Unicode-NSIS, Windows x86) automatisch bereit. Voraussetzung sind ein installiertes
Rust **1.93.1**, dessen Target **i686-pc-windows-msvc** und die Visual-Studio-C++-
Buildwerkzeuge einschließlich x86-Unterstützung. Einmalige Einrichtung durch den
Buildbetreiber: `rustup toolchain install 1.93.1 --profile minimal --target i686-pc-windows-msvc`.
Die Windows-Bundle-/Signierjobs richten dies explizit ein; der Buildhelper
installiert keine Toolchain. Er setzt nur für diesen DLL-Build das Rust-Flag
`-C target-feature=+crt-static` zur statischen CRT-Anbindung. Manueller Build: `node scripts/installer-guard.mjs`.

`../guard-source/` ist ein mechanischer öffentlicher FBForge-Suite-Snapshot von
Commit `f4b6cf322feeca31e4a908482486d03ef7d70138` einschließlich EUPL-1.2-Lizenz.
`source-manifest.json` benennt die exakten Quelldateien und SHA-256-Werte.
Die separate Workspace verändert keine Laufzeitabhängigkeiten der Anwendung.
Buildausgaben liegen unter `desktop/target/installer-guard/`, die bereitgestellte
DLL hier; beide sind generierte, nicht einzucheckende Artefakte.

Der geschützte Signierlauf baut zuerst diesen Guard und verwendet anschließend
denselben vorhandenen Authenticode-Signer wie für die Anwendung. Nach erfolgreicher
Signaturprüfung ersetzt seine temporäre Tauri-Konfiguration den Build durch
`installer-guard.mjs verify <sha256>`. Dieser spätere Hook prüft ausschließlich
die signierten Bytes und baut sie niemals neu. Die vorhandene Reihenfolge der
Anwendungs-, Uninstaller- und Setup-Signierung bleibt erhalten.

Ein gewöhnlicher Entwicklerbuild enthält eine **unsignierte** DLL und einen
unsignierten Installer. Er ist kein vertrauenswürdiger Releaseinstaller; die
native V2-Mutation muss ihn ablehnen. Positive Installations-/NVDA-Abnahmen
gehören zum geschützten signierten Kandidaten. Build-/Fixturetests sind keine
Windows-Installations- oder Authenticode-Laufzeitabnahme.

Offene Windows-Abnahme: signiertes Update von einer tatsächlich signierten
Vorgängerversion einschließlich Profilerhalt. Ein aktueller unsignierter
CI-Baseline darf dafür nicht als erfolgreich installierte Vorgängerversion gelten.
Die bestehende Installations-/Uninstall-/NVDA-Abnahme läuft im geschützten
Signierworkflow; sie wird durch diese Änderung weder ausgeführt noch als bestanden
behauptet. Der Uninstall-Mutationsvertrag bleibt ebenfalls separat abzunehmen.
