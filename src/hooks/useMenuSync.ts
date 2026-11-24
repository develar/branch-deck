import { useEffect } from "react"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useTauriListen } from "@/hooks/useTauriListen"
import { commands } from "@/lib/bindings"
import { logError } from "@/lib/errors"
import { FROM_MENU } from "@/lib/menu"
import { unattendedModeItem } from "@/lib/unattended"
import { setAutoSyncOnFocus, useAppSettings } from "@/stores/settings/appSettings"

/**
 * Keeps Auto-sync on Focus and Unattended Mode of the native menu and their settings in sync, in the main window: the
 * item the user chooses changes the setting, and every value of the setting, whichever window or process changed it
 * (the settings sync, `brd unattended` through the writer), checks the item. The backend checks them as its writer
 * loaded `settings.json` at startup (`configure_app_menu`). Unattended Mode switches through the one item of the mode
 * (`unattendedModeItem`), as the palette, Settings and the mark of the title bar do, with their toasts here.
 */
export function useMenuSync(): void {
  useTauriListen<boolean>("auto_sync_on_focus_toggled", checked => {
    setAutoSyncOnFocus(checked)
  })

  const autoSyncOnFocus = useAppSettings(state => state.autoSyncOnFocus)
  useEffect(() => {
    // a command of a Result never rejects
    void commands.setAutoSyncMenuChecked(autoSyncOnFocus).then(result => {
      if (result.status === "error") {
        logError("Cannot check Auto-sync on Focus in the menu", result.error)
      }
    })
  }, [autoSyncOnFocus])

  const unattended = useAppSettings(state => state.unattended)
  useAppCommand("unattended_mode", () => unattendedModeItem(unattended).onSelect(FROM_MENU))
  useEffect(() => {
    void commands.setUnattendedMenuChecked(unattended).then(result => {
      if (result.status === "error") {
        logError("Cannot check Unattended Mode in the menu", result.error)
      }
    })
  }, [unattended])
}
