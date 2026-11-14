; Change shell and registry context based on running
; architecture and chosen install mode.
!macro SetContext
  !if "${INSTALLMODE}" == "currentUser"
    SetShellVarContext current
  !else if "${INSTALLMODE}" == "perMachine"
    SetShellVarContext all
  !endif

  ${If} ${RunningX64}
    !if "${ARCH}" == "x64"
      SetRegView 64
    !else if "${ARCH}" == "arm64"
      SetRegView 64
    !else
      SetRegView 32
    !endif
  ${EndIf}
!macroend

; Sets AppUserModelId on a shortcut
!macro SetLnkAppUserModelId shortcut
  Push $6
  Push $7
  StrCpy $7 1
  StrCpy $0 0
  StrCpy $1 0
  StrCpy $2 0
  StrCpy $3 0
  StrCpy $4 0
  StrCpy $5 0
  ${Do}
    !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ShellLink} ${IID_IShellLink} r0 .r6
    ${If} $6 <> 0
    ${OrIf} $0 P= 0
      ${ExitDo}
    ${EndIf}
    ${IUnknown::QueryInterface} $0 '("${IID_IPersistFile}",.r1).r6'
    ${If} $6 <> 0
    ${OrIf} $1 P= 0
      ${ExitDo}
    ${EndIf}
    ${IPersistFile::Load} $1 '("${shortcut}",${STGM_READWRITE}).r6'
    ${If} $6 <> 0
      ${ExitDo}
    ${EndIf}
    ${IUnknown::QueryInterface} $0 '("${IID_IPropertyStore}",.r2).r6'
    ${If} $6 <> 0
    ${OrIf} $2 P= 0
      ${ExitDo}
    ${EndIf}
    System::Call 'Oleaut32::SysAllocString(w "${BUNDLEID}")p.r3'
    System::Call '*${SYSSTRUCT_PROPERTYKEY}(${PKEY_AppUserModel_ID})p.r4'
    System::Call '*${SYSSTRUCT_PROPVARIANT}(${VT_BSTR},,&i4 $3)p.r5'
    ${If} $3 P= 0
    ${OrIf} $4 P= 0
    ${OrIf} $5 P= 0
      ${ExitDo}
    ${EndIf}
    ${IPropertyStore::SetValue} $2 '($4,$5).r6'
    ${If} $6 <> 0
      ${ExitDo}
    ${EndIf}
    ${IPropertyStore::Commit} $2 '().r6'
    ${If} $6 <> 0
      ${ExitDo}
    ${EndIf}
    ${IPersistFile::Save} $1 '("${shortcut}",1).r6'
    ${If} $6 = 0
      StrCpy $7 0
    ${EndIf}
  ${LoopWhile} 0 = 1
  ${If} $5 P<> 0
    System::Free $5
  ${EndIf}
  ${If} $4 P<> 0
    System::Free $4
  ${EndIf}
  ${If} $3 P<> 0
    System::Call 'Oleaut32::SysFreeString(p $3)'
  ${EndIf}
  ${If} $2 P<> 0
    ${IUnknown::Release} $2 ""
  ${EndIf}
  ${If} $1 P<> 0
    ${IUnknown::Release} $1 ""
  ${EndIf}
  ${If} $0 P<> 0
    ${IUnknown::Release} $0 ""
  ${EndIf}
  ${If} $7 <> 0
    SetErrors
  ${EndIf}
  Pop $7
  Pop $6
!macroend

; Deletes jump list entries and recent destinations
!macro DeleteAppUserModelId
  !insertmacro ComHlpr_CreateInProcInstance ${CLSID_DestinationList} ${IID_ICustomDestinationList} r1 ""
  ${If} $1 P<> 0
    ${ICustomDestinationList::DeleteList} $1 '("${BUNDLEID}")'
    ${IUnknown::Release} $1 ""
  ${EndIf}
  !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ApplicationDestinations} ${IID_IApplicationDestinations} r1 ""
  ${If} $1 P<> 0
    ${IApplicationDestinations::SetAppID} $1 '("${BUNDLEID}")i.r0'
    ${If} $0 >= 0
      ${IApplicationDestinations::RemoveAllDestinations} $1 ''
    ${EndIf}
    ${IUnknown::Release} $1 ""
  ${EndIf}
!macroend

; Unpins a shortcut from Start menu and Taskbar
;
; From https://stackoverflow.com/a/42816728/16993372
!macro UnpinShortcut shortcut
  !insertmacro ComHlpr_CreateInProcInstance ${CLSID_StartMenuPin} ${IID_IStartMenuPinnedList} r0 ""
  ${If} $0 P<> 0
      System::Call 'SHELL32::SHCreateItemFromParsingName(ws, p0, g "${IID_IShellItem}", *p0r1)' "${shortcut}"
      ${If} $1 P<> 0
          ${IStartMenuPinnedList::RemoveFromList} $0 '(r1)'
          ${IUnknown::Release} $1 ""
      ${EndIf}
      ${IUnknown::Release} $0 ""
  ${EndIf}
!macroend

