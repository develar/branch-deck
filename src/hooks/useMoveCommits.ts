import { toast } from "@/components/ui/toast"
import type { Commit, MovedCommit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { syncBranches } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getSelectedProject } from "@/stores/repository"

/**
 * Moves commits to `targetBranch` (an existing or a new branch) by setting their `(branch)` prefix, or removes the
 * prefix (`null`): the commits fall back to their issue branch or to the unassigned commits. Then syncs, reveals the
 * moved commits in the target branch and offers an undo. Resolves (before the sync ends) to false, after an error
 * toast, if the commits could not be moved.
 */
export async function moveCommits(commits: readonly Commit[], targetBranch: string | null): Promise<boolean> {
  const result = await invokeCommand(() => commands.moveCommitsToBranch({
    repositoryPath: getSelectedProject()?.path ?? "",
    commitIds: commits.map(commit => commit.originalHash),
    targetBranch,
  }))
  if (result.status !== "ok") {
    notifyError(targetBranch === null ? "Failed to Remove the Prefix" : `Failed to Move Commits to ${targetBranch}`, result.error)
    return false
  }

  const { moved } = result.data
  if (moved.length === 0) {
    toast.add({ type: "info", title: targetBranch === null ? "No prefix to remove" : `Already in ${targetBranch}` })
    return true
  }

  requestReveal(targetBranch === null ? null : { branchName: targetBranch, commitIds: moved.map(it => it.newId) })
  showUndoToast(targetBranch === null ? `Removed the prefix of ${formatCommitCount(moved.length)}` : `Moved ${formatCommitCount(moved.length)} to ${targetBranch}`, () => undoMove(moved))
  // the form closes when the commits moved; the lists follow when the sync is done
  void syncBranches({ autoScroll: false, autoExpand: false })
  return true
}

/** Restores the messages the moved commits had before, in one history rewrite. */
export async function undoMove(moved: readonly MovedCommit[]): Promise<void> {
  const result = await invokeCommand(() => commands.rewordCommits({
    repositoryPath: getSelectedProject()?.path ?? "",
    rewrites: moved.map(it => ({ commitId: it.newId, message: it.previousMessage })),
  }))
  if (result.status !== "ok") {
    notifyError("Failed to Undo the Move", result.error)
    return
  }
  requestReveal(null)
  await syncBranches({ autoScroll: false, autoExpand: false })
}

const moveCommitsActions = { moveCommits }

export function useMoveCommits(): typeof moveCommitsActions {
  return moveCommitsActions
}
