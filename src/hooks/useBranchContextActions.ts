import { ArchiveXIcon, CombineIcon, CopyIcon, ExternalLinkIcon, FileDiffIcon, FlaskConicalIcon, FoldVerticalIcon, FolderInputIcon, GitBranchIcon, GitBranchPlusIcon, GitPullRequestCreateIcon, GitPullRequestIcon, Layers2Icon, LayersIcon, PencilIcon, PenLineIcon, RotateCcwIcon, ShieldCheckIcon, ShieldOffIcon, TagIcon, UnfoldVerticalIcon, UngroupIcon, UploadIcon } from "lucide-react"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import { restackBranch } from "@/hooks/useStackActions"
import { pushBranch, pushStack, pushWithChildren } from "@/lib/git/push"
import { copyText } from "@/lib/clipboard"
import { getNewPullRequestUrl, getPullRequestBase, getRetargetCommand, openNewPullRequest } from "@/lib/git/new-pull-request"
import { formatBlockedReason, formatPushStackLabel, formatPushWithChildrenLabel, stackChildrenPushOrder, stackPushOrder } from "@/lib/stack"
import { getListFixups } from "@/lib/autosquash"
import { branchReviewBlocker } from "@/lib/branch-review"
import { openBranchReview } from "@/lib/branch-review-window"
import { inTableOrder } from "@/lib/branch-selection"
import type { PatronusRobot } from "@/lib/bindings"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { canCancelRobot, canStartRobot, formatPushMode, getRobotRestartAction, openRobot, SAFE_PUSH_DESCRIPTION } from "@/lib/patronus"
import { runRobotAction } from "@/lib/patronus-actions"
import { openPullRequest } from "@/lib/pull-request"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getAutosquashPlan, getBranch, getBranchCommits, getBranchTableOrder, getIssueGroups, getMoveTargets, getStackChildren, getStackDescendants, isStacksEnabled } from "@/stores/branch-sync"
import { getSelectedBranches, selectBranches, setBranchTargetOpen } from "@/stores/commit-move"
import { openInline } from "@/stores/inline-row-action"
import { cancelPatronusRobot, getBranchRobot, getLandingAncestor, getLandingSpan, isPatronusAvailable, isRobotNeedingReview, restartPatronusRobot } from "@/stores/patronus"
import { getPullRequest } from "@/stores/pull-requests"
import { getStackCollapse, toggleStackCollapsed } from "@/stores/stack-collapse"

/** Branches listed in the "Move to Existing" submenu */
const MAX_MENU_TARGETS = 8

/**
 * Context menu of an active branch row: copy actions (the names, the pull request base, the retarget command), then
 * rename (name an issue branch; an issue group is renamed without a rewrite), move to another branch (with the other
 * selected branches, if the row is selected) and ungroup, then commit actions (issue
 * reference, amend, unapply), then history (review the changes, squash fixups, open or create the pull request), then Patronus (Safe Push,
 * Dry Run, the robot of the branch). An item that opens a form ends with "…". The right-click on
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
  nameActions.push(...getMoveMenuItems(branch.name))
  const ungroupLabel = getRemovePrefixLabel(branch.commits, getIssueGroups())
  // an issue group goes back to the issue branches of its keys, without a rewrite
  if (branch.origin !== "Issue" && ungroupLabel !== null) {
    nameActions.push({
      label: ungroupLabel === "Remove Prefix" || ungroupLabel === "Move Back" ? "Ungroup" : `Ungroup: ${ungroupLabel}`,
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

  const reviewBlocker = branchReviewBlocker(branch)
  const historyActions: MenuItem[] = [{
    label: "Review Changes",
    icon: FileDiffIcon,
    kbds: ["d"],
    disabled: reviewBlocker !== null,
    description: reviewBlocker ?? "The diff of the branch in its own window, the parts of its stack, the files of each code owner",
    onSelect: () => void openBranchReview(branch.name),
  }]
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
  groups.push(historyActions)
  if (isPatronusAvailable()) {
    groups.push(getPatronusMenuItems(branch, robot, openRobotItem))
  }
  return groups
}

/**
 * Safe Push…, Dry Run… (disabled with the reason of `canStartRobot`), then the restart (a push first when the branch
 * changed locally, as the chip offers it), the cancel and the page of the branch's robot. The approvals go into the
 * GitHub pull request: "Create Pull Request" is the history group's; no Space merge request (deprecated).
 */
