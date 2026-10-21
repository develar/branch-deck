/**
 * What a drag of whole branches does where it is dropped, beside the move of their commits onto another branch
 * ([Move Commits](../../spec/branch/move-commits.spec.md#drag-and-drop)): the Parked card and the park zone of the drag
 * bar park them, the unpark zone unparks them, the "Stack on" chip of a row stacks the branch there, the unstack zone
 * makes it a root. Each answer takes the branches or refuses them with the reason the target shows, the reason the menu
 * gives for the same action.
 */
import type { ParkedState } from "@/lib/parked"
import type { PushHold } from "@/lib/patronus"
import { formatPushHoldReason } from "@/lib/patronus"
import { joinNames } from "@/lib/text"
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
  /** The busy state of a rewrite of the repository that runs ("Parking docs…"), null when none runs: every drop waits */
  rewrite: string | null
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

/**
 * The move of the dragged commits into the branch `target` of a row, or back out of the branches (`target` null):
 * a row of the Parked card leaves a drag of branches that land to the card, which parks them (null). Refused while a
 * rewrite runs, and for a drag of whole branches while one of them can't be rewritten (`rewriteBlocker`), as Move to…
 * of their menu; a drag of commit rows moves during a sync, as Move to… of a commit
 */
export function moveDrop(branches: readonly string[], target: string | null, facts: BranchDropFacts): BranchDrop | null {
  if (target !== null && branches.length > 0 && facts.parked(target) !== null && showsParkDrop(branches, facts)) {
    return null
  }
  const cannot = target === null ? "Cannot move the commits back" : `Cannot move to ${target}`
  if (facts.rewrite !== null) {
    return { refusal: `${cannot}: ${facts.rewrite}` }
  }
  for (const name of branches) {
    const branch = facts.branch(name)
    const blocker = branch === undefined ? "The branch is gone" : rewriteBlocker(branch, facts.isSyncing)
    if (blocker !== null) {
      return { refusal: `Cannot move ${name} ${target === null ? "back" : `to ${target}`}: ${blocker}` }
    }
  }
  return { names: branches }
}

/** Whether the Parked card takes a drag of `names`: some of them land, not all are parked already */
export function showsParkDrop(names: readonly string[], facts: BranchDropFacts): boolean {
  return names.some(name => facts.parked(name) === null)
}

/**
 * The park of the dragged branches that land; those parked already stay as they are. Refused while a rewrite runs, when
 * all of them are parked, or when one that lands can't be parked now (`parkBlocker`)
 */
export function parkDrop(names: readonly string[], facts: BranchDropFacts): BranchDrop {
  const landing = names.filter(name => facts.parked(name) === null)
  const [only] = names
  if (landing.length === 0) {
    return { refusal: names.length === 1 && only !== undefined ? `Cannot park ${only}: It is parked already` : "Cannot park: They are parked already" }
  }
  if (facts.rewrite !== null) {
    return { refusal: `Cannot park ${joinNames(landing)}: ${facts.rewrite}` }
  }
  for (const name of landing) {
    const branch = facts.branch(name)
    const blocker = branch === undefined ? "The branch is gone" : parkBlocker(branch, facts.hold(name), facts.isSyncing)
    if (blocker !== null) {
      return { refusal: `Cannot park ${name}: ${blocker}` }
    }
  }
  return { names: landing }
}

/** Whether the unpark zone takes a drag of `names`: some of them are parked */
export function showsUnparkDrop(names: readonly string[], facts: BranchDropFacts): boolean {
  return names.some(name => facts.parked(name) !== null)
}

/**
 * The unpark of the dragged parked branches that have a line of their own; those that land stay as they are. A branch
 * parked with its parent goes along when the parent is dragged too, else the drag is refused: it carries the commits of
 * its parent, which only an unpark of the parent lets land
 */
export function unparkDrop(names: readonly string[], facts: BranchDropFacts): BranchDrop {
  if (facts.rewrite !== null) {
    return { refusal: `Cannot unpark: ${facts.rewrite}` }
  }
  if (facts.isSyncing) {
    return { refusal: "Cannot unpark: Wait for the sync to finish" }
  }
  const own: string[] = []
  for (const name of names) {
    const parked = facts.parked(name)
    if (parked === null) {
      continue
    }
    if (parked.with === null) {
      own.push(name)
    } else if (!names.includes(parked.with)) {
      return { refusal: `Cannot unpark ${name}: It carries the commits of parked ${parked.with}, drag ${parked.with} here` }
    }
  }
  return own.length === 0 ? { refusal: "Cannot unpark: None of them is parked" } : { names: own }
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
  const blocker = facts.rewrite ?? rewriteBlocker(branch, facts.isSyncing)
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
  const blocker = facts.rewrite ?? rewriteBlocker(branch, facts.isSyncing)
  return blocker === null ? { names } : { refusal: `Cannot unstack ${branch.name} from ${parent}: ${blocker}` }
}
