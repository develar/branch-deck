import { ArchiveXIcon, CombineIcon, CopyIcon, ExternalLinkIcon, FlaskConicalIcon, GitPullRequestCreateIcon, GitPullRequestIcon, Layers2Icon, LayersIcon, PencilIcon, PenLineIcon, RotateCcwIcon, ShieldCheckIcon, ShieldOffIcon, TagIcon, UngroupIcon, UploadIcon } from "lucide-react"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import { restackBranch } from "@/hooks/useStackActions"
import { pushStack } from "@/lib/git/push"
import { copyText } from "@/lib/clipboard"
import { getNewPullRequestUrl, openNewPullRequest } from "@/lib/git/new-pull-request"
import { getPullRequestBase, getRetargetCommand } from "@/lib/git/stacked-push-toast"
import { formatBlockedReason, formatPushStackLabel, stackPushOrder } from "@/lib/stack"
import { getListFixups } from "@/lib/autosquash"
import type { PatronusRobot } from "@/lib/bindings"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { canCancelRobot, canRestartRobot, canStartRobot, formatPushMode, openRobot, SAFE_PUSH_DESCRIPTION } from "@/lib/patronus"
import { runRobotAction } from "@/lib/patronus-actions"
import { createBranchMergeRequest } from "@/lib/patronus-merge-request"
import { openPullRequest } from "@/lib/pull-request"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getAutosquashPlan, getBranch, getIssueGroups, getStackChildren, getStackDescendants, isStacksEnabled } from "@/stores/branch-sync"
import { openInline } from "@/stores/inline-row-action"
import { cancelPatronusRobot, getBranchRobot, isPatronusAvailable, isRobotNeedingReview, restartPatronusRobot } from "@/stores/patronus"
import { getPullRequest } from "@/stores/pull-requests"

/**
 * Context menu of an active branch row: copy actions (the names, the pull request base, the retarget command), then
 * rename (name an issue branch; an issue group is renamed without a rewrite) and ungroup, then commit actions (issue
 * reference, amend, unapply), then history (squash fixups, open or create the pull request), then Patronus (Safe Push,
 * Dry Run, Create Merge Request, the robot of the branch). An item that opens a form ends with "…". The right-click on
 * the row and the "⋯" of its Actions cell show it.
 */
