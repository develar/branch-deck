import { CopyIcon, FoldVerticalIcon, LayersIcon, LinkIcon, ShieldEllipsisIcon, UnfoldVerticalIcon, WandSparklesIcon } from "lucide-react"
import { useStore } from "zustand"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { getBranchNextStep } from "@/hooks/useBranchNextStep"
import { pushRunsOf } from "@/hooks/useBranchPushes"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import { ungroupBranch } from "@/hooks/useMoveCommits"
import { unparkBranch } from "@/hooks/useParkBranch"
import { requestSafePush } from "@/hooks/useSafePush"
import { restackBranch } from "@/hooks/useStackActions"
import { getWholeStackMenuItems } from "@/hooks/useWholeStackActions"
import { formatSquashFixupsLabel, getListFixups } from "@/lib/autosquash"
import type { ActionId, ActionState, PullRequest } from "@/lib/bindings"
import { openBranchReviewWindow } from "@/lib/branch-review-window"
import { copyText } from "@/lib/clipboard"
import { getMarkFixedItem } from "@/lib/issue-actions"
import type { MenuItem, MenuItemGroups, SelectContext } from "@/lib/menu"
import { describeStart, formatStartLabel } from "@/lib/patronus"
import { openPullRequest } from "@/lib/pull-request"
import { pushFromButton, pushWithChildren } from "@/lib/push-run"
import { formatPushWithChildrenLabel, stackOnFixes, stackProblemOf } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import { describeStackBranchAlong } from "@/lib/whole-stack"
import { getRowPart } from "@/model/store"
import type { ActionItem, BlockerWords } from "@/presenters/actions"
import { ACTION_LOOKS, findAction, getRobotActionItems, presentAction } from "@/presenters/actions"
import type { BranchNextStep } from "@/presenters/next-step"
import { NO_NEXT_STEP } from "@/presenters/next-step"
import { getRetargetCommand } from "@/presenters/pull-request-url"
import { firstPushPending, firstPushWait, getPushButtonNow, isLandingPushAvailable } from "@/presenters/push-button"
import { getRequestReviewItem } from "@/presenters/review"
import { authStore } from "@/stores/auth"
import type { Branch } from "@/stores/branch-sync"
import {
  branchSyncStore,
  getAutosquashPlan,
  getBranch,
  getBranchTableOrder,
  getIssueGroups,
  getParkedState,
  getPushOrders,
  getStackAncestors,
  getStackBranch,
  getStackChildren,
  getStackDescendants,
  isBranchPushing,
  isStackPushing,
} from "@/stores/branch-sync"
import { commitMoveStore, getSelectedBranches, requestReveal, selectBranches, setBranchTargetOpen } from "@/stores/commit-move"
import { closeCreatePullRequestDialog, openCreatePullRequestDialog } from "@/stores/create-pull-request-dialog"
import { indicatorsStore } from "@/stores/indicators"
import { openInline } from "@/stores/inline-row-action"
import { getBranchRobot, getLandingBranchCount, getPushHold, isPatronusAvailable, isPatronusPending, patronusStore } from "@/stores/patronus"
import { getPullRequest, getStackPullRequest, isPullRequestsLoading, pullRequestsStore } from "@/stores/pull-requests"
import { requestPushConfirm } from "@/stores/push-confirm"
import { getReadinessTarget, recheckReviewReadiness, reviewReadinessStore } from "@/stores/review-readiness"
import { safePushStore } from "@/stores/safe-push"
import { repositorySettingsStore } from "@/stores/settings/repositorySettings"
import { getStackCollapse, toggleStackCollapsed } from "@/stores/stack-collapse"
import { getYouTrackKeyOfIssue } from "@/stores/youtrack-issues"

/** The actions Rust offers of the subject `key` now (`RepositoryView.actions`: a branch, `stack:<root>`, `robot:<id>`) */
export function actionsOf(key: string): readonly ActionState[] | undefined {
  return getRowPart(key, "actions")
}

