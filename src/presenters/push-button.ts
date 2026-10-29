// The push button of a branch (its row, its preview, its review window) and the items that push it, in words: what the
// push sends to the remote branch of the branch ("Push Branch (2)", "Force Push Branch (1)", "Push Stack (3 branches)"),
// what it replaces and drops, the other pushes its description offers, and why it can't push. Rust decides whether it
// can (the `push` and `push-with-children` actions of the row, `domain_policy::actions`), what each push sends
// (`RepositoryView.pushes`, `domain_policy::push`) and what a press does (`prepare` of the intent `Push`,
// `operations::push`); the page words the remote status of the row and those answers (spec/repository/outgoing.spec.md,
// "Push").
import type { ActionBlocker, ActionState, DroppedCommit } from "@/lib/bindings"
import { landedBranchDescription } from "@/lib/landed"
import { getPushColor } from "@/lib/push-summary"
import { buildRemoteStatusTexts } from "@/lib/remote-status"
import { formatPushStackLabel, formatPushWithChildrenLabel } from "@/lib/stack"
import { cutList } from "@/lib/text"
import type { UIColor } from "@/lib/ui-types"
import { getRowPart } from "@/model/store"
import type { SessionOperations } from "@/presenters/actions"
import { ACTION_LOOKS, findAction, formatActionBlocker } from "@/presenters/actions"
import type { PendingFact } from "@/presenters/next-step"
import { formatPendingFact } from "@/presenters/next-step"
import { getBranchesGitHubRepository, getPullRequestRetarget } from "@/presenters/pull-request-url"
import { formatStackPushLine } from "@/presenters/push"
import type { Branch, BranchSyncState, PushOrders } from "@/stores/branch-sync"
import { NO_OPERATIONS, pushOrdersIn } from "@/stores/branch-sync"
import { isPatronusAvailable } from "@/stores/patronus"
import { getPullRequest } from "@/stores/pull-requests"

/** What a press of the button pushes, besides what its state shows: the run of `push-run.ts` or of the stack push */
export interface PushButtonRuns {
  /** The push of the branch alone */
  readonly alone: () => void
  /** Push Stack: the branches it is built on that have work for the remote, then the branch */
  readonly stack: () => void
  /** Push with Children */
  readonly children: () => void
}

export interface PushButtonState {
  text: string
  /** The commits the push sends, the count of the label ("Force Push Branch (1)", the sum of a Push Stack); 0 without one */
  count: number
  color: UIColor
  tooltip: string
  /** The remote commits a force push drops, listed under the tooltip; null when it drops none */
  dropped: DroppedCommits | null
  disabled: boolean
  /**
   * The push drops remote commits the branch lacks: a click opens the description with them, and the Force Push button
   * there pushes. Any other push runs on the click.
   */
  confirm: boolean
  /**
   * The button pushes the stack (`PushOrders.button`): the ancestors that have work for the remote, root first, then
   * the branch; `action` is the push of the branch alone then
   */
  pushesStack: boolean
  /**
   * Another push the description offers after its text, while an ancestor has work for the remote: the push of the
   * branch alone when the button pushes the stack, else Push Stack
   */
  action: PushButtonAction | null
  /**
   * Push with Children, offered after `action` while branches stacked on this one have work for the remote, also while
   * the branch itself is up to date: its own push is disabled then, this one isn't
   */
  childrenAction: PushButtonAction | null
  /**
   * The open pull requests into a virtual branch the push retargets to the baseline first (`describePushRetargets`):
   * "Retargets #8 to master: GitHub may dismiss its approvals."; `warning` when one has approvals. Null without one.
   */
  retarget: { text: string; warning: boolean } | null
}

export interface PushButtonAction {
  /**
   * The item of the branch's menu with this push (`getBranchContextMenuItems`): the description of the button and the
   * menus offer one action
   */
  id: "push-branch-only" | "push-stack" | "push-with-children"
  /** The choice between the push of the button and this one, above the action button */
  description: string
  label: string
  run: () => void
}

export interface DroppedCommits {
  /** The newest ones, newest first (an Activity entry kept them without the email) */
  commits: readonly Omit<DroppedCommit, "authorEmail">[]
  /** How many more the push drops than listed */
  more: number
}

