import type { Commit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { syncBranches } from "@/stores/branch-sync"
import { getSelectedProject } from "@/stores/repository"

/**
 * Squashes the commits into the oldest of them with `message`, then syncs and offers an undo. Resolves (before the
 * sync ends) to false, after an error toast, if the commits could not be squashed; nothing changed then.
 */
export async function squashCommits(commits: readonly Commit[], message: string): Promise<boolean> {
  const repositoryPath = getSelectedProject()?.path ?? ""
  const result = await invokeCommand(() =>
    commands.squashCommits({
      repositoryPath,
      commitIds: commits.map(commit => commit.originalHash),
      message,
    }),
  )
  if (result.status !== "ok") {
    notifyError("Cannot squash the commits", result.error)
    return false
  }

  const { newHead, previousHead, squashedCount } = result.data
  showUndoToast(`Squashed ${formatCommitCount(squashedCount)}`, () => undoSquash(repositoryPath, newHead, previousHead))
  void syncBranches({ autoScroll: false, autoExpand: false })
  return true
}

/** Moves the branch back to the commit before the squash: the squash kept the files, so nothing else changes. */
export async function undoSquash(repositoryPath: string, expectedHead: string, previousHead: string): Promise<void> {
  const result = await invokeCommand(() => commands.restoreBranchHead({ repositoryPath, expectedHead, previousHead }))
  if (result.status !== "ok") {
    notifyError("Cannot undo the squash", result.error)
    return
  }
  await syncBranches({ autoScroll: false, autoExpand: false })
}
