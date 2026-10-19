import { PencilLineIcon } from "lucide-react"
import { commands } from "@/lib/bindings"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash } from "@/lib/commit"
import { getErrorDetails, notifyError } from "@/lib/errors"
import type { MenuItem } from "@/lib/menu"
import { branchSyncStore, syncBranches } from "@/stores/branch-sync"
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
  } catch (error) {
    // the command did not run (IPC failure)
    result = { status: "error", error: { message: getErrorDetails(error) } }
  }
  if (result.status !== "ok") {
    notifyError("Cannot edit the commit message", result.error)
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

/**
 * "Reword…" of a commit row: its preview opens with the message in edit, as E does there; disabled while a sync runs (a
 * reword is a rewrite)
 */
export function getRewordMenuItem(commit: CommitUnion, reword: (commit: CommitUnion) => void): MenuItem {
  const syncing = branchSyncStore.getState().isSyncing
  return {
    id: "reword",
    label: "Reword…",
    icon: PencilLineIcon,
    disabled: syncing,
    description: syncing ? "Wait for the sync to finish" : "Edits the message of the commit in its preview",
    onSelect: () => reword(commit),
  }
}
