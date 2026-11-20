/**
 * The start of the page: its root committed the first render and ran its effects, the last of them, so every listener
 * of an event that render mounts asked for its events (`useTauriListen`, `useAppCommand`). The page in the IDE tells
 * the host only then that it listens (`ui/notifications/initialized`, `src/platform/air/start.ts`): the IDE holds the
 * events of the view until it does. A page that failed to render starts too, so no host waits for it forever.
 */

let started = false
const waiters: (() => void)[] = []

/** Runs `callback` once the page started, at once when it did */
export function whenPageStarted(callback: () => void): void {
  if (started) {
    callback()
  } else {
    waiters.push(callback)
  }
}

/** The page started: the first render of its root ran its effects (`App`), or the page shows its failure */
export function markPageStarted(): void {
  if (started) {
    return
  }
  started = true
  for (const callback of waiters.splice(0)) {
    callback()
  }
}
