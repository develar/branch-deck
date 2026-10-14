import { useEffect, useState } from "react"

/**
 * How often a relative time ("just now", "5 minutes ago") is renewed. Its text changes by the minute at the finest, so
 * it is at most this late: "1 minute ago" shows by 75 s, not by 2 minutes as with a one-minute tick.
 */
export const RELATIVE_TIME_TICK_MS = 15_000

interface Clock {
  readonly listeners: Set<(now: number) => void>
  readonly timer: ReturnType<typeof setInterval>
}

/** One interval per interval length, shared by all its listeners; it runs only while one listens */
const clocks = new Map<number, Clock>()

/**
 * Calls `listener` with the current time every `intervalMs`, at the same moment as every other listener of the same
 * interval length. Returns the unsubscribe; the interval stops with its last listener.
 */
export function subscribeNow(intervalMs: number, listener: (now: number) => void): () => void {
  let clock = clocks.get(intervalMs)
  if (clock === undefined) {
    const listeners = new Set<(now: number) => void>()
    const timer = setInterval(() => {
      const now = Date.now()
      for (const it of listeners) {
        it(now)
      }
    }, intervalMs)
    clock = { listeners, timer }
    clocks.set(intervalMs, clock)
  }
  const { listeners, timer } = clock
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
    if (listeners.size === 0 && clocks.get(intervalMs)?.timer === timer) {
      clearInterval(timer)
      clocks.delete(intervalMs)
    }
  }
}

/**
 * The current time, taken at mount and renewed every `intervalMs`, so a component that shows how long ago something
 * happened re-renders as it ages. Subscribe only in the small component that renders the text: each tick re-renders
 * every subscriber.
 */
export function useNow(intervalMs: number): number {
  const [now, setNow] = useState(Date.now)
  useEffect(() => subscribeNow(intervalMs, setNow), [intervalMs])
  return now
}
