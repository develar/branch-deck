import { GitBranchIcon } from "lucide-react"
import { getCommitCopyMenuItems } from "@/hooks/useCommitCopyActions"
import type { CommitUnion } from "@/lib/commit"
import type { MenuItemGroups } from "@/lib/menu"

/** Context menu of unassigned commits: "Group into Branch", then the copy actions. */
export function getUnassignedCommitContextMenuItems(selectedCommits: readonly CommitUnion[], onGroupIntoBranch: () => void): MenuItemGroups {
  if (selectedCommits.length === 0) {
    return []
  }

  const isMultiple = selectedCommits.length > 1
  return [
    [
      {
        label: isMultiple ? `Group ${selectedCommits.length} Commits into Branch` : "Group into Branch",
        icon: GitBranchIcon,
        onSelect: onGroupIntoBranch,
      },
    ],
    getCommitCopyMenuItems(selectedCommits),
  ]
}

const unassignedCommitContextActions = { getContextMenuItems: getUnassignedCommitContextMenuItems }

export function useUnassignedCommitContextActions(): typeof unassignedCommitContextActions {
  return unassignedCommitContextActions
}
