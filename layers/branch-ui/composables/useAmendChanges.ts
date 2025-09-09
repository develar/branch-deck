import { commands, type UncommittedChangesResult } from "~/utils/bindings"

/**
 * Composable for amending uncommitted changes to a branch
 * Extracted from useBranchContextActions for better modularity
 */
export function useAmendChanges() {
  const toast = useToast()
  const { selectedProject } = useRepository()
  const { syncBranches } = useBranchSync()
  const { withRowProcessing, openInline } = useInlineRowAction()

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
    // Get the tip commit (first commit - newest after .rev() in backend)
    const tipCommit = branch.commits?.[0]
    if (!tipCommit) {
      toast.add({
        title: `${branch.name}: No Commits Found`,
        description: "Branch has no commits to amend changes to",
        color: "error",
      })
      return
    }

    // Ensure we have the uncommitted changes data with file list
    if (!diffData.value?.files) {
      toast.add({
        title: `${branch.name}: No Files to Amend`,
        description: "No uncommitted changes found to amend",
        color: "error",
      })
      return
    }

    const data = await withRowProcessing(
      branch.name,
      async () => {
        const result = await commands.amendUncommittedToBranch({
          repositoryPath: selectedProject.value?.path || "",
          branchName: branch.name,
          originalCommitId: tipCommit.originalHash,
          files: diffData.value!.files.map(f => f.filePath),
        })

        if (result.status !== "ok") {
          throw new Error(result.error)
        }

        // Handle the new AmendCommandResult structure
        if (result.data.status === "branchError") {
          const branchError = result.data.data
          if ("MergeConflict" in branchError) {
            // For conflicts, we need special handling
            throw { type: "conflict", data: branchError.MergeConflict }
          }
          else {
            throw new Error(branchError.Generic)
          }
        }

        return result.data.data
      },
      {
        processingMessage: `Amending changes to ${branch.name}...`,
        success: ({ amendedCommitId, rebasedToCommit }) => ({
          title: `${branch.name}: Changes Amended`,
          description: `Amended commit ${amendedCommitId.slice(0, 8)} → ${rebasedToCommit.slice(0, 8)}`,
          duration: 5000,
        }),
        error: (error) => {
          // Handle conflict case specially
          if (error && typeof error === "object" && "type" in error && error.type === "conflict" && "data" in error) {
            // Show conflict viewer - we need to handle this outside withRowProcessing
            openInline("amend-conflict", branch.name, error.data as import("~/utils/bindings").MergeConflictInfo)
            return { title: "Conflict detected", description: "Opening conflict resolver..." }
          }
          return {
            title: `${branch.name}: Failed to Amend Changes`,
            description: error instanceof Error ? error.message : "Failed to amend uncommitted changes",
          }
        },
      },
    )

    if (data) {
      // Sync branches on success
      await syncBranches({ autoScroll: false, autoExpand: false })
    }
  }

  const handleSubmit = (branch: ReactiveBranch) => {
    amendChanges(branch)
  }

  return {
    amendChanges: handleSubmit,
    loadUncommittedChanges,
    diffData,
    diffLoading,
    diffError,
  }
}
