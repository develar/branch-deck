import { useEffect, useRef } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useTauriCustomListen } from "@/hooks/useTauriListen"

/**
 * Calls `onFocus` whenever the current window gains focus.
 * Used by the main window (auto-sync on focus) and by sub-windows (refresh data from the main window).
 * The latest callback is always invoked, so it does not need to be memoized.
 */
export function useWindowFocus(onFocus: () => void): void {
  const onFocusRef = useRef(onFocus)
  useEffect(() => {
    onFocusRef.current = onFocus
  })

  useTauriCustomListen("appWindow.onFocusChanged", () => getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (focused) {
      onFocusRef.current()
    }
  }))
}
