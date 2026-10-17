import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import type { Branch } from "@/stores/branch-sync"
import { syncBranches } from "@/stores/branch-sync"
import { withRowProcessing } from "@/stores/inline-row-action"
import { vcsRequestFactory } from "@/stores/repository"

/** Removes the branch commits from HEAD and moves the branch to the unapplied state, then re-syncs. */
export async function unapplyBranch(branch: Branch): Promise<void> {
  if (branch.commits.length === 0) {
    // nothing to do: not an error
    toast.add({ type: "info", title: "No commits to unapply" })
    return
  }

  const originalCommitIds = branch.commits.map(commit => commit.originalHash)
  const data = await withRowProcessing(
    branch.name,
    async () => {
      const vcsRequest = vcsRequestFactory.createRequest()
      const result = await commands.unapplyBranch({
        repositoryPath: vcsRequest.repositoryPath,
        branchName: branch.name,
        branchPrefix: vcsRequest.branchPrefix,
        originalCommitIds,
      })
      if (result.status !== "ok") {
        throw new Error(result.error.message)
      }
      return result.data
    },
    {
      processingMessage: `Unapplying ${branch.name}…`,
      success: data => ({
        title: `Unapplied ${branch.name}`,
        description: `Removed ${formatCommitCount(data.commitsRemoved.length)} from HEAD · kept in ${data.unappliedBranchName}`,
      }),
      errorTitle: () => `Cannot unapply ${branch.name}`,
    },
  )

  if (data) {
    await syncBranches({ autoScroll: false, autoExpand: false })
  }
}