function getPatronusMenuItems(branch: Branch, robot: PatronusRobot | undefined, openRobotItem: MenuItem | null): MenuItem[] {
  const start = canStartRobot(branch, robot, getLandingAncestor(branch.name), getLandingSpan(branch.name))
  // a dry run pushes nothing into the target: it may run while an ancestor lands, or while a descendant lands the branch
  const dryRun = canStartRobot(branch, robot)
  const items: MenuItem[] = [
    { label: "Safe Push…", icon: ShieldCheckIcon, kbds: ["s"], disabled: !start.ok, description: start.ok ? SAFE_PUSH_DESCRIPTION : start.reason, onSelect: () => openInline("safe-push", branch.name) },
    { label: "Dry Run…", icon: FlaskConicalIcon, disabled: !dryRun.ok, ...(!dryRun.ok && { description: dryRun.reason }), onSelect: () => openInline("dry-run", branch.name) },
  ]
  if (robot !== undefined) {
    const mode = formatPushMode(robot.pushMode)
    const restart = getRobotRestartAction(robot, branch.name, branch.commits[0]?.hash, branch.remoteStatus, isRobotNeedingReview(robot.id))
    if (restart === "restart") {
      items.push({ label: `Restart ${mode}`, icon: RotateCcwIcon, onSelect: () => void runRobotAction(restartPatronusRobot, robot, `${mode} restarted`, `Cannot restart the ${mode}`) })
    }
    else if (restart === "pushAndRestart") {
      // the restart takes the commit just pushed, and its toast is the one of the action
      items.push({ label: `Push and Restart ${mode}`, icon: UploadIcon, disabled: branch.status === "Blocked", ...(branch.status === "Blocked" && { description: formatBlockedReason(branch) }), onSelect: () => void pushBranch(branch.name, { restartRobot: robot, announce: false }) })
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

/** "Create Pull Request": the GitHub form of a pull request of the pushed branch into its base, while it has none */
function getCreatePullRequestItems(branch: Branch): MenuItem[] {
  const url = getNewPullRequestUrl(branch)
  return url === null ? [] : [{ label: "Create Pull Request", icon: GitPullRequestCreateIcon, onSelect: () => openNewPullRequest(url) }]
}

/**
 * "Move to…" and "Move to Existing ›" of a branch row: all its commits, or those of every selected branch if the row
 * is selected, go to one branch; a whole issue by the issue groups, without a rewrite
 */
function getMoveMenuItems(name: string): MenuItem[] {
  const order = getBranchTableOrder()
  const selected = getSelectedBranches()
  const names = selected.includes(name) ? inTableOrder(selected, order) : [name]
  const openTarget = () => {
    if (!selected.includes(name)) {
      selectBranches(name, "only", order)
    }
    setBranchTargetOpen(true)
  }
  const items: MenuItem[] = [{ label: "Move to…", icon: FolderInputIcon, kbds: ["m"], onSelect: openTarget }]
  const targets = getMoveTargets(names.length === 1 ? name : null, MAX_MENU_TARGETS + names.length)
    .filter(target => !names.includes(target))
    .slice(0, MAX_MENU_TARGETS)
  if (targets.length > 0) {
    items.push({
      label: "Move to Existing",
      icon: GitBranchIcon,
      onSelect: () => {},
      children: [
        ...targets.map(target => ({ label: target, icon: GitBranchIcon, onSelect: () => void moveCommits(getBranchCommits(names), target) })),
        { label: "New Branch…", icon: GitBranchPlusIcon, onSelect: openTarget },
      ],
    })
  }
  return items
}

/** The most branches "Stack On…" lists; the newest come first */
const STACK_TARGET_LIMIT = 20

/** "Stack On…" (any other branch that isn't stacked on this one) and "Unstack" */
function getStackMenuItems(branch: Branch): MenuItem[] {
  const parent = branch.stack?.declaredParent ?? null
  const descendants = getStackDescendants(branch.name)
  const excluded = new Set([branch.name, ...descendants])
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
  // the stack folds into its root, from any of its rows, as ⌥← and ⌥→ do
  const collapse = getStackCollapse(branch.name)
  if (collapse !== null) {
    items.push(collapse.collapsed
      ? { label: "Expand Stack", icon: UnfoldVerticalIcon, kbds: ["alt", "arrowright"], onSelect: () => void toggleStackCollapsed(branch.name, false) }
      : { label: "Collapse Stack", icon: FoldVerticalIcon, kbds: ["alt", "arrowleft"], onSelect: () => void toggleStackCollapsed(branch.name, true) })
  }
  // the parents that have work for the remote are pushed first, so the pull request shows only the branch's commits
  const pushOrder = stackPushOrder(branch.name, getBranch)
  if (pushOrder.length > 1) {
    items.push({ label: formatPushStackLabel(pushOrder.length), icon: UploadIcon, disabled: branch.status === "Blocked", onSelect: () => void pushStack(branch.name) })
  }
  // the children are built on the branch: a push of it alone leaves their remote branches on its old head
  const childrenPushOrder = stackChildrenPushOrder(branch.name, descendants, getBranch)
  if (childrenPushOrder.some(name => name !== branch.name)) {
    items.push({ label: formatPushWithChildrenLabel(childrenPushOrder.length), icon: UploadIcon, onSelect: () => void pushWithChildren(branch.name) })
  }
  return items
}

export function useBranchContextActions(setExpanded: (name: string, expanded: boolean) => void) {
  return {
    getContextMenuItems: (branch: Branch) => getBranchContextMenuItems(branch, setExpanded),
  }
}
