import type { Commit } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { runRewriteIntent, undoRewrite, witnessed } from "@/lib/rewrite-run"
import { showToast } from "@/lib/toast-message"
import { showUndoToast } from "@/lib/undo-toast"
import { describeDropped } from "@/presenters/drop"
import { getSelectedRepositoryPath } from "@/stores/repository"

/**
 * Drops the commits and their files from HEAD for good, as the rewrite `drop` of the engine (`runRewriteIntent`) in the
 * repository of the press: the drop, the sync, then the toast with an undo, which brings them back with their files. A
 * drop that waited for another rewrite in the queue of the session drops the commits as that rewrite left them.
 * Resolves after the sync, to false, after an error toast or the info toast of a stopped wait, if nothing was dropped.
 */
export async function dropCommits(commits: readonly Commit[], surface: string | null = null): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const commitIds = commits.map(commit => commit.originalHash)
  const done = await runRewriteIntent(
    { label: `Dropping ${formatCommitCount(commits.length)}`, surface, repositoryPath },
    witnessed(repositoryPath, { rewrite: "drop", commitIds }),
    "Cannot drop the commits",
  )
  if (done === null || done.result.rewrite !== "dropped") {
    return false
  }
  const answer = done.result.answer
  const title = `Dropped ${formatCommitCount(answer.dropped.length)}`
  const description = describeDropped(answer) ?? undefined
  const token = done.undo
  if (token === null) {
    showToast({ title, description })
  } else {
    showUndoToast(repositoryPath, title, () => undoRewrite(repositoryPath, token, "Undoing the drop", "Cannot undo the drop"), description, done.outcome)
  }
  return true
}
