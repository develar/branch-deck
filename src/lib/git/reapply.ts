import { commands } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { runRewrite, runUndo } from "@/lib/rewrite"
import { branchSubject } from "@/stores/operations"
import { vcsRequestFactory } from "@/stores/repository"
import { getUnappliedBranch } from "@/stores/unapplied-branches"

/**
 * Puts the commits of the unapplied branch `name` (its full name) back on top of HEAD, their grouping prefix restored
 * and their files following, and deletes the unapplied branch, as one rewrite (`runRewrite`): the reapply, the sync that
 * builds the branch again, then the toast whose Undo takes the commits out again and keeps the unapplied branch.
 * Resolves to false, after an error toast, if nothing changed
 */
export async function reapplyBranch(name: string): Promise<boolean> {
  const branch = getUnappliedBranch(name)
  if (branch === undefined) {
    return false
  }
  const { repositoryPath } = vcsRequestFactory.createRequest()
  const result = await runRewrite(
    { id: "reapply", subject: branchSubject(name), objects: [name], label: `Reapplying ${branch.branch}` },
    `Cannot reapply ${branch.branch}`,
    wait => commands.reapplyBranch({ repositoryPath, branchName: branch.branch }, wait),
    data => ({
      sync: true,
      after: () =>
        showUndoToast(
          `Reapplied ${branch.branch}`,
          () => undoReapply(repositoryPath, branch.branch, data.undoToken ?? ""),
          `${formatCommitCount(data.reapplied.length)} back on top of HEAD`,
        ),
    }),
  )
  return result !== null
}

/** The undo of a reapply: HEAD and the files back, the unapplied branch again; a rewrite of its own, then the sync */
async function undoReapply(repositoryPath: string, branch: string, undoToken: string): Promise<boolean> {
  return await runUndo(
    { id: "undo", subject: `undo:${undoToken}`, objects: [branch], label: `Undoing the reapply of ${branch}`, repositoryPath },
    `Cannot undo the reapply of ${branch}`,
    wait => commands.undoReapplyBranch({ repositoryPath, undoToken }, wait),
  )
}
