import { useEffect } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { logError } from "@/lib/errors"

/**
 * Keeps the native title of the window at `title`: the page draws the title bar (`WindowTitleBar`), the title stays for
 * the Window menu, Mission Control and screen readers. `null` leaves it as the window was opened with.
 */
export function useWindowTitle(title: string | null): void {
  useEffect(() => {
    if (title !== null) {
      getCurrentWindow()
        .setTitle(title)
        .catch((error: unknown) => logError("Cannot retitle the window", error))
    }
  }, [title])
}
