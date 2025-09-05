import { commands, type UncommittedChangesResult } from "~/utils/bindings"

/**
 * Composable for amending uncommitted changes to a branch
 * Extracted from useBranchContextActions for better modularity
 */
export function useAmendChanges() {
  const toast = useToast()
  const { selectedProject } = useRepository()
  const { syncBranches } = useBranchSync()
  const inline = useInlineRowAction()

  // State for diff data when amending changes
  const diffData = ref<UncommittedChangesResult | null>(null)
  const diffLoading = ref(false)
  const diffError = ref<string | null>(null)

  // Load uncommitted changes for preview
  const loadUncommittedChanges = async () => {
    // Clear previous state
    diffData.value = null
    diffError.value = null
    diffLoading.value = true

    try {
      const result = await commands.getUncommittedChanges({
        repositoryPath: selectedProject.value?.path || "",
      })

      if (result.status === "ok") {
        diffData.value = result.data
      }
      else {
        diffError.value = result.error
      }
    }
    catch (error) {
      diffError.value = error instanceof Error ? error.message : "Failed to load uncommitted changes"
    }
    finally {
      diffLoading.value = false
    }
  }

  // Amend changes to the branch
  const amendChanges = async (branch: ReactiveBranch) => {
    // Get the tip commit (last commit in the branch)
    const tipCommit = branch.commits?.[branch.commits.length - 1]
    if (!tipCommit) {
      toast.add({
        title: `${branch.name}: No Commits Found`,
        description: "Branch has no commits to amend changes to",
        color: "error",
      })
      return
    }

    // Close current inline (the form)
    inline.closeInline()
    inline.processingKey.value = branch.name

    try {
      const result = await commands.amendUncommittedToBranch({
        repositoryPath: selectedProject.value?.path || "",
        branchName: branch.name,
        originalCommitId: tipCommit.originalHash,
        mainBranch: "master", // TODO: get from repository settings
      })

      if (result.status !== "ok") {
        toast.add({
          color: "error",
          title: `${branch.name}: Failed to Amend Changes`,
          description: result.error,
        })
        return
      }

      // Handle the new AmendCommandResult structure
      if (result.data.status === "branchError") {
        const branchError = result.data.data
        if ("MergeConflict" in branchError) {
          // Show conflict viewer inline at the same location as the form
          inline.openInline("amend-conflict", branch.name, branchError.MergeConflict)
          return // Don't sync, rebase was aborted
        }
        else {
          // Generic branch error
          toast.add({
            color: "error",
            title: `${branch.name}: Failed to Amend Changes`,
            description: branchError.Generic,
          })
          return
        }
      }

      // Success case
      const { amendedCommitId, rebasedToCommit } = result.data.data
      toast.add({
        color: "success",
        title: `${branch.name}: Changes Amended`,
        description: `Amended commit ${amendedCommitId.slice(0, 8)} → ${rebasedToCommit.slice(0, 8)}`,
        duration: 5000,
      })

      // Sync branches on success
      await syncBranches({ autoScroll: false, autoExpand: false })
    }
    catch (error) {
      toast.add({
        color: "error",
        title: `${branch.name}: Failed to Amend Changes`,
        description: error instanceof Error ? error.message : "Failed to amend uncommitted changes",
      })
    }
    finally {
      inline.processingKey.value = null
    }
  }

  const handleSubmit = (branch: ReactiveBranch) => {
    inline.withPostSubmit(() => amendChanges(branch))
  }

  return {
    amendChanges: handleSubmit,
    loadUncommittedChanges,
    diffData,
    diffLoading,
    diffError,
  }
}
