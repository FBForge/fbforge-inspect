# Signierbarer Windows-Kooperationsmarker

FBForge Inspect bettet vor der Authenticode-Signierung einen festen Marker in die
Windows-Programmressourcen ein. Der Cargo-`TARGET` entscheidet darüber; ein
Linux-Host erzeugt ihn auch beim Windows-Crossbuild. Andere Zielplattformen
schreiben keine Markerdatei. Die bestehenden Buildprüfungen, Tauri-Berechtigungen,
Ressourcen und die Koordinations-/Updaterreihenfolge bleiben erhalten.

Der std-only Buildhelper schreibt `OUT_DIR/fbforge-cooperation.bin`. Er hängt über
`tauri-build` 2.7.3 `WindowsAttributes::append_rc_content` folgende Resource an:

```rc
LANGUAGE 0, 0
17986 RCDATA "<RC-escaped OUT_DIR>/fbforge-cooperation.bin"
```

17986 ist der Integername 0x4642, RCDATA der Integertyp 10. Die neutrale numerische
Sprache 0 wird genau einmal eingebettet. Die Datei enthält ASCII `FBFORGE` plus
NUL, Little-Endian u16 Format 1, u16 Koordinationsprotokoll 1, u16 AppID-Bytelänge
und exakt `de.fbforge.inspect` als ASCII. Hinter der AppID steht kein NUL oder Padding.
RC-Pfade werden mit verdoppelten Anführungszeichen und maskierten Backslashes
angegeben; Steuerzeichen und nicht als UTF-8 darstellbare Pfade sperren den Build.

Die feste AppID muss zur nativen Koordinations-ID und `tauri.conf.json` passen;
die Native-Contracts-Tests prüfen diese drei Quellen gemeinsam. Ein abweichender
Konfigurationsoverride ist kein freigegebener Produktidentitätswechsel und
bekommt keinen angepassten Marker über Buildflags oder Umgebungsvariablen.

Der Marker erklärt nur die Kooperation mit Protokoll 1 für diese AppID. Erst die
Suite prüft auf demselben gesperrten Dateihandle das PE-Layout, den Marker sowie
die lokale Publisher-Policy, Authenticode und Zeit-/Kettennachweise. Er allein
beweist weder Publisher noch Version, laufende Prozesse, `Stopped`, externe
Java-/JAR-Dateien, Installergesundheit oder Ausführungsrechte. Es werden keine
Schlüssel, Signaturen, Installerargumente, NSIS-Vorlagen oder Guard-ABIs geändert.

## Nachweise und offene Plattformabnahmen

`cargo test --manifest-path desktop/native-contracts/Cargo.toml --locked` prüft
Byteformat, native AppID, Windows-GNU/MSVC-Zielauswahl, Nicht-Windows und RC-Pfade.
Der externe isolierte Handoff enthält zusätzlich den echten MinGW-windres-/PE-
Compilerlayoutnachweis samt privatem Suite-Coreparser. Dessen unsignierte kleine
Fixture erhält ausschließlich in Scratch einen synthetischen Certificate-
Directory-Eintrag und Dummyzertifikat am Dateiende, damit der strukturelle Parser
laufen kann. Das ist ausdrücklich keine Authenticode-/Publisherprüfung und wird
nicht mitgeliefert. Der Compiler legt Alignment außerhalb der Marker-Payload an.

Produktionssignierung bleibt nach dem regulären Build erforderlich. Native
Windows-/MSVC-Resourcecompiler-, signierte Publisher-/SIP-/ACL-/Sharing-, GUI-
und Installerabnahmen bleiben offen. GNU-Crossbuild und Strukturfixture ersetzen
sie nicht. Die Compile-only-Javaressource im isolierten Build ist kein laufender
Java-Engine-Nachweis. Prüfbefehle, Quellhashes und Grenzen stehen im externen
Handoff; bekannte unveränderte Connect-Lintfehler werden dort getrennt benannt.

Quellen: tatsächliche gepinnte `tauri-build`-2.7.3-Quelle (WindowsAttributes,
Windows-Targetzweig und Weitergabe an tauri-winres; Connect 0.3.6, Inspect 0.3.7),
[Build-API](https://docs.rs/tauri-build/2.7.3/tauri_build/struct.WindowsAttributes.html#method.append_rc_content),
[Microsoft RCDATA](https://learn.microsoft.com/en-us/windows/win32/menurc/rcdata-resource),
[Microsoft LANGUAGE](https://learn.microsoft.com/en-us/windows/win32/menurc/language-statement).
