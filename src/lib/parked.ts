/**
 * Parked branches ([Parked Branches](../../spec/branch/parked-branches.spec.md)): the branches the user keeps on the
 * local branch, never pushed or landed, until unparked. A branch built on a parked one carries its commits: it is parked
 * with it.
 */

/** Why a branch is parked */
export interface ParkedState {
  /** The note of its line, or of the line of `with`; empty without one */
  readonly note: string
  /** The parked branch it is built on, whose commits it carries; null when its own line parks it */
  readonly with: string | null
  /** Of the whole stack: `with` is a member of it, whose commits its stack branch holds */
  readonly wholeStack?: true
}

/**
 * The parked state of every branch of `names`: its own line in `parked` (branch -> note), else the nearest branch it is
 * built on (`parentOf`, up the chain) that has one, as the backend decides it (`sync_core::parked::parked_state`)
 */
export function collectParkedStates(
  names: readonly string[],
  parked: Readonly<Record<string, string>>,
  parentOf: (name: string) => string | null,
): ReadonlyMap<string, ParkedState> {
  const states = new Map<string, ParkedState>()
  if (Object.keys(parked).length === 0) {
    return states
  }
  for (const name of names) {
    const own = parked[name]
    if (own !== undefined) {
      states.set(name, { note: own, with: null })
      continue
    }
    const seen = new Set<string>([name])
    let parent = parentOf(name)
    while (parent !== null && !seen.has(parent)) {
      const note = parked[parent]
      if (note !== undefined) {
        states.set(name, { note, with: parent })
        break
      }
      seen.add(parent)
      parent = parentOf(parent)
    }
  }
  return states
}

/**
 * Why no push or robot starts: "Parked: Unpark it to push", "Built on parked auth: Unpark auth to push", of the whole
 * stack "Holds parked auth: land the stack in pieces"
 */
export function formatParkedBlocker(state: ParkedState): string {
  if (state.with === null) {
    return "Parked: Unpark it to push"
  }
  return state.wholeStack === true ? `Holds parked ${state.with}: land the stack in pieces` : `Built on parked ${state.with}: Unpark ${state.with} to push`
}

/** The first member of a whole stack (`members`, root first) that is parked: what keeps its stack branch local */
export function wholeStackParkedState(members: readonly string[], stateOf: (name: string) => ParkedState | null): ParkedState | null {
  for (const member of members) {
    const state = stateOf(member)
    if (state !== null) {
      return { note: state.note, with: state.with ?? member, wholeStack: true }
    }
  }
  return null
}

/** The hover of the node of a parked row: why it stays local, a line each */
export function formatParkedLabel(state: ParkedState): string {
  const why = state.note === "" ? "" : `: ${state.note}`
  return state.with === null ? `Parked, kept local${why}` : `Parked with ${state.with}, kept local${why}`
}
