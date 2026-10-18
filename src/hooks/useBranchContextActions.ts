import {
  ArchiveXIcon,
  CombineIcon,
  CopyIcon,
  FileDiffIcon,
  FlaskConicalIcon,
  FoldVerticalIcon,
  FolderInputIcon,
  GitBranchIcon,
  GitBranchPlusIcon,
  GitPullRequestCreateIcon,
  GitPullRequestIcon,
  Layers2Icon,
  LayersIcon,
  LinkIcon,
  PencilIcon,
  PenLineIcon,
  RefreshCwIcon,
  ShieldCheckIcon,
  ShieldOffIcon,
  TagIcon,
  UnfoldVerticalIcon,
  UngroupIcon,
  UploadIcon,
  WandSparklesIcon,
} from "lucide-react"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { getBranchNextStepOf, getGatedRobotActions } from "@/hooks/useBranchNextStep"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import { canPushNow, getPushButtonNow } from "@/hooks/usePushButton"
import { restackBranch } from "@/hooks/useStackActions"
import { formatSquashFixupsLabel, getListFixups } from "@/lib/autosquash"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import type { BranchNextStep } from "@/lib/branch-next-step"
import { branchReviewBlocker } from "@/lib/branch-review"
import { openBranchReviewWindow } from "@/lib/branch-review-window"
import { inTableOrder } from "@/lib/branch-selection"
import { copyText } from "@/lib/clipboard"
import { CREATE_PULL_REQUEST_LABEL, canCreatePullRequest, getPullRequestBase, getRetargetCommand } from "@/lib/git/new-pull-request"
import { pushOrConfirm } from "@/lib/git/push"
import { pushWithChildren } from "@/lib/git/stack-push"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { formatPushHoldReason, SAFE_PUSH_DESCRIPTION } from "@/lib/patronus"
import type { RobotAction } from "@/lib/patronus-actions"
import { openPullRequest } from "@/lib/pull-request"
import { getRequestReviewItem } from "@/lib/request-review"
import { buttonStackPushOrder, formatBlockedReason, formatPushWithChildrenLabel, offersPushWithChildren, stackChildrenPushOrder } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import type { Branch } from "@/stores/branch-sync"
import {
  branchSyncStore,
  getAutosquashPlan,
  getBranch,
  getBranchCommits,
  getBranchTableOrder,
  getIssueGroups,
  getMoveTargets,
  getStackChildren,
  getStackDescendants,
  isBranchPushing,
  isStackPushing,
  isStacksEnabled,
} from "@/stores/branch-sync"
import { getSelectedBranches, requestReveal, selectBranches, setBranchTargetOpen } from "@/stores/commit-move"
import { closeCreatePullRequestDialog, openCreatePullRequestDialog } from "@/stores/create-pull-request-dialog"
import { isProcessing, openInline } from "@/stores/inline-row-action"
import { getBranchRobot, getPushHold, isPatronusAvailable } from "@/stores/patronus"
import { getPullRequest } from "@/stores/pull-requests"
import { openRequestReview } from "@/stores/request-review-popover"
import { getReviewReadiness, recheckReviewReadiness } from "@/stores/review-readiness"
import { getStackCollapse, toggleStackCollapsed } from "@/stores/stack-collapse"

/** Branches listed in the "Move to Existing" submenu */
const MAX_MENU_TARGETS = 8

