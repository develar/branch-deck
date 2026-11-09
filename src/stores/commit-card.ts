import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"

/** The open card of a commit row's mark: its list (a `useId` of `CommitList`), its commit, and whether `I` opened it */
interface OpenCommitCard {
  readonly list: string
  readonly hash: string
  readonly byKey: boolean
}

/**
 * The one card of a commit row's mark open in the window (openspec/specs/commit/commit-row/spec.md, "Commit card"): the main
 * window shows a list per branch, and a card that opens in any of them closes the one of another
 */
const commitCardStore = createStore<{ readonly card: OpenCommitCard | null }>(() => ({ card: null }))

/** The open card of `list`, null while it has none */
export function useCommitCardOf(list: string): OpenCommitCard | null {
  return useStore(commitCardStore, state => (state.card?.list === list ? state.card : null))
}

/** Whether a card of a commit row's mark is open in the window: its Escape is the card's */
export function isCommitCardOpen(): boolean {
  return commitCardStore.getState().card !== null
}

/** Opens the card of `hash` in `list` by `I`, the focus in it; any other card closes */
export function openCommitCardByKey(list: string, hash: string): void {
  commitCardStore.setState({ card: { list, hash, byKey: true } })
}

/** Opens (by the hover of its mark) or closes the card of `hash` in `list`; the hover of the card `I` opened keeps it the key's */
export function setCommitCardOpen(list: string, hash: string, open: boolean): void {
  commitCardStore.setState(state => {
    const isThis = state.card?.list === list && state.card.hash === hash
    if (open) {
      return isThis ? state : { card: { list, hash, byKey: false } }
    }
    return isThis ? { card: null } : state
  })
}
