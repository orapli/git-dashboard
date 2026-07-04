; Git Dashboard - Windows installer (NSIS)
; Built on macOS/Linux with `makensis` or on Windows with NSIS 3.x.
; Invoked by scripts/build-windows-installer.sh with:
;   makensis -DAPP_VERSION=<ver> -DEXE_PATH=<path to git-dashboard.exe> installer.nsi

Unicode true
SetCompressor /SOLID lzma

!ifndef APP_VERSION
  !define APP_VERSION "0.0.0"
!endif
!ifndef EXE_PATH
  !define EXE_PATH "..\..\target\x86_64-pc-windows-gnu\release\git-dashboard.exe"
!endif

!define APP_NAME "Git Dashboard"
!define APP_EXE "git-dashboard.exe"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\GitDashboard"

!ifndef OUT_FILE
  !define OUT_FILE "GitDashboard-${APP_VERSION}-setup.exe"
!endif

Name "${APP_NAME} ${APP_VERSION}"
OutFile "${OUT_FILE}"
InstallDir "$PROGRAMFILES64\${APP_NAME}"
InstallDirRegKey HKLM "${UNINST_KEY}" "InstallLocation"
RequestExecutionLevel admin
ShowInstDetails show

!include "MUI2.nsh"
!define MUI_ICON "..\..\assets\icon.ico"
!define MUI_UNICON "..\..\assets\icon.ico"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Launch Git Dashboard"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Japanese"

Section "Install"
  SetOutPath "$INSTDIR"
  File "/oname=${APP_EXE}" "${EXE_PATH}"

  ; Start menu / desktop shortcuts
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortCut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
  CreateShortCut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  ; Uninstaller + Add/Remove Programs entry
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  DeleteRegKey HKLM "${UNINST_KEY}"
  ; Note: user settings (%APPDATA%) are intentionally left in place
SectionEnd