/**
 * Context menu of an active branch row: copy actions (the names, the pull request link, the pull request base, the
 * retarget command), then rename (name an issue branch; an issue group is renamed without a rewrite), move to another
 * branch (with the other selected branches, if the row is selected) and ungroup, then commit actions (issue reference,
 * amend, unapply), then history (review the changes, squash fixups, open or create the pull request), then the remote
 * (push, Safe Push, Dry Run, the actions of the branch's robot, `getGatedRobotActions`). An item that opens a form ends with
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
      id: "copy-pull-request-base",
      label: "Copy PR Base",
      icon: CopyIcon,
      onSelect: () => void copyText(getPullRequestBase(branch)),
    })
    const retargetCommand = getRetargetCommand(branch)
    if (retargetCommand !== null) {
      groups[0]!.push({ id: "copy-retarget-command", label: "Copy Retarget Command", icon: CopyIcon, onSelect: () => void copyText(retargetCommand) })
    }
  }

  const openPullRequestItem: MenuItem | null = pullRequest === undefined ? null : openPullRequestItemOf(pullRequest)
  // a conflict shows its fix in the expanded row, at the commit that does not apply
  const conflictCommit = branch.commits.find(commit => commit.error?.MergeConflict != null)
  if (conflictCommit !== undefined) {
    groups.unshift([
      {
        id: "fix-conflict",
        label: "Fix Conflict…",
        icon: WandSparklesIcon,
        onSelect: () => requestReveal({ branchName: branch.name, commitIds: [conflictCommit.originalHash] }),
      },
    ])
  }

  const robot = getBranchRobot(branch.name)
  const nextStep = getBranchNextStepOf(branch)
  const robotActions = robot === undefined ? [] : getBranchRobotActions(robot, branch, pullRequest)

  // a rewrite waits for a running sync, also of a row that shows its last state meanwhile (a quiet resync): the rewrites
  // stay in the menu, disabled with the reason; the rest rewrites nothing here (a push waits for the sync itself)
  const rewriteBlocker = getRewriteBlocker(branch)
  const rewrite = (items: MenuItem[]): MenuItem[] => (rewriteBlocker === null ? items : items.map(item => ({ ...item, disabled: true, description: rewriteBlocker })))

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
      id: "ungroup",
      label: ungroupLabel === "Remove Prefix" || ungroupLabel === "Move Back" ? "Ungroup" : `Ungroup: ${ungroupLabel}`,
      icon: UngroupIcon,
      // the branches stacked on it keep naming it as their parent and are built below it: confirm with them listed
      onSelect: () => (getStackChildren(branch.name).length > 0 ? openInline("ungroup", branch.name) : void moveCommits(branch.commits, null)),
    })
  }
  groups.push(rewrite(nameActions))
  if (isStacksEnabled()) {
    groups.push(getStackMenuItems(branch, rewrite))
  }

  const commitActions: MenuItem[] = [
    {
      id: "add-issue-reference",
      label: "Add Issue Reference…",
      icon: TagIcon,
      disabled: branch.allCommitsHaveIssueReferences,
      ...(branch.allCommitsHaveIssueReferences && { description: "Every commit of the branch has an issue reference" }),
      onSelect: () => openInline("issue-reference", branch.name),
    },
    {
      id: "amend-changes",
      label: "Amend Changes…",
      icon: PenLineIcon,
      onSelect: () => {
        // show the commits; the form loads the uncommitted changes when it opens
        setExpanded(branch.name, true)
        openInline("amend-changes", branch.name)
      },
    },
    {
      id: "unapply",
      label: "Unapply…",
      icon: ArchiveXIcon,
      onSelect: () => {
        // show the commits that will be unapplied
        setExpanded(branch.name, true)
        openInline("unapply", branch.name)
      },
    },
  ]
  groups.push(rewrite(commitActions))

  const reviewBlocker = branchReviewBlocker(branch)
  const historyActions: MenuItem[] = [
    {
      id: "review-changes",
      label: "Review Changes",
      icon: FileDiffIcon,
      kbds: ["d"],
      disabled: reviewBlocker !== null,
      description: reviewBlocker ?? "The diff of the branch, the parts of its stack, the files of each code owner, in a window",
      onSelect: () => void openBranchReviewWindow(branch.name),
    },
  ]
  const fixupCount = getListFixups(getAutosquashPlan(), branch.name).count
  if (fixupCount > 0) {
    historyActions.push(
      ...rewrite([
        {
          id: "squash-fixups",
          label: formatSquashFixupsLabel(fixupCount),
          icon: CombineIcon,
          onSelect: () => {
            // the confirmation opens from the button above the commits
            setExpanded(branch.name, true)
            openInline("squash-fixups", branch.name)
          },
        },
      ]),
    )
  }
  if (openPullRequestItem !== null) {
    historyActions.push(openPullRequestItem)
  }
  // the row's step when it is the one (a group nobody of it can approve says so), else the builder's item
  const requestReview =
    nextStep.kind === "requestReview" ? nextStep.item : getRequestReviewItem(pullRequest, getReviewReadiness(branch.name)?.readiness ?? null, () => openRequestReview(branch.name))
  if (requestReview !== null) {
    historyActions.push(requestReview)
  }
  // a pull request into another base: the step retargets one of a former parent, the menu always offers it
  if (nextStep.retarget !== null) {
    historyActions.push(nextStep.retarget)
  }
  historyActions.push(...getCreatePullRequestItems(branch))
  groups.push(historyActions)

  const remoteActions: MenuItem[] = [getPushMenuItem(branch)]
  if (isPatronusAvailable()) {
    remoteActions.push(...getPatronusStartItems(branch, nextStep))
  }
  // a listed robot keeps its actions while Patronus doesn't answer for a new start
  remoteActions.push(...robotActions)
  groups.push(remoteActions)
  return groups
}

/**
 * The push of the push button of the row, named as the button ("Push (2)", "Force Push (1)", "Push Stack (3
 * branches)", `getPushButtonNow`) and pushing as a click on it (`pushOrConfirm`: the stack while a parent has work for
 * the remote, a push that drops remote commits asks in the button's description); enabled as the button
 * (`canPushNow`), else with the reason. One item for the push of the branch and of its stack: the menu never pushes
 * past the confirmation the button asks for.
 */
