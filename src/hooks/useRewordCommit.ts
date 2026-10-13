import { commands } from "@/lib/bindings"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { syncBranches } from "@/stores/branch-sync"
import { getSelectedProject } from "@/stores/repository"

/**
 * Replaces the message of a commit, then re-syncs so the lists and the virtual branches show the new commit.
 * `onReworded` gets the new commit id before the sync starts. Returns false (after an error toast) if the reword failed.
 */
export async function rewordCommit(commit: CommitUnion, message: string, onReworded: (newCommitId: string) => void): Promise<boolean> {
  const commitId = getCommitHash(commit)
  const result = await commands.rewordCommit({
    repositoryPath: getSelectedProject()?.path ?? "",
    commitId,
    message,
  })
  if (result.status !== "ok") {
    notifyError("Failed to Edit the Commit Message", result.error)
    return false
  }

  const { newCommitId } = result.data
  if (newCommitId !== commitId) {
    onReworded(newCommitId)
    // refresh without auto-expand or auto-scroll
    await syncBranches({ autoScroll: false, autoExpand: false })
  }
  return true
}

const rewordCommitActions = { rewordCommit }

export function useRewordCommit(): typeof rewordCommitActions {
  return rewordCommitActions
}
