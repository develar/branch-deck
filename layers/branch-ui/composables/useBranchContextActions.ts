export interface BranchContextActionsReturn {
  // Methods
  getContextMenuItems: (branch: ReactiveBranch) => Array<Array<{ label: string, icon: string, disabled?: boolean, onSelect: () => void }>>
}

export function useBranchContextActions(options: { setExpanded: (item: ReactiveBranch, expanded: boolean) => void }): BranchContextActionsReturn {
  const { getCopyMenuItems } = useBranchCopyActions()

  const inline = useInlineRowAction()

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

  // Handle amend changes action (load diff and open form)
  const handleAmendChangesAction = (branch: ReactiveBranch) => {
    // Expand the branch to show commits
    options.setExpanded(branch, true)
    // Open inline form immediately (the dialog will load data when it opens)
    inline.openInline("amend-changes", branch.name)
  }

  return {
    // Methods
    getContextMenuItems,
  }
}
