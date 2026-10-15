import { toast } from "@/components/ui/toast"
import type { Commit, MoveCommitsResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { capitalize, countMoved, describeMove } from "@/lib/move-summary"
import { syncBranches } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getSelectedProject } from "@/stores/repository"

/**
 * Moves commits to `targetBranch` (an existing or a new branch), or back (`null`: out of their prefix branch or out of
 * the issue group of their key), with the smallest change (`move_commits_to_branch`): a whole issue goes by the issue
 * groups, without a rewrite, the other commits by their `(branch)` prefix. Then syncs, reveals the moved commits in the
 * target branch and offers an undo of the whole move. Resolves (before the sync ends) to false, after an error toast,
 * if the commits could not be moved.
 */
export async function moveCommits(commits: readonly Commit[], targetBranch: string | null): Promise<boolean> {
  const result = await invokeCommand(() => commands.moveCommitsToBranch({
    repositoryPath: getSelectedProject()?.path ?? "",
    commitIds: commits.map(commit => commit.originalHash),
    targetBranch,
  }))
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
  showUndoToast(moveTitle(moved, target, count), () => undoRegroup(moved.undoToken, "Cannot undo the move"), description === undefined ? undefined : capitalize(description))
  // the form closes when the commits moved; the lists follow when the sync is done
  void syncBranches({ autoScroll: false, autoExpand: false })
  return true
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

/** Moves back what a move, a rename or a restack wrote, HEAD and the issue groups in one transaction, then syncs */
export async function undoRegroup(token: string | null | undefined, failureTitle: string): Promise<void> {
  if (token == null) {
    return
  }
  const result = await invokeCommand(() => commands.undoRegroup({ repositoryPath: getSelectedProject()?.path ?? "", token }))
  if (result.status !== "ok") {
    notifyError(failureTitle, result.error)
    return
  }
  requestReveal(null)
  await syncBranches({ autoScroll: false, autoExpand: false })
}

const moveCommitsActions = { moveCommits }

export function useMoveCommits(): typeof moveCommitsActions {
  return moveCommitsActions
}
