import { commands } from "@/lib/bindings"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash } from "@/lib/commit"
import { getErrorDetails, notifyError } from "@/lib/errors"
import { syncBranches } from "@/stores/branch-sync"
import { getSelectedProject } from "@/stores/repository"

/**
 * Replaces the message of a commit, then re-syncs so the lists and the virtual branches show the new commit.
 * `onReworded` gets the new commit id before the sync starts. Returns false (after an error toast) if the reword failed.
 */
export async function rewordCommit(commit: CommitUnion, message: string, onReworded: (newCommitId: string) => void): Promise<boolean> {
  const commitId = getCommitHash(commit)
  let result: Awaited<ReturnType<typeof commands.rewordCommits>>
  try {
    result = await commands.rewordCommits({
      repositoryPath: getSelectedProject()?.path ?? "",
      rewrites: [{ commitId, message }],
    })
  }
  catch (error) {
    // the command did not run (IPC failure)
    result = { status: "error", error: { message: getErrorDetails(error) } }
  }
  if (result.status !== "ok") {
    notifyError("Failed to Edit the Commit Message", result.error)
    return false
  }

  const newCommitId = result.data.newCommitIds[commitId] ?? commitId
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
