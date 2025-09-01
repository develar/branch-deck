import { commands } from "~/utils/bindings"

export interface BranchContextActionsReturn {
  // State
  activeInline: Ref<ActiveInline | null>
  processingBranch: Ref<string | null>

  // Methods
  getContextMenuItems: (branch: ReactiveBranch) => Array<Array<{ label: string, icon: string, onSelect: () => void }>>
  hideInlineInput: () => void
  handleInlineSubmit: (issueReference: string, branch: ReactiveBranch) => void
  portalTargetIdFor: (branchName: string) => string
  // Row processing helpers
  isProcessing: (key: string) => boolean
  pulseClass: (key: string) => string
}

export function useBranchContextActions(): BranchContextActionsReturn {
  const toast = useToast()
  const { selectedProject } = useRepository()
  const { syncBranches } = useBranchSync()
  const { getCopyMenuItems } = useBranchCopyActions()

  // Inline row action for forms (issue-reference, etc.)
  const inline = useInlineRowAction()

  // Processing state delegates to inline processing key
  const processingBranch = inline.processingKey

  // Context menu items
  const getContextMenuItems = (branch: ReactiveBranch) => {
    const hasCommits = branch.commits?.length > 0
    const canShowIssueReference = hasCommits && !branch.hasError && branch.status !== "Syncing"

    const items = []

    // Copy actions at the end with separator
    items.push(getCopyMenuItems(branch.name))

    // Add Issue Reference action (if applicable)
    if (canShowIssueReference) {
      items.push([
        {
          label: branch.allCommitsHaveIssueReferences ? "Add Issue Reference (all have)" : "Add Issue Reference",
          icon: "i-lucide-tag",
          disabled: branch.allCommitsHaveIssueReferences,
          onSelect: () => inline.openInline("issue-reference", branch.name),
        },
      ])
    }

    return items
  }

  // Hide inline input
  const hideInlineInput = () => {
    inline.closeInline()
  }

  // Handle inline submission
  const handleInlineSubmit = (issueReference: string, branch: ReactiveBranch) => {
    inline.withPostSubmit(() => handleAddIssueReference(issueReference, branch))
  }

  // Handle adding issue reference
  const handleAddIssueReference = async (issueReference: string, branch: ReactiveBranch) => {
    // Get commits with hash and message
    const commits = branch.commits.map(commit => ({
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

    const data = await inline.withRowProcessing(
      branch.name,
      async () => {
        const result = await commands.addIssueReferenceToCommits({
          repositoryPath: selectedProject.value?.path || "",
          branchName: branch.name,
          commits,
          issueReference,
        })
        if (result.status !== "ok") {
          throw new Error(result.error)
        }
        return result.data
      },
      {
        success: ({ updatedCount, skippedCount }) => {
          if (updatedCount > 0) {
            let description = `Updated ${updatedCount} commit${updatedCount === 1 ? "" : "s"}`
            if (skippedCount > 0) {
              description += ` (${skippedCount} already had references)`
            }
            return { title: `${branch.name}: Added ${issueReference}`, description, duration: 5000 }
          }
          else {
            return { title: "No Changes Made", description: `All ${skippedCount} commits already have issue references`, duration: 5000 }
          }
        },
        error: error => ({
          title: `${branch.name}: Failed to Add Issue Reference`,
          description: error instanceof Error ? error.message : "Failed to add issue reference",
        }),
      },
    )

    if (data) {
      // Refresh branches without auto-expand or auto-scroll
      // noinspection ES6MissingAwait
      syncBranches({ autoScroll: false, autoExpand: false })
    }
  }

  return {
    // State
    activeInline: inline.activeInline,
    processingBranch,

    // Methods
    getContextMenuItems,
    hideInlineInput,
    handleInlineSubmit,
    portalTargetIdFor: inline.portalTargetIdFor,
    // Row processing helpers
    isProcessing: inline.isProcessing,
    pulseClass: inline.pulseClass,
  }
}
