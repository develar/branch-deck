import { useEffect } from "react"
import { useTauriListen } from "@/hooks/useTauriListen"
import { commands } from "@/lib/bindings"
import { logError } from "@/lib/errors"
import { setAutoSyncOnFocus, useAppSettings } from "@/stores/settings/appSettings"

/**
 * Keeps the native menu checkboxes and the app settings in sync, in both directions:
 * toggling the menu item updates the setting, and every setting value (including the initial one) is pushed to the menu.
 */
export function useMenuSync(): void {
  useTauriListen<boolean>("menu_auto_sync_toggled", (checked) => {
    setAutoSyncOnFocus(checked)
  })

  const autoSyncOnFocus = useAppSettings(state => state.autoSyncOnFocus)
  useEffect(() => {
    commands.updateMenuCheckbox("auto_sync_on_focus", autoSyncOnFocus)
      .then((result) => {
        if (result.status === "error") {
          logError("Failed to update menu checkbox state", result.error)
        }
      })
      .catch((error: unknown) => {
        logError("Failed to update menu checkbox state", error)
      })
  }, [autoSyncOnFocus])
}
