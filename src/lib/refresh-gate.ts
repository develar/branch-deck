/**
 * Why a status is asked for: after a sync, on window focus, after a push, by the refresh button, or by the timer of
 * something still running. Each status (pull requests, Patronus robots) decides which reasons ask now.
 */
export type RefreshReason = "sync" | "focus" | "push" | "manual" | "poll"

export interface RefreshPolicy {
  /** Whether `reason` asks now, `sinceStart` ms after the last request started and `sinceAnswer` ms after the last answer */
  readonly isDue: (reason: RefreshReason, sinceStart: number, sinceAnswer: number) => boolean
  /** The reasons that, while a request is in flight, ask once more after it; the others add nothing */
  readonly followUpReasons: ReadonlySet<RefreshReason>
}

interface Gate {
  /** `Date.now()` when the last request started */
  startedAt: number
  inFlight: Promise<void> | null
  /** A reason that came while a request was in flight and asks again after it */
  followUp: RefreshReason | null
}

export interface RefreshGates {
  /**
   * Runs `task` for `key` if the policy lets `reason` ask now; one task per key at a time. `fetchedAt` is `Date.now()`
   * of the last answer, 0 = never. `task` gets the reason it runs for (a follow-up runs with the reason that asked) and
   * must not throw.
   */
  refresh: (key: string, reason: RefreshReason, fetchedAt: number, task: (reason: RefreshReason) => Promise<void>) => Promise<void>
  /** Forgets every key (tests) */
  reset: () => void
}

export function createRefreshGates(policy: RefreshPolicy): RefreshGates {
  const gates = new Map<string, Gate>()
  return {
    refresh(key, reason, fetchedAt, task) {
      let gate = gates.get(key)
      if (gate === undefined) {
        gate = { startedAt: Number.NEGATIVE_INFINITY, inFlight: null, followUp: null }
        gates.set(key, gate)
      }
      if (gate.inFlight !== null) {
        if (policy.followUpReasons.has(reason)) {
          gate.followUp = reason
        }
        return gate.inFlight
      }
      const now = Date.now()
      if (!policy.isDue(reason, now - gate.startedAt, now - fetchedAt)) {
        return Promise.resolve()
      }

      gate.startedAt = now
      const current = gate
      const run = async (): Promise<void> => {
        try {
          let next: RefreshReason | null = reason
          while (next !== null) {
            current.followUp = null
            await task(next)
            next = current.followUp
          }
        }
        finally {
          current.inFlight = null
        }
      }
      current.inFlight = run()
      return current.inFlight
    },
    reset() {
      gates.clear()
    },
  }
}
