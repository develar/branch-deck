import { useEffect, useRef } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { useTauriCustomListen } from "@/hooks/useTauriListen"

/**
 * Calls `onFocus` whenever the current window gains focus, and `onBlur` when it loses it.
 * Used by the main window (auto-sync on focus, status refresh) and by sub-windows (refresh data from the main window).
 * The latest callbacks are always invoked, so they do not need to be memoized.
 */
export function useWindowFocus(onFocus: () => void, onBlur?: () => void): void {
  const callbacksRef = useRef({ onFocus, onBlur })
  useEffect(() => {
    callbacksRef.current = { onFocus, onBlur }
  })

  useTauriCustomListen("appWindow.onFocusChanged", () => getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (focused) {
      callbacksRef.current.onFocus()
    }
    else {
      callbacksRef.current.onBlur?.()
    }
  }))
}
