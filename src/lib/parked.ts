/**
 * Parked branches ([Parked Branches](../../spec/branch/parked-branches.spec.md)): the branches the user keeps on the
 * local branch, never pushed or landed, until unparked. Which branch is parked, also with the branch it is built on, is
 * Rust's (`domain_policy::parked`); these say it.
 */
import type { Parked, ParkedMember } from "@/lib/bindings"

/**
 * Why a branch is parked, as Rust decides it (`domain_policy::parked`, `BranchStructure.parked`); of the whole stack
 * (`wholeStack`) `with` is the member whose commits its stack branch holds
 */
export interface ParkedState extends Parked {
  readonly wholeStack?: true
}

/** The parked state of a whole stack, from the parked member of its stack branch (`BranchStructure.stackParked`) */
export function stackParkedState(parked: ParkedMember | null): ParkedState | null {
  return parked === null ? null : { note: parked.parked.note, with: parked.parked.with ?? parked.member, wholeStack: true }
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

/** The hover of the node of a parked row: why it stays local, a line each */
export function formatParkedLabel(state: ParkedState): string {
  const why = state.note === "" ? "" : `: ${state.note}`
  return state.with === null ? `Parked, kept local${why}` : `Parked with ${state.with}, kept local${why}`
}
