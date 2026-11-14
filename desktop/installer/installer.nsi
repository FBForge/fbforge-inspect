Unicode true
ManifestDPIAware true
; Add in `dpiAwareness` `PerMonitorV2` to manifest for Windows 10 1607+ (note this should not affect lower versions since they should be able to ignore this and pick up `dpiAware` `true` set by `ManifestDPIAware true`)
; Currently undocumented on NSIS's website but is in the Docs folder of source tree, see
; https://github.com/kichik/nsis/blob/5fc0b87b819a9eec006df4967d08e522ddd651c9/Docs/src/attributes.but#L286-L300
; https://github.com/tauri-apps/tauri/pull/10106
ManifestDPIAwareness PerMonitorV2

!if "{{compression}}" == "none"
  SetCompress off
!else
  ; Set the compression algorithm. We default to LZMA.
  SetCompressor /SOLID "{{compression}}"
!endif

; Keep above !include to stay ahead of any plugin command
; see https://github.com/tauri-apps/tauri/pull/15422#discussion_r3289239624
{{#if signed_plugins_path}}
!addplugindir "{{signed_plugins_path}}"
{{/if}}

!include MUI2.nsh
!include FileFunc.nsh
!include x64.nsh
!include WordFunc.nsh
!include "FileAssociation.nsh"
!include "Win\COM.nsh"
!include "Win\Propkey.nsh"
!include "StrFunc.nsh"
${StrCase}
${StrLoc}

{{#if installer_hooks}}
!include "{{installer_hooks}}"
{{/if}}

!define WEBVIEW2APPGUID "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

!define MANUFACTURER "{{manufacturer}}"
!define PRODUCTNAME "{{product_name}}"
!define VERSION "{{version}}"
!define VERSIONWITHBUILD "{{version_with_build}}"
!define HOMEPAGE "{{homepage}}"
!define INSTALLMODE "{{install_mode}}"
!if "${INSTALLMODE}" != "currentUser"
  !error "FBForge native V2 supports currentUser targets only"
!endif
!define LICENSE "{{license}}"
!define INSTALLERICON "{{installer_icon}}"
!define SIDEBARIMAGE "{{sidebar_image}}"
!define HEADERIMAGE "{{header_image}}"
!define UNINSTALLERICON "{{uninstaller_icon}}"
!define UNINSTALLERHEADERIMAGE "{{uninstaller_header_image}}"
!define MAINBINARYNAME "{{main_binary_name}}"
!define MAINBINARYSRCPATH "{{main_binary_path}}"
!define BUNDLEID "{{bundle_id}}"
!define COPYRIGHT "{{copyright}}"
!define OUTFILE "{{out_file}}"
!define ARCH "{{arch}}"
!define ADDITIONALPLUGINSPATH "{{additional_plugins_path}}"
!define ALLOWDOWNGRADES "{{allow_downgrades}}"
!define DISPLAYLANGUAGESELECTOR "{{display_language_selector}}"
!define INSTALLWEBVIEW2MODE "{{install_webview2_mode}}"
!define WEBVIEW2INSTALLERARGS "{{webview2_installer_args}}"
!define WEBVIEW2BOOTSTRAPPERPATH "{{webview2_bootstrapper_path}}"
!define WEBVIEW2INSTALLERPATH "{{webview2_installer_path}}"
!define MINIMUMWEBVIEW2VERSION "{{minimum_webview2_version}}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define MANUKEY "Software\${MANUFACTURER}"
!define MANUPRODUCTKEY "${MANUKEY}\${PRODUCTNAME}"
!define UNINSTALLERSIGNCOMMAND "{{uninstaller_sign_cmd}}"
!define ESTIMATEDSIZE "{{estimated_size}}"
!define STARTMENUFOLDER "{{start_menu_folder}}"

Var PassiveMode
Var UpdateMode
Var NoShortcutMode
Var OldMainBinaryName

; Hash exactly the signed application and rendered File map, before signing setup.
!tempfile FBFORGE_PAYLOAD_MAP
!tempfile FBFORGE_PAYLOAD_FILE
!tempfile FBFORGE_PAYLOAD_DEFINITION
!appendfile /CHARSET=UTF8 /RawNL "${FBFORGE_PAYLOAD_MAP}" '{"schemaVersion":1,"toolId":"${FBFORGE_GUARD_TOOL_ID}","appId":"${BUNDLEID}","version":"${VERSION}","optionalPrefixes":[]}$\n'
!appendfile /CHARSET=UTF8 /RawNL "${FBFORGE_PAYLOAD_MAP}" "${MAINBINARYSRCPATH}$\n${MAINBINARYNAME}.exe$\n"
{{#each resources}}
!appendfile /CHARSET=UTF8 /RawNL "${FBFORGE_PAYLOAD_MAP}" "{{no-escape @key}}$\n{{this.[1]}}$\n"
{{/each}}
{{#each binaries}}
!appendfile /CHARSET=UTF8 /RawNL "${FBFORGE_PAYLOAD_MAP}" "{{no-escape @key}}$\n{{this}}$\n"
{{/each}}
!execute '$\"node$\" $\"${FBFORGE_GUARD_DIR}/nsis/create-payload.mjs$\" $\"${FBFORGE_PAYLOAD_MAP}$\" $\"${FBFORGE_PAYLOAD_FILE}$\" $\"${FBFORGE_PAYLOAD_DEFINITION}$\" $\"${NSISDIR}$\"' = 0
!include "${FBFORGE_PAYLOAD_DEFINITION}"
!delfile "${FBFORGE_PAYLOAD_MAP}"
!delfile "${FBFORGE_PAYLOAD_DEFINITION}"

!insertmacro FBForgeV2Functions
!insertmacro FBForgeGuardFunctions "un."

Name "${PRODUCTNAME}"
BrandingText "${COPYRIGHT}"
OutFile "${OUTFILE}"

; This placeholder and any /D value are never an installation target.
; Only PathV2 replaces it before product writes.
!define PLACEHOLDER_INSTALL_DIR "placeholder\${PRODUCTNAME}"
InstallDir "${PLACEHOLDER_INSTALL_DIR}"

VIProductVersion "${VERSIONWITHBUILD}"
VIAddVersionKey "ProductName" "${PRODUCTNAME}"
VIAddVersionKey "FileDescription" "${PRODUCTNAME}"
VIAddVersionKey "LegalCopyright" "${COPYRIGHT}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
; Signed cooperation assertion, independently checked on the held installer PE.
VIAddVersionKey /LANG=1033 "FBForgeAppId" "${BUNDLEID}"
VIAddVersionKey /LANG=1033 "FBForgeInstallerProtocol" "1"
VIAddVersionKey /LANG=1033 "FBForgePayloadManifestSha256" "${FBFORGE_PAYLOAD_SHA256}"

# additional plugins
!addplugindir "${ADDITIONALPLUGINSPATH}"

; Uninstaller signing command
!if "${UNINSTALLERSIGNCOMMAND}" != ""
  !uninstfinalize '${UNINSTALLERSIGNCOMMAND}'
!endif

; Handle install mode, `perUser`, `perMachine` or `both`
!if "${INSTALLMODE}" == "perMachine"
  RequestExecutionLevel admin
!endif

!if "${INSTALLMODE}" == "currentUser"
  RequestExecutionLevel user
!endif

!if "${INSTALLMODE}" == "both"
  !define MULTIUSER_MUI
  !define MULTIUSER_INSTALLMODE_INSTDIR "${PRODUCTNAME}"
  !define MULTIUSER_INSTALLMODE_COMMANDLINE
  !if "${ARCH}" == "x64"
    !define MULTIUSER_USE_PROGRAMFILES64
  !else if "${ARCH}" == "arm64"
    !define MULTIUSER_USE_PROGRAMFILES64
  !endif
  !define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_KEY "${UNINSTKEY}"
  !define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_VALUENAME "CurrentUser"
  !define MULTIUSER_INSTALLMODEPAGE_SHOWUSERNAME
  !define MULTIUSER_INSTALLMODE_FUNCTION RestorePreviousInstallLocation
  !define MULTIUSER_EXECUTIONLEVEL Highest
  !include MultiUser.nsh
!endif

; Installer icon
!if "${INSTALLERICON}" != ""
  !define MUI_ICON "${INSTALLERICON}"
!endif

; Installer sidebar image
!if "${SIDEBARIMAGE}" != ""
  !define MUI_WELCOMEFINISHPAGE_BITMAP "${SIDEBARIMAGE}"
!endif

; Enable header images for installer and uninstaller pages when either image is configured.
!if "${HEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE
!else if "${UNINSTALLERHEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE
!endif

; Installer header image
!if "${HEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE_BITMAP "${HEADERIMAGE}"
!endif

; Uninstaller header image
!if "${UNINSTALLERHEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE_UNBITMAP "${UNINSTALLERHEADERIMAGE}"
!endif

; Uninstaller icon
!if "${UNINSTALLERICON}" != ""
  !define MUI_UNICON "${UNINSTALLERICON}"
!endif

; Define registry key to store installer language
!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "${MANUPRODUCTKEY}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "Installer Language"

; Installer pages, must be ordered as they appear
; 1. Welcome Page
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_WELCOME

; 2. License Page (if defined)
!if "${LICENSE}" != ""
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !insertmacro MUI_PAGE_LICENSE "${LICENSE}"
!endif

; 3. Install mode (if it is set to `both`)
!if "${INSTALLMODE}" == "both"
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !insertmacro MULTIUSER_PAGE_INSTALLMODE
!endif

; No nested historical uninstaller: parent lease, untrusted registry command
; and historical forced-close semantics cannot safely be delegated.

; 5. Choose install directory page
; No directory picker. Native V2 chooses existing or default target.

; 6. Start menu shortcut page
Var AppStartMenuFolder
!if "${STARTMENUFOLDER}" != ""
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !define MUI_STARTMENUPAGE_DEFAULTFOLDER "${STARTMENUFOLDER}"
!else
  !define MUI_PAGE_CUSTOMFUNCTION_PRE Skip
!endif
!insertmacro MUI_PAGE_STARTMENU Application $AppStartMenuFolder

; 7. Installation page
; Desktop choice belongs before the worker, never after native finalization.
Page custom FBForgeDesktopPage FBForgeDesktopPageLeave
Function FBForgeDesktopPage
  ${If} ${Silent}
  ${OrIf} $PassiveMode = 1
  ${OrIf} $UpdateMode = 1
  ${OrIf} $NoShortcutMode = 1
    Abort
  ${EndIf}
  !insertmacro MUI_HEADER_TEXT "Verknüpfungen" "Wählen Sie die gewünschte Desktopverknüpfung."
  ClearErrors
  nsDialogs::Create 1018
  Pop $FBForgeDesktopDialog
  IfErrors failed
  StrCmpS $FBForgeDesktopDialog "error" failed
  ${NSD_CreateCheckbox} 0 0 100% 24u "$(createDesktop)"
  Pop $FBForgeDesktopCheckbox
  IfErrors failed
  StrCmpS $FBForgeDesktopCheckbox 0 failed
  ${NSD_SetState} $FBForgeDesktopCheckbox $FBForgeDesktopChoice
  nsDialogs::Show
  Return
  failed:
    Call FBForgeInstallFailed
FunctionEnd
Function FBForgeDesktopPageLeave
  ${NSD_GetState} $FBForgeDesktopCheckbox $FBForgeDesktopChoice
FunctionEnd

; MUI normally writes Installer Language in its GUI InstFilesLeave callback.
; Disable that late write. The worker saves it explicitly before FinalizeV2.
!undef MUI_LANGDLL_REGISTRY_ROOT
!undef MUI_LANGDLL_REGISTRY_KEY
!undef MUI_LANGDLL_REGISTRY_VALUENAME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "${MANUPRODUCTKEY}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "Installer Language"


; 8. Finish page
;
; Don't auto jump to finish page after installation page,
; because the installation page has useful info that can be used debug any issues with the installer.
!define MUI_FINISHPAGE_NOAUTOCLOSE
; Show run app after installation.
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_FUNCTION RunMainBinary
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_FINISH

Function RunMainBinary
  Call FBForgeRequireCompletedV2
  ClearErrors
  nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" ""
  !insertmacro FBForgeCheckWrite
FunctionEnd

!define MUI_CUSTOMFUNCTION_ABORT FBForgeOnUserAbort
Function FBForgeOnUserAbort
  StrCpy $FBForgeInstallFailed 1
  SetErrorLevel 70
FunctionEnd
Function .onInstFailed
  StrCpy $FBForgeInstallFailed 1
  SetErrorLevel 70
FunctionEnd

; Uninstaller Pages
; 1. Confirm uninstall page
Var DeleteAppDataCheckbox
Var DeleteAppDataCheckboxState
!define /ifndef WS_EX_LAYOUTRTL         0x00400000
!define MUI_PAGE_CUSTOMFUNCTION_SHOW un.ConfirmShow
Function un.ConfirmShow ; Add add a `Delete app data` check box
  ; $1 inner dialog HWND
  ; $2 window DPI
  ; $3 style
  ; $4 x
  ; $5 y
  ; $6 width
  ; $7 height
  FindWindow $1 "#32770" "" $HWNDPARENT ; Find inner dialog
  System::Call "user32::GetDpiForWindow(p r1) i .r2"
  ${If} $(^RTL) = 1
    StrCpy $3 "${__NSD_CheckBox_EXSTYLE} | ${WS_EX_LAYOUTRTL}"
    IntOp $4 50 * $2
  ${Else}
    StrCpy $3 "${__NSD_CheckBox_EXSTYLE}"
    IntOp $4 0 * $2
  ${EndIf}
  IntOp $5 100 * $2
  IntOp $6 400 * $2
  IntOp $7 25 * $2
  IntOp $4 $4 / 96
  IntOp $5 $5 / 96
  IntOp $6 $6 / 96
  IntOp $7 $7 / 96
  System::Call 'user32::CreateWindowEx(i r3, w "${__NSD_CheckBox_CLASS}", w "$(deleteAppData)", i ${__NSD_CheckBox_STYLE}, i r4, i r5, i r6, i r7, p r1, i0, i0, i0) i .s'
  Pop $DeleteAppDataCheckbox
  SendMessage $HWNDPARENT ${WM_GETFONT} 0 0 $1
  SendMessage $DeleteAppDataCheckbox ${WM_SETFONT} $1 1
FunctionEnd
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE un.ConfirmLeave
Function un.ConfirmLeave
  SendMessage $DeleteAppDataCheckbox ${BM_GETCHECK} 0 0 $DeleteAppDataCheckboxState
FunctionEnd
!define MUI_PAGE_CUSTOMFUNCTION_PRE un.SkipIfPassive
!insertmacro MUI_UNPAGE_CONFIRM

; 2. Uninstalling Page
!insertmacro MUI_UNPAGE_INSTFILES

;Languages
{{#each languages}}
!insertmacro MUI_LANGUAGE "{{this}}"
{{/each}}
!insertmacro MUI_RESERVEFILE_LANGDLL
{{#each language_files}}
  !include "{{this}}"
{{/each}}

Function .onInit
  StrCpy $FBForgeInstallFailed 0
  StrCpy $FBForgeV2Started 0
  StrCpy $FBForgeV2Complete 0
  StrCpy $FBForgeGuardReleased 0
  StrCpy $FBForgeShortcutsFinalized 0
  StrCpy $FBForgeDesktopChoice 1
  SetErrorLevel 70
  ${GetOptions} $CMDLINE "/P" $PassiveMode
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/NS" $NoShortcutMode
  ${IfNot} ${Errors}
    StrCpy $NoShortcutMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/UPDATE" $UpdateMode
  ${IfNot} ${Errors}
    StrCpy $UpdateMode 1
  ${EndIf}

  !if "${DISPLAYLANGUAGESELECTOR}" == "true"
    !insertmacro MUI_LANGDLL_DISPLAY
  !endif

  !insertmacro SetContext

  ; Abort downgrades if they are disabled, in the modes that skip the reinstall page choice:
  ; silent, passive and update
  !if "${ALLOWDOWNGRADES}" == "false"
  ${If} ${Silent}
  ${OrIf} $PassiveMode = 1
  ${OrIf} $UpdateMode = 1
    ReadRegStr $R0 SHCTX "${UNINSTKEY}" "DisplayVersion"
    nsis_tauri_utils::SemverCompare "${VERSION}" $R0
    Pop $R0
    ; If downgrading
    ${If} $R0 = -1
      ${If} ${Silent}
        System::Call 'kernel32::AttachConsole(i -1)i.r0'
        ${If} $0 <> 0
          System::Call 'kernel32::GetStdHandle(i -11)i.r0'
          System::call 'kernel32::SetConsoleTextAttribute(i r0, i 0x0004)' ; set red color
          FileWrite $0 "$(silentDowngrades)"
        ${EndIf}
      ${Else}
        MessageBox MB_ICONSTOP "$(silentDowngrades)"
      ${EndIf}
      Abort
    ${EndIf}
  ${EndIf}
  !endif
FunctionEnd

Function FBForgeReadWebView2Version
  StrCpy $4 ""
  ${If} ${RunningX64}
    ReadRegStr $4 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${Else}
    ReadRegStr $4 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${EndIf}
  ${If} $4 == ""
    ReadRegStr $4 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2APPGUID}" "pv"
  ${EndIf}
  ClearErrors
  ${If} $4 == ""
    Return
  ${EndIf}
  ${VersionCompare} "0.0.0.0" "$4" $R0
  ${If} $R0 <> 2
    StrCpy $4 ""
    Return
  ${EndIf}
  !if "${MINIMUMWEBVIEW2VERSION}" != ""
    ${VersionCompare} "${MINIMUMWEBVIEW2VERSION}" "$4" $R0
    ${If} $R0 = 1
      StrCpy $4 ""
    ${EndIf}
  !endif
FunctionEnd

Section "-FBForge native preparation"
  Call FBForgeExtractPayload
  StrCpy $FBForgeV2Components ""

  Call FBForgePrepareV2
SectionEnd

Section WebView2
  Call FBForgeReadWebView2Version
  StrCmpS $4 "" 0 webview2_ready
  !if "${INSTALLWEBVIEW2MODE}" == "offlineInstaller"
    ; Only a fully embedded prerequisite. No bootstrapper, download or system
    ; updater invocation can bypass the Suite's native network settings.
    ; PublishedPending recovery has no stage: provision WebView2 separately.
    Call FBForgeRequireStagingV2
    Call FBForgeStagePathV2
    ClearErrors
    SetOutPath "$FBForgeV2Stage"
    !insertmacro FBForgeCheckWrite
    File "/oname=.fbforge-webview2-prerequisite.exe" "${WEBVIEW2INSTALLERPATH}"
    !insertmacro FBForgeCheckWrite
    ClearErrors
    ExecWait '"$FBForgeV2Stage\.fbforge-webview2-prerequisite.exe" ${WEBVIEW2INSTALLERARGS} /install' $1
    !insertmacro FBForgeCheckWrite
    ${If} $1 <> 0
      Call FBForgeInstallFailed
    ${EndIf}
    ; This is not a payload leaf. Remove it strictly before native StageHealth.
    ClearErrors
    Delete "$FBForgeV2Stage\.fbforge-webview2-prerequisite.exe"
    !insertmacro FBForgeCheckWrite
    SetOutPath "$PLUGINSDIR"
    !insertmacro FBForgeCheckWrite
    Call FBForgeReadWebView2Version
    StrCmpS $4 "" 0 webview2_ready
  !endif
  SetErrorLevel 70
  StrCpy $FBForgeInstallFailed 1
  MessageBox MB_OK|MB_ICONSTOP "${PRODUCTNAME}: Microsoft Edge WebView2 Runtime fehlt oder ist zu alt. Lassen Sie die erforderliche Laufzeit vor diesem Vorgang installieren. Es wird kein Online-Download und kein System-Updater gestartet." /SD IDOK
  Quit
  webview2_ready:
SectionEnd

Section Install
  Call FBForgePollV2
  StrCmpS $FBForgeV2Phase "v2:publishedPending" published_metadata
  StrCmpS $FBForgeV2Phase "v2:recoveryPublishedPending" published_metadata
  Call FBForgeRequireStagingV2
  StrCpy $INSTDIR $FBForgeV2Stage
  ClearErrors
  SetOutPath $INSTDIR
  !insertmacro FBForgeCheckWrite

  !ifmacrodef NSIS_HOOK_PREINSTALL
    Call FBForgeRequireStagingV2
    !insertmacro NSIS_HOOK_PREINSTALL
    !insertmacro FBForgeCheckWrite
  !endif


  ; Copy main executable
  File "${MAINBINARYSRCPATH}"
  !insertmacro FBForgeCheckWrite

  ; Copy resources
  {{#each resources_dirs}}
    CreateDirectory "$INSTDIR\\{{this}}"
    !insertmacro FBForgeCheckWrite
  {{/each}}
  {{#each resources}}
    File /a "/oname={{this.[1]}}" "{{no-escape @key}}"
    !insertmacro FBForgeCheckWrite
  {{/each}}

  ; Copy external binaries
  {{#each binaries}}
    File /a "/oname={{this}}" "{{no-escape @key}}"
    !insertmacro FBForgeCheckWrite
  {{/each}}

  Call FBForgeRequireStagingV2
  ; A prior interrupted offline prerequisite may remain in recovered staging.
  !if "${INSTALLWEBVIEW2MODE}" == "offlineInstaller"
    IfFileExists "$INSTDIR\.fbforge-webview2-prerequisite.exe" 0 no_prerequisite_residue
      ClearErrors
      Delete "$INSTDIR\.fbforge-webview2-prerequisite.exe"
      !insertmacro FBForgeCheckWrite
    no_prerequisite_residue:
  !endif
  ClearErrors
  WriteUninstaller "$INSTDIR\uninstall.exe"
  !insertmacro FBForgeCheckWrite
  Call FBForgePublishV2

  published_metadata:
  Call FBForgeRequirePublishedV2
  Call FBForgeTargetPathV2
  StrCpy $INSTDIR $FBForgeV2Target
  ClearErrors
  SetOutPath "$INSTDIR"
  !insertmacro FBForgeCheckWrite

  ; Create file associations
  {{#each file_associations as |association| ~}}
    {{#each association.ext as |ext| ~}}
       !insertmacro FBForgeAssociate "{{ext}}" "{{or association.name ext}}" "{{association-description association.description ext}}" "$INSTDIR\${MAINBINARYNAME}.exe,0" "Open with ${PRODUCTNAME}" "$INSTDIR\${MAINBINARYNAME}.exe $\"%1$\""
    {{/each}}
  {{/each}}

  ; Register deep links
  {{#each deep_link_protocols as |protocol| ~}}
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}" "URL Protocol" ""
    !insertmacro FBForgeCheckWrite
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}" "" "URL:${BUNDLEID} protocol"
    !insertmacro FBForgeCheckWrite
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}\DefaultIcon" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
    !insertmacro FBForgeCheckWrite
    WriteRegStr SHCTX "Software\Classes\\{{protocol}}\shell\open\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
    !insertmacro FBForgeCheckWrite
  {{/each}}

  ; Save $INSTDIR in registry for future installations
  WriteRegStr SHCTX "${MANUPRODUCTKEY}" "" $INSTDIR
  !insertmacro FBForgeCheckWrite

  !if "${INSTALLMODE}" == "both"
    ; Save install mode to be selected by default for the next installation such as updating
    ; or when uninstalling
    WriteRegStr SHCTX "${UNINSTKEY}" $MultiUser.InstallMode 1
    !insertmacro FBForgeCheckWrite
  !endif

  ; Retain the old value only for target-checked shortcut migration. A future
  ; native verified installation plan must name any executable cleanup explicitly.
  ClearErrors
  ReadRegStr $OldMainBinaryName SHCTX "${UNINSTKEY}" "MainBinaryName"
  ${If} ${Errors}
    StrCpy $OldMainBinaryName ""
    ClearErrors
  ${EndIf}

  ; Save current MAINBINARYNAME for future updates
  WriteRegStr SHCTX "${UNINSTKEY}" "MainBinaryName" "${MAINBINARYNAME}.exe"
  !insertmacro FBForgeCheckWrite

  ; Registry information for add/remove programs
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME}"
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "${UNINSTKEY}" "Publisher" "${MANUFACTURER}"
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "${UNINSTKEY}" "InstallLocation" "$\"$INSTDIR$\""
  !insertmacro FBForgeCheckWrite
  WriteRegStr SHCTX "${UNINSTKEY}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
  !insertmacro FBForgeCheckWrite
  WriteRegDWORD SHCTX "${UNINSTKEY}" "NoModify" "1"
  !insertmacro FBForgeCheckWrite
  WriteRegDWORD SHCTX "${UNINSTKEY}" "NoRepair" "1"
  !insertmacro FBForgeCheckWrite

  ClearErrors
  ${GetSize} "$INSTDIR" "/M=uninstall.exe /S=0K /G=0" $0 $1 $2
  !insertmacro FBForgeCheckWrite
  IntOp $0 $0 + ${ESTIMATEDSIZE}
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD SHCTX "${UNINSTKEY}" "EstimatedSize" "$0"
  !insertmacro FBForgeCheckWrite

  !if "${HOMEPAGE}" != ""
    WriteRegStr SHCTX "${UNINSTKEY}" "URLInfoAbout" "${HOMEPAGE}"
    !insertmacro FBForgeCheckWrite
    WriteRegStr SHCTX "${UNINSTKEY}" "URLUpdateInfo" "${HOMEPAGE}"
    !insertmacro FBForgeCheckWrite
    WriteRegStr SHCTX "${UNINSTKEY}" "HelpLink" "${HOMEPAGE}"
    !insertmacro FBForgeCheckWrite
  !endif

  ; Create start menu shortcut
  !insertmacro MUI_STARTMENU_WRITE_BEGIN Application
    Call CreateOrUpdateStartMenuShortcut
  !insertmacro MUI_STARTMENU_WRITE_END
  !insertmacro FBForgeCheckWrite

  ${If} $FBForgeDesktopChoice = 1
    Call CreateOrUpdateDesktopShortcut
  ${EndIf}

  !ifmacrodef NSIS_HOOK_POSTINSTALL
    Call FBForgeRequirePublishedV2
    !insertmacro NSIS_HOOK_POSTINSTALL
    !insertmacro FBForgeCheckWrite
  !endif

  ; Save language under the native plan, not in MUI's GUI leave callback.
  ClearErrors
  WriteRegStr HKCU "${MANUPRODUCTKEY}" "Installer Language" $LANGUAGE
  !insertmacro FBForgeCheckWrite
  StrCpy $FBForgeShortcutsFinalized 1
  Call FBForgeFinalizeV2

  ; Auto close this page for passive mode
  ${If} $PassiveMode = 1
    SetAutoClose true
  ${EndIf}
SectionEnd

Function .onInstSuccess
  Call FBForgeRequireCompletedV2
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    ClearErrors
    ${GetOptions} $CMDLINE "/R" $R0
    ${IfNot} ${Errors}
      ${GetOptions} $CMDLINE "/ARGS" $R0
      ${If} ${Errors}
        StrCpy $R0 ""
      ${EndIf}
      ClearErrors
      nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" "$R0"
      !insertmacro FBForgeCheckWrite
    ${EndIf}
  ${EndIf}
FunctionEnd

Function un.onInit
  Call un.FBForgeAcquire
  Call un.FBForgeRequireMutationEvidence
  !insertmacro SetContext

  !if "${INSTALLMODE}" == "both"
    !insertmacro MULTIUSER_UNINIT
  !endif

  !insertmacro MUI_UNGETLANGUAGE

  ${GetOptions} $CMDLINE "/P" $PassiveMode
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/UPDATE" $UpdateMode
  ${IfNot} ${Errors}
    StrCpy $UpdateMode 1
  ${EndIf}
FunctionEnd

Section Uninstall
  Call un.FBForgeRequireMutationEvidence

  !ifmacrodef NSIS_HOOK_PREUNINSTALL
    Call un.FBForgeRequireMutationEvidence
    !insertmacro NSIS_HOOK_PREUNINSTALL
  !endif


  ; Delete the app directory and its content from disk
  ; Copy main executable
  Delete "$INSTDIR\${MAINBINARYNAME}.exe"

  ; Delete resources
  {{#each resources}}
    Delete "$INSTDIR\\{{this.[1]}}"
  {{/each}}

  ; Delete external binaries
  {{#each binaries}}
    Delete "$INSTDIR\\{{this}}"
  {{/each}}

  ; Delete app associations
  {{#each file_associations as |association| ~}}
    {{#each association.ext as |ext| ~}}
      !insertmacro APP_UNASSOCIATE "{{ext}}" "{{or association.name ext}}"
    {{/each}}
  {{/each}}

  ; Delete deep links
  {{#each deep_link_protocols as |protocol| ~}}
    ReadRegStr $R7 SHCTX "Software\Classes\\{{protocol}}\shell\open\command" ""
    ${If} $R7 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
      DeleteRegKey SHCTX "Software\Classes\\{{protocol}}"
    ${EndIf}
  {{/each}}


  ; Delete uninstaller
  Delete "$INSTDIR\uninstall.exe"

  {{#each resources_ancestors}}
  RMDir "$INSTDIR\\{{this}}"
  {{/each}}
  RMDir "$INSTDIR"

  ; Remove shortcuts if not updating
  ${If} $UpdateMode <> 1
    !insertmacro DeleteAppUserModelId

    ; Remove start menu shortcut
    !insertmacro MUI_STARTMENU_GETFOLDER Application $AppStartMenuFolder
    !insertmacro IsShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      !insertmacro UnpinShortcut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
      Delete "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
      RMDir "$SMPROGRAMS\$AppStartMenuFolder"
    ${EndIf}
    !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      !insertmacro UnpinShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk"
      Delete "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    ${EndIf}

    ; Remove desktop shortcuts
    !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    Pop $0
    ${If} $0 = 1
      !insertmacro UnpinShortcut "$DESKTOP\${PRODUCTNAME}.lnk"
      Delete "$DESKTOP\${PRODUCTNAME}.lnk"
    ${EndIf}
  ${EndIf}

  ; Remove registry information for add/remove programs
  !if "${INSTALLMODE}" == "both"
    DeleteRegKey SHCTX "${UNINSTKEY}"
  !else if "${INSTALLMODE}" == "perMachine"
    DeleteRegKey HKLM "${UNINSTKEY}"
  !else
    DeleteRegKey HKCU "${UNINSTKEY}"
  !endif

  ; Removes the Autostart entry for ${PRODUCTNAME} from the HKCU Run key if it exists.
  ; This ensures the program does not launch automatically after uninstallation if it exists.
  ; If it doesn't exist, it does nothing.
  ; We do this when not updating (to preserve the registry value on updates)
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
  ${EndIf}

  ; Delete app data if the checkbox is selected
  ; and if not updating
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    ; Clear the install location $INSTDIR from registry
    DeleteRegKey SHCTX "${MANUPRODUCTKEY}"
    DeleteRegKey /ifempty SHCTX "${MANUKEY}"

    ; Clear the install language from registry
    DeleteRegValue HKCU "${MANUPRODUCTKEY}" "Installer Language"
    DeleteRegKey /ifempty HKCU "${MANUPRODUCTKEY}"
    DeleteRegKey /ifempty HKCU "${MANUKEY}"

    SetShellVarContext current
    RmDir /r "$APPDATA\${BUNDLEID}"
    RmDir /r "$LOCALAPPDATA\${BUNDLEID}"
  ${EndIf}

  !ifmacrodef NSIS_HOOK_POSTUNINSTALL
    Call un.FBForgeRequireMutationEvidence
    !insertmacro NSIS_HOOK_POSTUNINSTALL
  !endif

  ; Auto close if passive mode or updating
  ${If} $PassiveMode = 1
  ${OrIf} $UpdateMode = 1
    SetAutoClose true
  ${EndIf}
  Call un.FBForgeRelease
SectionEnd

Function RestorePreviousInstallLocation
  ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
  StrCmp $4 "" +2 0
    StrCpy $INSTDIR $4
FunctionEnd

Function Skip
  Abort
FunctionEnd

Function SkipIfPassive
  ${IfThen} $PassiveMode = 1  ${|} Abort ${|}
FunctionEnd
Function un.SkipIfPassive
  ${IfThen} $PassiveMode = 1  ${|} Abort ${|}
FunctionEnd

Function CreateOrUpdateStartMenuShortcut
  Call FBForgeRequirePublishedV2
  ; We used to use product name as MAINBINARYNAME
  ; migrate old shortcuts to target the new MAINBINARYNAME
  StrCpy $R0 0

  !insertmacro IsShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
  Pop $0
  !insertmacro FBForgeCheckWrite
  ${If} $0 = 1
    !insertmacro SetShortcutTarget "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro FBForgeCheckWrite
    StrCpy $R0 1
  ${EndIf}

  !insertmacro IsShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
  Pop $0
  !insertmacro FBForgeCheckWrite
  ${If} $0 = 1
    !insertmacro SetShortcutTarget "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro FBForgeCheckWrite
    StrCpy $R0 1
  ${EndIf}

  ${If} $R0 = 1
    Return
  ${EndIf}

  ; Historical WiX migration is not delegated by this template.
  ${If} $UpdateMode = 1
  ${OrIf} $NoShortcutMode = 1
    Return
  ${EndIf}

  !if "${STARTMENUFOLDER}" != ""
    CreateDirectory "$SMPROGRAMS\$AppStartMenuFolder"
    !insertmacro FBForgeCheckWrite
    !insertmacro FBForgeCreateShortcut "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  !else
    !insertmacro FBForgeCreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  !endif
FunctionEnd

Function CreateOrUpdateDesktopShortcut
  ${If} $FBForgeShortcutsFinalized = 1
    Return
  ${EndIf}
  Call FBForgeRequirePublishedV2
  ; We used to use product name as MAINBINARYNAME
  ; migrate old shortcuts to target the new MAINBINARYNAME
  !insertmacro IsShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\$OldMainBinaryName"
  Pop $0
  !insertmacro FBForgeCheckWrite
  ${If} $0 = 1
    !insertmacro SetShortcutTarget "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro FBForgeCheckWrite
    Return
  ${EndIf}

  ; Historical WiX migration is not delegated by this template.
  ${If} $UpdateMode = 1
  ${OrIf} $NoShortcutMode = 1
    Return
  ${EndIf}

  !insertmacro FBForgeCreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
FunctionEnd

; Runtime bytes stay data in the private plugin directory.
Function FBForgeExtractPayload
  ClearErrors
  InitPluginsDir
  !insertmacro FBForgeCheckWrite
  SetOutPath "$PLUGINSDIR"
  !insertmacro FBForgeCheckWrite
  File /oname=fbforge-payload.v1.json "${FBFORGE_PAYLOAD_FILE}"
  !insertmacro FBForgeCheckWrite
FunctionEnd
!delfile "${FBFORGE_PAYLOAD_FILE}"
