import { getCurrentWindow } from "@tauri-apps/api/window"
import { commands } from "@/lib/bindings"
import { isOnDragRegion } from "@/lib/drag-region"
import { logError, notifyError } from "@/lib/errors"
import { isLinux } from "@/lib/platform"
import { titleBar } from "@/lib/title-bar"

/*
 * What a native title bar does with the mouse beyond dragging, which Tauri's drag script (`data-tauri-drag-region`)
 * does: a double click as the system setting says (Tauri always maximizes), a right click opens the window manager's
 * window menu. Both on the empty parts of the row only, never on its controls.
 */

/** How far the mouse moves with the button down before a press on the title bar drags the window (GTK's threshold) */
const DRAG_THRESHOLD = 4

let installed = false

/** The double click of the system on the empty parts of the title bar */
function runDoubleClick(action: "toggle-maximize" | "minimize" | "none"): void {
  const window = getCurrentWindow()
  if (action === "toggle-maximize") {
    window.toggleMaximize().catch((error: unknown) => notifyError("Cannot maximize the window", error))
  } else if (action === "minimize") {
    window.minimize().catch((error: unknown) => notifyError("Cannot minimize the window", error))
  }
}

/**
 * Linux: a press on the empty parts drags the window only once the mouse moved (GTK's header bar does so). Tauri's drag
 * script asks the window manager to move the window on every press, whose grab takes the second click of a double click,
 * so the double click never reached the page.
 */
function installLinuxDrag(action: "toggle-maximize" | "minimize" | "none"): void {
  window.addEventListener(
    "mousedown",
    event => {
      if (event.button !== 0 || !isOnDragRegion(event.composedPath())) {
        return
      }
      event.preventDefault()
      event.stopImmediatePropagation()
      if (event.detail === 2) {
        runDoubleClick(action)
        return
      }
      const { screenX, screenY } = event
      const stop = () => {
        window.removeEventListener("mousemove", move, { capture: true })
        window.removeEventListener("mouseup", stop, { capture: true })
      }
      const move = (moved: MouseEvent) => {
        if ((moved.buttons & 1) === 0) {
          stop()
        } else if (Math.hypot(moved.screenX - screenX, moved.screenY - screenY) >= DRAG_THRESHOLD) {
          stop()
          getCurrentWindow()
            .startDragging()
            .catch((error: unknown) => logError("Cannot drag the window", error))
        }
      }
      window.addEventListener("mousemove", move, { capture: true })
      window.addEventListener("mouseup", stop, { capture: true })
    },
    { capture: true },
  )
}

/** Installs the handlers once per page (the window has one title bar) */
export function installTitleBarMouse(): void {
  if (installed || titleBar.kind === "native") {
    return
  }
  installed = true

  if (isLinux && titleBar.kind === "custom") {
    installLinuxDrag(titleBar.doubleClick)
  } else if (titleBar.doubleClick !== "toggle-maximize" && titleBar.doubleClick !== "host") {
    // before the drag script (a document listener) gets the second click; macOS maximizes on its mouseup. A host that
    // does the double click itself hears it from the page (`src/platform/air/title-bar.ts`)
    const action = titleBar.doubleClick
    const takeDoubleClick = (event: MouseEvent) => {
      if (event.button !== 0 || event.detail !== 2 || !isOnDragRegion(event.composedPath())) {
        return
      }
      event.preventDefault()
      event.stopImmediatePropagation()
      if (event.type === "mousedown") {
        runDoubleClick(action)
      }
    }
    window.addEventListener("mousedown", takeDoubleClick, { capture: true })
    window.addEventListener("mouseup", takeDoubleClick, { capture: true })
  }

  // a native title bar's right click; macOS has no window menu there
  if (titleBar.kind === "custom") {
    window.addEventListener("contextmenu", event => {
      if (event.defaultPrevented || !isOnDragRegion(event.composedPath())) {
        return
      }
      event.preventDefault()
      void commands.showWindowMenu().then(result => {
        if (result.status === "error") {
          logError("Cannot show the window menu", result.error)
        }
      })
    })
  }
}
