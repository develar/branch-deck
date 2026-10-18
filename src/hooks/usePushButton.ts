import { useMemo } from "react"
import { useStore } from "zustand"
import { useShallow } from "zustand/react/shallow"
import { useLandedIn } from "@/hooks/useLandedIn"
import type { DroppedCommit } from "@/lib/bindings"
import type { PushRetarget } from "@/lib/git/new-pull-request"
import { describePushRetargets, getPushRetargets } from "@/lib/git/new-pull-request"
import { pushBranch } from "@/lib/git/push"
import { pushStack, pushWithChildren } from "@/lib/git/stack-push"
import { landedBranchDescription } from "@/lib/landed"
import type { PushHold } from "@/lib/patronus"
import { formatPushHoldReason } from "@/lib/patronus"
import { dropsRemoteCommits, getPushAction, getPushColor } from "@/lib/push-summary"
import { formatStackPushLine } from "@/lib/push-up-to"
import { buildRemoteStatusTexts } from "@/lib/remote-status"
import {
  buttonStackPushOrder,
  formatBlockedReason,
  formatPushStackLabel,
  formatPushWithChildrenLabel,
  needsPush,
  offersPushWithChildren,
  stackAncestors,
  stackChildrenPushOrder,
  stackPushOrder,
} from "@/lib/stack"
import type { UIColor } from "@/lib/ui-types"
import type { StackBranchAlong } from "@/lib/whole-stack"
import { getStackBranchAlong } from "@/lib/whole-stack"
import type { Branch } from "@/stores/branch-sync"
import { getBranch, stackDescendantsIn, useBranchSync, useStackBranch } from "@/stores/branch-sync"
import { getPushHold, usePushHold, usePushHolds } from "@/stores/patronus"
import { pullRequestsStore, useStackPullRequest } from "@/stores/pull-requests"

export interface PushButtonState {
  text: string
  /** The commits the push sends, the count of the label ("Force Push (1)", the sum of a Push Stack); 0 without one */
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
   * The button pushes the stack (`buttonStackPushOrder`): the ancestors that have work for the remote, root first, then
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
  /** The choice between the push of the button and this one, above the action button */
  description: string
  label: string
  run: () => void
}

export interface DroppedCommits {
  /** The newest ones, newest first */
  commits: readonly DroppedCommit[]
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
  /** The branches the push button pushes, root first (`buttonStackPushOrder`); empty when it pushes the branch alone */
  buttonStack: readonly Branch[]
}

/** The branches stacked on a branch that have work for the remote, and the stack branch of its whole stack */
export interface UnpushedChildren {
  /** Parents first */
  names: readonly string[]
  /** The branches a Push with Children of the branch pushes, the branch included (`stackChildrenPushOrder`) */
  pushOrder: readonly string[]
  /**
   * The stack branch of the whole stack of a root (`{prefix}/stack/<root>`) Push with Children takes along
   * (`getStackBranchAlong`); null otherwise
   */
  stackBranch?: string | null
}

export interface PushButtonContext {
  isPushing?: boolean
  /** An ancestor has work for the remote: the button pushes the stack, or offers it */
  unpushedParent?: UnpushedParent | null
  /** A push of the branch alone leaves them as they are */
  unpushedChildren?: UnpushedChildren | null
  /** The baseline that has every commit of the branch ("origin/master"): the update archives it, nothing is left to push */
  landedIn?: string | null
  /** A Safe Push runs on the branch or lands it (`PushHold`): a push won't land, the button is disabled with the reason */
  hold?: PushHold | null
  /** The pull requests into a virtual branch the push retargets first: of the branch, or of each branch of its stack push */
  retargets?: readonly PushRetarget[]
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
  const [first, second] = names
  if (names.length === 0) {
    return `The whole stack branch ${stackBranch ?? ""} has commits to push.`
  }
  const children =
    names.length === 1
      ? `${first} is stacked on ${branchName} and has commits to push.`
      : `${names.length === 2 ? `${first} and ${second}` : `${first} and ${names.length - 1} more branches`} are stacked on ${branchName} and have commits to push.`
  return along === null ? children : `${children} ${along}`
}

