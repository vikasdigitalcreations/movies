; MovieBox installer hooks (Tauri NSIS)

!macro NSIS_HOOK_POSTINSTALL
  ; Start-menu shortcut to the one-page guide
  CreateShortCut "$SMPROGRAMS\MovieBox - Getting Started.lnk" "$INSTDIR\docs\Getting Started.html"
  ; Open the guide once, right after a normal (non-silent) install
  IfSilent +2
    ExecShell "open" "$INSTDIR\docs\Getting Started.html"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$SMPROGRAMS\MovieBox - Getting Started.lnk"
!macroend
