; Cablebag installer hooks. A Remote Desktop helper left running would keep cablebag-rdp.exe locked, so the
; installer stops this user's helpers before it copies the files (an update closes Cablebag itself first).
!macro NSIS_HOOK_PREINSTALL
  nsExec::Exec 'taskkill /F /IM cablebag-rdp.exe /FI "USERNAME eq $%USERNAME%"'
  Pop $0
  Sleep 300
!macroend
