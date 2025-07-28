import { type EventCallback, type EventName, listen } from "@tauri-apps/api/event"
import { error as logError } from "@tauri-apps/plugin-log"

export function scopedListen<T>(event: EventName, handler: EventCallback<T>) {
  let unlisten: (() => void) | null
  let isUnmounted = false
  // Set up event listeners for menu-triggered update checks
  // clean up listeners
  onScopeDispose(() => {
    isUnmounted = true
    if (unlisten) {
      unlisten()
      unlisten = null
    }
  })

  listen(event, handler)
    .then((it) => {
      if (isUnmounted) {
        it()
      }
      else {
        unlisten = it
      }
    })
    .catch((error) => {
      console.error(`Failed to listen to event ${event}:`, error)
      // noinspection JSIgnoredPromiseFromCall
      logError("Failed to listen to event", { keyValues: { event, error: error.toString() } })
    })
}