function getPushMenuItem(branch: Branch): MenuItem {
  const stackPushing = isStackPushing(branch.name)
  const isPushing = isBranchPushing(branch.name) || stackPushing
  const processing = isProcessing(branch.name)
  const enabled = canPushNow(branch, { isPushing, processing })
  const button = getPushButtonNow(branch, isPushing)
  let description: string | undefined
  if (!enabled) {
    description = getPushBlocker(branch, { isPushing, stackPushing, processing })
  } else if (button.pushesStack) {
    description = `Pushes ${joinNames(buttonStackPushOrder(branch.name, getBranch, name => getPushHold(name) !== null))}, parents first`
  }
  return {
    id: "push",
    label: button.text,
    icon: UploadIcon,
    kbds: ["p"],
    disabled: !enabled,
    ...(description !== undefined && { description }),
    onSelect: () => pushOrConfirm(branch.name, "row"),
  }
}

/**
 * Why a rewrite of the branch (rename, move, ungroup, restack, amend, unapply, issue reference, squash) can't run now:
 * it has no commits, it failed to sync, or a sync rebuilds the branches (also a quiet resync of a row that shows its last
 * state); null when it can
 */
function getRewriteBlocker(branch: Branch): string | null {
  if (branch.commits.length === 0) {
    return "The branch has no commits"
  }
  if (branch.hasError) {
    return "The branch failed to sync"
  }
  if (branch.status === "Syncing" || branchSyncStore.getState().isSyncing) {
    return "Wait for the sync to finish"
  }
  return null
}

/**
 * Why the push button of the branch is disabled: the blocked parent, a running push of the branch or of its stack, an
 * inline action, a sync that rebuilds the branch anew, nothing to push
 */
