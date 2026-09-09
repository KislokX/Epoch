; The shipped uninstall hook, run for real, against a throwaway tree.
;
; ## Why this exists
;
; `installer-hooks.nsh` deletes `%APPDATA%\Epoch` when the box is ticked, and on a developer's
; machine that folder is the model library -- a hundred gigabytes and, for a converted RVC
; timbre, the only copy in existence. So the hook could not be exercised without destroying the
; thing it protects, and for a while it was reasoned about instead of measured.
;
; Redirecting the real uninstaller was tried first and does not work: NSIS `$APPDATA` reads the
; shell folder, never the environment. Measured, rather than remembered --
;
;     nsis_appdata = C:\Users\<user>\AppData\Roaming
;     env_appdata  = ...\scratchpad\fakeroam
;
; So `run.sh` derives `hooks-under-test.nsh` from the shipped file by changing **one token** --
; where it deletes -- and prints the diff before compiling. Both guards, the shell variable
; context and the recursion are the shipped file byte for byte. `$%EPOCH_TEST_ROOT%` is expanded
; by `makensis` at compile time, so this can never resolve to a real `%APPDATA%` at run time.
;
; ## The harness says what it saw
;
; A run that deleted nothing because it never read its arguments looks exactly like a guard
; working, and the first version of this failed that way: four rows of "kept", all of them
; meaningless. The values are written to `saw.out` before the macro runs.
;
; The two variables are declared here the way the generated `installer.nsi` declares them, and
; read from the environment so every combination can be measured.

!include LogicLib.nsh
!include FileFunc.nsh

OutFile "hooktest.exe"
RequestExecutionLevel user
SilentInstall silent

Var UpdateMode
Var DeleteAppDataCheckboxState

!include "hooks-under-test.nsh"

Section
  StrCpy $UpdateMode 0
  StrCpy $DeleteAppDataCheckboxState 0
  ReadEnvStr $DeleteAppDataCheckboxState EPOCH_TEST_CHECKED
  ReadEnvStr $UpdateMode EPOCH_TEST_UPDATING
  FileOpen $9 "$EXEDIR\saw.out" w
  FileWrite $9 "checked=$DeleteAppDataCheckboxState updating=$UpdateMode root=$%EPOCH_TEST_ROOT%"
  FileClose $9
  !insertmacro NSIS_HOOK_POSTUNINSTALL
SectionEnd
