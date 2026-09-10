; Epoch's uninstall hook.
;
; ## What this fixes
;
; The uninstaller's "delete application data" checkbox is Tauri's, and it removes
; `%APPDATA%\<bundle identifier>` — for us `%APPDATA%\dev.epoch.world`. Epoch keeps everything
; under `%APPDATA%\Epoch` instead: the vault, the crew, the Worlds, and the Generative Library
; the Workshop downloads models into.
;
; **"the Worlds" was not true when this was written.** Until 2026-09-10 a World somebody made
; was written beside the binary, into the installer's own `packs` folder -- so this box never
; reached it, and a plain uninstall left it behind in a program folder with no program in it
; (measured). Worlds made now live in the vault's `worlds` folder under APPDATA, and the ones
; made before are moved there on the next launch (`epoch_engine::relocate`). The sentence above
; is true from that version on.
;
; Measured on 2026-09-08 with the box UNTICKED -- a silent uninstall and a reinstall:
;
;   %APPDATA%\Epoch              43,129 files   100.07 GB   left untouched
;   %APPDATA%\dev.epoch.world    does not exist
;   %LOCALAPPDATA%\dev.epoch.world  1,908 files    0.16 GB   the WebView2 profile
;
; So somebody who ticked the box to reclaim space reclaimed 0.16 GB of browser cache and left a
; hundred gigabytes behind, with nothing on screen saying so. A control that claims one thing and
; quietly does a much smaller one is the failure this project keeps naming; the checkbox now does
; what it says.
;
; ## And then somebody ticked it
;
; The reading above was the only one there was for a day, because the folder it protects is a
; model library and the owner had forbidden deleting it. That made this file a claim rather than
; a measurement, which is what `hooktest/` was written to fix -- and then the owner authorised
; the real thing, so both halves are measured now instead of one.
;
; Measured 2026-09-08, the box ticked on the real uninstaller, `BM_GETCHECK` reading 1:
;
;   %APPDATA%\Epoch              43,130 files   100.07 GB   deleted, in 99 s
;   %LOCALAPPDATA%\Epoch          the application            deleted
;   HKCU\...\Uninstall\Epoch      the entry                  deleted
;   C: free                      80.4 GB -> 180.8 GB
;
; **What did not go is worth writing down.** Tauri's own `RmDir /r "$LOCALAPPDATA\${BUNDLEID}"`,
; a few lines above where this macro is inserted, left 446 files and 0.04 GB of the WebView2
; profile behind -- 0.16 GB before, 0.04 GB after. Nothing held them when they were checked
; afterwards, so no cause is claimed here; only that a box saying *delete the application data*
; leaves that much of it.
;
; And an update was measured in the same session: installing a newer Epoch over an older one
; left %APPDATA%\Epoch intact, which is the `$UpdateMode` guard below doing the one thing this
; file could get catastrophically wrong.
;
; ## Two guards, and both matter
;
; `$DeleteAppDataCheckboxState` — the box must have been ticked. A silent uninstall (`/S`) never
; shows the page, the variable stays 0, and nothing here runs. That is the default and it is the
; right one: the ordinary uninstall still takes nothing that is not its own.
;
; `$UpdateMode` — installing a newer Epoch over an older one runs the old uninstaller first.
; Without this guard an update would delete the user's Worlds and their model library, which is
; the worst thing this file could possibly do. Tauri sets it, and the generated script already
; guards its own deletion the same way.
;
; Both variables are declared by the generated `installer.nsi` and this macro is inserted inside
; the uninstall section, after that script's own block, so they are in scope.

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    ; `current` rather than `all`: Epoch installs per user and its data is per user.
    SetShellVarContext current
    RMDir /r "$APPDATA\Epoch"
  ${EndIf}
!macroend
