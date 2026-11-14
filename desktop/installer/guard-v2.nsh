; Native V2 owns the actual target, private stage and owner-thread lease.
; These strings are receipts for the fixed signed template, not mutation grants.
!define FBFORGE_V2_MAX_POLLS 6000
Var FBForgeInstallFailed
Var FBForgeV2Started
Var FBForgeV2Phase
Var FBForgeV2Stage
Var FBForgeV2Target
Var FBForgeV2Components
Var FBForgeV2WaitMode
Var FBForgeV2PollCount
Var FBForgeV2Complete
Var FBForgeDesktopChoice
Var FBForgeDesktopCheckbox
Var FBForgeDesktopDialog

!macro FBForgeCheckWrite
  ${If} ${Errors}
    Call FBForgeInstallFailed
  ${EndIf}
!macroend

!macro FBForgeV2Functions
Function FBForgeInstallFailed
  StrCpy $FBForgeInstallFailed 1
  SetErrorLevel 70
  MessageBox MB_OK|MB_ICONSTOP "${PRODUCTNAME}: Der Vorgang konnte nicht sicher abgeschlossen werden. Die Anwendung wird nicht gestartet. Ausstehende Installation und Sicherung bleiben erhalten. Bitte schließen Sie die Anwendung selbst und versuchen Sie es erneut." /SD IDOK
  Quit
FunctionEnd

Function FBForgeV2ReadReply
  IfErrors denied
  Pop $FBForgeGuardReply
  IfErrors denied
  Pop $FBForgeGuardSentinel
  IfErrors denied
  StrCmpS $FBForgeGuardSentinel "fbforge-guard-v2-sentinel" 0 denied
  Return
  denied:
    Call FBForgeInstallFailed
FunctionEnd

Function FBForgePollV2
  StrCpy $FBForgeGuardReply ""
  Push "fbforge-guard-v2-sentinel"
  ClearErrors
  fbforge_suite_nsis_guard::PollV2 /NOUNLOAD
  Call FBForgeV2ReadReply
  StrCpy $FBForgeV2Phase $FBForgeGuardReply
FunctionEnd

; Called only from installation sections on NSIS's install worker. Each SDK
; call is nonblocking. The bounded wait never runs in a GUI page callback.
Function FBForgeWaitV2
  StrCpy $FBForgeV2PollCount 0
  again:
    IntOp $FBForgeV2PollCount $FBForgeV2PollCount + 1
    IntCmp $FBForgeV2PollCount ${FBFORGE_V2_MAX_POLLS} 0 0 denied
    Call FBForgePollV2
    StrCmpS $FBForgeV2WaitMode "prepare" prepare
    StrCmpS $FBForgeV2WaitMode "publish" publish
    StrCmpS $FBForgeV2WaitMode "finalize" finalize denied
  prepare:
    StrCmpS $FBForgeV2Phase "v2:preparing" pending
    StrCmpS $FBForgeV2Phase "v2:staging" ready
    StrCmpS $FBForgeV2Phase "v2:recoveryStaging" ready
    StrCmpS $FBForgeV2Phase "v2:publishedPending" ready
    StrCmpS $FBForgeV2Phase "v2:recoveryPublishedPending" ready denied
  publish:
    StrCmpS $FBForgeV2Phase "v2:publishing" pending
    StrCmpS $FBForgeV2Phase "v2:publishedPending" ready
    StrCmpS $FBForgeV2Phase "v2:recoveryPublishedPending" ready denied
  finalize:
    StrCmpS $FBForgeV2Phase "v2:finalizing" pending
    StrCmpS $FBForgeV2Phase "v2:complete" ready denied
  pending:
    Sleep 100
    Goto again
  ready:
    Return
  denied:
    Call FBForgeInstallFailed
FunctionEnd

Function FBForgeRequireStagingV2
  Call FBForgePollV2
  StrCmpS $FBForgeV2Phase "v2:staging" ready
  StrCmpS $FBForgeV2Phase "v2:recoveryStaging" ready
  Call FBForgeInstallFailed
  ready:
FunctionEnd

Function FBForgeRequirePublishedV2
  Call FBForgePollV2
  StrCmpS $FBForgeV2Phase "v2:publishedPending" ready
  StrCmpS $FBForgeV2Phase "v2:recoveryPublishedPending" ready
  Call FBForgeInstallFailed
  ready:
FunctionEnd

Function FBForgeTargetPathV2
  StrCpy $FBForgeGuardReply ""
  Push "fbforge-guard-v2-sentinel"
  ClearErrors
  fbforge_suite_nsis_guard::PathV2 /NOUNLOAD "target"
  Call FBForgeV2ReadReply
  StrCpy $R0 $FBForgeGuardReply 14
  StrCmpS $R0 "v2:targetPath:" 0 denied
  StrCpy $FBForgeV2Target $FBForgeGuardReply "" 14
  StrLen $R0 $FBForgeV2Target
  IntCmp $R0 4 0 denied
  StrCpy $R0 $FBForgeV2Target 1 1
  StrCmpS $R0 ":" 0 denied
  StrCpy $R0 $FBForgeV2Target 1 2
  StrCmpS $R0 "\" 0 denied
  Return
  denied:
    Call FBForgeInstallFailed
