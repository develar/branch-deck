import type { RewriteIntent, UndoToken } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { closeEventToasts, toastKeys } from "@/lib/event-toast"
import { runRewriteIntent, undoRewrite, witnessed } from "@/lib/rewrite-run"
import { showUndoToast } from "@/lib/undo-toast"
import { getBranch } from "@/stores/branch-sync"
import { getSelectedRepositoryPath } from "@/stores/repository"

/**
 * Pick Remote Commits (`bd pick --remote`): the commits someone else pushed to the remote branch, which a push would
 * drop, picked onto HEAD with the `(branch)` prefix, so the sync puts them into the branch and its push then drops
 * nothing. The rewrite `pickRemote` of the engine, queued behind the other rewrites by the session: the fetch and the
 * pick in Rust, leased on the remote tip the row shows (a remote that moved again picks nothing), the sync, then the
 * toast with Undo. Resolves to false, after an error toast, when nothing changed.
 */
export async function pickRemoteCommits(branchName: string, expectedRemote?: string | null): Promise<boolean> {
  const branch = getBranch(branchName)
  if (branch === undefined) {
    return false
  }
  // the repository of the click: the pick and its Undo go there, whichever the main window shows when they run
  const repositoryPath = getSelectedRepositoryPath()
  const rewrite = witnessed<Extract<RewriteIntent, { rewrite: "pickRemote" }>>(repositoryPath, {
    rewrite: "pickRemote",
    branchName,
    parentBranch: branch.stack?.effectiveBase ?? null,
    myEmail: branch.myEmail,
    expectedRemote: expectedRemote ?? branch.remoteStatus?.remoteTip ?? null,
  })
  const done = await runRewriteIntent({ repositoryPath }, rewrite)
  if (done?.result.rewrite !== "picked") {
    return false
  }
  // the toast of the remote commits of the branch told what this took in
  closeEventToasts(toastKeys.push(repositoryPath, branchName))
  const { answer } = done.result
  const { undo } = done
  showUndoToast(
    repositoryPath,
    `Picked the remote commits of ${branchName}`,
    () => undoPick(repositoryPath, branchName, undo),
    `${formatCommitCount(answer.picked.length)} into ${branchName} · its push drops nothing now`,
    done.outcome.op,
  )
  return true
}

/** The undo of a pick of remote commits: HEAD and the files back as they were; a rewrite of its own, then the sync */
async function undoPick(repositoryPath: string, branchName: string, token: UndoToken | null): Promise<boolean> {
  const label = `Undoing the pick of the remote commits of ${branchName}`
  return token !== null && (await undoRewrite(repositoryPath, token, label, `Cannot undo the pick of the remote commits of ${branchName}`))
}
