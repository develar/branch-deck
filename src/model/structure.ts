/**
 * The structure of a branch of the table as the page holds it: what Rust placed it with (`BranchView.structure`), every
 * field present. A module of its own, without the stores, so the fixtures of the page tests read it too
 */
import type { BranchSection, ParkedMember } from "@/lib/bindings"
import type { ParkedState } from "@/lib/parked"

/**
 * Where a branch is among the branches of the table, as Rust decides it for the page and `bd` alike
 * (`domain_policy::stack`, `domain_policy::parked`, sent as `BranchView.structure`), every field present: the page lays
 * the rows out of it and decides none of it
 */
export interface BranchStructure {
  /** The branch of the table it stacks on, its declared parent; null for a root or a parent the table lacks */
  readonly parent: string | null
  /** The branches stacked directly on it, in branch order */
  readonly children: readonly string[]
  /** The branches it is built on, root first: the chain of its effective bases */
  readonly ancestors: readonly string[]
  /** The branches stacked on it, directly or further down, each after its parent */
  readonly descendants: readonly string[]
  /** The main line of its stack below it, lane 0 of the stack graph; empty when nothing is stacked on it */
  readonly mainLine: readonly string[]
  /** Why it stays local: its own parked line, or a parked branch it is built on */
  readonly parked: ParkedState | null
  /** Of the root of a stack that forks: the first member of its stack branch that is parked */
  readonly stackParked: ParkedMember | null
  readonly section: BranchSection
}

const noBranches: readonly string[] = []

/** The structure of a branch with no stack and no parked line, and of a branch the table lacks */
export const ROOT_STRUCTURE: BranchStructure = {
  parent: null,
  children: noBranches,
  ancestors: noBranches,
  descendants: noBranches,
  mainLine: noBranches,
  parked: null,
  stackParked: null,
  section: "main",
}
