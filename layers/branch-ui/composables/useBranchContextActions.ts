import { commands, type UncommittedChangesResult } from "~/utils/bindings"

export interface BranchContextActionsReturn {
  // State
  activeInline: Ref<ActiveInline | null>
  processingBranch: Ref<string | null>
  diffData: Ref<UncommittedChangesResult | null>
  diffLoading: Ref<boolean>
  diffError: Ref<string | null>

  // Methods
  getContextMenuItems: (branch: ReactiveBranch) => Array<Array<{ label: string, icon: string, disabled?: boolean, onSelect: () => void }>>
  hideInlineInput: () => void
  handleInlineSubmit: (value: string, branch: ReactiveBranch) => void
  portalTargetIdFor: (branchName: string) => string
  // Row processing helpers
  isProcessing: (key: string) => boolean
  pulseClass: (key: string) => string
}

export function useBranchContextActions(options?: { setExpanded?: (item: ReactiveBranch, expanded: boolean) => void }): BranchContextActionsReturn {
  const toast = useToast()
  const { selectedProject } = useRepository()
  const { syncBranches } = useBranchSync()
  const { getCopyMenuItems } = useBranchCopyActions()

  // Inline row action for forms (issue-reference, etc.)
  const inline = useInlineRowAction()

  // Processing state delegates to inline processing key
  const processingBranch = inline.processingKey

  // State for diff data when amending changes
  const diffData = ref<UncommittedChangesResult | null>(null)
  const diffLoading = ref(false)
  const diffError = ref<string | null>(null)

  // Context menu items
  const getContextMenuItems = (branch: ReactiveBranch) => {
    const hasCommits = branch.commits?.length > 0
    const canShowIssueReference = hasCommits && !branch.hasError && branch.status !== "Syncing"
    const canAmendChanges = hasCommits && !branch.hasError && branch.status !== "Syncing"

    const items = []

    // Copy actions at the end with separator
    items.push(getCopyMenuItems(branch.name))

    const commitActions = []
    // Add Issue Reference action (if applicable)
    if (canShowIssueReference) {
      commitActions.push({
        label: branch.allCommitsHaveIssueReferences ? "Add Issue Reference (all have)" : "Add Issue Reference",
        icon: "i-lucide-tag",
        disabled: branch.allCommitsHaveIssueReferences,
        onSelect: () => inline.openInline("issue-reference", branch.name),
      })
    }

    // Amend Changes action (if applicable)
    if (canAmendChanges) {
      commitActions.push({
        label: "Amend Changes",
        icon: "i-lucide-edit-3",
        onSelect: () => handleAmendChangesAction(branch),
      })
    }

    if (commitActions.length > 0) {
      items.push(commitActions)
    }

    return items
  }

  // Hide inline input
  const hideInlineInput = () => {
    inline.closeInline()
  }

  // Handle amend changes action (load diff and open form)
  const handleAmendChangesAction = async (branch: ReactiveBranch) => {
    // Expand the branch to show commits
    if (options?.setExpanded) {
      options.setExpanded(branch, true)
    }

    // Open inline form immediately
    inline.openInline("amend-changes", branch.name)

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

  // Handle inline submission
  const handleInlineSubmit = (value: string, branch: ReactiveBranch) => {
    if (!inline.activeInline.value) {
      return
    }

    const actionType = inline.activeInline.value.type
    if (actionType === "issue-reference") {
      inline.withPostSubmit(() => handleAddIssueReference(value, branch))
    }
    else if (actionType === "amend-changes") {
      // For amend changes, we don't need the value (commit message)
      inline.withPostSubmit(() => handleAmendChanges(branch))
    }
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

  // Handle amending changes to branch
  const handleAmendChanges = async (branch: ReactiveBranch) => {
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

  return {
    // State
    activeInline: inline.activeInline,
    processingBranch,
    diffData,
    diffLoading,
    diffError,

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
