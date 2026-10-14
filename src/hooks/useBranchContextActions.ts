import { ArchiveXIcon, CombineIcon, GitPullRequestIcon, PencilIcon, PenLineIcon, TagIcon, UngroupIcon } from "lucide-react"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import { getListFixups } from "@/lib/autosquash"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { openPullRequest } from "@/lib/pull-request"
import type { Branch } from "@/stores/branch-sync"
import { getAutosquashPlan } from "@/stores/branch-sync"
import { openInline } from "@/stores/inline-row-action"
import { getPullRequest } from "@/stores/pull-requests"

/**
 * Context menu of an active branch row: copy actions, then rename (name an issue branch) and ungroup, then commit
 * actions (issue reference, amend, unapply), then history (squash fixups, the pull request).
 */
export function getBranchContextMenuItems(branch: Branch, setExpanded: (name: string, expanded: boolean) => void): MenuItemGroups {
  const groups: MenuItemGroups = [getBranchCopyMenuItems(branch.name)]

  const pullRequest = getPullRequest(branch.name)
  const openPullRequestItem: MenuItem | null = pullRequest === undefined
    ? null
    : { label: "Open Pull Request", icon: GitPullRequestIcon, onSelect: () => openPullRequest(pullRequest) }

  const canModify = branch.commits.length > 0 && !branch.hasError && branch.status !== "Syncing"
  if (!canModify) {
    // opening the pull request changes nothing
    if (openPullRequestItem !== null) {
      groups.push([openPullRequestItem])
    }
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

  const historyActions: MenuItem[] = []
  const fixupCount = getListFixups(getAutosquashPlan(), branch.name).count
  if (fixupCount > 0) {
    historyActions.push({
      label: `Squash Fixups (${fixupCount})`,
      icon: CombineIcon,
      onSelect: () => {
        // the confirmation opens from the button above the commits
        setExpanded(branch.name, true)
        openInline("squash-fixups", branch.name)
      },
    })
  }
  if (openPullRequestItem !== null) {
    historyActions.push(openPullRequestItem)
  }
  if (historyActions.length > 0) {
    groups.push(historyActions)
  }
  return groups
}

export function useBranchContextActions(setExpanded: (name: string, expanded: boolean) => void) {
  return {
    getContextMenuItems: (branch: Branch) => getBranchContextMenuItems(branch, setExpanded),
  }
}
