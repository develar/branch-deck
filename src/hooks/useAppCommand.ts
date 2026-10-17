import { useEffect, useEffectEvent } from "react"
import type { UnlistenFn } from "@tauri-apps/api/event"
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow"
import { registerAppCommand } from "@/lib/app-commands"
import type { AppCommand } from "@/lib/app-commands"
import { logError } from "@/lib/errors"

/**
 * Runs a command of the app's menu in this window for the lifetime of the component: from the native menu (its Tauri
 * event) and from the main menu and its keys (`runAppCommand`). The event is listened to by this window: the native
 * menu sends its commands to the main window, Settings to the focused one, and a global listener would get those of
 * the others too. The latest `handler` runs; it needn't be memoized.
 */
export function useAppCommand<T = undefined>(command: AppCommand, handler: (payload: T) => void): void {
  const run = useEffectEvent((payload: unknown) => handler(payload as T))
  useEffect(() => {
    let cancelled = false
    let unlisten: UnlistenFn | null = null
    getCurrentWebviewWindow()
      .listen<T>(command, event => run(event.payload))
      .then(fn => {
        if (cancelled) {
          fn()
        } else {
          unlisten = fn
        }
      })
      .catch((error: unknown) => logError(`Failed to listen to event ${command}`, error))
    const unregister = registerAppCommand(command, payload => run(payload))
    return () => {
      cancelled = true
      unlisten?.()
      unregister()
    }
  }, [command])
}
