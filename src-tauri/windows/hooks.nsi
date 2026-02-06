; SmartlabStudent NSIS Installer Hooks
; 1. HKLM Registry Run key (system-wide autostart)
; 2. Windows Scheduled Task (runs at logon, elevated)
; 3. Windows Firewall exception
; 4. SmartlabService (Windows Service - runs at boot, before login)
;
; Available Tauri NSIS variables:
;   $INSTDIR          - Installation directory
;   ${PRODUCTNAME}    - Product name from tauri.conf.json
;   ${MAINBINARYNAME} - The actual binary filename

!macro NSIS_HOOK_POSTINSTALL
  ; ============================================================
  ; 1. Registry autostart (HKLM for all users)
  ; ============================================================
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}" '"$INSTDIR\${MAINBINARYNAME}.exe"'
  DetailPrint "Added ${PRODUCTNAME} to system startup (HKLM)"

  ; ============================================================
  ; 2. Scheduled Task (runs at logon, elevated)
  ; ============================================================
  nsExec::ExecToLog 'schtasks /Delete /TN "${PRODUCTNAME}" /F'
  nsExec::ExecToLog 'schtasks /Create /TN "${PRODUCTNAME}" /TR "\"$INSTDIR\${MAINBINARYNAME}.exe\"" /SC ONLOGON /RL HIGHEST /F'
  Pop $0
  StrCmp $0 "0" task_ok
    nsExec::ExecToLog 'schtasks /Create /TN "${PRODUCTNAME}" /TR "\"$INSTDIR\${MAINBINARYNAME}.exe\"" /SC ONLOGON /F'
    DetailPrint "Created scheduled task (normal)"
    Goto task_done
  task_ok:
    DetailPrint "Created scheduled task (elevated)"
  task_done:

  ; ============================================================
  ; 3. Windows Firewall exception
  ; ============================================================
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${PRODUCTNAME}"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${PRODUCTNAME}" dir=in action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" enable=yes profile=any'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${PRODUCTNAME} Out" dir=out action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" enable=yes profile=any'

  ; Firewall for the service too
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="SmartlabService"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="SmartlabService" dir=in action=allow program="$INSTDIR\resources\smartlab-service.exe" enable=yes profile=any'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="SmartlabService Out" dir=out action=allow program="$INSTDIR\resources\smartlab-service.exe" enable=yes profile=any'
  DetailPrint "Added Windows Firewall exceptions"

  ; ============================================================
  ; 4. Install and start SmartlabService (Windows Service)
  ;    Runs at boot level, allows teacher to connect before login
  ; ============================================================

  ; Stop and remove old service if exists
  nsExec::ExecToLog '"$INSTDIR\resources\smartlab-service.exe" --uninstall'

  ; Install the service
  nsExec::ExecToLog '"$INSTDIR\resources\smartlab-service.exe" --install'
  Pop $0
  StrCmp $0 "0" svc_installed
    DetailPrint "Warning: Could not install SmartlabService"
    Goto svc_done
  svc_installed:
    DetailPrint "SmartlabService installed"

    ; Start the service immediately
    nsExec::ExecToLog 'sc start SmartlabService'
    DetailPrint "SmartlabService started"
  svc_done:

  DetailPrint "${PRODUCTNAME} installation complete"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Kill running app
  nsExec::ExecToLog 'taskkill /F /IM ${MAINBINARYNAME}.exe'

  ; Stop and uninstall the service
  nsExec::ExecToLog 'sc stop SmartlabService'
  ; Wait for service to stop
  Sleep 2000
  nsExec::ExecToLog '"$INSTDIR\resources\smartlab-service.exe" --uninstall'
  DetailPrint "SmartlabService uninstalled"

  ; Remove registry keys
  DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"

  ; Remove scheduled task
  nsExec::ExecToLog 'schtasks /Delete /TN "${PRODUCTNAME}" /F'

  ; Remove firewall rules
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${PRODUCTNAME}"'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${PRODUCTNAME} Out"'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="SmartlabService"'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="SmartlabService Out"'

  DetailPrint "${PRODUCTNAME} uninstall cleanup complete"
!macroend
