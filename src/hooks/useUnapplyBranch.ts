import { toast } from "@/components/ui/toast"
import type { RewriteIntent, UndoToken } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { runRewriteIntent, undoRewrite, witnessed } from "@/lib/rewrite-run"
import { showUndoToast } from "@/lib/undo-toast"
import type { Branch } from "@/stores/branch-sync"
import { getSelectedRepositoryPath } from "@/stores/repository"

export interface UnapplyOptions {
  /** The surface that started it and shows its spinner (`inlineSurface` of the confirmation) */
  surface?: string | null
}

/**
 * Takes the commits of the branch and their files out of HEAD and keeps the branch as an unapplied one: the rewrite
 * `unapply` of the engine, queued behind the other rewrites by the session, then the sync, then the toast with Undo,
 * which brings the commits, the files and the virtual branch back. Resolves to false, after an error toast, if nothing
 * changed.
 */
export async function unapplyBranch(branch: Branch, options: UnapplyOptions = {}): Promise<boolean> {
  if (branch.commits.length === 0) {
    // nothing to do: not an error
    toast.add({ type: "info", title: "No commits to unapply" })
    return false
  }
  // the repository of the click: the unapply and its Undo go there, whichever the main window shows when they run
  const repositoryPath = getSelectedRepositoryPath()
  const rewrite = witnessed<Extract<RewriteIntent, { rewrite: "unapply" }>>(repositoryPath, {
    rewrite: "unapply",
    branchName: branch.name,
    originalCommitIds: branch.commits.map(commit => commit.originalHash),
  })
  const spec = { surface: options.surface ?? null, repositoryPath }
  const done = await runRewriteIntent(spec, rewrite)
  if (done?.result.rewrite !== "unapplied") {
    return false
  }
  const { answer } = done.result
  const { undo } = done
  showUndoToast(
    repositoryPath,
    `Unapplied ${branch.name}`,
    () => undoUnapply(repositoryPath, branch.name, undo),
    `Removed ${formatCommitCount(answer.commitsRemoved.length)} from HEAD · kept in ${answer.unappliedBranchName}`,
    done.outcome,
  )
  return true
}

/** The undo of an unapply: HEAD, the files and the virtual branch back as they were; a rewrite of its own, then the sync */
async function undoUnapply(repositoryPath: string, branchName: string, token: UndoToken | null): Promise<boolean> {
  return token !== null && (await undoRewrite(repositoryPath, token, `Undoing the unapply of ${branchName}`, `Cannot undo the unapply of ${branchName}`))
}