; Set target path for a .lnk shortcut
!macro SetShortcutTarget shortcut target
  Push $6
  Push $7
  StrCpy $7 1
  StrCpy $0 0
  StrCpy $1 0
  ${Do}
    !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ShellLink} ${IID_IShellLink} r0 .r6
    ${If} $6 <> 0
    ${OrIf} $0 P= 0
      ${ExitDo}
    ${EndIf}
    ${IUnknown::QueryInterface} $0 '("${IID_IPersistFile}",.r1).r6'
    ${If} $6 <> 0
    ${OrIf} $1 P= 0
      ${ExitDo}
    ${EndIf}
    ${IPersistFile::Load} $1 '("${shortcut}",${STGM_READWRITE}).r6'
    ${If} $6 <> 0
      ${ExitDo}
    ${EndIf}
    ${IShellLink::SetPath} $0 '("${target}").r6'
    ${If} $6 <> 0
      ${ExitDo}
    ${EndIf}
    ${IPersistFile::Save} $1 '("${shortcut}",1).r6'
    ${If} $6 = 0
      StrCpy $7 0
    ${EndIf}
  ${LoopWhile} 0 = 1
  ${If} $1 P<> 0
    ${IUnknown::Release} $1 ""
  ${EndIf}
  ${If} $0 P<> 0
    ${IUnknown::Release} $0 ""
  ${EndIf}
  ${If} $7 <> 0
    SetErrors
  ${EndIf}
  Pop $7
  Pop $6
!macroend

!define /ifndef MAX_PATH 260
!define /ifndef SLGP_RAWPATH 0x4

; Test if a .lnk shortcut's target is target,
; use Pop to get the result, 1 is yes, 0 is no,
; note that this macro modifies $0, $1, $2, $3
;
; Example usage:
;   !insertmacro "IsShortCutTarget" "C:\Users\Public\Desktop\App.lnk" "C:\Program Files\App\App.exe"
;   Pop $0
;   ${If} $0 = 1
;     MessageBox MB_OK "shortcut target matches"
;   ${EndIf}
!macro IsShortcutTarget shortcut target
  Push $6
  Push $7
  StrCpy $3 0
  StrCpy $7 0
  ${If} ${FileExists} "${shortcut}"
    StrCpy $7 1
    StrCpy $0 0
    StrCpy $1 0
    ${Do}
      !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ShellLink} ${IID_IShellLink} r0 .r6
      ${If} $6 <> 0
      ${OrIf} $0 P= 0
        ${ExitDo}
      ${EndIf}
      ${IUnknown::QueryInterface} $0 '("${IID_IPersistFile}",.r1).r6'
      ${If} $6 <> 0
      ${OrIf} $1 P= 0
        ${ExitDo}
      ${EndIf}
      ${IPersistFile::Load} $1 '("${shortcut}",${STGM_READ}).r6'
      ${If} $6 <> 0
        ${ExitDo}
      ${EndIf}
      ${IShellLink::GetPath} $0 '(.r2,${NSIS_MAX_STRLEN},0,${SLGP_RAWPATH}).r6'
      ${If} $6 <> 0
        ${ExitDo}
      ${EndIf}
      StrCpy $7 0
      ${If} $2 == "${target}"
        StrCpy $3 1
      ${EndIf}
    ${LoopWhile} 0 = 1
    ${If} $1 P<> 0
      ${IUnknown::Release} $1 ""
    ${EndIf}
    ${If} $0 P<> 0
      ${IUnknown::Release} $0 ""
    ${EndIf}
  ${EndIf}
  ${If} $7 <> 0
    SetErrors
  ${EndIf}
  Pop $7
  Pop $6
  Push $3
!macroend

; Optional old association absence is not a write failure. Every actual write
; is checked separately, so a later helper cannot erase a previous failure.
!macro FBForgeAssociate EXT FILECLASS DESCRIPTION ICON COMMANDTEXT COMMAND
  ClearErrors
  ReadRegStr $R0 SHCTX "Software\Classes\.${EXT}" ""
  ${If} ${Errors}
    StrCpy $R0 ""
    ClearErrors
  ${EndIf}
  WriteRegStr SHCTX "Software\Classes\.${EXT}" "${FILECLASS}_backup" "$R0"
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "Software\Classes\.${EXT}" "" "${FILECLASS}"
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "Software\Classes\${FILECLASS}" "" `${DESCRIPTION}`
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "Software\Classes\${FILECLASS}\DefaultIcon" "" `${ICON}`
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "Software\Classes\${FILECLASS}\shell" "" "open"
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "Software\Classes\${FILECLASS}\shell\open" "" `${COMMANDTEXT}`
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "Software\Classes\${FILECLASS}\shell\open\command" "" `${COMMAND}`
  !insertmacro FBForgeCheckWrite
!macroend

; A same-name foreign shortcut is not owned by this product.
!macro FBForgeCreateShortcut shortcut target
  ${If} ${FileExists} "${shortcut}"
    !insertmacro IsShortcutTarget "${shortcut}" "${target}"
    Pop $0
    !insertmacro FBForgeCheckWrite
    ${If} $0 <> 1
      Call FBForgeInstallFailed
    ${EndIf}
  ${EndIf}
  CreateShortcut "${shortcut}" "${target}"
  !insertmacro FBForgeCheckWrite
  !insertmacro SetLnkAppUserModelId "${shortcut}"
  !insertmacro FBForgeCheckWrite
!macroend
