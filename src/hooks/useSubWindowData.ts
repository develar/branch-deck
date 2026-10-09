import { useSyncExternalStore } from "react"

function subscribe(onChange: () => void): () => void {
  // dispatched by the backend when an already open sub-window is reused with new data
  window.addEventListener("init-data-updated", onChange)
  return () => {
    window.removeEventListener("init-data-updated", onChange)
  }
}

function getSnapshot(): unknown {
  return window.__INIT_DATA__ ?? null
}

/**
 * Data passed to the sub-window by `openSubWindow` (`window.__INIT_DATA__`), or `null` if none was injected.
 * Re-renders when the window is reopened with new data.
 */
export function useSubWindowData<T>(): T | null {
  return useSyncExternalStore(subscribe, getSnapshot) as T | null
}
