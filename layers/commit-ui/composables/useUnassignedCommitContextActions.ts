import type { Commit, MissingCommit } from "~/utils/bindings"
import type { SyncedCommit } from "~/composables/branchSyncProvider"

// Union type for all supported commit types
type CommitUnion = Commit | SyncedCommit | MissingCommit

export interface UnassignedCommitContextActionsReturn {
  getContextMenuItems: (selectedCommits: CommitUnion[], onGroupIntoBranch: () => void) => Array<Array<{ label: string, icon: string, disabled?: boolean, onSelect: () => void }>>
}

export function useUnassignedCommitContextActions(): UnassignedCommitContextActionsReturn {
  const { getCopyMenuItems } = useCommitCopyActions()

  const getContextMenuItems = (selectedCommits: CommitUnion[], onGroupIntoBranch: () => void) => {
    if (selectedCommits.length === 0) {
      return []
    }

    const items = []
    const isMultiple = selectedCommits.length > 1

    // Primary action: Group into Branch
    items.push([
      {
        label: isMultiple
          ? `Group ${selectedCommits.length} Commits into Branch`
          : "Group into Branch",
        icon: "i-lucide-git-branch",
        onSelect: onGroupIntoBranch,
      },
    ])

    // Copy actions
    items.push(getCopyMenuItems(selectedCommits))

    return items
  }

  return {
    getContextMenuItems,
  }
}