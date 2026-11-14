# Entwicklungsartefakt erforderlich

Hier muss vor einem Compilerlauf die separat gebaute Unicode-x86-DLL
`fbforge_suite_nsis_guard.dll` mit den drei unveränderten V1-Exporten liegen.
Keine DLL wird aus dem Netz geladen oder in diesem Patch ausgeliefert. Vor
Distribution sind native Provenienz-/Signaturprüfung und Paketbindung erforderlich.
Der native Mutationsvorflug ist noch gesperrt. Dies ist kein Releaseinstaller.
