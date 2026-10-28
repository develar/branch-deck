import { undoRegroup } from "@/hooks/useStackActions"
import type { ParkResult, RewriteIntent } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import type { RewriteDone, RewriteRunSpec } from "@/lib/rewrite-run"
import { runRewriteIntent, witnessed } from "@/lib/rewrite-run"
import { joinNames } from "@/lib/text"
import { presentRewrite } from "@/presenters/rewrite"
import { getSelectedRepositoryPath } from "@/stores/repository"

export interface ParkOptions {
  /** The surface that started it and shows its spinner (`SelectContext.surface`) */
  surface?: string | null
}

/** Parks `branchName` with `note` (empty for none), or changes the note of a parked one (`parkBranches`) */
export function parkBranch(branchName: string, note: string, options: ParkOptions = {}): Promise<boolean> {
  return parkBranches([branchName], note, options)
}

/**
 * Parks `names` with `note` (empty for none), or changes the notes of parked ones: the rewrite `park` of the engine, no
 * rewrite of a commit, one commit of `refs/branch-deck/parked` under the repository lock for all of them, then the sync
 * that moves the rows into the Parked card, then the toast with Undo, which moves the store back to the commit it was
 * written on (compare-and-swap: a later write of the store makes the Undo stale). Queued behind the other rewrites by
 * the session.
 */
export async function parkBranches(names: readonly string[], note: string, options: ParkOptions = {}): Promise<boolean> {
  // the repository of the click: the park and its Undo go there, whichever the main window shows when they run
  const repositoryPath = getSelectedRepositoryPath()
  const rewrite = witnessed<Extract<RewriteIntent, { rewrite: "park" }>>(repositoryPath, { rewrite: "park", branches: names.map(branchName => ({ branchName, note })) })
  const done = await runRewriteIntent(parkSpec(repositoryPath, options), rewrite)
  const result = parkedOf(done)
  if (done === null || result === null) {
    return false
  }
  if (!result.changed) {
    return true
  }
  const changed = result.branches.filter(change => change.previousNote !== change.note)
  // a new note of a parked branch is no park
  const [renoted] = changed.length === 1 && changed[0]?.previousNote !== null ? changed : []
  const title = renoted === undefined ? `Parked ${joinNames(changed.map(change => change.branchName))}` : `Changed the park note of ${renoted.branchName}`
  offerUndo(repositoryPath, title, rewrite, done)
  return true
}

/** Unparks `branchName`: it lands as any other branch again (`unparkBranches`) */
export function unparkBranch(branchName: string, options: ParkOptions = {}): Promise<boolean> {
  return unparkBranches([branchName], options)
}

/**
 * Unparks `names` in one commit of the store (the rewrite `unpark`): they land as any other branch again; the toast's
 * Undo moves the store back, each branch parked again with its note
 */
export async function unparkBranches(names: readonly string[], options: ParkOptions = {}): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const rewrite = witnessed<Extract<RewriteIntent, { rewrite: "unpark" }>>(repositoryPath, { rewrite: "unpark", branchNames: [...names] })
  const done = await runRewriteIntent(parkSpec(repositoryPath, options), rewrite)
  if (done === null || parkedOf(done) === null) {
    return false
  }
  offerUndo(repositoryPath, `Unparked ${joinNames(names)}`, rewrite, done)
  return true
}

/** What a park or an unpark answered; null for another answer */
function parkedOf(done: RewriteDone | null): ParkResult | null {
  return done?.result.rewrite === "parked" ? done.result.answer : null
}

/**
 * The toast of a park or an unpark with its Undo: the store back in `repositoryPath`, the repository of the click,
 * whichever the main window shows when it is pressed
 */
function offerUndo(repositoryPath: string, title: string, rewrite: RewriteIntent, done: RewriteDone): void {
  const { phrase } = presentRewrite(rewrite)
  showUndoToast(repositoryPath, title, () => undoRegroup(repositoryPath, done.undo?.token, `Cannot undo ${phrase}`, `Undoing ${phrase}`), undefined, done.outcome.op)
}

function parkSpec(repositoryPath: string, options: ParkOptions): RewriteRunSpec {
  return { surface: options.surface, repositoryPath }
}
