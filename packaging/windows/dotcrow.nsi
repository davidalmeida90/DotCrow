; Instalador do DotCrow para Windows (NSIS).
; Compilado pelo workflow de release a partir de um dotcrow.exe já pronto:
;   makensis -DVERSION=0.1.0 packaging/windows/dotcrow.nsi
; Precisa do plugin EnVar (para editar o PATH); o workflow instala.

Unicode True
!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif

Name "DotCrow ${VERSION}"
OutFile "dotcrow-setup.exe"
InstallDir "$PROGRAMFILES64\DotCrow"
InstallDirRegKey HKLM "Software\DotCrow" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "DotCrow"
VIAddVersionKey "FileDescription" "Instalador do DotCrow"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Apache-2.0"
VIAddVersionKey "CompanyName" "Napoleão de Sousa Carvalho Junior"

!define MUI_ICON "dotcrow.ico"
!define MUI_UNICON "dotcrow.ico"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\DotCrow"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$SYSDIR\cmd.exe"
!define MUI_FINISHPAGE_RUN_PARAMETERS '/k dotcrow painel'
!define MUI_FINISHPAGE_RUN_TEXT "Abrir o painel do DotCrow agora"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "PortugueseBR"
!insertmacro MUI_LANGUAGE "English"

Section "DotCrow"
  SetOutPath "$INSTDIR"
  File "dotcrow.exe"
  File "dotcrow.ico"

  ; Coloca o DotCrow no PATH do sistema (precisa do plugin EnVar).
  EnVar::SetHKLM
  EnVar::AddValue "Path" "$INSTDIR"

  CreateDirectory "$SMPROGRAMS\DotCrow"
  CreateShortcut "$SMPROGRAMS\DotCrow\DotCrow (painel).lnk" "$SYSDIR\cmd.exe" "/k dotcrow painel" "$INSTDIR\dotcrow.ico"

  WriteRegStr HKLM "Software\DotCrow" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "DotCrow"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\dotcrow.ico"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "Napoleão de Sousa Carvalho Junior"
  WriteRegStr HKLM "${UNINST_KEY}" "URLInfoAbout" "https://dotcrow.si"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  EnVar::SetHKLM
  EnVar::DeleteValue "Path" "$INSTDIR"

  Delete "$INSTDIR\dotcrow.exe"
  Delete "$INSTDIR\dotcrow.ico"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\DotCrow\DotCrow (painel).lnk"
  RMDir "$SMPROGRAMS\DotCrow"

  DeleteRegKey HKLM "${UNINST_KEY}"
  DeleteRegKey HKLM "Software\DotCrow"
SectionEnd