/** What the words of a blocker of the branch read of its row */
function wordsOf(branch: Branch, step?: string | null): BlockerWords {
  return { subject: branch.name, statusText: branch.statusText, ...(step !== undefined && { step }) }
}

/** The item of the action `id` of the branch as Rust offers it (`presentAction`); null while it doesn't */
function branchItem(branch: Branch, id: ActionId, item: ActionItem, step?: string | null, actions: readonly ActionState[] | undefined = actionsOf(branch.name)): MenuItem | null {
  return presentAction(actions, id, item, wordsOf(branch, step))
}

/**
 * A start of the next step of a subject (Safe Push, Dry Run, its rarer forms): Rust's availability, and what runs on the
 * row over it, which the presented step's reason says already (`step`, `presentNextStep`)
 */
export function withStepReason(item: MenuItem | null, step: string | null): MenuItem | null {
  return item === null || step === null || item.disabled === true ? item : { ...item, disabled: true, description: step }
}

/** The items that are there, in order */
function present(...items: (MenuItem | null)[]): MenuItem[] {
  return items.filter(item => item !== null)
}

/**
 * Context menu of an active branch row, its actions by how often they are wanted (`spec/branch/branch-row.spec.md`, "Branch
 * menu"), each offered and disabled as Rust decides it (`domain_policy::actions`, `presentAction`): the fix of a
 * conflict, then the landing (Safe Push first, the actions of the branch's robot, the rarer starts in "Patronus ›", the
 * push), the review (review the changes, open or create the pull request), the shape (rename, move, stack, ungroup), the
 * stack view, the commits (amend, issue reference, squash fixups), what sets the branch aside (park, unapply), and the
 * copies in "Copy ›". An item with a key never goes into a submenu. An item that opens a form ends with "…"; an item shows
 * its key only where the focused row has it (`useBranchRowKeys`, `BranchTableCard`). The right-click on the row and the
 * "⋯" of its Actions cell show it.
 */
