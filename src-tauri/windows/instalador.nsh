; Pasos propios del instalador de Parabellum (tauri.conf.json › bundle.windows.nsis.installerHooks).
; La marca «instalado.txt» junto al exe hace que Parabellum guarde sus datos en %APPDATA%\Parabellum
; (plataforma/windows/mod.rs › data_root): desinstalar no se lleva instancias ni mundos.

!macro NSIS_HOOK_POSTINSTALL
  FileOpen $0 "$INSTDIR\instalado.txt" w
  FileWrite $0 "Parabellum instalado con su instalador.$\r$\n"
  FileWrite $0 "Tus instancias, mundos y ajustes estan en %APPDATA%\Parabellum: desinstalar no los borra.$\r$\n"
  FileWrite $0 "$\r$\nParabellum installed with its installer.$\r$\n"
  FileWrite $0 "Your instances, worlds and settings are in %APPDATA%\Parabellum: uninstalling does not delete them.$\r$\n"
  FileClose $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Lo que no puso el instalador: la marca y los exe viejos que deja la actualización automática.
  Delete "$INSTDIR\instalado.txt"
  Delete "$INSTDIR\Parabellum.exe.old*"
  Delete "$INSTDIR\Parabellum.nuevo.exe"
!macroend
