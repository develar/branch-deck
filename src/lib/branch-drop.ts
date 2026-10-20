/**
 * What a drag of whole branches does where it is dropped, beside the move of their commits onto another branch
 * ([Move Commits](../../spec/branch/move-commits.spec.md#drag-and-drop)): the Parked card parks them, the unpark zone
 * unparks them, the "Stack on" chip of a row stacks the branch there, the unstack zone makes it a root. Each answer
 * takes the branches or refuses them with the reason the target shows, the reason the menu gives for the same action.
 */
import type { ParkedState } from "@/lib/parked"
import type { PushHold } from "@/lib/patronus"
import { formatPushHoldReason } from "@/lib/patronus"
import type { Branch } from "@/stores/branch-sync"

/** What a drop decides by: the branches as the last sync left them */
export interface BranchDropFacts {
  branch: (name: string) => Branch | undefined
  parked: (name: string) => ParkedState | null
  hold: (name: string) => PushHold | null
  /** The branches stacked on `name`, directly or further down */
  descendants: (name: string) => readonly string[]
  stacksEnabled: boolean
  /** A sync rebuilds the branches */
  isSyncing: boolean
}

/** A drop target's answer: the branches it acts on, or why it refuses the drag ("Cannot park auth: …") */
export type BranchDrop = { readonly names: readonly string[] } | { readonly refusal: string }

/**
 * Why a rewrite of the branch (rename, move, ungroup, restack, amend, unapply, issue reference, squash) can't run now:
 * it has no commits, it failed to sync, or a sync rebuilds the branches (also a quiet resync of a row that shows its last
 * state); null when it can
 */
export function rewriteBlocker(branch: Branch, isSyncing: boolean): string | null {
  if (branch.commits.length === 0) {
    return "The branch has no commits"
  }
  if (branch.hasError) {
    return "The branch failed to sync"
  }
  if (branch.status === "Syncing" || isSyncing) {
    return "Wait for the sync to finish"
  }
  return null
}

/**
 * Why a branch that is not parked can't be parked now: a robot lands it (it lands whatever a park says), it has no
 * commits, or a sync rebuilds the branches; null when it can
 */
export function parkBlocker(branch: Branch, hold: PushHold | null, isSyncing: boolean): string | null {
  if (hold !== null && hold.kind !== "landed") {
    return `${formatPushHoldReason(branch.name, hold)}: it lands whatever a park says`
  }
  if (branch.commits.length === 0) {
    return "The branch has no commits"
  }
  if (branch.status === "Syncing" || isSyncing) {
    return "Wait for the sync to finish"
  }
  return null
}

/** Whether the Parked card takes a drag of `names`: some of them land, not all are parked already */
export function showsParkDrop(names: readonly string[], facts: BranchDropFacts): boolean {
  return names.some(name => facts.parked(name) === null)
}

/** The park of the dragged branches, refused when one of them is parked already or can't be parked now (`parkBlocker`) */
export function parkDrop(names: readonly string[], facts: BranchDropFacts): BranchDrop {
  for (const name of names) {
    const parked = facts.parked(name)
    if (parked !== null) {
      return { refusal: `Cannot park ${name}: ${parked.with === null ? "It is parked already" : `It is parked with ${parked.with}`}` }
    }
    const branch = facts.branch(name)
    const blocker = branch === undefined ? "The branch is gone" : parkBlocker(branch, facts.hold(name), facts.isSyncing)
    if (blocker !== null) {
      return { refusal: `Cannot park ${name}: ${blocker}` }
    }
  }
  return { names }
}

/** Whether the unpark zone takes a drag of `names`: some of them are parked */
export function showsUnparkDrop(names: readonly string[], facts: BranchDropFacts): boolean {
  return names.some(name => facts.parked(name) !== null)
}

/**
 * The unpark of the dragged branches that have a line of their own; a branch parked with its parent goes along when the
 * parent is dragged too, else the drag is refused: it carries the commits of its parent, which only an unpark of the
 * parent lets land
 */
export function unparkDrop(names: readonly string[], facts: BranchDropFacts): BranchDrop {
  if (facts.isSyncing) {
    return { refusal: "Cannot unpark: Wait for the sync to finish" }
  }
  const own: string[] = []
  for (const name of names) {
    const parked = facts.parked(name)
    if (parked === null) {
      return { refusal: `Cannot unpark ${name}: It is not parked` }
    }
    if (parked.with === null) {
      own.push(name)
    } else if (!names.includes(parked.with)) {
      return { refusal: `Cannot unpark ${name}: It carries the commits of parked ${parked.with}, drag ${parked.with} here` }
    }
  }
  return { names: own }
}

/**
 * The stack of the one dragged branch `name` on `target`, as "Stack On…" of its menu: null where the chip doesn't show
 * (stacked branches off, more than one branch dragged, the row of the branch itself); refused on a branch stacked on it
 * (a cycle), on its parent already, or while the branch can't be rewritten (`rewriteBlocker`)
 */
export function stackOnDrop(names: readonly string[], target: string, facts: BranchDropFacts): BranchDrop | null {
  const [name] = names
  if (!facts.stacksEnabled || names.length !== 1 || name === undefined || name === target) {
    return null
  }
  const branch = facts.branch(name)
  if (branch === undefined) {
    return null
  }
  if (branch.stack?.declaredParent === target) {
    return { refusal: `Cannot stack ${name} on ${target}: It is stacked on ${target} already` }
  }
  if (facts.descendants(name).includes(target)) {
    return { refusal: `Cannot stack ${name} on ${target}: ${target} is stacked on it` }
  }
  const blocker = rewriteBlocker(branch, facts.isSyncing)
  return blocker === null ? { names } : { refusal: `Cannot stack ${name} on ${target}: ${blocker}` }
}

/**
 * The unstack of the one dragged branch: null where the zone doesn't show (stacked branches off, more than one branch
 * dragged, a branch stacked on none); refused while it can't be rewritten (`rewriteBlocker`)
 */
export function unstackDrop(names: readonly string[], facts: BranchDropFacts): BranchDrop | null {
  const [name] = names
  if (!facts.stacksEnabled || names.length !== 1 || name === undefined) {
    return null
  }
  const branch = facts.branch(name)
  const parent = branch?.stack?.declaredParent ?? null
  if (branch === undefined || parent === null) {
    return null
  }
  const blocker = rewriteBlocker(branch, facts.isSyncing)
  return blocker === null ? { names } : { refusal: `Cannot unstack ${branch.name} from ${parent}: ${blocker}` }
}
