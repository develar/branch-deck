import { commands } from "~/utils/bindings"

/**
 * Composable to sync menu checkbox states with app settings
 * Sets up bidirectional sync between menu and settings store
 */
export function useMenuSync() {
  const appSettings = useAppSettingsStore()

  scopedListen("menu_auto_sync_toggled", (event) => {
    // Update settings when menu is toggled
    appSettings.autoSyncOnFocus = event.payload as boolean
  })

  watch(() => appSettings.autoSyncOnFocus, async (newValue) => {
    console.log("Updating menu checkbox state:", newValue)
    commands.updateMenuCheckbox("auto_sync_on_focus", newValue)
      .catch(error => console.error("Failed to initialize menu checkbox state:", error))
  }, { immediate: true })
}
