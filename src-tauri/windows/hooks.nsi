; SmartlabStudent NSIS Installer Hooks
; Implements Veyon/NetSupport-style autostart:
; 1. HKLM Registry Run key (system-wide)
; 2. Windows Scheduled Task (runs at logon, auto-restart)
; 3. Windows Firewall exception
;
; Available Tauri NSIS variables:
;   $INSTDIR          - Installation directory
;   ${PRODUCTNAME}    - Product name from tauri.conf.json
;   ${MAINBINARYNAME} - The actual binary filename (e.g. SmartlabStudent.exe)

!macro NSIS_HOOK_POSTINSTALL
  ; ============================================================
  ; 1. Registry autostart (HKLM for all users)
  ; ============================================================
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  DetailPrint "Added ${PRODUCTNAME} to system startup (HKLM)"

  ; ============================================================
  ; 2. Scheduled Task (like Veyon - runs at logon, auto-restart)
  ; ============================================================
  
  ; Delete existing task first (ignore errors)
  nsExec::ExecToLog 'schtasks /Delete /TN "${PRODUCTNAME}" /F'
  
  ; Create scheduled task that runs at user logon with highest privileges
  nsExec::ExecToLog 'schtasks /Create /TN "${PRODUCTNAME}" /TR "\"$INSTDIR\${MAINBINARYNAME}.exe\"" /SC ONLOGON /RL HIGHEST /F'
  Pop $0
  StrCmp $0 "0" task_ok
    ; Fallback: create task without HIGHEST privilege
    nsExec::ExecToLog 'schtasks /Create /TN "${PRODUCTNAME}" /TR "\"$INSTDIR\${MAINBINARYNAME}.exe\"" /SC ONLOGON /F'
    DetailPrint "Created scheduled task for auto-start (normal)"
    Goto task_done
  task_ok:
    DetailPrint "Created scheduled task for auto-start (elevated)"
  task_done:

  ; ============================================================
  ; 3. Windows Firewall exception (like Veyon/NetSupport)
  ; ============================================================
  
  ; Remove old rules first
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${PRODUCTNAME}"'
  
  ; Add inbound firewall rule
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${PRODUCTNAME}" dir=in action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" enable=yes profile=any'
  
  ; Add outbound firewall rule
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${PRODUCTNAME} Out" dir=out action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" enable=yes profile=any'
  
  DetailPrint "Added Windows Firewall exceptions"
  DetailPrint "${PRODUCTNAME} installation complete - auto-start configured"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Kill running process before uninstall
  nsExec::ExecToLog 'taskkill /F /IM ${MAINBINARYNAME}.exe'
  
  ; Remove registry keys
  DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
  DetailPrint "Removed ${PRODUCTNAME} from startup registry"
  
  ; Remove scheduled task
  nsExec::ExecToLog 'schtasks /Delete /TN "${PRODUCTNAME}" /F'
  DetailPrint "Removed ${PRODUCTNAME} scheduled task"
  
  ; Remove firewall rules
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${PRODUCTNAME}"'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${PRODUCTNAME} Out"'
  DetailPrint "Removed ${PRODUCTNAME} firewall rules"
  
  DetailPrint "${PRODUCTNAME} uninstall cleanup complete"
!macroend
