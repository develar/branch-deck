import { toast } from "@/components/ui/toast"
import type { CommandError, Commit, MovedCommit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { getErrorDetails, notifyError } from "@/lib/errors"
import { syncBranches } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getSelectedProject } from "@/stores/repository"

type CommandResult<T> = { status: "ok", data: T } | { status: "error", error: CommandError }

/** Runs a command; an IPC failure (the command did not run) becomes an error result as well. */
async function invoke<T>(run: () => Promise<CommandResult<T>>): Promise<CommandResult<T>> {
  try {
    return await run()
  }
  catch (error) {
    return { status: "error", error: { message: getErrorDetails(error) } }
  }
}

function commitCount(count: number): string {
  return count === 1 ? "1 commit" : `${count} commits`
}

/**
 * Moves commits to `targetBranch` (an existing or a new branch) by setting their `(branch)` prefix, or removes the
 * prefix (`null`): the commits fall back to their issue branch or to the unassigned commits. Then syncs, reveals the
 * moved commits in the target branch and offers an undo. Resolves (before the sync ends) to false, after an error
 * toast, if the commits could not be moved.
 */
export async function moveCommits(commits: readonly Commit[], targetBranch: string | null): Promise<boolean> {
  const result = await invoke(() => commands.moveCommitsToBranch({
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
  showUndoToast(moved, targetBranch === null ? `Removed the prefix of ${commitCount(moved.length)}` : `Moved ${commitCount(moved.length)} to ${targetBranch}`)
  // the form closes when the commits moved; the lists follow when the sync is done
  void syncBranches({ autoScroll: false, autoExpand: false })
  return true
}

function showUndoToast(moved: readonly MovedCommit[], title: string): void {
  const id = toast.add({
    type: "success",
    title,
    timeout: 10_000,
    actionProps: {
      children: "Undo",
      onClick: () => {
        toast.close(id)
        void undoMove(moved)
      },
    },
  })
}

/** Restores the messages the moved commits had before, in one history rewrite. */
async function undoMove(moved: readonly MovedCommit[]): Promise<void> {
  const result = await invoke(() => commands.rewordCommits({
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