/** The nearest branch a stacked branch is built on that has work for the remote */
export interface UnpushedParent {
  name: string
  /** The parent has a remote branch, only without some of its commits */
  onRemote: boolean
  /** The branches a Push Stack of the branch pushes, the branch included */
  stackSize: number
  /** The branches the push button pushes, root first (`PushOrders.button`); empty when it pushes the branch alone */
  buttonStack: readonly Branch[]
}

/** The branches stacked on a branch that have work for the remote, and the stack branch of its whole stack */
export interface UnpushedChildren {
  /** Parents first */
  names: readonly string[]
  /** The branches a Push with Children of the branch pushes, the branch included (`ChildrenPushOrder.order`) */
  pushOrder: readonly string[]
  /**
   * The stack branch of the whole stack of a root (`{prefix}/stack/<root>`) Push with Children takes along
   * (`ChildrenPushOrder.stackBranch`); null otherwise
   */
  stackBranch?: string | null
}

export interface PushButtonContext {
  isPushing?: boolean
  /**
   * Why Rust says the push of the branch can't run now (the `push` action of its row, `domain_policy::actions`): it
   * stays local, a Safe Push holds it or landed it, the sync failed or rebuilds it, nothing to push; null while it can
   */
  blocker?: ActionBlocker | null
  /** Rust offers Push with Children of the branch now (the `push-with-children` action of its row) */
  childrenEnabled?: boolean
  /** An ancestor has work for the remote: the button pushes the stack, or offers it */
  unpushedParent?: UnpushedParent | null
  /** A push of the branch alone leaves them as they are */
  unpushedChildren?: UnpushedChildren | null
  /** The baseline that has every commit of the branch ("origin/master"): the update archives it, nothing is left to push */
  landedIn?: string | null
  /** The pull requests into a virtual branch the push retargets first: of the branch, or of each branch of its stack push */
  retargets?: readonly PushRetarget[]
  /** What the actions of the description run */
  runs?: PushButtonRuns
  /** The operations of the session, whose words a push busy behind one says; none: "Another operation" */
  operations?: SessionOperations
}

/**
 * The push of the branch to its own remote branch, named for what it pushes beside Push to master, which pushes the copy
 * into the target (spec/repository/outgoing.spec.md, "Push")
 */
export const PUSH_BRANCH = ACTION_LOOKS.push.label
/** The push of the branch that replaces commits of its remote branch */
export const FORCE_PUSH_BRANCH = "Force Push Branch"

const NO_RUNS: PushButtonRuns = { alone: () => {}, stack: () => {}, children: () => {} }

/** The state of a button that can't push for a reason of the rules (`rule`) or of the sync: disabled, the reason its description */
function disabledState(tooltip: string): PushButtonState {
  return {
    text: PUSH_BRANCH,
    count: 0,
    color: "neutral",
    tooltip,
    dropped: null,
    disabled: true,
    confirm: false,
    pushesStack: false,
    action: null,
    childrenAction: null,
    retarget: null,
  }
}

/**
 * "login isn't on the remote yet. Pushing oauth alone works: its pull request includes login's commits either way.":
 * the choice between a push of the branch and of the stack. The pull request of the branch goes into the baseline with
 * the commits of the branches it is built on, so the push of the branch alone leaves only their own remote branches as
 * they are.
 */
function describeUnpushedParent(branchName: string, parent: UnpushedParent): string {
  const alone = `Pushing ${branchName} alone works: its pull request includes`
  return parent.onRemote
    ? `${parent.name} has commits the remote lacks. ${alone} them either way, and the remote branch of ${parent.name} stays as it is.`
    : `${parent.name} isn't on the remote yet. ${alone} ${parent.name}'s commits either way.`
}

/** One line per branch a stack push sends, root first, in the words of a push (`formatStackPushLine`) */
function describeStackPush(branches: readonly Branch[]): string {
  return `Pushes ${branches.length} branches, parents first:\n${branches.map(branch => formatStackPushLine(branch)).join("\n")}`
}

/** The colour of a stack push: the branch's own, unless it is neutral (none of its commits are the user's) and an ancestor's isn't */
function stackPushColor(branch: Branch, stack: readonly Branch[]): UIColor {
  const own = getPushColor(branch) ?? "primary"
  return own !== "neutral" ? own : (stack.map(it => getPushColor(it)).find(color => color !== null && color !== "neutral") ?? own)
}

/**
 * "oauth and login are stacked on auth and have commits to push.": what a push of the branch alone leaves out; "The
 * whole stack branch user-name/stack/auth goes along." for the stack branch of the whole stack of a root
 */
