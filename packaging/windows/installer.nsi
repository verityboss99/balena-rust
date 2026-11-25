!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "2.1.9"
!endif
!ifndef ARCH
  !define ARCH "x64"
!endif

Name "balenaEtcher ${VERSION}"
OutFile "${OUT_FILE}"
RequestExecutionLevel admin
Unicode True
SetCompressor /SOLID lzma

!if "${ARCH}" == "x64"
  InstallDir "$PROGRAMFILES64\balenaEtcher"
  !define REG_VIEW 64
!else
  InstallDir "$PROGRAMFILES32\balenaEtcher"
  !define REG_VIEW 32
!endif

!define MUI_ICON "${ICON_PATH}"
!define MUI_UNICON "${ICON_PATH}"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${LICENSE_PATH}"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\balena-etcher.exe"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetRegView ${REG_VIEW}
  SetOutPath "$INSTDIR"
  File "${BIN_DIR}\balena-etcher.exe"
  File "${BIN_DIR}\etcher-util.exe"
  File "${LICENSE_PATH}"

  CreateDirectory "$SMPROGRAMS\balenaEtcher"
  CreateShortcut "$SMPROGRAMS\balenaEtcher\balenaEtcher.lnk" "$INSTDIR\balena-etcher.exe" "" "$INSTDIR\balena-etcher.exe" 0
  CreateShortcut "$SMPROGRAMS\balenaEtcher\Uninstall balenaEtcher.lnk" "$INSTDIR\Uninstall.exe"
  CreateShortcut "$DESKTOP\balenaEtcher.lnk" "$INSTDIR\balena-etcher.exe" "" "$INSTDIR\balena-etcher.exe" 0

  WriteUninstaller "$INSTDIR\Uninstall.exe"

  !define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\balenaEtcher"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "balenaEtcher"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "Balena Ltd."
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\balena-etcher.exe"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" "$\"$INSTDIR\Uninstall.exe$\""
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetRegView ${REG_VIEW}
  Delete "$DESKTOP\balenaEtcher.lnk"
  RMDir /r "$SMPROGRAMS\balenaEtcher"
  Delete "$INSTDIR\balena-etcher.exe"
  Delete "$INSTDIR\etcher-util.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\balenaEtcher"
SectionEnd
