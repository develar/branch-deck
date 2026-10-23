import { PencilLineIcon } from "lucide-react"
import { undoRegroup } from "@/hooks/useStackActions"
import { commands } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash, getSubjectLine } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem } from "@/lib/menu"
import { runRewrite } from "@/lib/rewrite"
import { branchSyncStore, getBranchesOfCommits } from "@/stores/branch-sync"
import type { OperationKey } from "@/stores/operations"
import { branchSubject, getOperations, operationItem } from "@/stores/operations"
import { getSelectedRepositoryPath } from "@/stores/repository"

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
 * Replaces the message of a commit as one rewrite (`runRewrite`, the regroup of `bd reword`), then re-syncs so the lists
 * and the virtual branches show the new commit, and offers its Undo in a toast and in Activity. `onFollow(from, to)`
 * moves a preview on `from` to `to`: on the save to the new commit, before the sync starts, and on the Undo back to the
 * old one. A message unchanged after the cleanup rewrites nothing and offers no Undo. Returns false (after an error
 * toast) if the reword failed.
 */
export async function rewordCommit(commit: CommitUnion, message: string, onFollow: (from: string, to: string) => void, surface: string | null = null): Promise<boolean> {
  const commitId = getCommitHash(commit)
  const { id, subject, objects } = rewordOperation(commitId)
  // the repository of the save: the reword and its Undo go there, whichever the main window shows when they run
  const repositoryPath = getSelectedRepositoryPath()
  const result = await runRewrite(
    { id, subject, objects, label: "Editing the commit message", surface, repositoryPath },
    "Cannot edit the commit message",
    () => commands.reword({ repositoryPath, rewords: [{ commit: commitId, text: { message } }] }),
    data => {
      const newCommitId = data.newCommitIds[commitId] ?? commitId
      const token = data.undoToken
      if (token == null || newCommitId === commitId) {
        return { sync: false }
      }
      onFollow(commitId, newCommitId)
      return {
        sync: true,
        after: () =>
          showUndoToast(
            `Reworded ${formatShortHash(commitId)}`,
            () => undoRegroup(repositoryPath, token, "Cannot undo the reword", "Undoing the reword", () => onFollow(newCommitId, commitId)),
            getSubjectLine(message).trim(),
          ),
      }
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