function describeUnpushedChildren(branchName: string, names: readonly string[], stackBranch: string | null): string {
  const along = stackBranch === null ? null : `The whole stack branch ${stackBranch} goes along.`
  if (names.length === 0) {
    return `The whole stack branch ${stackBranch ?? ""} has commits to push.`
  }
  // the first child named, two of two (`cutList`: never "and 1 more branch")
  const { shown, more } = cutList(names, 1)
  const [first, second] = shown
  const children =
    names.length === 1
      ? `${first} is stacked on ${branchName} and has commits to push.`
      : `${more === 0 ? `${first} and ${second}` : `${first} and ${more} more branches`} are stacked on ${branchName} and have commits to push.`
  return along === null ? children : `${children} ${along}`
}

/**
 * The push button of `branch` (`PushButtonState`) as the view has it: what Rust says against its push (`blocker`), the
 * pushes Rust orders (`unpushedParent`, `unpushedChildren`) and the remote status of the row in words
 */
export function getPushButtonState(
  branch: Branch,
  {
    isPushing = false,
    blocker = null,
    childrenEnabled = false,
    unpushedParent = null,
    unpushedChildren = null,
    landedIn = null,
    retargets = [],
    runs = NO_RUNS,
    operations = NO_OPERATIONS,
  }: PushButtonContext = {},
): PushButtonState {
  const words = { subject: branch.name, statusText: branch.statusText, operations }
  // the children of a branch a merged Safe Push landed push without it; parked, held, unbuilt branches keep theirs
  const children = childrenEnabled && landedIn === null ? childrenActionOf(branch.name, unpushedChildren, runs) : null
  // a running push keeps its progress until it finished
  if (!isPushing && blocker !== null && (blocker.code === "rule" || blocker.code === "syncFailed" || blocker.code === "noCommits" || blocker.code === "busy")) {
    return { ...disabledState(formatActionBlocker("push", blocker, words)), childrenAction: children }
  }
  const remoteStatus = branch.remoteStatus
  const parent = branch.stack?.effectiveBase ?? null
  const texts = remoteStatus != null ? buildRemoteStatusTexts(remoteStatus, parent) : null
  const remoteAction = remoteStatus?.action ?? null
  const pushAction = remoteAction?.kind ?? null

  let text: string = PUSH_BRANCH
  let count = 0
  let tooltip = "Push the branch to its remote branch"
  let dropped: DroppedCommits | null = null
  if (remoteStatus != null && texts != null) {
    // a label counts the commits the push sends, whoever authored them; the description says how many are the user's
    const { commitsAhead } = remoteStatus
    const withCount = (label: string): string => (commitsAhead > 0 ? `${label} (${commitsAhead})` : label)
    tooltip = texts.tooltip
    if (pushAction !== "upToDate") {
      count = commitsAhead
    }
    if (pushAction === "create") {
      // one name for every push of the branch: that it creates the remote branch, the description says
      text = withCount(PUSH_BRANCH)
    } else if (pushAction === "force") {
      text = withCount(FORCE_PUSH_BRANCH)
      tooltip = buildRemoteStatusTexts(remoteStatus, parent, { forcePush: true }).tooltip
      const { commitsBehind, droppedCommits } = remoteStatus
      if (commitsBehind > 0 && droppedCommits.length > 0) {
        dropped = { commits: droppedCommits, more: Math.max(0, commitsBehind - droppedCommits.length) }
      }
    } else if (pushAction === "push") {
      text = withCount(PUSH_BRANCH)
    }
  }

  let action: PushButtonAction | null = null
  let color = getPushColor(branch) ?? "primary"
  let pushesStack = false
  let retarget: PushButtonState["retarget"] = null
  if (landedIn !== null) {
    // the update archives the branch: a push is still possible, but there is nothing of the user's left to send
    tooltip = landedBranchDescription(landedIn)
    color = "neutral"
  } else if (pushAction !== "upToDate") {
    const stack = unpushedParent?.buttonStack ?? []
    if (unpushedParent !== null && stack.length > 1) {
      // the remote branches of its parents, and their pull requests, stay what the branch is built on: the button
      // pushes them too
      pushesStack = true
      count = stack.reduce((sum, it) => sum + (it.remoteStatus?.commitsAhead ?? 0), 0)
      text = formatPushStackLabel(stack.length)
      tooltip = describeStackPush(stack)
      color = stackPushColor(branch, stack)
      action = {
        id: "push-branch-only",
        description: describeUnpushedParent(branch.name, unpushedParent),
        label: `Push ${branch.name} Only`,
        run: runs.alone,
      }
    } else if (unpushedParent !== null) {
      action = {
        id: "push-stack",
        description: describeUnpushedParent(branch.name, unpushedParent),
        label: formatPushStackLabel(unpushedParent.stackSize),
        run: runs.stack,
      }
    }
    // the pull requests of the branches this push sends
    const pushed = new Set(pushesStack ? stack.map(it => it.name) : [branch.name])
    retarget = describePushRetargets(
      retargets.filter(it => pushed.has(it.branchName)),
      pushesStack,
    )
  }

  return {
    text,
    count,
    color,
    tooltip,
    dropped,
    // a sync running is no reason: the push waits for it (`WaitingForSync`); a branch the sync rebuilds anew is
    disabled: isPushing || blocker !== null,
    confirm: remoteAction?.kind === "force" && remoteAction.drops,
    pushesStack,
    action,
    childrenAction: children,
    retarget,
  }
}

