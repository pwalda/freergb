; SPDX-License-Identifier: GPL-2.0-or-later
; freeRGB installer hooks for Tauri's NSIS template. freergb-setup.ps1 does the work.

; The installer is 32-bit: plain powershell.exe would be the 32-bit one, which sees the
; 32-bit registry view. Sysnative reaches the real System32.
!define FREERGB_POWERSHELL '$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe'
!define FREERGB_PS '"${FREERGB_POWERSHELL}" -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\freergb-setup.ps1"'
; Inline, because before files are copied the new script is not there yet. The services run
; files from this folder, so they stop before those are replaced.
!define FREERGB_STOP_SERVICES `"${FREERGB_POWERSHELL}" -NoProfile -ExecutionPolicy Bypass -Command "Stop-Service -Name freeRGB-OpenRGB, freeRGB-Sensors -Force -ErrorAction SilentlyContinue; Get-Process OpenRGB -ErrorAction SilentlyContinue | Where-Object { $$_.Path -eq '$INSTDIR\openrgb\OpenRGB.exe' } | Stop-Process -Force"`

!macro NSIS_HOOK_PREINSTALL
  ; OpenRGB locks its files while it runs. Close the app first, since it would start
  ; OpenRGB again, then stop the service and any OpenRGB started from this folder.
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  nsExec::ExecToLog `${FREERGB_STOP_SERVICES}`
  Pop $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Questions only when someone is watching; updates keep what was chosen before.
  ${If} $UpdateMode <> 1
  ${AndIf} $PassiveMode <> 1
  ${AndIfNot} ${Silent}
    ; Before the service step, which defers to another OpenRGB's service.
    nsExec::ExecToStack '${FREERGB_PS} find-openrgb'
    Pop $0
    Pop $1
    ${If} $0 = 0
      MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "OpenRGB is already installed on this PC (version $1).$\r$\n$\r$\nfreeRGB includes OpenRGB 1.1. Replace the existing OpenRGB with it?$\r$\n$\r$\nYes uninstalls the existing OpenRGB, including its app and shortcuts. Uninstalling freeRGB later does not bring it back.$\r$\nNo keeps it, and freeRGB uses it." IDNO freergb_keep_openrgb
      DetailPrint "Uninstalling the existing OpenRGB"
      nsExec::ExecToLog '${FREERGB_PS} remove-openrgb'
      Pop $0
      ${If} $0 <> 0
        MessageBox MB_ICONEXCLAMATION "The existing OpenRGB could not be uninstalled (code $0). freeRGB will use it. You can uninstall it from Settings > Apps and run this installer again."
      ${EndIf}
      freergb_keep_openrgb:
    ${EndIf}
  ${EndIf}

  DetailPrint "Starting OpenRGB as a service"
  nsExec::ExecToLog '${FREERGB_PS} install-service'
  Pop $0
  ${If} $0 <> 0
    DetailPrint "The OpenRGB service could not be set up (code $0). freeRGB will start OpenRGB itself."
  ${EndIf}

  DetailPrint "Starting the sensor service"
  nsExec::ExecToLog '${FREERGB_PS} install-sensors'
  Pop $0
  ${If} $0 <> 0
    DetailPrint "The sensor service could not be set up (code $0). CPU temperature looks will need LibreHardwareMonitor."
  ${EndIf}

  ${If} $UpdateMode <> 1
  ${AndIf} $PassiveMode <> 1
  ${AndIfNot} ${Silent}
    nsExec::ExecToStack '${FREERGB_PS} find-pawnio'
    Pop $0
    Pop $1
    ${If} $0 = 0
      MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "PawnIO is already installed on this PC (version $1).$\r$\n$\r$\nReinstall it with PawnIO 2.2.0, downloaded from PawnIO's official release?$\r$\n$\r$\nUsually not needed. Choose Yes if RGB memory or the motherboard does not show up in freeRGB." IDNO freergb_skip_pawnio
      DetailPrint "Reinstalling PawnIO"
      nsExec::ExecToLog '${FREERGB_PS} install-pawnio -Reinstall'
    ${Else}
      MessageBox MB_YESNO|MB_ICONQUESTION "Install PawnIO as well?$\r$\n$\r$\nOpenRGB needs the PawnIO driver to light RGB memory and some motherboards. It is downloaded from PawnIO's official release." IDNO freergb_skip_pawnio
      DetailPrint "Installing PawnIO"
      nsExec::ExecToLog '${FREERGB_PS} install-pawnio'
    ${EndIf}
    Pop $0
    ${If} $0 <> 0
      MessageBox MB_ICONEXCLAMATION "PawnIO could not be installed (code $0). You can install it later from https://pawnio.eu."
    ${EndIf}
    freergb_skip_pawnio:
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro CheckIfAppIsRunning "$INSTDIR\${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  ${If} $UpdateMode = 1
    nsExec::ExecToLog `${FREERGB_STOP_SERVICES}`
    Pop $0
  ${Else}
    nsExec::ExecToLog '${FREERGB_PS} remove-service'
    Pop $0
    nsExec::ExecToLog '${FREERGB_PS} remove-sensors'
    Pop $0
  ${EndIf}
!macroend
