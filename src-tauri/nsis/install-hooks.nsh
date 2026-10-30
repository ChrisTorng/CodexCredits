; Keep CodexCredits as the internal NSIS name to preserve the upgrade install directory
; and uninstall registry key. Only customize the user-visible display name.
!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "Codex Credits"
  !if "${STARTMENUFOLDER}" != ""
    ${If} ${FileExists} "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
      Delete "$SMPROGRAMS\$AppStartMenuFolder\Codex Credits.lnk"
      Rename "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk" "$SMPROGRAMS\$AppStartMenuFolder\Codex Credits.lnk"
    ${EndIf}
  !else
    ${If} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
      Delete "$SMPROGRAMS\Codex Credits.lnk"
      Rename "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$SMPROGRAMS\Codex Credits.lnk"
    ${EndIf}
  !endif
  ${If} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
    Delete "$DESKTOP\Codex Credits.lnk"
    Rename "$DESKTOP\${PRODUCTNAME}.lnk" "$DESKTOP\Codex Credits.lnk"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  !if "${STARTMENUFOLDER}" != ""
    Delete "$SMPROGRAMS\$AppStartMenuFolder\Codex Credits.lnk"
  !else
    Delete "$SMPROGRAMS\Codex Credits.lnk"
  !endif
  Delete "$DESKTOP\Codex Credits.lnk"
!macroend
