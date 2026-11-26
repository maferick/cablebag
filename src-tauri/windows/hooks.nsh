; Cablebag installer hooks.
!macro NSIS_HOOK_PREINSTALL
  ; The installer asks "only for me" or "for everyone on this PC", and defaults to everyone on an account with
  ; administrator rights unless the registry says an earlier copy was for one user. Copies installed before the
  ; installer asked (up to 0.21.4) lack that note, so an update would put a second copy in Program Files. An
  ; update (or passive install) therefore stays for this user when only a copy for this user is found.
  StrCpy $R9 0
  ${If} $PassiveMode = 1
  ${OrIf} $UpdateMode = 1
    StrCpy $R9 1
  ${EndIf}
  ${If} $R9 = 1
  ${AndIf} $MultiUser.InstallMode == "AllUsers"
    ReadRegStr $R0 HKCU "${UNINSTKEY}" "InstallLocation"
    ReadRegStr $R1 HKLM "${UNINSTKEY}" "InstallLocation"
    ${If} $R0 != ""
    ${AndIf} $R1 == ""
      RMDir $INSTDIR
      Call MultiUser.InstallMode.CurrentUser
      SetOutPath $INSTDIR
    ${EndIf}
  ${EndIf}

  ; A Remote Desktop helper left running would keep cablebag-rdp.exe locked, so the installer stops this user's
  ; helpers before it copies the files (an update closes Cablebag itself first).
  nsExec::Exec 'taskkill /F /IM cablebag-rdp.exe /FI "USERNAME eq $%USERNAME%"'
  Pop $0
  Sleep 300
!macroend
