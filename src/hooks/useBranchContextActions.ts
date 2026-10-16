import { ArchiveXIcon, CombineIcon, CopyIcon, FileDiffIcon, FlaskConicalIcon, FoldVerticalIcon, FolderInputIcon, GitBranchIcon, GitBranchPlusIcon, GitPullRequestCreateIcon, GitPullRequestIcon, Layers2Icon, LayersIcon, LinkIcon, PencilIcon, PenLineIcon, ShieldCheckIcon, TagIcon, UnfoldVerticalIcon, UngroupIcon, UploadIcon } from "lucide-react"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import { canPushNow } from "@/hooks/usePushButton"
import { restackBranch } from "@/hooks/useStackActions"
import { pushOrConfirm, pushStack, pushWithChildren } from "@/lib/git/push"
import { copyText } from "@/lib/clipboard"
import { getNewPullRequestUrl, getPullRequestBase, getRetargetCommand, openNewPullRequest } from "@/lib/git/new-pull-request"
import { formatBlockedReason, formatPushStackLabel, formatPushWithChildrenLabel, stackChildrenPushOrder, stackPushOrder } from "@/lib/stack"
import { getListFixups } from "@/lib/autosquash"
import { branchReviewBlocker } from "@/lib/branch-review"
import { openBranchReview } from "@/lib/branch-review-window"
import { inTableOrder } from "@/lib/branch-selection"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { canStartRobot, SAFE_PUSH_DESCRIPTION } from "@/lib/patronus"
import type { RobotAction } from "@/lib/patronus-actions"
import { getRobotActions } from "@/lib/patronus-actions"
import { openPullRequest } from "@/lib/pull-request"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getAutosquashPlan, getBranch, getBranchCommits, getBranchTableOrder, getIssueGroups, getMoveTargets, getStackChildren, getStackDescendants, isBranchPushing, isStackPushing, isStacksEnabled } from "@/stores/branch-sync"
import { getSelectedBranches, selectBranches, setBranchTargetOpen } from "@/stores/commit-move"
import { isProcessing, openInline } from "@/stores/inline-row-action"
import { getBranchRobot, getLandingAncestor, getLandingSpan, isPatronusAvailable } from "@/stores/patronus"
import { getPullRequest } from "@/stores/pull-requests"
import { getStackCollapse, toggleStackCollapsed } from "@/stores/stack-collapse"

/** Branches listed in the "Move to Existing" submenu */
const MAX_MENU_TARGETS = 8

/**
 * Context menu of an active branch row: copy actions (the names, the pull request link, the pull request base, the
 * retarget command), then rename (name an issue branch; an issue group is renamed without a rewrite), move to another
 * branch (with the other selected branches, if the row is selected) and ungroup, then commit actions (issue reference,
 * amend, unapply), then history (review the changes, squash fixups, open or create the pull request), then the remote
 * (push, Safe Push, Dry Run, the actions of the branch's robot, `getRobotActions`). An item that opens a form ends with
 * "…"; an item shows its key only where the focused row has it (`useBranchRowKeys`, `BranchTableCard`). The right-click
 * on the row and the "⋯" of its Actions cell show it.
 */
