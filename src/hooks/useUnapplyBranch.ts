import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import type { Branch } from "@/stores/branch-sync"
import { syncBranches } from "@/stores/branch-sync"
import { withRowProcessing } from "@/stores/inline-row-action"
import { vcsRequestFactory } from "@/stores/repository"

/** Removes the branch commits from HEAD and moves the branch to the unapplied state, then re-syncs. */
export async function unapplyBranch(branch: Branch): Promise<void> {
  if (branch.commits.length === 0) {
    toast.add({
      type: "error",
      title: `${branch.name}: No Commits to Unapply`,
      description: "Branch has no commits to unapply",
    })
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
        title: `${branch.name}: Branch Unapplied`,
        description: `${data.commitsRemoved.length} commits removed from HEAD, branch moved to ${data.unappliedBranchName}`,
        duration: 5000,
      }),
      error: error => ({
        title: `${branch.name}: Failed to Unapply Branch`,
        description: error instanceof Error ? error.message : "Failed to unapply branch",
      }),
    },
  )

  if (data) {
    await syncBranches({ autoScroll: false, autoExpand: false })
  }
}

const unapplyActions = { unapplyBranch }

export function useUnapplyBranch(): typeof unapplyActions {
  return unapplyActions
}
