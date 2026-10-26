/**
 * The structure of a branch of the table as the page holds it: what Rust placed it with (`BranchView.structure`), every
 * field present. A module of its own, without the stores, so the fixtures of the page tests read it too
 */
import type { BranchSection, ParkedMember, WholeStackView } from "@/lib/bindings"
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
  /**
   * Of the root of a stack: its whole stack, which one Safe Push lands (`domain_policy::stack::whole_stack_of`, by
   * declared parent): its stack branch while it forks, else the last branch of a straight chain; null for a branch
   * alone and for one that heads no stack
   */
  readonly wholeStack: WholeStackView | null
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
  wholeStack: null,
  section: "main",
}

/** The branches of each section of the table, in branch order */
export interface TableRows {
  /** Every branch but the parked ones and those parked with them */
  readonly main: readonly string[]
  /** The parked branches and those built on one */
  readonly parked: readonly string[]
}

/** The rows of a table without branches */
export const NO_ROWS: TableRows = { main: noBranches, parked: noBranches }

function sameNames(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((name, index) => name === b[index])
}

/**
 * `order` split by the section Rust placed each branch in (`BranchStructure.section`): the view sends the order once
 * and the section with the structure, never a second list of the same names. `main` is `order` itself while nothing is
 * parked, and `last` stays while neither section changed
 */
export function rowsOf(order: readonly string[], branches: Readonly<Record<string, { readonly structure: BranchStructure }>>, last: TableRows): TableRows {
  const main: string[] = []
  const parked: string[] = []
  for (const name of order) {
    const section = branches[name]?.structure.section
    if (section === "parked") {
      parked.push(name)
    } else if (section === "main") {
      main.push(name)
    }
  }
  if (sameNames(last.main, main) && sameNames(last.parked, parked)) {
    return last
  }
  return { main: parked.length === 0 && main.length === order.length ? order : main, parked }
}
