import { error as logToBackend, warn as warnToBackend } from "@tauri-apps/plugin-log"

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

/** What the engine reports of a ResizeObserver whose rendering update skipped an observation it resized again */
const RESIZE_LOOP = "ResizeObserver loop completed with undelivered notifications."
/** More loops than this within `RESIZE_LOOP_WINDOW_MS` are a cycle, which renders the window again and again */
const RESIZE_LOOPS_OF_A_CYCLE = 2
const RESIZE_LOOP_WINDOW_MS = 1000
/** When the window reported its last loops */
const resizeLoops: number[] = []

/**
 * A ResizeObserver loop: one is no failure, its notifications come in the next frame (the observers of a library, timed
 * as the specification says, can make one under load), and the log has a warning of it; more than
 * `RESIZE_LOOPS_OF_A_CYCLE` within a second are a cycle, an error (docs/contributing/frontend.md, "A flushSync in the
 * delivery of a ResizeObserver")
 */
function logResizeLoop(): void {
  const now = performance.now()
  resizeLoops.push(now)
  while (resizeLoops[0]! < now - RESIZE_LOOP_WINDOW_MS) {
    resizeLoops.shift()
  }
  if (resizeLoops.length > RESIZE_LOOPS_OF_A_CYCLE) {
    logUncaughtError("A ResizeObserver loop repeats in the page", `${resizeLoops.length} loops within ${RESIZE_LOOP_WINDOW_MS} ms`)
    return
  }
  console.warn(RESIZE_LOOP)
  warnToBackend(`A ResizeObserver loop of the page: ${RESIZE_LOOP}`).catch((logFailure: unknown) => {
    console.error("Failed to forward the warning to backend log", logFailure)
  })
}

window.addEventListener("error", event => {
  // the engine's event of a loop carries no error, only its words
  if (event.error === null && event.message === RESIZE_LOOP) {
    logResizeLoop()
    return
  }
  logUncaughtError("Uncaught error of the page", event.error ?? event.message)
})
window.addEventListener("unhandledrejection", event => {
  logUncaughtError("Unhandled rejection of the page", event.reason)
})
