import {
  ArchiveXIcon,
  CirclePauseIcon,
  CirclePlayIcon,
  CombineIcon,
  CopyIcon,
  FileDiffIcon,
  FlaskConicalIcon,
  FoldVerticalIcon,
  FolderInputIcon,
  GitPullRequestCreateIcon,
  GitPullRequestIcon,
  Layers2Icon,
  LayersIcon,
  LinkIcon,
  PencilIcon,
  PenLineIcon,
  RefreshCwIcon,
  ShieldCheckIcon,
  ShieldEllipsisIcon,
  ShieldOffIcon,
  SlidersHorizontalIcon,
  TagIcon,
  UnfoldVerticalIcon,
  UngroupIcon,
  UploadIcon,
  WandSparklesIcon,
} from "lucide-react"
import { useStore } from "zustand"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { getBranchNextStepNow, getBranchNextStepOf, getGatedRobotActions, nextStepSources, useNextStepStores } from "@/hooks/useBranchNextStep"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import { unparkBranch } from "@/hooks/useParkBranch"
import { canPushNow, firstPushPending, firstPushWait, getPushButtonNow, isLandingPushAvailable } from "@/hooks/usePushButton"
import { requestSafePush } from "@/hooks/useSafePush"
import { restackBranch } from "@/hooks/useStackActions"
import { getWholeStackMenuItems } from "@/hooks/useWholeStackActions"
import { formatSquashFixupsLabel, getListFixups } from "@/lib/autosquash"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import { fixBlocker, parkBlocker, rewriteBlocker } from "@/lib/branch-drop"
import type { BranchNextStep } from "@/lib/branch-next-step"
import { formatPendingFact } from "@/lib/branch-next-step"
import { branchReviewBlocker } from "@/lib/branch-review"
import { openBranchReviewWindow } from "@/lib/branch-review-window"
import { copyText } from "@/lib/clipboard"
import { CREATE_PULL_REQUEST_LABEL, canCreatePullRequest, getRetargetCommand } from "@/lib/git/new-pull-request"
import { pushOrConfirm } from "@/lib/git/push"
import { pushWithChildren } from "@/lib/git/stack-push"
import { getMarkFixedItem } from "@/lib/issue-actions"
import type { MenuItem, MenuItemGroups, SelectContext } from "@/lib/menu"
import { formatParkedBlocker } from "@/lib/parked"
import { describeStart, formatStartLabel, getPullRequestStart } from "@/lib/patronus"
import type { RobotAction } from "@/lib/patronus-actions"
import { openPullRequest } from "@/lib/pull-request"
import { getRequestReviewItem } from "@/lib/request-review"
import { describePushWithChildrenBlocker, formatBlockedReason, formatPushWithChildrenLabel, hasStackFix, stackOnFixes, stackProblemOf } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import { describeStackBranchAlong } from "@/lib/whole-stack"
import { formatPushHoldReason } from "@/presenters/reasons"
import type { Branch } from "@/stores/branch-sync"
import {
  branchSyncStore,
  getAutosquashPlan,
  getBranch,
  getBranchTableOrder,
  getIssueGroups,
  getOpenPullRequest,
  getPushOrders,
  getStackAncestors,
  getStackBranch,
  getParkedState,
  getStackChildren,
  getStackDescendants,
  getUnparkedStackDescendants,
  isBranchPushing,
  isStackPushing,
  isStacksEnabled,
} from "@/stores/branch-sync"
import { commitMoveStore, getSelectedBranches, requestReveal, selectBranches, setBranchTargetOpen } from "@/stores/commit-move"
import { closeCreatePullRequestDialog, openCreatePullRequestDialog } from "@/stores/create-pull-request-dialog"
import { openInline } from "@/stores/inline-row-action"
import type { Operations } from "@/stores/operations"
import { branchSubject, getOperations, isObjectBusy, operationItem } from "@/stores/operations"
import {
  getBranchRobot,
  getLandedWithChildrenReason,
  getLandingBranchCount,
  getPatronusGitHubRepository,
  getPushHold,
  isPatronusAvailable,
  isPatronusOffered,
  isPatronusPending,
} from "@/stores/patronus"
import { getPullRequest, getStackPullRequest, isPullRequestsLoading } from "@/stores/pull-requests"
import { openRequestReview } from "@/stores/request-review-popover"
import { getReadinessTarget, recheckReviewReadiness } from "@/stores/review-readiness"
import { getStackCollapse, toggleStackCollapsed } from "@/stores/stack-collapse"
import { getYouTrackKeyOfIssue, youTrackAuthStore } from "@/stores/youtrack"

