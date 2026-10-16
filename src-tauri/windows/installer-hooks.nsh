; Current-user sign-in option. Never changes task/configuration databases.
!include "nsDialogs.nsh"
Var FolderSyncStartupBox
Var FolderSyncStartupState

Page custom FolderSyncStartupPage FolderSyncStartupLeave

Function FolderSyncStartupPage
  IfSilent 0 +2
    Abort
  !insertmacro MUI_HEADER_TEXT "FolderSync startup" "Optional startup for your Windows account"
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}
  ${NSD_CreateLabel} 0 0 100% 60u "FolderSync installs for the current Windows user only. Starting at sign-in opens the saved configuration in the tray. This option does not enable any task, monitoring, schedule, or automatic deletion. You can change it later in Settings."
  Pop $0
  ${NSD_CreateCheckbox} 0 70u 100% 24u "Start FolderSync in the tray when I sign in"
  Pop $FolderSyncStartupBox
  ${NSD_Uncheck} $FolderSyncStartupBox
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "FolderSync"
  ${If} $0 != ""
    ${NSD_Check} $FolderSyncStartupBox
  ${EndIf}
  nsDialogs::Show
FunctionEnd

Function FolderSyncStartupLeave
  ${NSD_GetState} $FolderSyncStartupBox $FolderSyncStartupState
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/STARTUP=" $1
  ${IfNot} ${Errors}
    ${If} $1 == "1"
      StrCpy $FolderSyncStartupState 1
    ${ElseIf} $1 == "0"
      StrCpy $FolderSyncStartupState 0
    ${Else}
      SetErrorLevel 2
      Quit
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $FolderSyncStartupState == ""
    ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "FolderSync"
    ${If} $0 != ""
      StrCpy $FolderSyncStartupState 1
    ${EndIf}
  ${EndIf}
  ${If} $FolderSyncStartupState == 1
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "FolderSync" '$\"$INSTDIR\${MAINBINARYNAME}.exe$\" --background'
  ${ElseIf} $FolderSyncStartupState == 0
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "FolderSync"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode != 1
  ${AndIf} $DeleteAppDataCheckboxState == 1
    ClearErrors
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --remove-local-data' $0
    ${If} ${Errors}
      MessageBox MB_ICONSTOP "FolderSync local data could not be removed safely. Close FolderSync and retry. Files in the synchronized folders are never removed by uninstall." /SD IDOK
      Abort
    ${ElseIf} $0 != 0
      MessageBox MB_ICONSTOP "FolderSync local data could not be removed safely. Close FolderSync and check the application data folders before retrying." /SD IDOK
      Abort
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode != 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "FolderSync"
  ${EndIf}
!macroend
