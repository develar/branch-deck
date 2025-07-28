import { onMounted, onScopeDispose } from "vue"
import { getCurrentWindow } from "@tauri-apps/api/window"

/**
 * Hook to sync data when a sub-window gains focus.
 * Useful for refreshing stores or data from the main window.
 *
 * @param onFocus - Callback to run when the window gains focus
 */
export function useSubWindowFocusSync(onFocus: () => void) {
  onMounted(async () => {
    const appWindow = getCurrentWindow()

    // Listen for focus changes
    const unlisten = await appWindow.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        // Window gained focus - call the provided callback
        onFocus()
      }
    })

    // Clean up on scope disposal
    onScopeDispose(() => {
      unlisten()
    })
  })
}