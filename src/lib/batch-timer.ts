/** A run scheduled once until it runs or is canceled (`batchTimer`) */
export interface BatchTimer {
  /** Schedules the run, unless one is scheduled already: what is queued until it runs goes with it */
  schedule(): void
  /** Drops the scheduled run */
  cancel(): void
}

/**
 * Runs `run` `ms` after the first `schedule` of a batch, or on the next task while the window of the page never showed:
 * WebKit throttles the timers of a hidden page to a second, and the main window waits hidden for its first rows
 * (`markWindowContentReady`), which a sync brings in a batch of its events
 */
export function batchTimer(run: () => void, ms: number): BatchTimer {
  let scheduled = false
  let timer: ReturnType<typeof setTimeout> | undefined
  let nextTask: MessageChannel | null = null
  let windowShown = false
  const fire = () => {
    if (!scheduled) {
      return
    }
    scheduled = false
    clearTimeout(timer)
    run()
  }
  return {
    schedule() {
      if (scheduled) {
        return
      }
      scheduled = true
      windowShown ||= typeof document === "undefined" || document.visibilityState !== "hidden"
      if (windowShown) {
        timer = setTimeout(fire, ms)
        return
      }
      if (nextTask === null) {
        nextTask = new MessageChannel()
        nextTask.port1.onmessage = fire
      }
      nextTask.port2.postMessage(null)
    },
    cancel() {
      scheduled = false
      clearTimeout(timer)
    },
  }
}
