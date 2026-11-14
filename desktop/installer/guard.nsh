; Fixed local identity, never read from renderer, metadata or command line.
!define FBFORGE_GUARD_APP_ID "de.fbforge.inspect"
!addplugindir /x86-unicode "${__FILEDIR__}/guard"
!include "${__FILEDIR__}/safe-utils.nsh"
Var FBForgeGuardReply
Var FBForgeGuardSentinel
Var FBForgeGuardHeld
Var FBForgeGuardReleased
Var FBForgeShortcutsFinalized

; The three exports keep their exact existing ABI. Only an exact fresh reply
; with a consumed sentinel and no NSIS error is accepted.
!macro FBForgeGuardFunctions PREFIX
Function ${PREFIX}FBForgeAcquire
  Push "fbforge-guard-v1-sentinel"
  ClearErrors
  fbforge_suite_nsis_guard::AcquireV1 "${FBFORGE_GUARD_APP_ID}"
  IfErrors denied
  Pop $FBForgeGuardReply
  IfErrors denied
  StrCmp $FBForgeGuardReply "v1:held" 0 denied
  Pop $FBForgeGuardSentinel
  IfErrors denied
  StrCmp $FBForgeGuardSentinel "fbforge-guard-v1-sentinel" 0 denied
  StrCpy $FBForgeGuardHeld 1
  StrCpy $FBForgeGuardReleased 0
  Return
  denied:
    SetErrorLevel 70
    MessageBox MB_OK|MB_ICONSTOP "${PRODUCTNAME}: Die Koordination ist belegt oder nicht sicher verfügbar. Bitte schließen Sie die Anwendung selbst und versuchen Sie es erneut." /SD IDOK
    Quit
FunctionEnd

Function ${PREFIX}FBForgeRequireHeld
  StrCmp $FBForgeGuardHeld 1 0 denied
  Push "fbforge-guard-v1-sentinel"
  ClearErrors
  fbforge_suite_nsis_guard::StatusV1
  IfErrors denied
  Pop $FBForgeGuardReply
  IfErrors denied
  StrCmp $FBForgeGuardReply "v1:held" 0 denied
  Pop $FBForgeGuardSentinel
  IfErrors denied
  StrCmp $FBForgeGuardSentinel "fbforge-guard-v1-sentinel" 0 denied
  Return
  denied:
    SetErrorLevel 70
    MessageBox MB_OK|MB_ICONSTOP "${PRODUCTNAME}: Der gehaltene Koordinationsnachweis ist nicht mehr verfügbar. Der Vorgang wird beendet." /SD IDOK
    Quit
FunctionEnd

Function ${PREFIX}FBForgeRequireMutationEvidence
  Call ${PREFIX}FBForgeRequireHeld
  ; Deliberately unavailable native installed-image/process evidence. A lease
  ; does not prove absence of legacy apps. This development slice cannot mutate.
  SetErrorLevel 70
  MessageBox MB_OK|MB_ICONSTOP "${PRODUCTNAME}: Der erforderliche native Bestands- und Prozessnachweis ist noch nicht verfügbar. Es werden keine Produkt-, Registry- oder Nutzdaten geändert." /SD IDOK
  Quit
FunctionEnd

Function ${PREFIX}FBForgeRelease
  StrCmp $FBForgeGuardReleased 1 done
  Call ${PREFIX}FBForgeRequireHeld
  Push "fbforge-guard-v1-sentinel"
  ClearErrors
  fbforge_suite_nsis_guard::ReleaseV1
  IfErrors denied
  Pop $FBForgeGuardReply
  IfErrors denied
  StrCmp $FBForgeGuardReply "v1:released" 0 denied
  Pop $FBForgeGuardSentinel
  IfErrors denied
  StrCmp $FBForgeGuardSentinel "fbforge-guard-v1-sentinel" 0 denied
  StrCpy $FBForgeGuardHeld 0
  StrCpy $FBForgeGuardReleased 1
  done:
    Return
  denied:
    SetErrorLevel 70
    MessageBox MB_OK|MB_ICONSTOP "${PRODUCTNAME}: Die Freigabe konnte nicht bestätigt werden. Die Anwendung wird nicht gestartet." /SD IDOK
    Quit
FunctionEnd
!macroend
