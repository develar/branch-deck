import { useEffect } from "react"
import { runAppCommand } from "@/lib/app-commands"
import { matchMainMenuKey } from "@/lib/main-menu"
import { hasCustomTitleBar } from "@/lib/title-bar"

/**
 * The keys of the main menu where it replaces the native menu (a custom title bar on Windows and Linux): the
 * accelerators of the native menu went with it, by the physical key as theirs. In every window, as the native ones reached the focused window: Sync and
 * Update Project run in the main window (`runAppCommand` sends them there), Settings and the zoom here. Prevented,
 * so WebView2 doesn't reload on Ctrl+R. A window with a native menu leaves the keys to it.
 */
export function useMainMenuKeys(): void {
  useEffect(() => {
    if (!hasCustomTitleBar) {
      return
    }
    const listener = (event: KeyboardEvent) => {
      const match = event.defaultPrevented ? null : matchMainMenuKey(event)
      if (match !== null) {
        event.preventDefault()
        runAppCommand(match.command, match.payload)
      }
    }
    window.addEventListener("keydown", listener)
    return () => window.removeEventListener("keydown", listener)
  }, [])
}
