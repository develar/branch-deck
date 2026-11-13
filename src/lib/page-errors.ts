import { error as logToBackend } from "@tauri-apps/plugin-log"

/*
 * The errors of the page that nothing caught reach the log of the app (`BranchDeck.log`), not only the console of the
 * webview, which the release app has no inspector for (openspec/specs/repository/title-bar/spec.md, "A window whose page
 * fails says so"): a script that throws as it loads, a handler that throws, a promise nobody awaits. The entry of the page
 * imports this module first, so it hears the modules after it; it imports nothing of the page for the same reason.
 */

/** Logs an error that nothing caught, with its stack and, for a render, the components it was thrown in */
export function logUncaughtError(title: string, error: unknown, componentStack?: string): void {
  console.error(title, error)
  const keyValues: Record<string, string | undefined> = {
    error: error instanceof Error ? error.message : String(error),
    stack: error instanceof Error ? error.stack : undefined,
    component_stack: componentStack,
  }
  logToBackend(title, { keyValues }).catch((logFailure: unknown) => {
    console.error("Failed to forward error to backend log", logFailure)
  })
}

window.addEventListener("error", event => {
  logUncaughtError("Uncaught error of the page", event.error ?? event.message)
})
window.addEventListener("unhandledrejection", event => {
  logUncaughtError("Unhandled rejection of the page", event.reason)
})
