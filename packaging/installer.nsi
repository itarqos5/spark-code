; SPDX-License-Identifier: MIT
; The packaging script supplies VERSION, OUTPUT_FILE and PAYLOAD_INCLUDE.
Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "WinVer.nsh"
!include "x64.nsh"
!include "WinMessages.nsh"
!include "Win\COM.nsh"
!include "Win\Propkey.nsh"
!include "${PAYLOAD_INCLUDE}"

Name "Spark Code"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\spark-code"
InstallDirRegKey HKCU "Software\spark-code" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 32
BrandingText "Spark Code ${VERSION}"
ShowInstDetails show
ShowUninstDetails show
VIProductVersion "${VERSION_QUAD}"
VIAddVersionKey /LANG=1033 "ProductName" "Spark Code"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "FileDescription" "Spark Code per-user setup"
VIAddVersionKey /LANG=1033 "LegalCopyright" "MIT-licensed Spark Code contributors"

!define MUI_ICON "${APP_ICON}"
!define MUI_UNICON "${APP_ICON}"
!define SPARK_APP_ID "SparkCode.Desktop"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "Install Spark Code for your Windows user account.$\r$\n$\r$\nThis installer contains a native desktop app and terminal app. Provider CLIs and account sign-in are separate and are never installed or started automatically.$\r$\n$\r$\nClose Spark Code before installing. Windows 10/11 x64 is the intended platform; see INSTALL.md for tested-build limitations."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TEXT "Spark Code has been installed.$\r$\n$\r$\nOpen it from the Start menu. If you selected PATH integration, open a new terminal and run spark-code for the terminal UI, or spark-code gui for the desktop UI.$\r$\n$\r$\nExisting terminals may need to be fully closed and reopened."
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
    SetShellVarContext current
    ${IfNot} ${RunningX64}
        MessageBox MB_OK|MB_ICONSTOP "Spark Code requires 64-bit Windows."
        Abort
    ${EndIf}
    ${IfNot} ${AtLeastWin10}
        MessageBox MB_OK|MB_ICONSTOP "Spark Code requires Windows 10 or later."
        Abort
    ${EndIf}
    SetRegView 64
FunctionEnd

; The stable ID matches the native process. This uses only stock NSIS helpers
; and Windows COM. Metadata failures leave the working shortcut in place.
Function SetShortcutAppId
    Exch $0
    Push $1
    Push $2
    Push $3
    Push $4
    Push $5
    Push $6
    Push $7
    Push $8
    StrCpy $1 0
    StrCpy $2 0
    StrCpy $3 0
    StrCpy $4 0
    StrCpy $5 0
    StrCpy $6 0
    System::Call 'ole32::CoInitializeEx(p0,i2)i.r8'
    !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ShellLink} ${IID_IShellLink} r1 ".r7"
    ${If} $7 != 0
        Goto shortcut_metadata_failed
    ${EndIf}
    ${IUnknown::QueryInterface} $1 '("${IID_IPersistFile}",.r2).r7'
    ${If} $7 != 0
        Goto shortcut_metadata_failed
    ${EndIf}
    ${IPersistFile::Load} $2 '("$0",${STGM_READWRITE}).r7'
    ${If} $7 != 0
        Goto shortcut_metadata_failed
    ${EndIf}
    ${IUnknown::QueryInterface} $1 '("${IID_IPropertyStore}",.r3).r7'
    ${If} $7 != 0
        Goto shortcut_metadata_failed
    ${EndIf}
    System::Call '*${SYSSTRUCT_PROPERTYKEY}(${PKEY_AppUserModel_ID})p.r4'
    ${If} $4 P= 0
        StrCpy $7 -2147024882 ; E_OUTOFMEMORY
        Goto shortcut_metadata_failed
    ${EndIf}
    System::Call 'shlwapi::SHStrDupW(w"${SPARK_APP_ID}",*p.r6)i.r7'
    ${If} $7 != 0
        Goto shortcut_metadata_failed
    ${EndIf}
    System::Call '*${SYSSTRUCT_PROPVARIANT}(${VT_LPWSTR},,)p.r5'
    ${If} $5 P= 0
        StrCpy $7 -2147024882
        Goto shortcut_metadata_failed
    ${EndIf}
    ; The string pointer begins at byte 8 on both supported NSIS pointer widths.
    ; Write only the union; do not overwrite the already-initialized variant type.
    IntOp $7 $5 + 8
    System::Call '*$7(p r6)'
    ${IPropertyStore::SetValue} $3 '($4,$5).r7'
    ${If} $7 != 0
        Goto shortcut_metadata_failed
    ${EndIf}
    ${IPropertyStore::Commit} $3 '.r7'
    ${If} $7 != 0
        Goto shortcut_metadata_failed
    ${EndIf}
    ${IPersistFile::Save} $2 '("$0",1).r7'
    ${If} $7 != 0
        Goto shortcut_metadata_failed
    ${EndIf}
    Goto shortcut_metadata_cleanup

shortcut_metadata_failed:
    DetailPrint "Spark Code shortcut created; optional taskbar identity metadata could not be set ($7)."
