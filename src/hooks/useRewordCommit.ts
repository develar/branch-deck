import { PencilLineIcon } from "lucide-react"
import { undoRegroup } from "@/hooks/useStackActions"
import { commands } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash, getSubjectLine } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem } from "@/lib/menu"
import { runRewrite } from "@/lib/rewrite"
import { branchSyncStore, getBranchesOfCommits, resolveCommitsOf } from "@/stores/branch-sync"
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
 * old one. A message unchanged after the cleanup rewrites nothing and offers no Undo. A reword that waited for another
 * rewrite in the queue rewords the commit as that rewrite left it. Returns false (after an error toast) if the reword
 * failed.
 */
export async function rewordCommit(commit: CommitUnion, message: string, onFollow: (from: string, to: string) => void, surface: string | null = null): Promise<boolean> {
  const commitId = getCommitHash(commit)
  const { id, subject, objects } = rewordOperation(commitId)
  // the repository of the save: the reword and its Undo go there, whichever the main window shows when they run
  const repositoryPath = getSelectedRepositoryPath()
  // the commit the reword rewrites, once its turn in the queue comes
  let target = commitId
  const result = await runRewrite(
    { id, subject, objects, label: "Editing the commit message", surface, repositoryPath },
    "Cannot edit the commit message",
    () => {
      // the id as the rewrite before it in the queue left it (`resolveCommitsOf`); a missing commit goes by its hash
      target = "originalHash" in commit ? (resolveCommitsOf(repositoryPath, [commit])[0]?.originalHash ?? commitId) : commitId
      return commands.reword({ repositoryPath, rewords: [{ commit: target, text: { message } }] })
    },
    data => {
      const newCommitId = data.newCommitIds[target] ?? target
      const token = data.undoToken
      if (token == null || newCommitId === target) {
        return { sync: false }
      }
      onFollow(target, newCommitId)
      return {
        sync: true,
        after: () =>
          showUndoToast(
            `Reworded ${formatShortHash(target)}`,
            () => undoRegroup(repositoryPath, token, "Cannot undo the reword", "Undoing the reword", () => onFollow(newCommitId, target)),
            getSubjectLine(message).trim(),
          ),
      }
    },
  )
  return result !== null
}

/**
 * "Reword…" of a commit row: its preview opens with the message in edit, as E does there; disabled while a sync runs (a
 * reword is a rewrite) and while the reword of its branch runs (`operationItem`, a second start joins it); while
 * another rewrite runs it stays enabled, and the save waits for that rewrite in the queue
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
