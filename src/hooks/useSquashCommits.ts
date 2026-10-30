import type { Commit, UndoToken } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import type { RewriteDone } from "@/lib/rewrite-run"
import { runRewriteIntent, undoRewrite, witnessed } from "@/lib/rewrite-run"
import { showToast } from "@/lib/toast-message"
import { showUndoToast } from "@/lib/undo-toast"
import { getSelectedRepositoryPath } from "@/stores/repository"

/**
 * Squashes the commits into the oldest of them with `message`, as the rewrite `squash` of the engine
 * (`runRewriteIntent`) in the repository of the press: the squash, the sync, then the toast with an undo. A squash that
 * waited for another rewrite in the queue of the session squashes the commits as that rewrite left them (the session
 * follows their ids). Resolves after the sync, to false, after an error toast or the info toast of a stopped wait, if
 * the commits were not squashed; nothing changed then.
 */
export async function squashCommits(commits: readonly Commit[], message: string, surface: string | null = null): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const commitIds = commits.map(commit => commit.originalHash)
  const done = await runRewriteIntent(
    {
      label: `Squashing ${formatCommitCount(commits.length)}`,
      surface,
      repositoryPath,
    },
    witnessed(repositoryPath, { rewrite: "squash", commitIds, message }),
    "Cannot squash the commits",
  )
  if (done === null || done.result.rewrite !== "squashed") {
    return false
  }
  showSquashedToast(repositoryPath, `Squashed ${formatCommitCount(done.result.answer.squashedCount)}`, done)
  return true
}

/**
 * The toast `title` of a squash or a squash of fixups of `repositoryPath`, with its Undo, also in Activity: the rewrite
 * `undo` of its `squash` token moves the branch back to the commit before the squash, which kept the files, so nothing
 * else changes
 */
export function showSquashedToast(repositoryPath: string, title: string, done: RewriteDone): void {
  const token = done.undo
  if (token === null) {
    showToast({ title })
    return
  }
  showUndoToast(repositoryPath, title, () => undoSquash(repositoryPath, token), undefined, done.outcome.op)
}

/** The Undo of a squash in `repositoryPath`, the repository it was made in: a rewrite of its own, queued as any */
function undoSquash(repositoryPath: string, token: UndoToken): Promise<boolean> {
  return undoRewrite(repositoryPath, token, "Undoing the squash", "Cannot undo the squash")
}