export function getPushButtonState(
  branch: Branch,
  { isPushing = false, unpushedParent = null, unpushedChildren = null, landedIn = null, hold = null, retargets = [] }: PushButtonContext = {},
): PushButtonState {
  // a blocked stacked branch keeps its last build, which lacks its parent's state: pushing it would mislead
  if (branch.status === "Blocked") {
    return {
      text: "Push",
      count: 0,
      color: "neutral",
      tooltip: formatBlockedReason(branch),
      dropped: null,
      disabled: true,
      confirm: false,
      pushesStack: false,
      action: null,
      childrenAction: null,
      retarget: null,
    }
  }
  // a push won't land while the Safe Push runs; a running push keeps its progress until it finished
  if (hold !== null && !isPushing) {
    return {
      text: "Push",
      count: 0,
      color: "neutral",
      tooltip: formatPushHoldReason(branch.name, hold),
      dropped: null,
      disabled: true,
      confirm: false,
      pushesStack: false,
      action: null,
      childrenAction: null,
      retarget: null,
    }
  }
  const remoteStatus = branch.remoteStatus
  const parent = branch.stack?.effectiveBase ?? null
  const texts = remoteStatus != null ? buildRemoteStatusTexts(remoteStatus, parent) : null
  const pushAction = getPushAction(branch)

  let text = "Push"
  let count = 0
  let tooltip = "Push branch to remote"
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
      text = withCount("Create Remote")
    } else if (pushAction === "force") {
      text = withCount("Force Push")
      tooltip = buildRemoteStatusTexts(remoteStatus, parent, { forcePush: true }).tooltip
      const { commitsBehind, droppedCommits } = remoteStatus
      if (commitsBehind > 0 && droppedCommits.length > 0) {
        dropped = { commits: droppedCommits, more: Math.max(0, commitsBehind - droppedCommits.length) }
      }
    } else if (pushAction === "push") {
      text = withCount("Push")
    }
  }

  let action: PushButtonAction | null = null
  let childrenAction: PushButtonAction | null = null
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
        description: describeUnpushedParent(branch.name, unpushedParent),
        label: `Push ${branch.name} Only`,
        run: () => void pushBranch(branch.name),
      }
    } else if (unpushedParent !== null) {
      action = {
        description: describeUnpushedParent(branch.name, unpushedParent),
        label: formatPushStackLabel(unpushedParent.stackSize),
        run: () => void pushStack(branch.name),
      }
    }
    // the pull requests of the branches this push sends
    const pushed = new Set(pushesStack ? stack.map(it => it.name) : [branch.name])
    retarget = describePushRetargets(
      retargets.filter(it => pushed.has(it.branchName)),
      pushesStack,
    )
  }
  // the children have work while the branch's own push has any or none: the root of a stack pushed already offers them
  if (landedIn === null && unpushedChildren !== null) {
    const stackBranch = unpushedChildren.stackBranch ?? null
    childrenAction = {
      description: describeUnpushedChildren(branch.name, unpushedChildren.names, stackBranch),
      label: formatPushWithChildrenLabel(unpushedChildren.pushOrder, stackBranch !== null),
      run: () => void pushWithChildren(branch.name),
    }
  }

  return {
    text,
    count,
    color,
    tooltip,
    dropped,
    // a sync running is no reason: the push waits for it (`pushOne`); a branch the sync rebuilds anew is
    disabled: isPushing || branch.status === "Syncing" || pushAction === "upToDate",
    confirm: remoteStatus != null && dropsRemoteCommits(remoteStatus),
    pushesStack,
    action,
    childrenAction,
    retarget,
  }
}

/**
 * Whether the push button of the branch is enabled, for P in the branch preview and on the focused row of the branch
 * table: synced without an error, with commits, something to push, no push running, the branch not rebuilt anew by a sync
 * running (a push during a sync waits for it), no inline action on the row
 */
export function canPushNow(branch: Branch, { isPushing, processing }: { isPushing: boolean; processing: boolean }): boolean {
  return !branch.hasError && branch.commitCount > 0 && !processing && !getPushButtonState(branch, { isPushing, hold: getPushHold(branch.name) }).disabled
}

/** The push button of a branch row and of the branch preview */
export function usePushButton(branch: Branch, isPushing: boolean): PushButtonState {
  const unpushedParent = useUnpushedParent(branch)
  const unpushedChildren = useUnpushedChildren(branch.name)
  const landedIn = useLandedIn(branch.name)
  const hold = usePushHold(branch.name)
  const retargets = usePushRetargets(retargetCandidates(branch.name, unpushedParent))
  return useMemo(
    () => getPushButtonState(branch, { isPushing, unpushedParent, unpushedChildren, landedIn, hold, retargets }),
    [branch, isPushing, unpushedParent, unpushedChildren, landedIn, hold, retargets],
  )
}

/** The branches whose pull requests the push button may retarget: those of its stack push, root first, else the branch */
function retargetCandidates(branchName: string, unpushedParent: UnpushedParent | null): string[] {
  const stack = (unpushedParent?.buttonStack ?? []).map(it => it.name)
  return stack.includes(branchName) ? stack : [...stack, branchName]
}