export function getBranchContextMenuItems(branch: Branch, setExpanded: (name: string, expanded: boolean) => void): MenuItemGroups {
  const groups: MenuItemGroups = [getBranchCopyMenuItems(branch.name)]
  if (isStacksEnabled()) {
    groups[0]!.push({
      label: "Copy PR Base",
      icon: CopyIcon,
      onSelect: () => void copyText(getPullRequestBase(branch)),
    })
    const retargetCommand = getRetargetCommand(branch)
    if (retargetCommand !== null) {
      groups[0]!.push({ label: "Copy Retarget Command", icon: CopyIcon, onSelect: () => void copyText(retargetCommand) })
    }
  }

  const pullRequest = getPullRequest(branch.name)
  const openPullRequestItem: MenuItem | null = pullRequest === undefined
    ? null
    // the key of the branch preview
    : { label: "Open Pull Request", icon: GitPullRequestIcon, kbds: ["o"], onSelect: () => openPullRequest(pullRequest) }

  const robot = getBranchRobot(branch.name)
  const openRobotItem: MenuItem | null = robot === undefined
    ? null
    : { label: "Open in Patronus", icon: ExternalLinkIcon, onSelect: () => openRobot(robot) }

  const canModify = branch.commits.length > 0 && !branch.hasError && branch.status !== "Syncing"
  if (!canModify) {
    // opening the pull request or the robot changes nothing
    const openItems = [openPullRequestItem, openRobotItem].filter(item => item !== null)
    if (openItems.length > 0) {
      groups.push(openItems)
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
  const ungroupLabel = getRemovePrefixLabel(branch.commits, getIssueGroups())
  if (branch.origin === "Prefix" && ungroupLabel !== null) {
    nameActions.push({
      label: ungroupLabel === "Remove Prefix" ? "Ungroup" : `Ungroup: ${ungroupLabel}`,
      icon: UngroupIcon,
      // the branches stacked on it lose their parent: confirm with them listed
      onSelect: () => getStackChildren(branch.name).length > 0 ? openInline("ungroup", branch.name) : void moveCommits(branch.commits, null),
    })
  }
  groups.push(nameActions)
  if (isStacksEnabled()) {
    groups.push(getStackMenuItems(branch))
  }

  const commitActions: MenuItem[] = [
    {
      label: branch.allCommitsHaveIssueReferences ? "Add Issue Reference (all have)" : "Add Issue Reference…",
      icon: TagIcon,
      disabled: branch.allCommitsHaveIssueReferences,
      onSelect: () => openInline("issue-reference", branch.name),
    },
    {
      label: "Amend Changes…",
      icon: PenLineIcon,
      onSelect: () => {
        // show the commits; the form loads the uncommitted changes when it opens
        setExpanded(branch.name, true)
        openInline("amend-changes", branch.name)
      },
    },
    {
      label: "Unapply…",
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
      label: `Squash Fixups (${fixupCount})…`,
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
  historyActions.push(...getCreatePullRequestItems(branch))
  if (historyActions.length > 0) {
    groups.push(historyActions)
  }
  if (isPatronusAvailable()) {
    groups.push(getPatronusMenuItems(branch, robot, openRobotItem))
  }
  return groups
}

/**
 * Safe Push…, Dry Run… (disabled with the reason of `canStartRobot`), Create Merge Request…, then the restart, the cancel
 * and the page of the branch's robot
 */
function getPatronusMenuItems(branch: Branch, robot: PatronusRobot | undefined, openRobotItem: MenuItem | null): MenuItem[] {
  const start = canStartRobot(branch, robot)
  const items: MenuItem[] = [
    { label: "Safe Push…", icon: ShieldCheckIcon, disabled: !start.ok, description: start.ok ? SAFE_PUSH_DESCRIPTION : start.reason, onSelect: () => openInline("safe-push", branch.name) },
    { label: "Dry Run…", icon: FlaskConicalIcon, disabled: !start.ok, ...(!start.ok && { description: start.reason }), onSelect: () => openInline("dry-run", branch.name) },
    getCreateMergeRequestItem(branch),
  ]
  if (robot !== undefined) {
    const mode = formatPushMode(robot.pushMode)
    if (canRestartRobot(robot, branch.name, branch.remoteStatus?.remoteTip ?? null, isRobotNeedingReview(robot.id))) {
      items.push({ label: `Restart ${mode}`, icon: RotateCcwIcon, onSelect: () => void runRobotAction(restartPatronusRobot, robot, `${mode} restarted`, `Cannot restart the ${mode}`) })
    }
    if (canCancelRobot(robot)) {
      items.push({ label: `Cancel ${mode}`, icon: ShieldOffIcon, onSelect: () => void runRobotAction(cancelPatronusRobot, robot, `${mode} is canceling`, `Cannot cancel the ${mode}`) })
    }
  }
  if (openRobotItem !== null) {
    items.push(openRobotItem)
  }
  return items
}

/**
 * "Create Merge Request…": the step of the Safe Push form on its own, so the code owners approve before a start; it
 * pushes the branch first when the remote lacks its tip, which a blocked branch can't
 */
function getCreateMergeRequestItem(branch: Branch): MenuItem {
  const blocked = branch.status === "Blocked"
  return {
    label: "Create Merge Request…",
    icon: GitPullRequestCreateIcon,
    disabled: blocked,
    description: blocked ? formatBlockedReason(branch) : "Pushes the branch if needed and opens its Space merge request, for the code-owner approvals.",
    onSelect: () => void createBranchMergeRequest(branch.name),
  }
}

/** "Create Pull Request": the GitHub form of a pull request of the pushed branch into its base, while it has none */
function getCreatePullRequestItems(branch: Branch): MenuItem[] {
  const url = getNewPullRequestUrl(branch)
  return url === null ? [] : [{ label: "Create Pull Request", icon: GitPullRequestCreateIcon, onSelect: () => openNewPullRequest(url) }]
}

/** The most branches "Stack On…" lists; the newest come first */
const STACK_TARGET_LIMIT = 20

/** "Stack On…" (any other branch that isn't stacked on this one) and "Unstack" */
function getStackMenuItems(branch: Branch): MenuItem[] {
  const parent = branch.stack?.declaredParent ?? null
  const excluded = new Set([branch.name, ...getStackDescendants(branch.name)])
  const targets = branchSyncStore.getState().branchOrder.filter(name => !excluded.has(name) && name !== parent).slice(0, STACK_TARGET_LIMIT)
  const items: MenuItem[] = [
    {
      label: "Stack On…",
      icon: LayersIcon,
      disabled: targets.length === 0,
      onSelect: () => {},
      children: targets.map(target => ({ label: target, onSelect: () => void restackBranch(branch.name, target) })),
    },
  ]
  if (parent !== null) {
    items.push({ label: `Unstack from ${parent}`, icon: Layers2Icon, onSelect: () => void restackBranch(branch.name, null) })
  }
  // the parents that have work for the remote are pushed first, so the pull request shows only the branch's commits
  const pushOrder = stackPushOrder(branch.name, getBranch)
  if (pushOrder.length > 1) {
    items.push({ label: formatPushStackLabel(pushOrder.length), icon: UploadIcon, disabled: branch.status === "Blocked", onSelect: () => void pushStack(branch.name) })
  }
  return items
}

export function useBranchContextActions(setExpanded: (name: string, expanded: boolean) => void) {
  return {
    getContextMenuItems: (branch: Branch) => getBranchContextMenuItems(branch, setExpanded),
  }
}