/**
 * Context menu of an active branch row, its actions by how often they are wanted (`spec/branch/branch-row.spec.md`, "Branch
 * menu"): the fix of a conflict, then the landing (Safe Push first, the actions of the branch's robot,
 * `getGatedRobotActions`, the rarer starts in "Patronus ›", the push), the review (review the changes, open or create the
 * pull request), the shape (rename, move, stack, ungroup), the stack view, the commits (amend, issue reference, squash
 * fixups), what sets the branch aside (park, unapply), and the copies in "Copy ›". An item with a key never goes into a
 * submenu. An item that opens a form ends with "…"; an item shows its key only where the focused row has it
 * (`useBranchRowKeys`, `BranchTableCard`). The right-click on the row and the "⋯" of its Actions cell show it.
 */
export function getBranchContextMenuItems(branch: Branch, setExpanded: (name: string, expanded: boolean) => void, operations: Operations): MenuItemGroups {
  const groups: MenuItemGroups = []
  const pullRequest = getPullRequest(branch.name)
  const openPullRequestItem: MenuItem | null = pullRequest === undefined ? null : openPullRequestItemOf(pullRequest)
  // a conflict shows its fix in the expanded row, at the commit that does not apply
  const conflictCommit = branch.commits.find(commit => commit.error?.MergeConflict != null)
  if (conflictCommit !== undefined) {
    groups.push([
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

  // a rewrite waits for a running sync, also of a row that shows its last state meanwhile (a quiet resync), and for a
  // rewrite that runs (`operationItem`): the rewrites stay in the menu, disabled with the reason, the running one says
  // so; the rest rewrites nothing here (a push waits for the sync itself)
  const rewriteBlocker = getRewriteBlocker(branch)
  const rewrite = (items: MenuItem[]): MenuItem[] => items.map(item => rewriteItem(branch, item, operations, rewriteBlocker))

  // until Patronus answers its starts are offered, disabled with what they wait for: hidden only off Space
  const patronus = isPatronusOffered()
  const landingActions: MenuItem[] = []
  // the build of what its Safe Push lands needs a branch outside it: the fix first ("Stack lane-run on stills-codec")
  if (nextStep.landingFix !== null) {
    landingActions.push(...rewrite([nextStep.landingFix]))
  }
  if (patronus) {
    landingActions.push(getSafePushMenuItem(branch, nextStep))
  }
  // a listed robot keeps its actions while Patronus doesn't answer for a new start
  landingActions.push(...robotActions)
  if (patronus) {
    landingActions.push(getPatronusMenuItem(branch, nextStep))
  }
  landingActions.push(...getPushMenuItems(branch))
  // the push of the branches stacked on it, beside the push of the branch: offered on every branch with children
  const pushWithChildrenItem = getPushWithChildrenItem(branch.name)
  if (pushWithChildrenItem !== null) {
    landingActions.push(pushWithChildrenItem)
  }
  // the issue of the branch, once its work landed: the backend refuses an issue resolved already
  const youTrackKey = getYouTrackKeyOfIssue(branch.issueKey)
  if (youTrackKey !== null) {
    landingActions.push({ ...getMarkFixedItem(youTrackKey, operations), label: `Mark ${youTrackKey} Fixed` })
  }
  groups.push(landingActions)

  const reviewBlocker = branchReviewBlocker(branch)
  const reviewActions: MenuItem[] = [
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
  if (openPullRequestItem !== null) {
    reviewActions.push(openPullRequestItem)
  }
  // the row's step when it is the one (a group nobody of it can approve says so), else the builder's item
  const requestReview = nextStep.kind === "requestReview" ? nextStep.item : getRequestReviewItem(getOpenPullRequest(branch.name), () => openRequestReview(branch.name))
  if (requestReview !== null) {
    reviewActions.push(requestReview)
  }
  // a pull request into another base: the step retargets one into a virtual branch, the menu always offers it
  if (nextStep.retarget !== null) {
    reviewActions.push(nextStep.retarget)
  }
  reviewActions.push(...getCreatePullRequestItems(branch))
  groups.push(reviewActions)

  const shapeActions: MenuItem[] = [getRenameItem(branch), getMoveMenuItem(branch.name)]
  if (isStacksEnabled()) {
    shapeActions.push(...getStackShapeItems(branch))
  }
  const ungroupLabel = getRemovePrefixLabel(branch.commits, getIssueGroups())
  // an issue group goes back to the issue branches of its keys, without a rewrite
  if (branch.origin !== "Issue" && ungroupLabel !== null) {
    shapeActions.push({
      id: "ungroup",
      label: ungroupLabel === "Remove Prefix" || ungroupLabel === "Move Back" ? "Ungroup" : `Ungroup: ${ungroupLabel}`,
      icon: UngroupIcon,
      // the branches stacked on it keep naming it as their parent and are built below it: confirm with them listed
      onSelect: context =>
        getStackChildren(branch.name).length > 0
          ? openInline("ungroup", branch.name)
          : void moveCommits(branch.commits, null, { id: "ungroup", label: `Ungrouping ${branch.name}`, surface: context.surface }),
    })
  }
  // a stack problem blocks the rewrites, except the fixes Rust offers for it (`StackProblemView.fixes`)
  const fixIds = getStackFixIds(branch)
  groups.push(
    shapeActions.map(item => (fixIds.has(item.id ?? "") ? rewriteItem(branch, item, operations, getFixBlocker(branch)) : rewriteItem(branch, item, operations, rewriteBlocker))),
  )
  if (isStacksEnabled()) {
    const stackView = getStackViewItems(branch)
    if (stackView.length > 0) {
      groups.push(stackView)
    }
  }

  const commitActions: MenuItem[] = [
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
      id: "add-issue-reference",
      label: "Add Issue Reference…",
      icon: TagIcon,
      disabled: branch.allCommitsHaveIssueReferences,
      ...(branch.allCommitsHaveIssueReferences && { description: "Every commit of the branch has an issue reference" }),
      onSelect: () => openInline("issue-reference", branch.name),
    },
  ]
  const fixupCount = getListFixups(getAutosquashPlan(), branch.name).count
  if (fixupCount > 0) {
    commitActions.push({
      id: "squash-fixups",
      label: formatSquashFixupsLabel(fixupCount),
      icon: CombineIcon,
      onSelect: () => {
        // the confirmation opens from the button above the commits
        setExpanded(branch.name, true)
        openInline("squash-fixups", branch.name)
      },
    })
  }
  groups.push(rewrite(commitActions))

  // what takes the branch out of the landing: kept local, or its commits archived
  const unapply: MenuItem = {
    id: "unapply",
    label: "Unapply…",
    icon: ArchiveXIcon,
    onSelect: () => {
      // show the commits that will be unapplied
      setExpanded(branch.name, true)
      openInline("unapply", branch.name)
    },
  }
  groups.push([...getParkMenuItems(branch, operations), ...rewrite([unapply])])

  groups.push([getCopyMenuItem(branch, pullRequest)])
  return groups
}

/**
 * "Copy ›": the names of the branch (⌘C on the focused row copies the full one), the link of its pull request, and the
 * retarget command of an open pull request into a virtual branch
 */
function getCopyMenuItem(branch: Branch, pullRequest: PullRequest | undefined): MenuItem {
  const children = getBranchCopyMenuItems(branch.name)
  if (pullRequest !== undefined) {
    children.push({ id: "copy-pull-request-link", label: "Copy Pull Request Link", icon: LinkIcon, onSelect: () => void copyText(pullRequest.url) })
  }
  // an open pull request into a virtual branch: the push retargets it to the baseline, the command does it by hand
  const retargetCommand = getRetargetCommand(branch)
  if (retargetCommand !== null) {
    children.push({ id: "copy-retarget-command", label: "Copy Retarget Command", icon: CopyIcon, onSelect: () => void copyText(retargetCommand) })
  }
  return { id: "copy", label: "Copy", icon: CopyIcon, onSelect: () => {}, children }
}

/**
 * "Park Branch…" (the note form) of a branch to land, "Edit Park Note…" and "Unpark Branch" of a parked one, "Unpark
 * ‹parent›" of a branch parked with its parent: no rewrite, the parked branches of `refs/branch-deck/parked`. A park
 * waits for a robot that lands the branch: it lands whatever the park says; an open pull request stays open, a merge on
 * GitHub lands it, which the item says.
 */
function getParkMenuItems(branch: Branch, operations: Operations): MenuItem[] {
  const name = branch.name
  const parked = getParkedState(name)
  const writes = (item: MenuItem): MenuItem => {
    if (branch.status === "Syncing" || branchSyncStore.getState().isSyncing) {
      return { ...item, disabled: true, description: "Wait for the sync to finish" }
    }
    return operationItem(operations, item, { id: item.id ?? item.label, subject: branchSubject(name), objects: [name], kind: "rewrite" })
  }
  if (parked?.with != null) {
    const parent = parked.with
    return [
      writes({ id: "unpark", label: `Unpark ${parent}`, icon: CirclePlayIcon, description: `${name} carries the commits of ${parent}`, onSelect: () => void unparkBranch(parent) }),
    ]
  }
  if (parked !== null) {
    return [
      writes({ id: "park", label: "Edit Park Note…", icon: CirclePauseIcon, onSelect: () => openInline("park", name) }),
      writes({ id: "unpark", label: "Unpark Branch", icon: CirclePlayIcon, description: "It lands as any other branch again", onSelect: () => void unparkBranch(name) }),
    ]
  }
  const blocker = parkBlocker(branch, getPushHold(name), branchSyncStore.getState().isSyncing)
  if (blocker !== null) {
    return [{ id: "park", label: "Park Branch…", icon: CirclePauseIcon, disabled: true, description: blocker, onSelect: () => {} }]
  }
  const pullRequest = getPullRequest(name)
  const description =
    pullRequest?.state === "open"
      ? `Keeps it local; #${pullRequest.number} stays open, a merge on GitHub still lands it`
      : "Keeps it local: no push or Safe Push sends it, and it is no move of yours"
  return [writes({ id: "park", label: "Park Branch…", icon: CirclePauseIcon, description, onSelect: () => openInline("park", name) })]
}

/**
 * The push of the push button of the row, named as the button ("Push (2)", "Force Push (1)", "Push Stack (3
 * branches)", `getPushButtonNow`) and pushing as a click on it (`pushOrConfirm`: the stack while a parent has work for
 * the remote, a push that drops remote commits asks in the button's description); enabled as the button
 * (`canPushNow`), else with the reason. One item for the push of the branch and of its stack: the menu never pushes
 * past the confirmation the button asks for. After it, while the button pushes the stack, the second way its
 * description offers: "Push oauth Only".
 */
function getPushMenuItems(branch: Branch): MenuItem[] {
  const stackPushing = isStackPushing(branch.name)
  const isPushing = isBranchPushing(branch.name) || stackPushing
  const processing = isObjectBusy(branch.name)
  const enabled = canPushNow(branch, { isPushing, processing })
  const button = getPushButtonNow(branch, isPushing)
  let description: string | undefined
  if (!enabled) {
    description = getPushBlocker(branch, { isPushing, stackPushing, processing })
  } else if (button.pushesStack) {
    description = `Pushes ${joinNames(getPushOrders(branch.name).button)}, parents first`
  }
  const push: MenuItem = {
    id: "push",
    label: button.text,
    icon: UploadIcon,
    kbds: ["p"],
    disabled: !enabled,
    ...(description !== undefined && { description }),
    onSelect: () => pushOrConfirm(branch.name, "row"),
  }
  // the second way the description of the button offers while it pushes the stack: the branch alone. Its Push Stack
  // while the button pushes the branch alone stays in the description, beside the confirmation an ancestor asks for
  const other = button.action
  if (other?.id !== "push-branch-only") {
    return [push]
  }
  return [
    push,
    {
      id: other.id,
      label: other.label,
      disabled: !enabled,
      description: enabled ? other.description : description,
      onSelect: other.run,
    },
  ]
}

/**
 * A rewrite of the branch as the sync and the operations leave it: disabled with the reason of `getRewriteBlocker`, else
 * running or waiting for another rewrite (`operationItem`, the item's `id` names its operation)
 */
function rewriteItem(branch: Branch, item: MenuItem, operations: Operations, blocker: string | null = getRewriteBlocker(branch)): MenuItem {
  if (blocker !== null) {
    return { ...item, disabled: true, description: blocker }
  }
  return operationItem(operations, item, { id: item.id ?? item.label, subject: branchSubject(branch.name), objects: [branch.name], kind: "rewrite" })
}

/** Why a rewrite of the branch can't run now (`rewriteBlocker`); null when it can */
function getRewriteBlocker(branch: Branch): string | null {
  return rewriteBlocker(branch, branchSyncStore.getState().isSyncing)
}

/** Why a fix of the stack problem of the branch can't run now (`fixBlocker`); null when it can */
function getFixBlocker(branch: Branch): string | null {
  return fixBlocker(branch, branchSyncStore.getState().isSyncing)
}

/** "Rename…", "Name This Branch…" of an issue branch: the inline form of the row */
function getRenameItem(branch: Branch): MenuItem {
  return { id: "rename", label: branch.origin === "Issue" ? "Name This Branch…" : "Rename…", icon: PencilIcon, onSelect: () => openInline("rename", branch.name) }
}

/** The ids of the menu items that fix the stack problem of the branch: they run while the problem blocks the others */
function getStackFixIds(branch: Branch): ReadonlySet<string> {
  const problem = stackProblemOf(branch)
  const ids = new Set<string>()
  if (problem !== null) {
    if (stackOnFixes(problem).length > 0) {
      ids.add("stack-on")
    }
    if (hasStackFix(problem, "Unstack")) {
      ids.add("unstack")
    }
    if (hasStackFix(problem, "Rename")) {
      ids.add("rename")
    }
  }
  return ids
}

/**
 * The fixes of the stack problem of the branch as Rust offers them (`StackProblemView.fixes`), each the item of the
 * branch menu that runs it, for the alert of the problem: "Stack On ‹parent›", "Unstack", "Rename…"; a reword is the
 * commit's ("Reword…"), so none here. Waits only for the sync (`isSyncing`, read by the caller's selector) and the
 * operations: the problem doesn't block its fixes. Empty for a branch without a problem.
 */
export function getBranchStackFixItems(branch: Branch, operations: Operations, isSyncing: boolean): MenuItem[] {
  const problem = stackProblemOf(branch)
  if (problem === null) {
    return []
  }
  const items: MenuItem[] = stackOnFixes(problem).map(parent => ({ ...getStackOnTargetItem(branch.name, parent), label: `Stack On ${parent}`, icon: LayersIcon }))
  if (hasStackFix(problem, "Unstack")) {
    items.push(getUnstackItem(branch.name, branch.stack?.declaredParent ?? null))
  }
  if (hasStackFix(problem, "Rename")) {
    items.push(getRenameItem(branch))
  }
  const blocker = fixBlocker(branch, isSyncing)
  return items.map(item => rewriteItem(branch, item, operations, blocker))
}

/**
 * Why the push button of the branch is disabled: the blocked parent, a running push of the branch or of its stack, an
 * inline action, a sync that rebuilds the branch anew, nothing to push
 */
function getPushBlocker(branch: Branch, { isPushing, stackPushing, processing }: { isPushing: boolean; stackPushing: boolean; processing: boolean }): string {
  const parked = getParkedState(branch.name)
  if (parked !== null && !isPushing) {
    return formatParkedBlocker(parked)
  }
  if (branch.status === "Blocked") {
    return formatBlockedReason(branch)
  }
  // the last sync did not build it: its virtual branch is an older build, which no push takes
  if (branch.hasError) {
    return branch.statusText === "" ? "The branch failed to sync" : `The branch failed to sync: ${branch.statusText}`
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
 * "Patronus ›", the rarer starts beside "Safe Push" (S), each disabled with the reasons of the next step of the branch
 * (`getBranchNextStep`): "Safe Push With Profile…" always opens the popover at the row's slot, its profile to choose (a
 * comment on a pull request can't choose one); "Dry Run" starts at once when it has nothing to say, else opens it
 * (`requestSafePush`); "Check Approvals Again" asks for the approvals that count anew. The approvals go into the GitHub pull request: "Create Pull Request…" is the review group's; no Space merge
 * request (deprecated).
 */
function getPatronusMenuItem(branch: Branch, nextStep: BranchNextStep): MenuItem {
  const blocker = nextStep.safePushBlocker
  const fromPullRequest = getPullRequestStart(getPatronusGitHubRepository(), getPullRequest(branch.name), "rebaseAutosquash", getReadinessTarget())
  const profileBlocker = blocker ?? (fromPullRequest === null ? null : `A comment on #${fromPullRequest.number} runs the default checks: it can't choose a profile`)
  const children: MenuItem[] = [
    {
      id: "safe-push-with-profile",
      label: "Safe Push With Profile…",
      icon: SlidersHorizontalIcon,
      disabled: profileBlocker !== null,
      description: profileBlocker ?? "Chooses the merge profile of the Safe Push, which decides its checks",
      onSelect: () => void requestSafePush(branch.name, { mode: "rebaseAutosquash", via: "key" }),
    },
    {
      id: "dry-run",
      label: "Dry Run",
      icon: FlaskConicalIcon,
      disabled: nextStep.dryRunBlocker !== null,
      description: nextStep.dryRunBlocker ?? describeStart("dryRun", branch.name, getLandingBranchCount(branch.name), getReadinessTarget()),
      onSelect: () => void requestSafePush(branch.name, { mode: "dryRun", via: "click" }),
    },
    {
      id: "check-approvals",
      label: "Check Approvals Again",
      icon: RefreshCwIcon,
      description: "Reads the reviews of the pull request from GitHub and asks Patronus whose approvals count",
      onSelect: () => void recheckReviewReadiness(branch.name),
    },
  ]
  return { id: "patronus", label: "Patronus", icon: ShieldEllipsisIcon, onSelect: () => {}, children }
}

/**
 * "Safe Push" (S; "Safe Push · 3" for a stacked branch): the start of the row (`requestSafePush`), disabled with the
 * reason of `getSafePushBlocker`
 */
function getSafePushMenuItem(branch: Branch, nextStep: BranchNextStep = getBranchNextStepOf(branch)): MenuItem {
  const branchCount = getLandingBranchCount(branch.name)
  const blocker = nextStep.safePushBlocker
  return {
    id: "safe-push",
    label: formatStartLabel("rebaseAutosquash", branchCount),
    icon: ShieldCheckIcon,
    kbds: ["s"],
    disabled: blocker !== null,
    description: blocker ?? describeStart("rebaseAutosquash", branch.name, branchCount, getReadinessTarget()),
    onSelect: () => void requestSafePush(branch.name, { mode: "rebaseAutosquash", via: "click" }),
  }
}

/**
 * Why a Safe Push of the branch can't start now, null when it can: for every surface that offers it (the slot, the
 * menu, S on the row, the branch preview, the review, the stack header), the next step of the branch
 * (`getBranchNextStep`): the branch and its robots (`landing.start` of the view), landed or merged, then what Patronus would refuse
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
 * closed one, as the menu offers both), "Safe Push" where Patronus is and the branch has work, "Push" for a branch
 * with work, disabled with why while its first push is a step of its landing or the answers that tell it are to come
 * (`firstPushWait`: "⋯" pushes it alone). The same items as the menu; Push has no key, P is the previous change there.
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
  // offered until Patronus says the repository is not on Space: disabled with what it waits for before its answer
  if (hasPush && isPatronusOffered()) {
    items.push(getSafePushMenuItem(branch))
  }
  if (hasPush) {
    // the push of the button alone: its second way is in "⋯"; a first push that Safe Push and Create Pull Request make
    // is disabled with why, so the title bar keeps its buttons where they are when the answers that decide it come
    const push = getPushMenuItems(branch)[0]!
    const pending = firstPushPending(isPatronusPending(), isPullRequestsLoading())
    const waits = isBranchPushing(branch.name)
      ? null
      : firstPushWait(branch, { landingPushes: isLandingPushAvailable(branch.name), patronus: isPatronusAvailable(), held: getPushHold(branch.name) !== null, pending })
    items.push({ ...push, kbds: undefined, ...(waits !== null && { disabled: true, description: waits }) })
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
 * popover of its Safe Push with a merge profile that skips the code-owner review; null on any other step
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
      void requestSafePush(branch.name, { mode: "rebaseAutosquash", via: "click", bypassReview: true })
    },
  }
}

/**
 * "Create Pull Request…" of a branch without a live pull request, on GitHub, pushed or not (the dialog pushes it); null
 * otherwise. Disabled with the reason once nothing of it is left to land (`landed`, the next step's `landed`: "Landed in
 * master with oauth"): a pull request of commits that are in the target has nothing to review; and while GitHub hasn't
 * listed the pull requests (`isPullRequestsLoading`): the branch may have one.
 */
export function getCreatePullRequestItem(branch: Branch, landed: string | null = getBranchNextStepNow(branch.name)?.landed ?? null): MenuItem | null {
  if (!canCreatePullRequest(branch)) {
    return null
  }
  // the pull request of the branch is not known yet: never a second one of a branch that has one
  const blocker = isPullRequestsLoading() ? formatPendingFact("pullRequests") : landed
  return {
    id: "create-pull-request",
    label: CREATE_PULL_REQUEST_LABEL,
    icon: GitPullRequestCreateIcon,
    ...(blocker !== null && { disabled: true, description: blocker }),
    onSelect: () => openCreatePullRequestDialog(branch.name),
  }
}

/**
 * "Move to…" of a branch row: all its commits, or those of every selected branch if the row is selected, go to one
 * branch, a new one or an existing one of the form's chips; a whole issue by the issue groups, without a rewrite
 */
function getMoveMenuItem(name: string): MenuItem {
  return {
    id: "move-to",
    label: "Move to…",
    icon: FolderInputIcon,
    kbds: ["m"],
    onSelect: () => {
      if (!getSelectedBranches().includes(name)) {
        selectBranches(name, "only", getBranchTableOrder())
      }
      setBranchTargetOpen(true)
    },
  }
}

/** The most branches "Stack On…" lists; the newest come first */
const STACK_TARGET_LIMIT = 20

/**
 * "Stack On…" (any other branch that isn't stacked on this one; of a branch with a stack problem, the parents its fixes
 * name) and "Unstack from ‹parent›" ("Unstack" where a stack problem offers it to a branch without a parent): the
 * rewrites of the shape group, which waits for the sync
 */
function getStackShapeItems(branch: Branch): MenuItem[] {
  const parent = branch.stack?.declaredParent ?? null
  const problem = stackProblemOf(branch)
  const fixTargets = problem === null ? [] : stackOnFixes(problem)
  const excluded = new Set([branch.name, ...getStackDescendants(branch.name)])
  const targets =
    fixTargets.length > 0
      ? fixTargets
      : branchSyncStore
          .getState()
          .branchOrder.filter(name => !excluded.has(name) && name !== parent)
          .slice(0, STACK_TARGET_LIMIT)
  const items: MenuItem[] = [
    {
      id: "stack-on",
      label: "Stack On…",
      icon: LayersIcon,
      disabled: targets.length === 0,
      ...(targets.length === 0 && { description: "No other branch to stack on" }),
      onSelect: () => {},
      children: targets.map(target => getStackOnTargetItem(branch.name, target)),
    },
  ]
  if (parent !== null || (problem !== null && hasStackFix(problem, "Unstack"))) {
    items.push(getUnstackItem(branch.name, parent))
  }
  return items
}

/** The item of "Stack On…" that stacks the branch on `target`, named by the target */
function getStackOnTargetItem(name: string, target: string): MenuItem {
  return { label: target, onSelect: (context: SelectContext) => void restackBranch(name, target, { surface: context.surface }) }
}

/**
 * The collapse of the stack and, on every branch of a stack, "Whole Stack ›", the actions of the whole stack of its root
 * (`getWholeStackMenuItems`; Push with Children stays beside the push of the root), so a small stack without an island
 * has them in the menu and the preview of each of its branches; none for a branch of no stack. Neither rewrites the
 * branch: they don't wait for the sync.
 */
function getStackViewItems(branch: Branch): MenuItem[] {
  const items: MenuItem[] = []
  // the stack folds into its root, from any of its rows, as ⌥← and ⌥→ do
  const collapse = getStackCollapse(branch.name)
  if (collapse !== null) {
    items.push(
      collapse.collapsed
        ? { id: "expand-stack", label: "Expand Stack", icon: UnfoldVerticalIcon, kbds: ["alt", "arrowright"], onSelect: () => void toggleStackCollapsed(branch.name, false) }
        : { id: "collapse-stack", label: "Collapse Stack", icon: FoldVerticalIcon, kbds: ["alt", "arrowleft"], onSelect: () => void toggleStackCollapsed(branch.name, true) },
    )
  }
  // every branch of a stack is a piece of the whole stack: one submenu, its root's, on each of them
  const root = getStackAncestors(branch.name)[0] ?? (getStackDescendants(branch.name).length > 0 ? branch.name : null)
  const wholeStack = root === null ? [] : getWholeStackMenuItems(root, { withPush: false })
  if (wholeStack.length > 0) {
    items.push({ id: "whole-stack", label: "Whole Stack", icon: LayersIcon, onSelect: () => {}, children: wholeStack })
  }
  return items
}

/**
 * "Push with Children (3 branches)" of a branch, for its menu and the "⋯" of its stack island: the branch and the
 * branches built on it that have work for the remote, parents first, in one push, as Rust orders them
 * (`ChildrenPush`); of the root of a stack that forks, the stack branch of its whole stack too, while it goes along,
 * counted and named in the hover. Every branch with children offers it, also during a sync (the push waits for it and
 * pushes what it leaves); null for a branch without children to land. Disabled with the reason while a running Safe Push
 * holds the branch (`PushHold`: a push of the stack it runs in won't land, and every Safe Push in a stack holds its
 * root), while the stack is being pushed, and while it would push nothing beyond the branch itself
 * (`describePushWithChildrenBlocker`), also once merged Safe Pushes landed it and every branch built on it ("Landed in
 * master with deploy and side-test: nothing of air and its children is left to push"). A branch a merged one landed is
 * only left out: its children push without it.
 */
export function getPushWithChildrenItem(name: string): MenuItem | null {
  const children = getPushOrders(name).children
  // the parked ones stay local: Push with Children of a branch to land leaves them out
  if (children === null) {
    return null
  }
  const parked = getParkedState(name)
  if (parked !== null) {
    return { id: "push-with-children", label: "Push with Children", icon: UploadIcon, disabled: true, description: formatParkedBlocker(parked), onSelect: () => {} }
  }
  // a running Safe Push holds the stack; a merged one that landed the branch leaves its children to push
  const landedHold = getPushHold(name)
  const hold = landedHold?.kind === "landed" ? null : landedHold
  // the pull request of the whole stack follows its branches: its stack branch goes along
  const stack = getStackBranch(name)
  const withStackBranch = children.stackBranch?.kind === "along"
  const descendants = getUnparkedStackDescendants(name)
  let reason: string | null
  if (hold !== null) {
    reason = formatPushHoldReason(name, hold)
  } else if (isStackPushing(name)) {
    reason = "The stack is being pushed"
  } else if (!children.offers && branchSyncStore.getState().isSyncing && [name, ...descendants].some(it => getBranch(it)?.remoteStatus == null)) {
    // the statuses the sync computes say what is left to push
    reason = "Wait for the sync to finish"
  } else {
    reason = describePushWithChildrenBlocker(name, children, getLandedWithChildrenReason(name))
  }
  const description = reason ?? (withStackBranch && stack !== undefined ? describeStackBranchAlong(stack, getStackPullRequest(name)) : null)
  return {
    id: "push-with-children",
    label: children.offers ? formatPushWithChildrenLabel(children.order, withStackBranch) : "Push with Children",
    icon: UploadIcon,
    disabled: reason !== null,
    ...(description !== null && { description }),
    onSelect: () => void pushWithChildren(name),
  }
}

/** By branch: the menu built last, and the store states it was built from (`getBranchMenuNow`) */
const shownMenus = new Map<string, { sources: readonly unknown[]; groups: MenuItemGroups }>()

/**
 * The menu of the branch as the stores are now (`getBranchContextMenuItems`), one object while the stores it reads stay:
 * the selector of `useBranchMenu`. A builder called in a component would be memoized by the compiler on its arguments,
 * so a menu that stays mounted (reopened while it fades out) would show its first state.
 */
export function getBranchMenuNow(name: string, setExpanded: (name: string, expanded: boolean) => void): MenuItemGroups | null {
  const branch = getBranch(name)
  if (branch === undefined) {
    return null
  }
  const sources = [...nextStepSources(), commitMoveStore.getState(), youTrackAuthStore.getState(), setExpanded]
  const shown = shownMenus.get(name)
  if (shown !== undefined && shown.sources.length === sources.length && shown.sources.every((source, index) => source === sources[index])) {
    return shown.groups
  }
  const groups = getBranchContextMenuItems(branch, setExpanded, getOperations())
  shownMenus.set(name, { sources, groups })
  return groups
}

/** The menu of the branch, rendered again when a store it reads changes (the operations among them); null without the branch */
export function useBranchMenu(name: string, setExpanded: (name: string, expanded: boolean) => void): MenuItemGroups | null {
  useStore(commitMoveStore, () => getBranchMenuNow(name, setExpanded))
  useStore(youTrackAuthStore, () => getBranchMenuNow(name, setExpanded))
  return useNextStepStores(() => getBranchMenuNow(name, setExpanded))
}

/**
 * "Unstack from ‹parent›" of a stacked branch, as a menu item: its menu, the notice of a parent it was built past, the
 * alert of a stack problem; "Unstack" of a branch whose stack problem leaves it without a parent (conflicting parents)
 */
function getUnstackItem(name: string, parent: string | null): MenuItem {
  return {
    id: "unstack",
    label: parent === null ? "Unstack" : `Unstack from ${parent}`,
    icon: Layers2Icon,
    onSelect: context => void restackBranch(name, null, { surface: context.surface }),
  }
}

/**
 * "Unstack from ‹parent›" of a stacked branch as the operations leave it (`operationItem`: running, or waiting for
 * another rewrite, or for the sync), for a surface beside the menu; null for a branch that is not stacked
 */
export function getBranchUnstackItem(branch: Branch, operations: Operations): MenuItem | null {
  const parent = branch.stack?.declaredParent ?? null
  if (parent === null) {
    return null
  }
  return rewriteItem(branch, getUnstackItem(branch.name, parent), operations)
}
