import { commands } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { closeEventToasts, toastKeys } from "@/lib/event-toast"
import { runRewrite, runUndo } from "@/lib/rewrite"
import { getBranch } from "@/stores/branch-sync"
import { branchSubject } from "@/stores/operations"
import { vcsRequestFactory } from "@/stores/repository"

/**
 * Pick Remote Commits (`bd pick --remote`): the commits someone else pushed to the remote branch, which a push would
 * drop, picked onto HEAD with the `(branch)` prefix, so the sync puts them into the branch and its push then drops
 * nothing. One rewrite (`runRewrite`): the fetch and the pick in Rust, leased on the remote tip the row shows (a remote
 * that moved again picks nothing), the sync, then the toast with Undo. Resolves to false, after an error toast, when
 * nothing changed.
 */
export async function pickRemoteCommits(branchName: string, expectedRemote?: string | null): Promise<boolean> {
  const branch = getBranch(branchName)
  if (branch === undefined) {
    return false
  }
  const { repositoryPath } = vcsRequestFactory.createRequest()
  const result = await runRewrite(
    { id: "pick-remote", subject: branchSubject(branchName), objects: [branchName], label: `Picking the remote commits of ${branchName}`, surface: null },
    `Cannot pick the remote commits of ${branchName}`,
    wait =>
      commands.pickRemoteCommits(
        {
          repositoryPath,
          branchName,
          parentBranch: branch.stack?.effectiveBase ?? null,
          myEmail: branch.myEmail,
          expectedRemote: expectedRemote ?? branch.remoteStatus?.remoteTip ?? null,
        },
        wait,
      ),
    data => ({
      sync: true,
      after: () => {
        // the toast of the remote commits of the branch told what this took in
        closeEventToasts(toastKeys.push(repositoryPath, branchName))
        const undoToken = data.undoToken
        showUndoToast(
          `Picked the remote commits of ${branchName}`,
          undoToken === null ? async () => false : () => undoPick(repositoryPath, branchName, undoToken),
          `${formatCommitCount(data.picked.length)} into ${branchName} · its push drops nothing now`,
        )
      },
    }),
  )
  return result !== null
}

/** The undo of a pick of remote commits: HEAD and the files back as they were; a rewrite of its own, then the sync */
async function undoPick(repositoryPath: string, branchName: string, undoToken: string): Promise<boolean> {
  return await runUndo(
    { id: "undo", subject: `undo:${undoToken}`, objects: [branchName], label: `Undoing the pick of the remote commits of ${branchName}`, repositoryPath },
    `Cannot undo the pick of the remote commits of ${branchName}`,
    wait => commands.undoPickRemoteCommits({ repositoryPath, undoToken }, wait),
  )
}