function getPushBlocker(branch: Branch, { isPushing, stackPushing, processing }: { isPushing: boolean; stackPushing: boolean; processing: boolean }): string {
  if (branch.status === "Blocked") {
    return formatBlockedReason(branch)
  }
  const hold = getPushHold(branch.name)
  if (hold !== null && !isPushing) {
    return formatPushHoldReason(branch.name, hold)
  }
  if (stackPushing) {
    return "The stack is being pushed"
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
 * Safe Push…, Dry Run… and Check Approvals Again, disabled with the reasons of the next step of the branch
 * (`getBranchNextStep`). The approvals go into the GitHub pull request: "Create Pull Request…" is the history group's; no
 * Space merge request (deprecated).
 */
function getPatronusStartItems(branch: Branch, nextStep: BranchNextStep): MenuItem[] {
  return [
    getSafePushMenuItem(branch, nextStep),
    {
      id: "dry-run",
      label: "Dry Run…",
      icon: FlaskConicalIcon,
      disabled: nextStep.dryRunBlocker !== null,
      ...(nextStep.dryRunBlocker !== null && { description: nextStep.dryRunBlocker }),
      onSelect: () => openInline("dry-run", branch.name),
    },
    {
      id: "check-approvals",
      label: "Check Approvals Again",
      icon: RefreshCwIcon,
      description: "Reads the reviews of the pull request from GitHub and asks Patronus whose approvals count",
      onSelect: () => void recheckReviewReadiness(branch.name),
    },
  ]
}

/** "Safe Push…" (S): the Safe Push form below the row, disabled with the reason of `getSafePushBlocker` */
function getSafePushMenuItem(branch: Branch, nextStep: BranchNextStep = getBranchNextStepOf(branch)): MenuItem {
  const blocker = nextStep.safePushBlocker
  return {
    id: "safe-push",
    label: "Safe Push…",
    icon: ShieldCheckIcon,
    kbds: ["s"],
    disabled: blocker !== null,
    description: blocker ?? SAFE_PUSH_DESCRIPTION,
    onSelect: () => openInline("safe-push", branch.name),
  }
}

/**
 * Why a Safe Push of the branch can't start now, null when it can: for every surface that offers it (the slot, the
 * menu, S on the row, the branch preview, the review, the stack header), the next step of the branch
 * (`getBranchNextStep`): the branch and its robots (`canStartRobot`), landed or merged, then what Patronus would refuse
 * (the approvals that count, a draft, a pull request into another base; a stacked branch as its root). Patronus must
 * be on the repository; the surfaces offer it only then.
 */
export function getSafePushBlocker(branch: Branch): string | null {
  return getBranchNextStepOf(branch).safePushBlocker
}

/** An action of a branch in a review: a menu item, with the URL a window opens itself (the pull request) */
export interface BranchActionItem extends MenuItem {
  readonly url?: string
}

/**
 * The actions on a branch in its review window: its pull request (Open; Create without a live one, also beside a
 * closed one, as the menu offers both), "Safe Push…" where Patronus is and the branch has work, "Push" for a branch
 * with work. The same items as the menu; Push has no key, P is the previous change there.
 */
export function getBranchReviewItems(branch: Branch): BranchActionItem[] {
  const items: BranchActionItem[] = []
  const pullRequest = getPullRequest(branch.name)
  if (pullRequest !== undefined) {
    items.push({ ...openPullRequestItemOf(pullRequest), url: pullRequest.url })
  }
  // the review window opens the dialog itself, with what the main window sends for it
  items.push(...getCreatePullRequestItems(branch))
  // as the push button of the row: only a branch that synced without an error has something to push
  const hasPush = !branch.hasError && branch.commitCount > 0
  if (hasPush && isPatronusAvailable()) {
    items.push(getSafePushMenuItem(branch))
  }
  if (hasPush) {
    const push = getPushMenuItem(branch)
    items.push({ ...push, kbds: undefined })
  }
  return items
}

/**
 * The robot's actions in the branch menu (`getGatedRobotActions`): its pull request is the branch's one, which "Open Pull
 * Request" already opens
 */
function getBranchRobotActions(robot: PatronusRobot, branch: Branch, pullRequest: PullRequest | undefined): RobotAction[] {
  return getGatedRobotActions(robot, branch).filter(item => item.id !== "open-robot-pull-request" || pullRequest?.url !== robot.pullRequest?.url)
}

/**
 * "Open Pull Request" with the key of the focused row and of the branch preview; a closed one says so, beside the
 * Create Pull Request… a branch with only a closed one gets
 */
export function openPullRequestItemOf(pullRequest: PullRequest): MenuItem {
  return {
    id: "open-pull-request",
    label: "Open Pull Request",
    icon: GitPullRequestIcon,
    kbds: ["o"],
    ...(pullRequest.state === "closed" && { description: `#${pullRequest.number} is closed` }),
    onSelect: () => openPullRequest(pullRequest),
  }
}

/**
 * "Open Pull Request" of a branch with one, else "Create Pull Request…" of a branch on GitHub; null for neither:
 * the step of a branch row after a robot that needs the code-owner review, whose approvals count in the pull request
 */
export function getPullRequestItem(branch: Branch): MenuItem | null {
  const pullRequest = getPullRequest(branch.name)
  return pullRequest === undefined ? (getCreatePullRequestItems(branch)[0] ?? null) : openPullRequestItemOf(pullRequest)
}

/**
 * "Create Pull Request…": the dialog of a pull request of the branch into its base (title, description, reviewers, the
 * user as its assignee), which pushes the branch first, while it has none
 */
function getCreatePullRequestItems(branch: Branch): MenuItem[] {
  const item = getCreatePullRequestItem(branch)
  return item === null ? [] : [item]
}

/**
 * "Safe Push Without Review…" of a branch whose approvals count only in a pull request it doesn't have (row 7): the
 * Safe Push form with a merge profile that skips the code-owner review; null on any other step
 */
export function getSafePushWithoutReviewItem(branch: Branch, nextStep: BranchNextStep = getBranchNextStepOf(branch)): MenuItem | null {
  const withoutReview = nextStep.withoutReview
  if (withoutReview === null) {
    return null
  }
  return {
    id: "safe-push-without-review",
    label: "Safe Push Without Review…",
    icon: ShieldOffIcon,
    disabled: withoutReview.blocker !== null,
    description: withoutReview.blocker ?? "Starts the Safe Push with a merge profile that skips the code-owner review, such as Large Scale Change",
    onSelect: () => {
      closeCreatePullRequestDialog()
      requestReveal({ branchName: branch.name, commitIds: [] })
      openInline("safe-push", branch.name, { bypassReview: true })
    },
  }
}

/** "Create Pull Request…" of a branch without a live pull request, on GitHub, pushed or not (the dialog pushes it); null otherwise */
export function getCreatePullRequestItem(branch: Branch): MenuItem | null {
  return canCreatePullRequest(branch)
    ? { id: "create-pull-request", label: CREATE_PULL_REQUEST_LABEL, icon: GitPullRequestCreateIcon, onSelect: () => openCreatePullRequestDialog(branch.name) }
    : null
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
      id: "move-to-existing",
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
function getStackMenuItems(branch: Branch, rewrite: (items: MenuItem[]) => MenuItem[]): MenuItem[] {
  const parent = branch.stack?.declaredParent ?? null
  const descendants = getStackDescendants(branch.name)
  const excluded = new Set([branch.name, ...descendants])
  const targets = branchSyncStore
    .getState()
    .branchOrder.filter(name => !excluded.has(name) && name !== parent)
    .slice(0, STACK_TARGET_LIMIT)
  // a restack is a regroup: it waits for the sync; the fold of the stack and its push don't
  const items: MenuItem[] = rewrite([
    {
      id: "stack-on",
      label: "Stack On…",
      icon: LayersIcon,
      disabled: targets.length === 0,
      ...(targets.length === 0 && { description: "No other branch to stack on" }),
      onSelect: () => {},
      children: targets.map(target => ({ label: target, onSelect: () => void restackBranch(branch.name, target) })),
    },
  ])
  if (parent !== null) {
    items.push(...rewrite([{ id: "unstack", label: `Unstack from ${parent}`, icon: Layers2Icon, onSelect: () => void restackBranch(branch.name, null) }]))
  }
  // the stack folds into its root, from any of its rows, as ⌥← and ⌥→ do
  const collapse = getStackCollapse(branch.name)
  if (collapse !== null) {
    items.push(
      collapse.collapsed
        ? { id: "expand-stack", label: "Expand Stack", icon: UnfoldVerticalIcon, kbds: ["alt", "arrowright"], onSelect: () => void toggleStackCollapsed(branch.name, false) }
        : { id: "collapse-stack", label: "Collapse Stack", icon: FoldVerticalIcon, kbds: ["alt", "arrowleft"], onSelect: () => void toggleStackCollapsed(branch.name, true) },
    )
  }
  const children = getPushWithChildrenItem(branch.name)
  if (children !== null) {
    items.push(children)
  }
  return items
}

/**
 * "Push with Children (3 branches)" of a branch, for its menu and the header of its stack island: the branch and the
 * branches built on it that have work for the remote, parents first (`stackChildrenPushOrder`); null while it would
 * push only the branch itself. Disabled with the reason while a Safe Push holds the branch (`PushHold`): a push of the
 * stack it runs in won't land, and every Safe Push in a stack holds its root.
 */
export function getPushWithChildrenItem(name: string): MenuItem | null {
  const isHeld = (it: string): boolean => getPushHold(it) !== null
  // the children are built on the branch: a push of it alone leaves their remote branches on its old head
  const order = stackChildrenPushOrder(name, getStackDescendants(name), getBranch, isHeld)
  const hold = getPushHold(name)
  const all = hold === null ? order : stackChildrenPushOrder(name, getStackDescendants(name), getBranch)
  if (!offersPushWithChildren(name, all)) {
    return null
  }
  const reason = hold === null ? (isStackPushing(name) ? "The stack is being pushed" : null) : formatPushHoldReason(name, hold)
  return {
    id: "push-with-children",
    label: formatPushWithChildrenLabel(hold === null ? order : all),
    icon: UploadIcon,
    disabled: reason !== null,
    ...(reason !== null && { description: reason }),
    onSelect: () => void pushWithChildren(name),
  }
}

export function useBranchContextActions(setExpanded: (name: string, expanded: boolean) => void) {
  return {
    getContextMenuItems: (branch: Branch) => getBranchContextMenuItems(branch, setExpanded),
  }
}
