import { toast } from "@/components/ui/toast"
import { offerUnstackAfterMove, undoRegroup } from "@/hooks/useStackActions"
import type { Commit, MoveCommitsResult, RewriteIntent } from "@/lib/bindings"
import { countMoved, describeDoneMove } from "@/lib/move-summary"
import { runRewriteIntent, witnessed } from "@/lib/rewrite-run"
import { showUndoToast } from "@/lib/undo-toast"
import { branchSyncStore, getBranchesOfCommits } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getSelectedRepositoryPath } from "@/stores/repository"

export interface MoveOptions {
  /** The busy state, "Moving 3 commits to auth" by default */
  label?: string
  /** The surface that started it and shows its spinner (`SelectContext.surface`) */
  surface?: string | null
}

/**
 * Moves commits to `targetBranch` (an existing or a new branch), or back (`null`: out of their prefix branch or out of
 * the issue group of their key), with the smallest change: a whole issue goes by the issue groups, without a rewrite,
 * the other commits by their `(branch)` prefix. The rewrite `move` of the engine (`runRewriteIntent`), named on the
 * table the window shows: the move, the sync, then the toast with an undo of the whole move and the moved commits
 * revealed in the target branch; when a stacked branch of the move builds alone afterwards, the toast also offers its
 * unstack. A move that waits for another rewrite in the queue of the session moves the commits as that rewrite left
 * them (the session follows their ids); a move of other commits or to another target is a move of its own, never one
 * that joins it. Resolves after the sync, to false, after an error toast, if the commits could not be moved.
 */
export async function moveCommits(commits: readonly Commit[], targetBranch: string | null, options: MoveOptions = {}): Promise<boolean> {
  const commitIds = commits.map(commit => commit.originalHash)
  const sources = getBranchesOfCommits(commitIds)
  // the repository of the move: the move and its Undo go there, whichever the main window shows when they run
  const repositoryPath = getSelectedRepositoryPath()
  const rewrite = witnessed<Extract<RewriteIntent, { rewrite: "move" }>>(repositoryPath, { rewrite: "move", commitIds, targetBranch })
  return runMove(repositoryPath, rewrite, sources, targetBranch, options)
}

/**
 * Ungroup of the branch `branchName`: every commit of it goes back where its key groups it, the rewrite `ungroup` of the
 * engine (the move back of the commits the branch has when it runs), as `moveCommits` tells it; a second Ungroup of the
 * branch joins it in the session, and its menu item runs meanwhile
 */
export async function ungroupBranch(branchName: string, options: MoveOptions = {}): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const rewrite = witnessed<Extract<RewriteIntent, { rewrite: "ungroup" }>>(repositoryPath, { rewrite: "ungroup", branchName })
  return runMove(repositoryPath, rewrite, [branchName], null, options)
}

/** Runs the move `rewrite` of the commits of `sources` to `targetBranch` and tells how it went (`moveCommits`) */
async function runMove(repositoryPath: string, rewrite: RewriteIntent, sources: readonly string[], targetBranch: string | null, options: MoveOptions): Promise<boolean> {
  const stacked = stackedBranchesOfMove(sources, targetBranch)
  const done = await runRewriteIntent({ repositoryPath, label: options.label, surface: options.surface }, rewrite)
  if (done === null || done.result.rewrite !== "moved") {
    return false
  }
  const moved = done.result.answer
  const count = countMoved(moved)
  if (count === 0) {
    toast.add({ type: "info", title: targetBranch === null ? "Nothing to move back" : `Already in ${moved.target ?? targetBranch}` })
    return true
  }
  const target = moved.target ?? null
  requestReveal(target === null ? null : { branchName: target, commitIds: movedIds(moved) })
  const { title, description } = describeDoneMove(moved, target, count)
  const toastId = showUndoToast(repositoryPath, title, () => undoRegroup(repositoryPath, done.undo?.token, "Cannot undo the move", "Undoing the move"), description, done.outcome)
  void offerUnstackAfterMove(toastId, stacked, target)
  return true
}

/** The target and the source branches of a move that are stacked, as the last sync knows them: the target first */
function stackedBranchesOfMove(sources: readonly string[], targetBranch: string | null): string[] {
  const { branches } = branchSyncStore.getState()
  const names = [...new Set([...(targetBranch === null ? [] : [targetBranch]), ...sources])]
  return names.filter(name => (branches[name]?.stack?.declaredParent ?? null) !== null)
}

/** The ids of the commits a move changed, after it */
export function movedIds(result: MoveCommitsResult): string[] {
  return result.commits.filter(commit => commit.change !== "Unchanged").map(commit => commit.newId)
}