/** Push with Children of the branch below the push of the branch alone, while its children have work (`UnpushedChildren`) */
function childrenActionOf(branchName: string, unpushedChildren: UnpushedChildren | null, runs: PushButtonRuns): PushButtonAction | null {
  if (unpushedChildren === null) {
    return null
  }
  const stackBranch = unpushedChildren.stackBranch ?? null
  return {
    id: "push-with-children",
    description: describeUnpushedChildren(branchName, unpushedChildren.names, stackBranch),
    label: formatPushWithChildrenLabel(unpushedChildren.pushOrder, stackBranch !== null),
    run: runs.children,
  }
}

/** Why Rust says the push of `name` can't run now (the `push` action of its row); null while it can or isn't offered */
export function pushBlockerOf(actions: readonly ActionState[] | undefined): ActionBlocker | null {
  const push = findAction(actions, "push")
  return push?.availability.kind === "disabled" ? push.availability.reason : null
}

/** Rust offers Push with Children of the branch now */
export function childrenPushEnabled(actions: readonly ActionState[] | undefined): boolean {
  return findAction(actions, "push-with-children")?.availability.kind === "enabled"
}

/**
 * The first push of a branch is a step of its landing: Safe Push and Create Pull Request push the branch themselves
 * (`pushForAction`), so the row, the preview and the review window offer no push of a branch the remote lacks, only
 * its menu does, with Push with Children of a root. A push stays while it runs, while a Safe Push holds the branch
 * (the disabled push says why and offers the robot), for a branch with a live pull request, which the push updates
 * (`useLandingPushes`; a root whose Push with Children updates the open pull request of its whole stack, the callers),
 * and in a repository with neither Patronus nor GitHub, where it is the one way to share the branch.
 */
export function isFirstPushInLanding(branch: Branch, { landingPushes, held = false }: { landingPushes: boolean; held?: boolean }): boolean {
  return landingPushes && !held && branch.remoteStatus?.action.kind === "create"
}

/**
 * Why the push of the preview footer and of the review window's title bar is disabled, so the button keeps its place
 * where it acts on its own and where it doesn't: a first push that the landing of the branch makes
 * (`isFirstPushInLanding`: Safe Push and Create Pull Request… push it first, "⋯" pushes it alone), or, while Patronus
 * or GitHub haven't answered whether it does, what it waits for (`pending`). Null: the push acts
 */
export function firstPushWait(
  branch: Branch,
  { landingPushes, patronus, held, pending }: { landingPushes: boolean; patronus: boolean; held: boolean; pending: PendingFact | null },
): string | null {
  if (held || branch.remoteStatus?.action.kind !== "create") {
    return null
  }
  if (landingPushes) {
    return `${patronus ? "Safe Push and Create Pull Request… push" : "Create Pull Request… pushes"} ${branch.name} first: "⋯" pushes it alone`
  }
  return pending === null ? null : formatPendingFact(pending)
}

/** What a first push waits for to know whether the landing of the branch makes it: Patronus, then the pull requests */
export function firstPushPending(patronusPending: boolean, pullRequestsLoading: boolean): PendingFact | null {
  return patronusPending ? "patronus" : pullRequestsLoading ? "pullRequests" : null
}

