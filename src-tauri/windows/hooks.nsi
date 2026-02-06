; SmartlabStudent NSIS Installer Hooks
; Implements Veyon/NetSupport-style autostart:
; 1. HKLM Registry Run key (system-wide)
; 2. Windows Scheduled Task (runs at logon, auto-restart)
; 3. Windows Firewall exception

!macro NSIS_HOOK_POSTINSTALL
  ; ============================================================
  ; 1. Registry autostart (HKLM for all users)
  ; ============================================================
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "SmartlabStudent" '"$INSTDIR\SmartlabStudent.exe"'
  DetailPrint "Added SmartlabStudent to system startup (HKLM)"

  ; ============================================================
  ; 2. Scheduled Task (like Veyon - runs at logon, auto-restart)
  ; ============================================================
  
  ; Delete existing task first (ignore errors)
  nsExec::ExecToLog 'schtasks /Delete /TN "SmartlabStudent" /F'
  
  ; Create scheduled task that runs at user logon with highest privileges
  nsExec::ExecToLog 'schtasks /Create /TN "SmartlabStudent" /TR "\"$INSTDIR\SmartlabStudent.exe\"" /SC ONLOGON /RL HIGHEST /F'
  Pop $0
  StrCmp $0 "0" task_ok
    ; Fallback: create task without HIGHEST privilege
    nsExec::ExecToLog 'schtasks /Create /TN "SmartlabStudent" /TR "\"$INSTDIR\SmartlabStudent.exe\"" /SC ONLOGON /F'
    DetailPrint "Created scheduled task for auto-start (normal)"
    Goto task_done
  task_ok:
    DetailPrint "Created scheduled task for auto-start (elevated)"
  task_done:

  ; ============================================================
  ; 3. Windows Firewall exception (like Veyon/NetSupport)
  ; ============================================================
  
  ; Remove old rules first
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="SmartlabStudent"'
  
  ; Add inbound firewall rule
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="SmartlabStudent" dir=in action=allow program="$INSTDIR\SmartlabStudent.exe" enable=yes profile=any'
  
  ; Add outbound firewall rule
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="SmartlabStudent Out" dir=out action=allow program="$INSTDIR\SmartlabStudent.exe" enable=yes profile=any'
  
  DetailPrint "Added Windows Firewall exceptions"
  DetailPrint "SmartlabStudent installation complete - auto-start configured"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Kill running process before uninstall
  nsExec::ExecToLog 'taskkill /F /IM SmartlabStudent.exe'
  
  ; Remove registry keys
  DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "SmartlabStudent"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "SmartlabStudent"
  DetailPrint "Removed SmartlabStudent from startup registry"
  
  ; Remove scheduled task
  nsExec::ExecToLog 'schtasks /Delete /TN "SmartlabStudent" /F'
  DetailPrint "Removed SmartlabStudent scheduled task"
  
  ; Remove firewall rules
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="SmartlabStudent"'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="SmartlabStudent Out"'
  DetailPrint "Removed SmartlabStudent firewall rules"
  
  DetailPrint "SmartlabStudent uninstall cleanup complete"
!macroend