/** The pull requests into a virtual branch a push of `names` retargets first (`getPushRetargets`), from the stores */
function usePushRetargets(names: readonly string[]): readonly PushRetarget[] {
  // the base they go into is the baseline: a new one renders again, and the selector below reads it
  useBranchSync(state => state.baselineBranch)
  // a string, so the store notifies only when a retarget changes
  const retargets = useStore(pullRequestsStore, () => JSON.stringify(getPushRetargets(names)))
  return useMemo(() => JSON.parse(retargets) as PushRetarget[], [retargets])
}

/**
 * The push button of the branch as the stores have it now, for the surfaces that list it as a menu item (the branch
 * menu, the branch preview, the review): its label ("Push (2)", "Force Push (1)", "Push Stack (3 branches)") and what a
 * click pushes, as `usePushButton` renders it
 */
export function getPushButtonNow(branch: Branch, isPushing: boolean): PushButtonState {
  const isHeld = (name: string): boolean => getPushHold(name) !== null
  const unpushedParent = getUnpushedParent(branch.name, getBranch, isHeld)
  const retargets = getPushRetargets(retargetCandidates(branch.name, unpushedParent))
  return getPushButtonState(branch, { isPushing, unpushedParent, hold: getPushHold(branch.name), retargets })
}

/** `useUnpushedParent` outside a component: the nearest ancestor with work for the remote, and the stack pushes */
export function getUnpushedParent(name: string, branchOf: (name: string) => Branch | undefined, isHeld: (name: string) => boolean): UnpushedParent | null {
  const parent = stackAncestors(name, branchOf).findLast(it => needsPush(it) && !isHeld(it.name))
  if (parent === undefined) {
    return null
  }
  return {
    name: parent.name,
    onRemote: parent.remoteStatus?.exists === true,
    stackSize: stackPushOrder(name, branchOf, { isHeld }).length,
    buttonStack: buttonStackPushOrder(name, branchOf, isHeld).map(it => branchOf(it)!),
  }
}

/** The nearest branch a stacked branch is built on that has work for the remote, and the stack pushes of the branch */
export function useUnpushedParent(branch: Branch): UnpushedParent | null {
  const name = branch.name
  const holds = usePushHolds()
  const isHeld = (it: string): boolean => holds.has(it)
  // each selector returns branch records (compared one by one) or a number, so the store notifies only when they change;
  // a held ancestor has nothing to push now: its Safe Push lands it
  const parent = useBranchSync(state => stackAncestors(name, it => state.branches[it]).findLast(it => needsPush(it) && !isHeld(it.name)))
  const stackSize = useBranchSync(state => (parent === undefined ? 1 : stackPushOrder(name, it => state.branches[it], { isHeld }).length))
  const buttonStack = useBranchSync(useShallow(state => buttonStackPushOrder(name, it => state.branches[it], isHeld).map(it => state.branches[it]!)))
  return useMemo(() => {
    if (parent === undefined) {
      return null
    }
    return { name: parent.name, onRemote: parent.remoteStatus?.exists === true, stackSize, buttonStack }
  }, [parent, stackSize, buttonStack])
}

/**
 * The branches stacked on a branch, while a push of them has work for the remote, and of a root the stack branch of its
 * whole stack while Push with Children takes it along (`useStackBranchAlong`)
 */
export function useUnpushedChildren(branchName: string): UnpushedChildren | null {
  const holds = usePushHolds()
  // a string, so the store notifies only when the order changes
  const order = useBranchSync(state =>
    stackChildrenPushOrder(
      branchName,
      stackDescendantsIn(state, branchName),
      name => state.branches[name],
      name => holds.has(name),
    ).join("\n"),
  )
  const along = useStackBranchAlong(branchName)
  const stackBranch = useBranchSync(state => (along?.kind === "along" ? (state.stackBranches[branchName]?.branchName ?? null) : null))
  return useMemo(() => {
    const pushOrder = order === "" ? [] : order.split("\n")
    return offersPushWithChildren(branchName, pushOrder, along) ? { names: pushOrder.filter(name => name !== branchName), pushOrder, stackBranch } : null
  }, [order, branchName, along, stackBranch])
}

/**
 * What Push with Children of `root` does with the stack branch of its whole stack (`getStackBranchAlong`), re-rendering
 * when it changes; null for a branch that heads no stack that forks
 */
export function useStackBranchAlong(root: string): StackBranchAlong | null {
  const stack = useStackBranch(root)
  const pullRequest = useStackPullRequest(root)
  // a string, so a new store state with the same answer renders nothing
  const along = JSON.stringify(getStackBranchAlong(root, stack, pullRequest))
  return useMemo(() => JSON.parse(along) as StackBranchAlong | null, [along])
}
