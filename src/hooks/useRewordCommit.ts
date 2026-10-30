import { PencilLineIcon } from "lucide-react"
import { undoRegroup } from "@/hooks/useStackActions"
import type { Outcome } from "@/lib/bindings"
import type { CommitUnion } from "@/lib/commit"
import { getCommitHash, getSubjectLine } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem } from "@/lib/menu"
import { runRewriteIntent, witnessed } from "@/lib/rewrite-run"
import { showUndoToast } from "@/lib/undo-toast"
import { getSelectedRepositoryPath } from "@/stores/repository"

/** The surface of a reword: the editor of the commit message, which shows its progress */
export const REWORD_SURFACE = "commit-message"

/** The commit a reword gave a new id, and that id, as its core answered it; null when no message changed */
function rewordedOf(outcome: Outcome): { readonly from: string; readonly to: string } | null {
  const { result } = outcome
  if (result.kind !== "rewrite" || result.rewrite !== "reworded" || outcome.undo === null) {
    return null
  }
  // one commit is reworded: the id the session resolved it to, through the rewrites before it in the queue
  const entry = Object.entries(result.answer.newCommitIds).find(([from, to]) => from !== to)
  return entry === undefined ? null : { from: entry[0], to: entry[1] }
}

/**
 * Replaces the message of a commit as the rewrite `reword` of the engine (`runRewriteIntent`, the regroup of `bd
 * reword`) in the repository of the save, then the sync that shows the new commit, and offers its Undo in a toast and in
 * Activity. `onFollow(from, to)` moves a preview on `from` to `to`: once the reword is written, before its sync shows it,
 * and on the Undo back to the old one. A message unchanged after the cleanup rewrites nothing and offers no Undo. A
 * reword that waited for another rewrite in the queue of the session rewords the commit as that rewrite left it (the
 * session follows its id). Returns false (after an error toast) if the reword failed.
 */
export async function rewordCommit(commit: CommitUnion, message: string, onFollow: (from: string, to: string) => void, surface: string | null = null): Promise<boolean> {
  const commitId = getCommitHash(commit)
  // the repository of the save: the reword and its Undo go there, whichever the main window shows when they run
  const repositoryPath = getSelectedRepositoryPath()
  const done = await runRewriteIntent(
    {
      label: "Editing the commit message",
      surface,
      repositoryPath,
      onWritten: outcome => {
        const reworded = rewordedOf(outcome)
        if (reworded !== null) {
          onFollow(reworded.from, reworded.to)
        }
      },
    },
    witnessed(repositoryPath, { rewrite: "reword", rewords: [{ commit: commitId, text: { message } }] }),
    "Cannot edit the commit message",
  )
  if (done === null) {
    return false
  }
  const reworded = rewordedOf(done.outcome)
  const token = done.undo?.token
  if (reworded === null || token === undefined) {
    return true
  }
  const { from, to } = reworded
  showUndoToast(
    repositoryPath,
    `Reworded ${formatShortHash(from)}`,
    () => undoRegroup(repositoryPath, token, "Cannot undo the reword", "Undoing the reword", () => onFollow(to, from)),
    getSubjectLine(message).trim(),
    done.outcome.op,
  )
  return true
}

/**
 * "Reword…" of a commit row: its preview opens with the message in edit, as E does there; enabled during a sync and
 * while another rewrite runs, a reword of the branch included: the save goes to the session at once and waits there in
 * the queue (a save of the same commit with the same message joins it)
 */
export function getRewordMenuItem(commit: CommitUnion, reword: (commit: CommitUnion) => void): MenuItem {
  return {
    id: "reword",
    label: "Reword…",
    icon: PencilLineIcon,
    description: "Edits the message of the commit in its preview",
    onSelect: () => reword(commit),
  }
}
