import { emitTo } from "@tauri-apps/api/event"
import { logError } from "@/lib/errors"

/**
 * The commands of the app's menu, one function each wherever they come from: the native menu (its Tauri event), the
 * main menu of a custom title bar (☰, `src/lib/main-menu.ts`) and its keys (`useMainMenuKeys`). The window that runs a
 * command registers it (`useAppCommand`); one that doesn't (a sub-window for Sync) sends it to the main window as the
 * native menu would.
 */
export type AppCommand = "sync-branches" | "update-project" | "open_settings" | "open_color_selector" | "zoom" | "install_agent_cli" | "check_for_updates"

const handlers = new Map<AppCommand, (payload: unknown) => void>()

export function registerAppCommand(command: AppCommand, handler: (payload: unknown) => void): () => void {
  handlers.set(command, handler)
  return () => {
    if (handlers.get(command) === handler) {
      handlers.delete(command)
    }
  }
}

/** Runs a command here, or in the main window, which has its handler */
export function runAppCommand(command: AppCommand, payload?: unknown): void {
  const handler = handlers.get(command)
  if (handler !== undefined) {
    handler(payload)
    return
  }
  emitTo("main", command, payload).catch((error: unknown) => logError(`Cannot send ${command} to the main window`, error))
}
