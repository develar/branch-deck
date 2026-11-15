import { error as logToBackend, warn as warnToBackend } from "@tauri-apps/plugin-log"
import { LOOP_LOG_MS, LOOP_REPEAT_MS, LOOP_REPEATS, loopOfOurs } from "@/lib/resize-loops"

/*
 * The errors of the page that nothing caught reach the log of the app (`BranchDeck.log`), not only the console of the
 * webview, which the release app has no inspector for (openspec/specs/repository/title-bar/spec.md, "A window whose page
 * fails says so"): a script that throws as it loads, a handler that throws, a promise nobody awaits. The entry of the page
 * imports this module first, so it hears the modules after it; it imports nothing of the page for the same reason, but
 * the policy of the ResizeObserver loops (src/lib/resize-loops.ts), constants and an attribution with no effect of its own.
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
/** When the window reported its last loops of the libraries */
const resizeLoops: number[] = []
/** When each kind of line of the loops was logged last, and how many loops of that kind it left out since */
const loopLines = new Map<string, { at: number; since: number }>()

/**
 * Logs the line of a kind of loop the first time, then once in `LOOP_LOG_MS` at most, with the count of the loops it
 * left out: a loop in every frame is no 60 lines a second in the log of the app
 */
function logLoopLine(kind: string, now: number, log: (more: string) => void): void {
  const last = loopLines.get(kind)
  if (last !== undefined && now - last.at < LOOP_LOG_MS) {
    last.since++
    return
  }
  log(last === undefined || last.since === 0 ? "" : `; ${last.since} more since the last line`)
  loopLines.set(kind, { at: now, since: 0 })
}

/**
 * A ResizeObserver loop (docs/contributing/frontend.md, "A flushSync in the delivery of a ResizeObserver"): one that left
 * an element of our observers pending is an error, as they let go what a change resizes in their delivery
 * (src/lib/observe-size.ts); one of the observers of a library alone is a warning, as its notifications come in the next
 * frame (a library times its observers as the specification says, and makes one under load), and `LOOP_REPEATS` of
 * them within `LOOP_REPEAT_MS` are an error, a loop that repeats (src/lib/resize-loops.ts). Each kind of line comes once
 * in `LOOP_LOG_MS` at most (`logLoopLine`)
 */
function logResizeLoop(): void {
  const now = performance.now()
  // weighed as the observers of the page tell its elements, by the engine's boxes in the next update
  loopOfOurs(ours => weighResizeLoop(now, ours))
}

/** A loop of `now`, which left `ours` of the observers of the page pending */
function weighResizeLoop(now: number, ours: readonly string[]): void {
  if (ours.length > 0) {
    logLoopLine("ours", now, more => logUncaughtError("A ResizeObserver loop of an observer of the page", `left ${ours.join(", ")} pending${more}`))
    return
  }
  resizeLoops.push(now)
  while (resizeLoops[0]! <= now - LOOP_REPEAT_MS) {
    resizeLoops.shift()
  }
  if (resizeLoops.length >= LOOP_REPEATS) {
    logLoopLine("repeats", now, more => logUncaughtError("A ResizeObserver loop repeats in the page", `${resizeLoops.length} loops within ${LOOP_REPEAT_MS / 1000} s${more}`))
    return
  }
  logLoopLine("library", now, more => {
    console.warn(RESIZE_LOOP)
    warnToBackend(`A ResizeObserver loop of the page: ${RESIZE_LOOP}${more}`).catch((logFailure: unknown) => {
      console.error("Failed to forward the warning to backend log", logFailure)
    })
  })
}

window.addEventListener("error", event => {
  // the engine's event of a loop carries no error, only its words; the page weighs and logs it, so the engine reports it
  // to no console as an uncaught error (WebKit did, a line of the error level for a loop the page has as a warning)
  if (event.error === null && event.message === RESIZE_LOOP) {
    event.preventDefault()
    logResizeLoop()
    return
  }
  logUncaughtError("Uncaught error of the page", event.error ?? event.message)
})
window.addEventListener("unhandledrejection", event => {
  logUncaughtError("Unhandled rejection of the page", event.reason)
})
