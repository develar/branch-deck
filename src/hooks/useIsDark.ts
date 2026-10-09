import { useSyncExternalStore } from "react"

const QUERY = "(prefers-color-scheme: dark)"

function subscribe(onChange: () => void): () => void {
  const mediaQuery = window.matchMedia(QUERY)
  mediaQuery.addEventListener("change", onChange)
  return () => {
    mediaQuery.removeEventListener("change", onChange)
  }
}

function getSnapshot(): boolean {
  return window.matchMedia(QUERY).matches
}

/** Whether the OS colour scheme is dark; the app has no manual theme toggle. */
export function useIsDark(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, () => false)
}