/** Whether Safe Push or Create Pull Request is there to push the branch, and no live pull request shows it (`useLandingPushes`) */
export function landingPushes(patronus: boolean, gitHub: boolean, pullRequestState: string | undefined): boolean {
  return (patronus || gitHub) && (pullRequestState === undefined || pullRequestState === "closed")
}

/** `useLandingPushes` outside a component, for the items built on demand */
export function isLandingPushAvailable(branchName: string): boolean {
  return landingPushes(isPatronusAvailable(), getBranchesGitHubRepository() !== null, getPullRequest(branchName)?.state)
}

/**
 * The push button of the branch in `state`, the view as the caller read it (`branchSyncStore.getState()` of a menu built
 * on open), for the surfaces that list it as a menu item (the branch menu, the branch preview, the review): its label
 * ("Push Branch (2)", "Force Push Branch (1)", "Push Stack (3 branches)") and what a click pushes, as the button renders
 * it, its other pushes running `runs`
 */
export function getPushButtonNow(branch: Branch, isPushing: boolean, runs: PushButtonRuns, state: BranchSyncState): PushButtonState {
  const unpushedParent = unpushedParentIn(state, branch.name)
  const retargets = getPushRetargets(retargetCandidates(branch.name, unpushedParent))
  const actions = getRowPart(branch.name, "actions")
  return getPushButtonState(branch, { isPushing, blocker: pushBlockerOf(actions), unpushedParent, retargets, runs, operations: state.operations })
}

/** The branches whose pull requests the push button may retarget: those of its stack push, root first, else the branch */
export function retargetCandidates(branchName: string, unpushedParent: UnpushedParent | null): string[] {
  const stack = (unpushedParent?.buttonStack ?? []).map(it => it.name)
  return stack.includes(branchName) ? stack : [...stack, branchName]
}

/**
 * The nearest branch `name` is built on that its Push Stack pushes, and its stack pushes, as Rust orders them
 * (`BranchPush`): null while none of its ancestors has work for the remote (a held one ends them)
 */
export function unpushedParentIn(state: BranchSyncState, name: string): UnpushedParent | null {
  const orders = pushOrdersIn(state, name)
  const parent = parentOf(state, orders)
  if (parent === undefined) {
    return null
  }
  return {
    name: parent.name,
    onRemote: parent.remoteStatus?.exists === true,
    stackSize: orders.stack.length,
    buttonStack: orders.button.flatMap(it => state.branches[it] ?? []),
  }
}

/** The nearest ancestor of a Push Stack: the one before the branch */
export function parentOf(state: BranchSyncState, orders: PushOrders): Branch | undefined {
  const name = orders.stack.at(-2)
  return name === undefined ? undefined : state.branches[name]
}

/** A pull request the push of a branch retargets first (`getPushRetargets`), as the push description says it */
export interface PushRetarget {
  readonly branchName: string
  readonly number: number
  /** The baseline as the remote names it */
  readonly base: string
  /** Someone approved it: GitHub may dismiss the approvals of a pull request whose base changes */
  readonly approved: boolean
}

/** The open pull requests into a virtual branch that a push of `branchNames` retargets first (the `retarget` effects of its plan) */
export function getPushRetargets(branchNames: readonly string[]): PushRetarget[] {
  return branchNames.flatMap(branchName => {
    const retarget = getPullRequestRetarget(branchName)
    if (retarget === null) {
      return []
    }
    const { number, reviewers } = retarget.pullRequest
    return [{ branchName, number, base: retarget.base, approved: reviewers.some(reviewer => reviewer.state === "approved") }]
  })
}

/**
 * What the description of a push says before it retargets pull requests (`getPushRetargets`): "Retargets #8 to master:
 * GitHub may dismiss its approvals." for one with approvals, a line per pull request, each named by its branch when the
 * push sends several (`ofBranches`); null without a retarget
 */
export function describePushRetargets(retargets: readonly PushRetarget[], ofBranches: boolean): { text: string; warning: boolean } | null {
  if (retargets.length === 0) {
    return null
  }
  const lines = retargets.map(
    retarget =>
      `Retargets #${retarget.number}${ofBranches ? ` of ${retarget.branchName}` : ""} to ${retarget.base}${retarget.approved ? ": GitHub may dismiss its approvals" : ""}.`,
  )
  return { text: lines.join("\n"), warning: retargets.some(retarget => retarget.approved) }
}
