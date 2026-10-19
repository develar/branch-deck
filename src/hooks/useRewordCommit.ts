import { PencilLineIcon } from "lucide-react"
import { commands } from "@/lib/bindings"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash } from "@/lib/commit"
import type { MenuItem } from "@/lib/menu"
import { runRewrite } from "@/lib/rewrite"
import { branchSyncStore, getBranchesOfCommits } from "@/stores/branch-sync"
import type { OperationKey } from "@/stores/operations"
import { branchSubject, getOperations, operationItem } from "@/stores/operations"
import { getSelectedProject } from "@/stores/repository"

/** The surface of a reword: the editor of the commit message, which shows its progress */
export const REWORD_SURFACE = "commit-message"

/** The operation of a reword of the commit: a rewrite of its branch */
function rewordOperation(commitId: string): OperationKey & { readonly branchName: string | null } {
  const branchName = getBranchesOfCommits([commitId])[0] ?? null
  return {
    id: "reword",
    subject: branchName === null ? `commit:${commitId}` : branchSubject(branchName),
    objects: branchName === null ? [] : [branchName],
    kind: "rewrite",
    branchName,
  }
}

/**
 * Replaces the message of a commit as one rewrite (`runRewrite`), then re-syncs so the lists and the virtual branches
 * show the new commit. `onReworded` gets the new commit id before the sync starts. Returns false (after an error toast)
 * if the reword failed.
 */
export async function rewordCommit(commit: CommitUnion, message: string, onReworded: (newCommitId: string) => void, surface: string | null = null): Promise<boolean> {
  const commitId = getCommitHash(commit)
  const { id, subject, objects } = rewordOperation(commitId)
  const result = await runRewrite(
    { id, subject, objects, label: "Editing the commit message", surface },
    "Cannot edit the commit message",
    () => commands.rewordCommits({ repositoryPath: getSelectedProject()?.path ?? "", rewrites: [{ commitId, message }] }),
    data => {
      const newCommitId = data.newCommitIds[commitId] ?? commitId
      if (newCommitId === commitId) {
        return { sync: false }
      }
      onReworded(newCommitId)
      return { sync: true }
    },
  )
  return result !== null
}

/**
 * "Reword…" of a commit row: its preview opens with the message in edit, as E does there; disabled while a sync runs (a
 * reword is a rewrite) and while a rewrite runs (`operationItem`)
 */
export function getRewordMenuItem(commit: CommitUnion, reword: (commit: CommitUnion) => void): MenuItem {
  const syncing = branchSyncStore.getState().isSyncing
  const item: MenuItem = {
    id: "reword",
    label: "Reword…",
    icon: PencilLineIcon,
    disabled: syncing,
    description: syncing ? "Wait for the sync to finish" : "Edits the message of the commit in its preview",
    onSelect: () => reword(commit),
  }
  return operationItem(getOperations(), item, rewordOperation(getCommitHash(commit)))
}
