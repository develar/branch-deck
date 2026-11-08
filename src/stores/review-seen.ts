import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

/**
 * The files the review window showed its reader, by the head they were shown at (spec/branch/review-tour.spec.md, "In
 * the Review Window": the files of a step the reader saw): a file shown on purpose (its row, J and K at its card, N and
 * P, a place) or whose card passed entirely through the viewport of the diffs. The state of the window, never
 * persisted: another scope of the same head keeps them, a new head counts from zero, as Approve counts the files it
 * opened.
 */
const reviewSeenStore = createStore<{ readonly heads: ReadonlyMap<string, ReadonlySet<string>> }>(() => ({ heads: new Map() }))

const NOTHING_SEEN: ReadonlySet<string> = new Set()

/** The files seen at `head`; none without a head */
export function useSeenPaths(head: string | null): ReadonlySet<string> {
  return useStore(reviewSeenStore, state => (head === null ? NOTHING_SEEN : (state.heads.get(head) ?? NOTHING_SEEN)))
}

/** `paths` are seen at `head`; the same state when all of them were */
export function markSeen(head: string | null, paths: readonly string[]): void {
  if (head === null || paths.length === 0) {
    return
  }
  reviewSeenStore.setState(state => {
    const seen = state.heads.get(head) ?? NOTHING_SEEN
    const fresh = paths.filter(path => !seen.has(path))
    if (fresh.length === 0) {
      return state
    }
    return { heads: new Map(state.heads).set(head, new Set([...seen, ...fresh])) }
  })
}
