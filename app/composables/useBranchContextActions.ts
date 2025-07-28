import { commands } from "~/utils/bindings"

export interface BranchContextActionsReturn {
  // State
  inlineInputActiveBranch: Ref<string | null>
  processingBranch: Ref<string | null>

  // Methods
  getContextMenuItems: (branch: ReactiveBranch) => Array<Array<{ label: string, icon: string, onSelect: () => void }>>
  hideInlineInput: () => void
  handleInlineSubmit: (issueReference: string, branch: ReactiveBranch) => void
}

export function useBranchContextActions(): BranchContextActionsReturn {
  const toast = useToast()
  const { copyToClipboard } = useCopyToClipboard()
  const { effectiveBranchPrefix, selectedProject } = useRepository()
  const { syncBranches } = useBranchSync()

  // State for inline input
  const inlineInputActiveBranch = ref<string | null>(null)

  // Processing state
  const processingBranch = ref<string | null>(null)

  // Get full branch name with prefix
  const getFullBranchName = (branchName: string) => {
    return `${effectiveBranchPrefix.value}/${branchName}`
  }

  // Context menu items
  const getContextMenuItems = (branch: ReactiveBranch) => {
    const hasCommits = branch.commits.length > 0
    const canAddIssueReference = hasCommits && !branch.hasError && branch.status !== "Syncing"

    const items = []

    // Add Issue Reference action (if applicable)
    if (canAddIssueReference) {
      items.push([
        {
          label: "Add Issue Reference",
          icon: "i-lucide-tag",
          onSelect: () => showInlineInput(branch),
        },
      ])
    }

    // Copy actions at the end with separator
    items.push([
      {
        label: "Copy Branch Name",
        icon: "i-lucide-copy",
        onSelect: () => {
          // noinspection JSIgnoredPromiseFromCall
          copyToClipboard(branch.name)
        },
      },
      {
        label: "Copy Full Branch Name",
        icon: "i-lucide-copy",
        onSelect: () => {
          // noinspection JSIgnoredPromiseFromCall
          copyToClipboard(getFullBranchName(branch.name))
        },
      },
    ])

    return items
  }

  // Show inline input for adding issue reference
  const showInlineInput = (branch: ReactiveBranch) => {
    const targetBranch = branch.name
    // always close first if open
    if (inlineInputActiveBranch.value) {
      inlineInputActiveBranch.value = null
      // Use nextTick to ensure dialog closes before reopening
      nextTick(() => {
        inlineInputActiveBranch.value = targetBranch
      })
    }
    else {
      inlineInputActiveBranch.value = targetBranch
    }
  }

  // Hide inline input
  const hideInlineInput = () => {
    inlineInputActiveBranch.value = null
  }

  // Handle inline submission
  const handleInlineSubmit = (issueReference: string, branch: ReactiveBranch) => {
    hideInlineInput()
    nextTick(() => {
      // noinspection JSIgnoredPromiseFromCall
      handleAddIssueReference(issueReference, branch)
    })
  }

  // Handle adding issue reference
  const handleAddIssueReference = async (issueReference: string, branch: ReactiveBranch) => {
    // Get commits with hash and message
    const commits = Array.from(branch.commits.values()).map(commit => ({
      hash: commit.originalHash,
      message: commit.message,
    }))

    // Validate we have commits to update
    if (commits.length === 0) {
      toast.add({
        title: `${branch.name}: No Commits Found`,
        description: "Branch has no commits to add issue reference to",
        color: "error",
      })
      return
    }

    // Show processing state
    processingBranch.value = branch.name
    try {
      const result = await commands.addIssueReferenceToCommits({
        repositoryPath: selectedProject.value?.path || "",
        branchName: branch.name,
        commits,
        issueReference,
      })

      if (result.status === "ok") {
        const { updatedCount, skippedCount } = result.data

        let title: string
        let description: string

        if (updatedCount > 0) {
          title = `${branch.name}: Added ${issueReference}`
          description = `Updated ${updatedCount} commit${updatedCount === 1 ? "" : "s"}`
          if (skippedCount > 0) {
            description += ` (${skippedCount} already had references)`
          }
        }
        else {
          title = "No Changes Made"
          description = `All ${skippedCount} commits already have issue references`
        }

        toast.add({
          title,
          description,
          color: "success",
          duration: 5000,
        })

        // Refresh branches without auto-expand or auto-scroll
        syncBranches({ autoScroll: false, autoExpand: false })
      }
      else {
        throw new Error(result.error)
      }
    }
    catch (error) {
      toast.add({
        title: `${branch.name}: Failed to Add Issue Reference`,
        description: error instanceof Error ? error.message : "Failed to add issue reference",
        color: "error",
      })
    }
    finally {
      processingBranch.value = null
    }
  }

  return {
    // State
    inlineInputActiveBranch,
    processingBranch,

    // Methods
    getContextMenuItems,
    hideInlineInput,
    handleInlineSubmit,
  }
}
