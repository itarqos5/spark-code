; SPDX-License-Identifier: MIT
; The packaging script supplies VERSION, OUTPUT_FILE and PAYLOAD_INCLUDE.
Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "WinVer.nsh"
!include "x64.nsh"
!include "WinMessages.nsh"
!include "${PAYLOAD_INCLUDE}"

Name "spark-code"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\spark-code"
InstallDirRegKey HKCU "Software\spark-code" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 32
BrandingText "spark-code ${VERSION}"
ShowInstDetails show
ShowUninstDetails show
VIProductVersion "${VERSION_QUAD}"
VIAddVersionKey /LANG=1033 "ProductName" "spark-code"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "FileDescription" "spark-code per-user setup"
VIAddVersionKey /LANG=1033 "LegalCopyright" "MIT-licensed spark-code contributors"

!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "Install spark-code for your Windows user account.$\r$\n$\r$\nThis installer contains a native desktop app and terminal app. Provider CLIs and account sign-in are separate and are never installed or started automatically.$\r$\n$\r$\nClose spark-code before installing. Windows 10/11 x64 is the intended platform; see INSTALL.md for tested-build limitations."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TEXT "spark-code has been installed.$\r$\n$\r$\nOpen it from the Start menu. If you selected PATH integration, open a new terminal and run spark-code for the terminal UI, or spark-code gui for the desktop UI.$\r$\n$\r$\nExisting terminals may need to be fully closed and reopened."
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
    SetShellVarContext current
    ${IfNot} ${RunningX64}
        MessageBox MB_OK|MB_ICONSTOP "spark-code requires 64-bit Windows."
        Abort
    ${EndIf}
    ${IfNot} ${AtLeastWin10}
        MessageBox MB_OK|MB_ICONSTOP "spark-code requires Windows 10 or later."
        Abort
    ${EndIf}
    SetRegView 64
FunctionEnd

Section "spark-code applications (required)" MainSection
    SectionIn RO
    SetShellVarContext current
    SetRegView 64
    SetOverwrite on
    !insertmacro InstallPayload
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    WriteRegStr HKCU "Software\spark-code" "InstallDir" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "DisplayName" "spark-code"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "Publisher" "spark-code contributors"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "DisplayIcon" "$INSTDIR\spark-code-desktop.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "URLInfoAbout" "https://github.com/itarqos5/spark-code"
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\spark-code" "NoRepair" 1
    CreateDirectory "$SMPROGRAMS\spark-code"
    CreateShortcut "$SMPROGRAMS\spark-code\spark-code.lnk" "$INSTDIR\spark-code-desktop.exe"
    CreateShortcut "$SMPROGRAMS\spark-code\Uninstall spark-code.lnk" "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Add spark-code to my PATH" PathSection
    nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\manage-user-path.ps1" -Action Add -InstallDir "$INSTDIR"'
    Pop $0
    ${If} $0 != 0
        MessageBox MB_OK|MB_ICONEXCLAMATION "spark-code was installed, but PATH integration failed. See the installer details. You can still launch it from the Start menu."
    ${Else}
        SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
    ${EndIf}
SectionEnd

Section "Desktop shortcut" DesktopSection
    CreateShortcut "$DESKTOP\spark-code.lnk" "$INSTDIR\spark-code-desktop.exe"
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
        MessageBox MB_OK|MB_ICONSTOP "PATH cleanup failed. Nothing has been removed. Close spark-code and retry, or use the manual instructions in INSTALL.md."
        Abort
    ${EndIf}
    SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
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