FunctionEnd

Function FBForgeStagePathV2
  Call FBForgeRequireStagingV2
  StrCpy $FBForgeGuardReply ""
  Push "fbforge-guard-v2-sentinel"
  ClearErrors
  fbforge_suite_nsis_guard::PathV2 /NOUNLOAD "staging"
  Call FBForgeV2ReadReply
  StrCpy $R0 $FBForgeGuardReply 15
  StrCmpS $R0 "v2:stagingPath:" 0 denied
  StrCpy $FBForgeV2Stage $FBForgeGuardReply "" 15
  StrLen $R0 $FBForgeV2Stage
  IntCmp $R0 4 0 denied
  StrCpy $R0 $FBForgeV2Stage 1 1
  StrCmpS $R0 ":" 0 denied
  StrCpy $R0 $FBForgeV2Stage 1 2
  StrCmpS $R0 "\" 0 denied
  Return
  denied:
    Call FBForgeInstallFailed
FunctionEnd

Function FBForgePrepareV2
  StrCmpS $FBForgeV2Started 1 denied
  StrCpy $FBForgeV2Started 1
  SetErrorLevel 70
  StrCpy $FBForgeGuardReply ""
  Push "fbforge-guard-v2-sentinel"
  ClearErrors
  fbforge_suite_nsis_guard::PrepareV2 /NOUNLOAD "${FBFORGE_GUARD_TOOL_ID}" "$PLUGINSDIR" "$FBForgeV2Components"
  Call FBForgeV2ReadReply
  StrCmpS $FBForgeGuardReply "v2:preparing" 0 denied
  StrCpy $FBForgeV2WaitMode "prepare"
  Call FBForgeWaitV2
  Call FBForgeTargetPathV2
  StrCmpS $FBForgeV2Phase "v2:staging" stage
  StrCmpS $FBForgeV2Phase "v2:recoveryStaging" stage done
  stage:
    Call FBForgeStagePathV2
  done:
    Return
  denied:
    Call FBForgeInstallFailed
FunctionEnd

Function FBForgePublishV2
  Call FBForgeRequireStagingV2
  ; A SetOutPath stage CWD would itself prevent the native directory rename.
  ClearErrors
  SetOutPath "$PLUGINSDIR"
  !insertmacro FBForgeCheckWrite
  Push "fbforge-guard-v2-sentinel"
  ClearErrors
  fbforge_suite_nsis_guard::PublishV2 /NOUNLOAD
  Call FBForgeV2ReadReply
  StrCmpS $FBForgeGuardReply "v2:publishing" 0 denied
  StrCpy $FBForgeV2WaitMode "publish"
  Call FBForgeWaitV2
  Call FBForgeTargetPathV2
  Return
  denied:
    Call FBForgeInstallFailed
FunctionEnd

Function FBForgeFinalizeV2
  Call FBForgeRequirePublishedV2
  ClearErrors
  SetOutPath "$PLUGINSDIR"
  !insertmacro FBForgeCheckWrite
  Push "fbforge-guard-v2-sentinel"
  ClearErrors
  fbforge_suite_nsis_guard::FinalizeV2 /NOUNLOAD
  Call FBForgeV2ReadReply
  StrCmpS $FBForgeGuardReply "v2:finalizing" 0 denied
  StrCpy $FBForgeV2WaitMode "finalize"
  Call FBForgeWaitV2
  StrCpy $FBForgeV2Complete 1
  ; This exact fresh ACK follows native TargetPlan/Lease drop, never GUI unload.
  Push "fbforge-guard-v2-sentinel"
  ClearErrors
  fbforge_suite_nsis_guard::ReleaseV2 /NOUNLOAD
  Call FBForgeV2ReadReply
  StrCmpS $FBForgeGuardReply "v2:released" 0 denied
  StrCpy $FBForgeGuardReleased 1
  SetErrorLevel 0
  Return
  denied:
    Call FBForgeInstallFailed
FunctionEnd

Function FBForgeRequireCompletedV2
  StrCmpS $FBForgeInstallFailed 1 denied
  StrCmpS $FBForgeV2Complete 1 0 denied
  StrCmpS $FBForgeGuardReleased 1 0 denied
  Call FBForgePollV2
  StrCmpS $FBForgeV2Phase "v2:released" 0 denied
  Return
  denied:
    Call FBForgeInstallFailed
FunctionEnd
!macroend