export function getBranchContextMenuItems(branch: Branch, setExpanded: (name: string, expanded: boolean) => void): MenuItemGroups {
  const groups: MenuItemGroups = []
  const pullRequest = getPullRequest(branch.name)
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

  const nextStep = getBranchNextStep(branch.name) ?? NO_NEXT_STEP
  const robot = getBranchRobot(branch.name)
  // its pull request is the branch's one, which "Open Pull Request" already opens
  const robotActions = robot === undefined ? [] : getRobotActionItems(robot).filter(item => item.id !== "open-robot-pull-request" || pullRequest?.url !== robot.pullRequest?.url)
  const landingActions = present(
    // the build of what its Safe Push lands needs a branch outside it: the fix first ("Stack lane-run on stills-codec")
    nextStep.landingFix,
    getSafePushMenuItem(branch, nextStep),
  )
  // a listed robot keeps its actions while Patronus doesn't answer for a new start
  landingActions.push(...robotActions, ...present(getPatronusMenuItem(branch, nextStep)), ...getPushMenuItems(branch))
  // the push of the branches stacked on it, beside the push of the branch: offered on every branch with children
  landingActions.push(...present(getPushWithChildrenItem(branch.name)))
  // the commits someone else pushed to its remote branch: the question of the push button lists them with the pick
  landingActions.push(
    ...present(
      branchItem(branch, "pick-remote-commits", {
        description: "The remote has commits your branch doesn't: take them into the branch instead of dropping them",
        onSelect: () => requestPushConfirm(branch.name, "row"),
      }),
    ),
  )
  // the issue of the branch, once its work landed: the backend refuses an issue resolved already
  const youTrackKey = getYouTrackKeyOfIssue(branch.issueKey)
  if (youTrackKey !== null) {
    landingActions.push({ ...getMarkFixedItem(youTrackKey, branchSyncStore.getState().operations), label: `Mark ${youTrackKey} Fixed` })
  }
  groups.push(landingActions)

  groups.push(
    present(
      branchItem(branch, "review-changes", {
        description: "The diff of the branch, the parts of its stack, the files of each code owner, in a window",
        onSelect: () => void openBranchReviewWindow(branch.name),
      }),
      pullRequest === undefined ? null : openPullRequestItemOf(pullRequest),
      // the row's step when it is the one (a group nobody of it can approve says so), else the action's item
      nextStep.kind === "requestReview" ? nextStep.item : getRequestReviewItem(branch.name),
      // a pull request into another base: the step retargets one into a virtual branch, the menu always offers it
      findAction(actionsOf(branch.name), "retarget-pull-request") === undefined ? null : nextStep.retarget,
      getCreatePullRequestItem(branch),
    ),
  )

  groups.push(present(getRenameItem(branch), getMoveMenuItem(branch), ...getStackShapeItems(branch), getUngroupItem(branch)))
  const stackView = getStackViewItems(branch)
  if (stackView.length > 0) {
    groups.push(stackView)
  }

  const fixupCount = getListFixups(getAutosquashPlan(), branch.name).count
  groups.push(
    present(
      branchItem(branch, "amend-changes", {
        onSelect: () => {
          // show the commits; the form loads the uncommitted changes when it opens
          setExpanded(branch.name, true)
          openInline("amend-changes", branch.name)
        },
      }),
      branchItem(branch, "add-issue-reference", { onSelect: () => openInline("issue-reference", branch.name) }),
      // the fixups of the autosquash plan, which the page loads beside the view
      fixupCount === 0
        ? null
        : branchItem(branch, "squash-fixups", {
            label: formatSquashFixupsLabel(fixupCount),
            onSelect: () => {
              // the confirmation opens from the button above the commits
              setExpanded(branch.name, true)
              openInline("squash-fixups", branch.name)
            },
          }),
    ),
  )

  // what takes the branch out of the landing: kept local, or its commits archived
  const unapply = branchItem(branch, "unapply", {
    onSelect: () => {
      // show the commits that will be unapplied
      setExpanded(branch.name, true)
      openInline("unapply", branch.name)
    },
  })
  groups.push([...getParkMenuItems(branch), ...present(unapply)])

  groups.push([getCopyMenuItem(branch, pullRequest)])
  return groups.filter(group => group.length > 0)
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
function getParkMenuItems(branch: Branch): MenuItem[] {
  const name = branch.name
  const parked = getParkedState(name)
  if (parked?.with != null) {
    const parent = parked.with
    return present(branchItem(branch, "unpark", { label: `Unpark ${parent}`, description: `${name} carries the commits of ${parent}`, onSelect: () => void unparkBranch(parent) }))
  }
  if (parked !== null) {
    return present(
      branchItem(branch, "park", { label: "Edit Park Note…", onSelect: () => openInline("park", name) }),
      branchItem(branch, "unpark", { description: "It lands as any other branch again", onSelect: () => void unparkBranch(name) }),
    )
  }
  const pullRequest = getPullRequest(name)
  const description =
    pullRequest?.state === "open"
      ? `Keeps it local; #${pullRequest.number} stays open, a merge on GitHub still lands it`
      : "Keeps it local: no push or Safe Push sends it, and it is no move of yours"
  return present(branchItem(branch, "park", { description, onSelect: () => openInline("park", name) }))
}

/**
 * The push of the push button of the row, named as the button ("Push (2)", "Force Push (1)", "Push Stack (3
 * branches)", `getPushButtonNow`) and pushing as a click on it (`pushFromButton`: the stack while a parent has work for
 * the remote, a push that drops remote commits asks in the button's description); as Rust decides the push
 * (`domain_policy::actions`): running while its push runs in the session, disabled with the reason, a push of a stack
 * that sends the branch among them ("Pushing login…"). One item for the push of the branch and of its stack: the menu
 * never pushes past the confirmation the button asks for. After it, while the button pushes the stack, the second way
 * its description offers: "Push oauth Only".
 */
function getPushMenuItems(branch: Branch): MenuItem[] {
  const isPushing = isBranchPushing(branch.name) || isStackPushing(branch.name)
  const button = getPushButtonNow(branch, isPushing, pushRunsOf(branch.name))
  const push = branchItem(branch, "push", { label: button.text, onSelect: () => pushFromButton(branch.name, "row") })
  if (push === null) {
    return []
  }
  const enabled = push.disabled !== true
  const item = enabled && button.pushesStack ? { ...push, description: `Pushes ${joinNames(getPushOrders(branch.name).button)}, parents first` } : push
  // the second way the description of the button offers while it pushes the stack: the branch alone. Its Push Stack
  // while the button pushes the branch alone stays in the description, beside the confirmation an ancestor asks for
  const other = button.action
  if (other?.id !== "push-branch-only") {
    return [item]
  }
  return [
    item,
    {
      id: other.id,
      label: other.label,
      disabled: !enabled,
      description: enabled ? other.description : push.description,
      onSelect: other.run,
    },
  ]
}

/** "Rename…", "Name This Branch…" of an issue branch: the inline form of the row */
function getRenameItem(branch: Branch, actions?: readonly ActionState[]): MenuItem | null {
  return branchItem(branch, "rename", { label: branch.origin === "Issue" ? "Name This Branch…" : "Rename…", onSelect: () => openInline("rename", branch.name) }, undefined, actions)
}

/**
 * The fixes of the stack problem of the branch as Rust offers them (`StackProblemView.fixes`), each the item of the
 * branch menu that runs it, for the alert of the problem: "Stack On ‹parent›", "Unstack", "Rename…"; a reword is the
 * commit's ("Reword…"), so none here. Rust runs them while the problem blocks the rest, running while the session runs
 * one. Empty for a branch without a problem. A component passes the actions it subscribed to (`actions`,
 * `useRowPart`), so its items follow them.
 */
export function getBranchStackFixItems(branch: Branch, actions: readonly ActionState[] | undefined = actionsOf(branch.name)): MenuItem[] {
  const problem = stackProblemOf(branch)
  if (problem === null) {
    return []
  }
  const stackOn = (parent: string): MenuItem | null =>
    branchItem(branch, "stack-on", { label: `Stack On ${parent}`, onSelect: context => void restackBranch(branch.name, parent, { surface: context.surface }) }, undefined, actions)
  return present(
    ...stackOnFixes(problem).map(stackOn),
    problem.fixes.includes("Unstack") ? getUnstackItem(branch, actions) : null,
    problem.fixes.includes("Rename") ? getRenameItem(branch, actions) : null,
  )
}

/**
 * "Patronus ›", the rarer starts beside "Safe Push" (S), each as Rust offers it (`domain_policy::actions`, the reasons of
 * the next step of the branch): "Safe Push With Profile…" always opens the popover at the row's slot, its profile to
 * choose (a comment on a pull request can't choose one); "Dry Run" starts at once when it has nothing to say, else opens
 * it (`requestSafePush`); "Check Approvals Again" asks for the approvals that count anew. The approvals go into the
 * GitHub pull request: "Create Pull Request…" is the review group's; no Space merge request (deprecated). None off Space.
 */
function getPatronusMenuItem(branch: Branch, nextStep: BranchNextStep): MenuItem | null {
  const children = present(
    withStepReason(
      branchItem(
        branch,
        "safe-push-with-profile",
        {
          description: "Chooses the merge profile of the Safe Push, which decides its checks",
          onSelect: () => void requestSafePush(branch.name, { mode: "rebaseAutosquash", via: "key" }),
        },
        nextStep.safePushBlocker,
      ),
      nextStep.safePushBlocker,
    ),
    withStepReason(
      branchItem(
        branch,
        "dry-run",
        {
          description: describeStart("dryRun", branch.name, getLandingBranchCount(branch.name), getReadinessTarget()),
          onSelect: () => void requestSafePush(branch.name, { mode: "dryRun", via: "click" }),
        },
        nextStep.dryRunBlocker,
      ),
      nextStep.dryRunBlocker,
    ),
    branchItem(branch, "check-approvals", {
      description: "Reads the reviews of the pull request from GitHub and asks Patronus whose approvals count",
      onSelect: () => void recheckReviewReadiness(branch.name),
    }),
  )
  return children.length === 0 ? null : { id: "patronus", label: "Patronus", icon: ShieldEllipsisIcon, onSelect: () => {}, children }
}

/**
 * "Safe Push" (S; "Safe Push · 3" for a stacked branch): the start of the row (`requestSafePush`), as Rust offers it,
 * disabled with the reason of the next step of the branch (the slot, the menu, S on the row, the branch preview, the
 * review and the stack header say the same)
 */
function getSafePushMenuItem(branch: Branch, nextStep: BranchNextStep = getBranchNextStep(branch.name) ?? NO_NEXT_STEP): MenuItem | null {
  const branchCount = getLandingBranchCount(branch.name)
  const item = branchItem(
    branch,
    "safe-push",
    {
      label: formatStartLabel("rebaseAutosquash", branchCount),
      description: describeStart("rebaseAutosquash", branch.name, branchCount, getReadinessTarget()),
      onSelect: () => void requestSafePush(branch.name, { mode: "rebaseAutosquash", via: "click" }),
    },
    nextStep.safePushBlocker,
  )
  return withStepReason(item, nextStep.safePushBlocker)
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
  items.push(...present(getCreatePullRequestItem(branch)))
  // as the push button of the row: only a branch that synced without an error has something to push
  const hasPush = !branch.hasError && branch.commitCount > 0
  if (hasPush) {
    // offered until Patronus says the repository is not on Space: disabled with what it waits for before its answer
    items.push(...present(getSafePushMenuItem(branch)))
    // the push of the button alone: its second way is in "⋯"; a first push that Safe Push and Create Pull Request make
    // is disabled with why, so the title bar keeps its buttons where they are when the answers that decide it come
    const push = getPushMenuItems(branch)[0]
    if (push !== undefined) {
      const pending = firstPushPending(isPatronusPending(), isPullRequestsLoading())
      const waits = isBranchPushing(branch.name)
        ? null
        : firstPushWait(branch, { landingPushes: isLandingPushAvailable(branch.name), patronus: isPatronusAvailable(), held: getPushHold(branch.name) !== null, pending })
      items.push({ ...push, kbds: undefined, ...(waits !== null && { disabled: true, description: waits }) })
    }
  }
  return items
}

/**
 * "Open Pull Request" with the key of the focused row and of the branch preview; a closed one says so, beside the
 * Create Pull Request… a branch with only a closed one gets
 */
export function openPullRequestItemOf(pullRequest: PullRequest): MenuItem {
  const look = ACTION_LOOKS["open-pull-request"]
  return {
    id: "open-pull-request",
    label: look.label,
    icon: look.icon,
    kbds: [...look.kbds],
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
  return pullRequest === undefined ? getCreatePullRequestItem(branch) : openPullRequestItemOf(pullRequest)
}

/**
 * "Safe Push Without Review…" of a branch whose approvals count only in a pull request it doesn't have (rows 7 and 11
 * while GitHub doesn't list the pull requests): the popover of its Safe Push with a merge profile that skips the
 * code-owner review, as Rust offers it; null on any other step
 */
export function getSafePushWithoutReviewItem(branch: Branch, nextStep: BranchNextStep = getBranchNextStep(branch.name) ?? NO_NEXT_STEP): MenuItem | null {
  const blocker = nextStep.withoutReview?.blocker ?? null
  const item = branchItem(
    branch,
    "safe-push-without-review",
    {
      description: "Starts the Safe Push with a merge profile that skips the code-owner review, such as Large Scale Change",
      onSelect: () => {
        closeCreatePullRequestDialog()
        void requestSafePush(branch.name, { mode: "rebaseAutosquash", via: "click", bypassReview: true })
      },
    },
    blocker,
  )
  return withStepReason(item, blocker)
}

/**
 * "Create Pull Request…" of a branch without a live pull request, on GitHub, pushed or not (the dialog pushes it), as
 * Rust offers it: disabled while GitHub hasn't listed the pull requests (the branch may have one) and once all of it
 * landed (a pull request of commits that are in the target has nothing to review); null otherwise. A component passes the
 * actions it subscribed to (`useRowPart`)
 */
export function getCreatePullRequestItem(branch: Branch, actions: readonly ActionState[] | undefined = actionsOf(branch.name)): MenuItem | null {
  return presentAction(actions, "create-pull-request", { onSelect: () => openCreatePullRequestDialog(branch.name) }, wordsOf(branch))
}

/**
 * "Move to…" of a branch row: all its commits, or those of every selected branch if the row is selected, go to one
 * branch, a new one or an existing one of the form's chips; a whole issue by the issue groups, without a rewrite
 */
function getMoveMenuItem(branch: Branch): MenuItem | null {
  const name = branch.name
  return branchItem(branch, "move-to", {
    onSelect: () => {
      if (!getSelectedBranches().includes(name)) {
        selectBranches(name, "only", getBranchTableOrder())
      }
      setBranchTargetOpen(true)
    },
  })
}

/** The most branches "Stack On…" lists; the newest come first */
const STACK_TARGET_LIMIT = 20

/**
 * "Stack On…" (any other branch that isn't stacked on this one; of a branch with a stack problem, the parents its fixes
 * name) and "Unstack from ‹parent›" ("Unstack" where a stack problem offers it to a branch without a parent), as Rust
 * offers them (stacked branches on, the fixes of a problem running while it blocks the rest): the rewrites of the shape
 * group. A "Stack On…" with no branch to list is disabled
 */
function getStackShapeItems(branch: Branch): (MenuItem | null)[] {
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
  const stackOn = branchItem(branch, "stack-on", { onSelect: () => {}, children: targets.map(target => getStackOnTargetItem(branch.name, target)) })
  const listed = stackOn !== null && targets.length === 0 && stackOn.disabled !== true ? { ...stackOn, disabled: true, description: "No other branch to stack on" } : stackOn
  return [listed, getUnstackItem(branch)]
}

/** The item of "Stack On…" that stacks the branch on `target`, named by the target */
function getStackOnTargetItem(name: string, target: string): MenuItem {
  return { label: target, onSelect: (context: SelectContext) => void restackBranch(name, target, { surface: context.surface }) }
}

/**
 * "Ungroup" ("Ungroup: ‹what›"): an issue group goes back to the issue branches of its keys, without a rewrite; a branch
 * whose children keep naming it as their parent confirms with them listed. None of an issue branch, or of one whose
 * commits carry no prefix to remove (`getRemovePrefixLabel`)
 */
function getUngroupItem(branch: Branch): MenuItem | null {
  const ungroupLabel = getRemovePrefixLabel(branch.commits, getIssueGroups())
  if (branch.origin === "Issue" || ungroupLabel === null) {
    return null
  }
  return branchItem(branch, "ungroup", {
    label: ungroupLabel === "Remove Prefix" || ungroupLabel === "Move Back" ? "Ungroup" : `Ungroup: ${ungroupLabel}`,
    onSelect: context => (getStackChildren(branch.name).length > 0 ? openInline("ungroup", branch.name) : void ungroupBranch(branch.name, { surface: context.surface })),
  })
}

/**
 * The collapse of the stack and, on every branch of a stack, "Whole Stack ›", the actions of the whole stack of its root
 * (`getWholeStackMenuItems`; Push with Children stays beside the push of the root), so a small stack without an island
 * has them in the menu and the preview of each of its branches; none for a branch of no stack
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
 * counted and named in the hover. Rust offers it on every branch with children to land, also during a sync (the push
 * waits for it and pushes what it leaves), and disables it with the reason (`domain_policy::actions`): parked, a running
 * Safe Push holds the branch, the statuses of a running sync are to come, nothing beyond the branch is left to push, a
 * push that holds the branch; running while it runs. Null for a branch without children to land.
 */
export function getPushWithChildrenItem(name: string): MenuItem | null {
  const branch = getBranch(name)
  const children = getPushOrders(name).children
  if (branch === undefined || children === null) {
    return null
  }
  // the pull request of the whole stack follows its branches: its stack branch goes along
  const stack = getStackBranch(name)
  const withStackBranch = children.stackBranch?.kind === "along"
  const along = withStackBranch && stack !== undefined ? describeStackBranchAlong(stack, getStackPullRequest(name)) : undefined
  return branchItem(branch, "push-with-children", {
    label: children.offers ? formatPushWithChildrenLabel(children.order, withStackBranch) : undefined,
    ...(along !== undefined && { description: along }),
    onSelect: () => void pushWithChildren(name),
  })
}

/**
 * The states of every store a menu of a branch or a whole stack reads: the view (its actions, its steps and the
 * operations of the session), the robots, the pull requests and the readiness its words still read, the busy states
 * this window gave its operations, the starts that run, the settings, a move of commits, the sign-ins; a change
 * of any is a new menu
 */
export function menuSources(): readonly unknown[] {
  return [
    branchSyncStore.getState(),
    patronusStore.getState(),
    pullRequestsStore.getState(),
    reviewReadinessStore.getState(),
    indicatorsStore.getState(),
    safePushStore.getState(),
    repositorySettingsStore.getState(),
    commitMoveStore.getState(),
    authStore.getState(),
  ]
}

/** Renders again with `select` when a store a menu reads changes (`menuSources`) */
export function useMenuStores<T>(select: () => T): T {
  useStore(branchSyncStore, select)
  useStore(patronusStore, select)
  useStore(pullRequestsStore, select)
  useStore(reviewReadinessStore, select)
  useStore(safePushStore, select)
  useStore(repositorySettingsStore, select)
  useStore(commitMoveStore, select)
  useStore(authStore, select)
  return useStore(indicatorsStore, select)
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
  const sources = [...menuSources(), setExpanded]
  const shown = shownMenus.get(name)
  if (shown !== undefined && shown.sources.length === sources.length && shown.sources.every((source, index) => source === sources[index])) {
    return shown.groups
  }
  const groups = getBranchContextMenuItems(branch, setExpanded)
  shownMenus.set(name, { sources, groups })
  return groups
}

/** The menu of the branch, rendered again when a store it reads changes (the view's operations among them); null without the branch */
export function useBranchMenu(name: string, setExpanded: (name: string, expanded: boolean) => void): MenuItemGroups | null {
  return useMenuStores(() => getBranchMenuNow(name, setExpanded))
}

/**
 * "Unstack from ‹parent›" of a stacked branch, as Rust offers it, for its menu, the notice of a parent it was built past
 * and the alert of a stack problem; "Unstack" of a branch whose stack problem leaves it without a parent (conflicting
 * parents); null for a branch that is not stacked
 */
function getUnstackItem(branch: Branch, actions?: readonly ActionState[]): MenuItem | null {
  const parent = branch.stack?.declaredParent ?? null
  return branchItem(
    branch,
    "unstack",
    { label: parent === null ? "Unstack" : `Unstack from ${parent}`, onSelect: context => void restackBranch(branch.name, null, { surface: context.surface }) },
    undefined,
    actions,
  )
}

/**
 * "Unstack from ‹parent›" of a stacked branch as Rust offers it (running while the session unstacks it, waiting behind
 * another rewrite), for a surface beside the menu; null for a branch that is not stacked. A component passes the actions
 * it subscribed to (`actions`, `useRowPart`)
 */
export function getBranchUnstackItem(branch: Branch, actions: readonly ActionState[] | undefined = actionsOf(branch.name)): MenuItem | null {
  return (branch.stack?.declaredParent ?? null) === null ? null : getUnstackItem(branch, actions)
}
