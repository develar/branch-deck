import { useEffect } from "react"
import { useTauriListen } from "@/hooks/useTauriListen"
import { commands } from "@/lib/bindings"
import { logError } from "@/lib/errors"
import { setAutoSyncOnFocus, useAppSettings } from "@/stores/settings/appSettings"

/**
 * Keeps Auto-sync on Focus of the native menu and the setting in sync, in the main window: the item the user checks
 * sets the setting, and every value of the setting, whichever window changed it (the settings sync), checks the item.
 * The backend checks it as `settings.json` says at startup (`stored_auto_sync_on_focus`).
 */
export function useMenuSync(): void {
  useTauriListen<boolean>("auto_sync_on_focus_toggled", checked => {
    setAutoSyncOnFocus(checked)
  })

  const autoSyncOnFocus = useAppSettings(state => state.autoSyncOnFocus)
  useEffect(() => {
    commands
      .setAutoSyncMenuChecked(autoSyncOnFocus)
      .then(result => {
        if (result.status === "error") {
          logError("Cannot check Auto-sync on Focus in the menu", result.error)
        }
      })
      .catch((error: unknown) => {
        logError("Cannot check Auto-sync on Focus in the menu", error)
      })
  }, [autoSyncOnFocus])
}
