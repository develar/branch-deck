import { toast } from "@/components/ui/toast"
import { offerUnstackAfterMove, undoRegroup } from "@/hooks/useStackActions"
import type { Commit, MoveCommitsResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { capitalize, countMoved, describeMove } from "@/lib/move-summary"
import { branchSyncStore, syncBranches } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getSelectedProject } from "@/stores/repository"

/**
 * Moves commits to `targetBranch` (an existing or a new branch), or back (`null`: out of their prefix branch or out of
 * the issue group of their key), with the smallest change (`move_commits_to_branch`): a whole issue goes by the issue
 * groups, without a rewrite, the other commits by their `(branch)` prefix. Then syncs, reveals the moved commits in the
 * target branch and offers an undo of the whole move; when a stacked branch of the move builds alone afterwards, the
 * toast also offers its unstack. Resolves (before the sync ends) to false, after an error toast, if the commits could
 * not be moved.
 */
export async function moveCommits(commits: readonly Commit[], targetBranch: string | null): Promise<boolean> {
  const stacked = stackedBranchesOfMove(commits, targetBranch)
  const result = await invokeCommand(() =>
    commands.moveCommitsToBranch({
      repositoryPath: getSelectedProject()?.path ?? "",
      commitIds: commits.map(commit => commit.originalHash),
      targetBranch,
    }),
  )
  if (result.status !== "ok") {
    notifyError(targetBranch === null ? "Cannot move the commits back" : `Cannot move the commits to ${targetBranch}`, result.error)
    return false
  }

  const moved = result.data
  const count = countMoved(moved)
  if (count === 0) {
    toast.add({ type: "info", title: targetBranch === null ? "Nothing to move back" : `Already in ${moved.target ?? targetBranch}` })
    return true
  }

  const target = moved.target ?? null
  requestReveal(target === null ? null : { branchName: target, commitIds: movedIds(moved) })
  const description = describeMove(moved, target, true)
  const toastId = showUndoToast(
    moveTitle(moved, target, count),
    () => undoRegroup(moved.undoToken, "Cannot undo the move"),
    description === undefined ? undefined : capitalize(description),
  )
  // the form closes when the commits moved; the lists follow when the sync is done
  void syncBranches({ autoScroll: false, autoExpand: false })
  // after the toast: the move stays as fast as before
  void offerUnstackAfterMove(toastId, stacked, target)
  return true
}

/** The target and the source branches of a move that are stacked, as the last sync knows them: the target first */
function stackedBranchesOfMove(commits: readonly Commit[], targetBranch: string | null): string[] {
  const { branches } = branchSyncStore.getState()
  const sources = Object.values(branches)
    .filter(branch => commits.some(commit => branch.commitIndex[commit.originalHash] !== undefined))
    .map(branch => branch.name)
  const names = [...new Set([...(targetBranch === null ? [] : [targetBranch]), ...sources])]
  return names.filter(name => (branches[name]?.stack?.declaredParent ?? null) !== null)
}

function moveTitle(result: MoveCommitsResult, target: string | null, count: number): string {
  if (target !== null) {
    return `Moved ${formatCommitCount(count)} to ${target}`
  }
  const onlyPrefixes = result.commits.every(commit => commit.change === "PrefixRemoved" || commit.change === "Unchanged")
  return onlyPrefixes ? `Removed the prefix of ${formatCommitCount(count)}` : `Moved ${formatCommitCount(count)} back`
}

/** The ids of the commits a move changed, after it */
export function movedIds(result: MoveCommitsResult): string[] {
  return result.commits.filter(commit => commit.change !== "Unchanged").map(commit => commit.newId)
}

const moveCommitsActions = { moveCommits }

export function useMoveCommits(): typeof moveCommitsActions {
  return moveCommitsActions
}
