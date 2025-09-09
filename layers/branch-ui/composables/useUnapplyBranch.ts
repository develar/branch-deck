import { commands, type UnapplyBranchResult } from "~/utils/bindings"

/**
 * Composable for unapplying branches (removing commits from HEAD and shelving the branch)
 * Similar structure to useAmendChanges for consistent UX patterns
 */
export function useUnapplyBranch() {
  const toast = useToast()
  const { vcsRequestFactory } = useRepository()
  const { syncBranches } = useBranchSync()
  const { withRowProcessing } = useInlineRowAction()

  // Unapply a branch
  const unapplyBranch = async (branch: ReactiveBranch) => {
    if (!branch.commits || branch.commits.length === 0) {
      toast.add({
        title: `${branch.name}: No Commits to Unapply`,
        description: "Branch has no commits to unapply",
        color: "error",
      })
      return
    }

    // Extract original commit IDs from branch commits
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
          throw new Error(result.error)
        }

        return result.data
      },
      {
        processingMessage: `Unapplying ${branch.name}...`,
        success: (data: UnapplyBranchResult) => ({
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
      // Sync branches on success to refresh the branch list
      await syncBranches({ autoScroll: false, autoExpand: false })
    }
  }

  const handleSubmit = (branch: ReactiveBranch) => {
    unapplyBranch(branch)
  }

  return {
    unapplyBranch: handleSubmit,
  }
}