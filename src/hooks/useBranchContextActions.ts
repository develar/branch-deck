import { ArchiveXIcon, PencilIcon, PenLineIcon, TagIcon, UngroupIcon } from "lucide-react"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import type { Branch } from "@/stores/branch-sync"
import { openInline } from "@/stores/inline-row-action"

/**
 * Context menu of an active branch row: copy actions, then rename (name an issue branch) and ungroup, then commit
 * actions (issue reference, amend, unapply).
 */
export function getBranchContextMenuItems(branch: Branch, setExpanded: (name: string, expanded: boolean) => void): MenuItemGroups {
  const groups: MenuItemGroups = [getBranchCopyMenuItems(branch.name)]

  const canModify = branch.commits.length > 0 && !branch.hasError && branch.status !== "Syncing"
  if (!canModify) {
    return groups
  }

  const nameActions: MenuItem[] = [
    {
      label: branch.origin === "Issue" ? "Name This Branch…" : "Rename…",
      icon: PencilIcon,
      onSelect: () => openInline("rename", branch.name),
    },
  ]
  const ungroupLabel = getRemovePrefixLabel(branch.commits)
  if (branch.origin === "Prefix" && ungroupLabel !== null) {
    nameActions.push({
      label: ungroupLabel === "Remove Prefix" ? "Ungroup" : `Ungroup: ${ungroupLabel}`,
      icon: UngroupIcon,
      onSelect: () => void moveCommits(branch.commits, null),
    })
  }
  groups.push(nameActions)

  const commitActions: MenuItem[] = [
    {
      label: branch.allCommitsHaveIssueReferences ? "Add Issue Reference (all have)" : "Add Issue Reference",
      icon: TagIcon,
      disabled: branch.allCommitsHaveIssueReferences,
      onSelect: () => openInline("issue-reference", branch.name),
    },
    {
      label: "Amend Changes",
      icon: PenLineIcon,
      onSelect: () => {
        // show the commits; the form loads the uncommitted changes when it opens
        setExpanded(branch.name, true)
        openInline("amend-changes", branch.name)
      },
    },
    {
      label: "Unapply",
      icon: ArchiveXIcon,
      onSelect: () => {
        // show the commits that will be unapplied
        setExpanded(branch.name, true)
        openInline("unapply", branch.name)
      },
    },
  ]
  groups.push(commitActions)
  return groups
}

export function useBranchContextActions(setExpanded: (name: string, expanded: boolean) => void) {
  return {
    getContextMenuItems: (branch: Branch) => getBranchContextMenuItems(branch, setExpanded),
  }
}
