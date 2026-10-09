import { getCurrentWindow } from "@tauri-apps/api/window"
import { logError } from "@/lib/errors"

// Escape closes a sub-window. Installed once at startup for the lifetime of the window, not per component.
function installEscapeToClose(): void {
  if (typeof window === "undefined") {
    return
  }

  let currentWindow: ReturnType<typeof getCurrentWindow>
  try {
    currentWindow = getCurrentWindow()
  }
  catch {
    // window metadata unavailable: treat as main
    return
  }
  if (currentWindow.label === "main") {
    return
  }

  window.addEventListener("keydown", (event) => {
    // components that handle Escape themselves (popovers, inline forms) stop propagation or prevent default
    if (event.key === "Escape" && !event.defaultPrevented) {
      currentWindow.close().catch((error: unknown) => {
        logError("Failed to close sub-window", error)
      })
    }
  })
}

installEscapeToClose()
