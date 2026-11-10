import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

/**
 * The files the review window showed its reader, by the head they were shown at (openspec/specs/branch/review-tour/spec.md, "In
 * the Review Window": the files of a step the reader saw): a file shown on purpose (its row, J and K at its card, N and
 * P, a place) or whose card passed entirely through the viewport of the diffs, of a file the story shows in parts each
 * of its parts. The state of the window, never persisted: another scope of the same head keeps them, a new head counts
 * from zero, as Approve counts the files it opened.
 */
const reviewSeenStore = createStore<{ readonly heads: ReadonlyMap<string, ReadonlySet<string>> }>(() => ({ heads: new Map() }))

/** The parts of the files in parts the reader saw, by the head: no view reads them, only `markCardsSeen` */
const partsSeen = new Map<string, Set<string>>()

const NOTHING_SEEN: ReadonlySet<string> = new Set()

/** The files seen at `head`; none without a head */
export function useSeenPaths(head: string | null): ReadonlySet<string> {
  return useStore(reviewSeenStore, state => (head === null ? NOTHING_SEEN : (state.heads.get(head) ?? NOTHING_SEEN)))
}

/** `paths` are seen at `head`; the same state when all of them were */
function markSeen(head: string | null, paths: readonly string[]): void {
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

/**
 * The cards the reader saw at `head` (passed through the view, shown on purpose), each by its file and its key: the file
 * of each is seen, of a file the story shows in parts (`parts`: the keys of its sections by its path) once every part
 * of it was. Returns the files of `cards` seen at `head` now
 */
export function markCardsSeen(
  head: string | null,
  cards: readonly { readonly path: string; readonly key: string }[],
  parts: ReadonlyMap<string, readonly string[]>,
): readonly string[] {
  if (head === null) {
    return []
  }
  let seen = partsSeen.get(head)
  if (seen === undefined) {
    seen = new Set()
    partsSeen.set(head, seen)
  }
  for (const card of cards) {
    seen.add(card.key)
  }
  const paths = [...new Set(cards.map(card => card.path))].filter(path => parts.get(path)?.every(key => seen.has(key)) ?? true)
  markSeen(head, paths)
  return paths
}
