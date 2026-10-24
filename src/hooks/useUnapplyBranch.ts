import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { runRewrite, runUndo } from "@/lib/rewrite"
import type { Branch } from "@/stores/branch-sync"
import { branchSubject } from "@/stores/operations"
import { vcsRequestFactory } from "@/stores/repository"

export interface UnapplyOptions {
  /** The surface that started it and shows its spinner (`inlineSurface` of the confirmation) */
  surface?: string | null
}

/**
 * Takes the commits of the branch and their files out of HEAD and keeps the branch as an unapplied one, as one rewrite
 * (`runRewrite`): the unapply, the sync, then the toast with Undo, which brings the commits, the files and the virtual
 * branch back. Resolves to false, after an error toast, if nothing changed.
 */
export async function unapplyBranch(branch: Branch, options: UnapplyOptions = {}): Promise<boolean> {
  if (branch.commits.length === 0) {
    // nothing to do: not an error
    toast.add({ type: "info", title: "No commits to unapply" })
    return false
  }
  const { repositoryPath } = vcsRequestFactory.createRequest()
  const result = await runRewrite(
    { id: "unapply", subject: branchSubject(branch.name), objects: [branch.name], label: `Unapplying ${branch.name}`, surface: options.surface ?? null },
    `Cannot unapply ${branch.name}`,
    wait =>
      commands.unapplyBranch(
        {
          repositoryPath,
          branchName: branch.name,
          originalCommitIds: branch.commits.map(commit => commit.originalHash),
        },
        wait,
      ),
    data => ({
      sync: true,
      after: () =>
        showUndoToast(
          `Unapplied ${branch.name}`,
          () => undoUnapply(repositoryPath, branch.name, data.undoToken),
          `Removed ${formatCommitCount(data.commitsRemoved.length)} from HEAD · kept in ${data.unappliedBranchName}`,
        ),
    }),
  )
  return result !== null
}

/** The undo of an unapply: HEAD, the files and the virtual branch back as they were; a rewrite of its own, then the sync */
async function undoUnapply(repositoryPath: string, branchName: string, undoToken: string): Promise<boolean> {
  return await runUndo(
    { id: "undo", subject: `undo:${undoToken}`, objects: [branchName], label: `Undoing the unapply of ${branchName}`, repositoryPath },
    `Cannot undo the unapply of ${branchName}`,
    wait => commands.undoUnapplyBranch({ repositoryPath, undoToken }, wait),
  )
}
