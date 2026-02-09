; NSIS hooks for NexVPN installer
; Cleans up the actual app data directory on uninstall

!macro NSIS_HOOK_POSTUNINSTALL
  ; The built-in "Delete app data" checkbox uses $APPDATA\${BUNDLEID}
  ; but the app actually stores state in $APPDATA\nexvpn
  ; So we also clean that path when user opts to delete app data
  ${If} $DeleteAppDataCheckboxState == ${BST_CHECKED}
    RMDir /r "$APPDATA\nexvpn"
  ${EndIf}
!macroend
