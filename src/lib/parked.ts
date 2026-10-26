/**
 * Parked branches ([Parked Branches](../../spec/branch/parked-branches.spec.md)): the branches the user keeps on the
 * local branch, never pushed or landed, until unparked. Which branch is parked, also with the branch it is built on, is
 * Rust's (`domain_policy::parked`); these say it.
 */
import type { Parked } from "@/lib/bindings"

/** Why a branch is parked, as Rust decides it (`domain_policy::parked`, `BranchStructure.parked`) */
export type ParkedState = Parked

/** Why no push or robot starts: "Parked: Unpark it to push", "Built on parked auth: Unpark auth to push" */
export function formatParkedBlocker(state: ParkedState): string {
  return state.with === null ? "Parked: Unpark it to push" : `Built on parked ${state.with}: Unpark ${state.with} to push`
}

/** The hover of the node of a parked row: why it stays local, a line each */
export function formatParkedLabel(state: ParkedState): string {
  const why = state.note === "" ? "" : `: ${state.note}`
  return state.with === null ? `Parked, kept local${why}` : `Parked with ${state.with}, kept local${why}`
}