export function getBranchContextMenuItems(branch: Branch, setExpanded: (name: string, expanded: boolean) => void): MenuItemGroups {
  const groups: MenuItemGroups = [getBranchCopyMenuItems(branch.name)]
  const pullRequest = getPullRequest(branch.name)
  if (pullRequest !== undefined) {
    groups[0]!.push({ id: "copy-pull-request-link", label: "Copy Pull Request Link", icon: LinkIcon, onSelect: () => void copyText(pullRequest.url) })
  }
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

  const openPullRequestItem: MenuItem | null = pullRequest === undefined ? null : openPullRequestItemOf(pullRequest)

  const robot = getBranchRobot(branch.name)
  const robotActions = robot === undefined ? [] : getBranchRobotActions(robot, branch, pullRequest)

  // a rewrite waits for a running sync, also of a row that shows its last state meanwhile (a quiet resync)
  const canModify = branch.commits.length > 0 && !branch.hasError && branch.status !== "Syncing" && !branchSyncStore.getState().isSyncing
  if (!canModify) {
    // opening the pull request or the robot changes nothing, nor does canceling the robot
    const openItems = [openPullRequestItem, ...robotActions.filter(item => item.id === "cancel-robot" || item.id === "open-robot" || item.id === "open-robot-pull-request")]
      .filter(item => item !== null)
    if (openItems.length > 0) {
      groups.push(openItems)
    }
    return groups
  }

  const nameActions: MenuItem[] = [
    {
      id: "rename",
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
      // the branches stacked on it keep naming it as their parent and are built below it: confirm with them listed
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
    id: "review-changes",
    label: "Review Changes",
    icon: FileDiffIcon,
    kbds: ["d"],
    disabled: reviewBlocker !== null,
    description: reviewBlocker ?? "The diff of the branch, the parts of its stack, the files of each code owner",
    onSelect: () => openBranchReview(branch.name),
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

  const remoteActions: MenuItem[] = [getPushMenuItem(branch)]
  if (isPatronusAvailable()) {
    remoteActions.push(...getPatronusStartItems(branch, robot))
  }
  // a listed robot keeps its actions while Patronus doesn't answer for a new start
  remoteActions.push(...robotActions)
  groups.push(remoteActions)
  return groups
}

/**
 * "Push": what the push button of the row pushes (`pushOrConfirm`: the stack while a parent has work for the remote, a
 * push that drops remote commits asks in the button's description), enabled as the button (`canPushNow`), else with
 * the reason
 */
function getPushMenuItem(branch: Branch): MenuItem {
  const isPushing = isBranchPushing(branch.name) || isStackPushing(branch.name)
  const processing = isProcessing(branch.name)
  const enabled = canPushNow(branch, { isPushing, processing })
  return {
    id: "push",
    label: "Push",
    icon: UploadIcon,
    kbds: ["p"],
    disabled: !enabled,
    ...(!enabled && { description: getPushBlocker(branch, { isPushing, processing }) }),
    onSelect: () => pushOrConfirm(branch.name, "row"),
  }
}

/**
 * Why the push button of the branch is disabled: the blocked parent, a running push or inline action, a sync that
 * rebuilds the branch anew, nothing to push
 */
function getPushBlocker(branch: Branch, { isPushing, processing }: { isPushing: boolean, processing: boolean }): string {
  if (branch.status === "Blocked") {
    return formatBlockedReason(branch)
  }
  if (isPushing) {
    return "The branch is being pushed"
  }
  if (branch.status === "Syncing") {
    return "Wait for the sync to finish"
  }
  if (processing) {
    return "Wait for the action on the branch to finish"
  }
  return "The remote branch is up to date"
}

/**
 * Safe Push…, Dry Run… (disabled with the reason of `canStartRobot`). The approvals go into the GitHub pull request:
 * "Create Pull Request" is the history group's; no Space merge request (deprecated).
 */
function getPatronusStartItems(branch: Branch, robot: PatronusRobot | undefined): MenuItem[] {
  // a dry run pushes nothing into the target: it may run while an ancestor lands, or while a descendant lands the branch
  const dryRun = canStartRobot(branch, robot)
  return [
    getSafePushMenuItem(branch),
    { id: "dry-run", label: "Dry Run…", icon: FlaskConicalIcon, disabled: !dryRun.ok, ...(!dryRun.ok && { description: dryRun.reason }), onSelect: () => openInline("dry-run", branch.name) },
  ]
}

/** "Safe Push…" (S): the Safe Push form below the row, disabled with the reason of `getSafePushBlocker` */
function getSafePushMenuItem(branch: Branch): MenuItem {
  const blocker = getSafePushBlocker(branch)
  return { id: "safe-push", label: "Safe Push…", icon: ShieldCheckIcon, kbds: ["s"], disabled: blocker !== null, description: blocker ?? SAFE_PUSH_DESCRIPTION, onSelect: () => openInline("safe-push", branch.name) }
}

/**
 * Why a Safe Push of the branch can't start now, null when it can: for every surface that offers it (the menu, S on the
 * row, the branch preview, the review). Patronus must be on the repository; the surfaces offer it only then.
 */
export function getSafePushBlocker(branch: Branch): string | null {
  if (branch.hasError) {
    return "The branch failed to sync"
  }
  if (branch.commitCount === 0) {
    return "No commits to push"
  }
  if (isProcessing(branch.name)) {
    return "Wait for the action on the branch to finish"
  }
  const start = canStartRobot(branch, getBranchRobot(branch.name), getLandingAncestor(branch.name), getLandingSpan(branch.name))
  return start.ok ? null : start.reason
}

/** An action of a branch in a review: a menu item, with the URL a window opens itself (the pull request) */
export interface BranchActionItem extends MenuItem {
  readonly url?: string
}

/**
 * The actions on a branch where it is read, the branch preview and the review: its pull request (Open, or Create of a
 * pushed branch), "Safe Push…" where Patronus is and the branch has work, "Push" for a branch with work. The same items
 * as the menu; in the review Push has no key, P is the previous change there.
 */
export function getBranchReviewItems(branch: Branch, surface: "preview" | "review"): BranchActionItem[] {
  const items: BranchActionItem[] = []
  const pullRequest = getPullRequest(branch.name)
  if (pullRequest === undefined) {
    const url = getNewPullRequestUrl(branch)
    if (url !== null) {
      items.push({ ...getCreatePullRequestItems(branch)[0]!, url })
    }
  }
  else {
    items.push({ ...openPullRequestItemOf(pullRequest), url: pullRequest.url })
  }
  // as the push button of the row: only a branch that synced without an error has something to push
  const hasPush = !branch.hasError && branch.commitCount > 0
  if (hasPush && isPatronusAvailable()) {
    items.push(getSafePushMenuItem(branch))
  }
  if (hasPush) {
    const push = getPushMenuItem(branch)
    items.push(surface === "review" ? { ...push, kbds: undefined } : push)
  }
  return items
}

/** The robot's actions in the branch menu: its pull request is the branch's one, which "Open Pull Request" already opens */
function getBranchRobotActions(robot: PatronusRobot, branch: Branch, pullRequest: PullRequest | undefined): RobotAction[] {
  return getRobotActions(robot, branch).filter(item => item.id !== "open-robot-pull-request" || pullRequest?.url !== robot.pullRequest?.url)
}

/** "Open Pull Request" with the key of the focused row and of the branch preview */
function openPullRequestItemOf(pullRequest: PullRequest): MenuItem {
  return { id: "open-pull-request", label: "Open Pull Request", icon: GitPullRequestIcon, kbds: ["o"], onSelect: () => openPullRequest(pullRequest) }
}

/**
 * "Open Pull Request" of a branch with one, else "Create Pull Request" of a pushed branch on GitHub; null for neither:
 * the step of a branch row after a robot that needs the code-owner review, whose approvals count in the pull request
 */
export function getPullRequestItem(branch: Branch): MenuItem | null {
  const pullRequest = getPullRequest(branch.name)
  return pullRequest === undefined ? getCreatePullRequestItems(branch)[0] ?? null : openPullRequestItemOf(pullRequest)
}

/** "Create Pull Request": the GitHub form of a pull request of the pushed branch into its base, while it has none */
function getCreatePullRequestItems(branch: Branch): MenuItem[] {
  const url = getNewPullRequestUrl(branch)
  return url === null ? [] : [{ id: "create-pull-request", label: "Create Pull Request", icon: GitPullRequestCreateIcon, onSelect: () => openNewPullRequest(url) }]
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
  const items: MenuItem[] = [{ id: "move-to", label: "Move to…", icon: FolderInputIcon, kbds: ["m"], onSelect: openTarget }]
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