shortcut_metadata_cleanup:
    ; SHStrDupW uses the COM task allocator. IPropertyStore copied the value.
    ${If} $6 P<> 0
        System::Call 'ole32::CoTaskMemFree(p r6)'
    ${EndIf}
    ${If} $5 P<> 0
        System::Free $5
    ${EndIf}
    ${If} $4 P<> 0
        System::Free $4
    ${EndIf}
    !insertmacro ComHlpr_SafeRelease $3
    !insertmacro ComHlpr_SafeRelease $2
    !insertmacro ComHlpr_SafeRelease $1
    ${If} $8 >= 0
        System::Call 'ole32::CoUninitialize()'
    ${EndIf}
    Pop $8
    Pop $7
    Pop $6
    Pop $5
    Pop $4
    Pop $3
    Pop $2
    Pop $1
    Pop $0
FunctionEnd

Section "Spark Code applications (required)" MainSection
    SectionIn RO
    SetShellVarContext current
    SetRegView 64
    SetOverwrite on
    !insertmacro InstallPayload
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    WriteRegStr HKCU "Software\spark-code" "InstallDir" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "DisplayName" "Spark Code"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "Publisher" "Spark Code contributors"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "DisplayIcon" "$INSTDIR\spark-code-desktop.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "URLInfoAbout" "https://github.com/itarqos5/spark-code"
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "NoRepair" 1
    ; Remove only shortcuts created by the pre-branding installer, when upgrading.
    Delete "$SMPROGRAMS\spark-code\spark-code.lnk"
    Delete "$SMPROGRAMS\spark-code\Uninstall spark-code.lnk"
    RMDir "$SMPROGRAMS\spark-code"
    CreateDirectory "$SMPROGRAMS\Spark Code"
    CreateShortcut "$SMPROGRAMS\Spark Code\Spark Code.lnk" "$INSTDIR\spark-code-desktop.exe" "" "$INSTDIR\spark-code-desktop.exe" 0
    Push "$SMPROGRAMS\Spark Code\Spark Code.lnk"
    Call SetShortcutAppId
    CreateShortcut "$SMPROGRAMS\Spark Code\Uninstall Spark Code.lnk" "$INSTDIR\Uninstall.exe" "" "$INSTDIR\Uninstall.exe" 0
SectionEnd

Section "Add spark-code to my PATH" PathSection
    nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\manage-user-path.ps1" -Action Add -InstallDir "$INSTDIR"'
    Pop $0
    ${If} $0 != 0
        MessageBox MB_OK|MB_ICONEXCLAMATION "Spark Code was installed, but PATH integration failed. See the installer details. You can still launch it from the Start menu."
    ${Else}
        SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
    ${EndIf}
SectionEnd

Section "Desktop shortcut" DesktopSection
    Delete "$DESKTOP\spark-code.lnk"
    CreateShortcut "$DESKTOP\Spark Code.lnk" "$INSTDIR\spark-code-desktop.exe" "" "$INSTDIR\spark-code-desktop.exe" 0
    Push "$DESKTOP\Spark Code.lnk"
    Call SetShortcutAppId
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
    !insertmacro MUI_DESCRIPTION_TEXT ${MainSection} "Install the desktop and terminal apps for this user, plus a Start-menu shortcut. No administrator access is requested."
    !insertmacro MUI_DESCRIPTION_TEXT ${PathSection} "Enable the exact spark-code command in newly opened terminals. Only your user PATH is changed."
    !insertmacro MUI_DESCRIPTION_TEXT ${DesktopSection} "Add a desktop shortcut that opens the graphical app."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
    SetShellVarContext current
    SetRegView 64
    ; Abort before deleting files if owned PATH cleanup cannot be completed.
    nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\manage-user-path.ps1" -Action Remove -InstallDir "$INSTDIR"'
    Pop $0
    ${If} $0 != 0
        MessageBox MB_OK|MB_ICONSTOP "PATH cleanup failed. Nothing has been removed. Close Spark Code and retry, or use the manual instructions in INSTALL.md."
        Abort
    ${EndIf}
    SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
    Delete "$DESKTOP\Spark Code.lnk"
    Delete "$SMPROGRAMS\Spark Code\Spark Code.lnk"
    Delete "$SMPROGRAMS\Spark Code\Uninstall Spark Code.lnk"
    RMDir "$SMPROGRAMS\Spark Code"
    ; Also clean only the exact known legacy shortcuts; never recurse.
    Delete "$DESKTOP\spark-code.lnk"
    Delete "$SMPROGRAMS\spark-code\spark-code.lnk"
    Delete "$SMPROGRAMS\spark-code\Uninstall spark-code.lnk"
    RMDir "$SMPROGRAMS\spark-code"
    !insertmacro UninstallPayload
    Delete "$INSTDIR\Uninstall.exe"
    ; Non-recursive: never delete project files or user-created files.
    RMDir "$INSTDIR"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code"
    DeleteRegValue HKCU "Software\spark-code" "InstallDir"
    DeleteRegKey /ifempty HKCU "Software\spark-code"
    ; Deliberately preserve %LOCALAPPDATA%\spark-code and all provider settings.
SectionEnd
