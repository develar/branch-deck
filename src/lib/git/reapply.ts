import type { RewriteIntent, UndoToken } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { runRewriteIntent, undoRewrite, witnessed } from "@/lib/rewrite-run"
import { showUndoToast } from "@/lib/undo-toast"
import { getSelectedRepositoryPath } from "@/stores/repository"
import { getUnappliedBranch } from "@/stores/unapplied-branches"

/**
 * Puts the commits of the unapplied branch `name` (its full name) back on top of HEAD, their grouping prefix restored
 * and their files following, and deletes the unapplied branch: the rewrite `reapply` of the engine, queued behind the
 * other rewrites by the session, then the sync that builds the branch again, then the toast whose Undo takes the commits
 * out again and keeps the unapplied branch. Resolves to false, after an error toast, if nothing changed
 */
export async function reapplyBranch(name: string): Promise<boolean> {
  const branch = getUnappliedBranch(name)
  if (branch === undefined) {
    return false
  }
  // the repository of the click: the reapply and its Undo go there, whichever the main window shows when they run
  const repositoryPath = getSelectedRepositoryPath()
  const rewrite = witnessed<Extract<RewriteIntent, { rewrite: "reapply" }>>(repositoryPath, { rewrite: "reapply", branchName: branch.branch })
  const done = await runRewriteIntent({ repositoryPath }, rewrite)
  if (done?.result.rewrite !== "reapplied") {
    return false
  }
  const { answer } = done.result
  const { undo } = done
  showUndoToast(
    repositoryPath,
    `Reapplied ${branch.branch}`,
    () => undoReapply(repositoryPath, branch.branch, undo),
    `${formatCommitCount(answer.reapplied.length)} back on top of HEAD`,
    done.outcome,
  )
  return true
}

/** The undo of a reapply: HEAD and the files back, the unapplied branch again; a rewrite of its own, then the sync */
async function undoReapply(repositoryPath: string, branch: string, token: UndoToken | null): Promise<boolean> {
  return token !== null && (await undoRewrite(repositoryPath, token, `Undoing the reapply of ${branch}`, `Cannot undo the reapply of ${branch}`))
}
