import { useMemo } from "react"
import { useShallow } from "zustand/react/shallow"
import { useLandedIn } from "@/hooks/useLandedIn"
import type { DroppedCommit } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { pushBranch, pushStack, pushWithChildren } from "@/lib/git/push"
import { landedBranchDescription } from "@/lib/landed"
import { dropsRemoteCommits, getPushAction, getPushColor } from "@/lib/push-summary"
import { buildRemoteStatusTexts, formatForcePushPart } from "@/lib/remote-status"
import { buttonStackPushOrder, formatBlockedReason, formatPushStackLabel, formatPushWithChildrenLabel, needsPush, stackAncestors, stackChildrenPushOrder, stackPushOrder } from "@/lib/stack"
import type { UIColor } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"
import { stackDescendantsIn, useBranchSync } from "@/stores/branch-sync"

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
  /** Push with Children, offered after `action`, while branches stacked on this one have work for the remote */
  childrenAction: PushButtonAction | null
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

/** The branches stacked on a branch that have work for the remote */
export interface UnpushedChildren {
  /** Parents first */
  names: readonly string[]
  /** The branches a Push with Children of the branch pushes, the branch included */
  pushCount: number
}

export interface PushButtonContext {
  isSyncing?: boolean
  isPushing?: boolean
  /** The pull request of the branch shows the parent's commits until the parent is pushed */
  unpushedParent?: UnpushedParent | null
  /** A push of the branch alone leaves them as they are */
  unpushedChildren?: UnpushedChildren | null
  /** The baseline that has every commit of the branch ("origin/master"): the update archives it, nothing is left to push */
  landedIn?: string | null
}

/** "login isn't on the remote yet. Pushing oauth alone works; …": the choice between a push of the branch and of the stack */
function describeUnpushedParent(branchName: string, parent: UnpushedParent): string {
  const alone = `Pushing ${branchName} alone works; a pull request includes`
  return parent.onRemote
    ? `${parent.name} has commits the remote lacks. ${alone} them until ${parent.name} is pushed.`
    : `${parent.name} isn't on the remote yet. ${alone} ${parent.name}'s commits until ${parent.name} is pushed.`
}

/**
 * One line per branch a stack push sends, root first, in the words of a push: "login · 2 commits · creates the remote
 * branch", "auth · 1 commit · force push" for a rebased one, with what a force push replaces or drops
 * (`formatForcePushPart`): "auth · 1 commit · force push, replaces 1 commit amended here"
 */
function describeStackPush(branches: readonly Branch[]): string {
  const lines = branches.map((branch) => {
    const action = getPushAction(branch)
    const parts = [branch.name]
    const ahead = branch.remoteStatus?.commitsAhead ?? 0
    if (ahead > 0) {
      parts.push(formatCommitCount(ahead))
    }
    if (action === "create") {
      parts.push("creates the remote branch")
    }
    else if (action === "force" && branch.remoteStatus != null) {
      parts.push(formatForcePushPart(branch.remoteStatus, branch.stack?.effectiveBase ?? null))
    }
    return parts.join(" · ")
  })
  return `Pushes ${branches.length} branches, parents first:\n${lines.join("\n")}`
}

/** The colour of a stack push: the branch's own, unless it is neutral (none of its commits are the user's) and an ancestor's isn't */
function stackPushColor(branch: Branch, stack: readonly Branch[]): UIColor {
  const own = getPushColor(branch) ?? "primary"
  return own !== "neutral" ? own : stack.map(it => getPushColor(it)).find(color => color !== null && color !== "neutral") ?? own
}

/** "oauth and login are stacked on auth and have commits to push.": what a push of the branch alone leaves out */
function describeUnpushedChildren(branchName: string, names: readonly string[]): string {
  const [first, second] = names
  if (names.length === 1) {
    return `${first} is stacked on ${branchName} and has commits to push.`
  }
  const subject = names.length === 2 ? `${first} and ${second}` : `${first} and ${names.length - 1} more branches`
  return `${subject} are stacked on ${branchName} and have commits to push.`
}

export function getPushButtonState(
  branch: Branch,
  { isSyncing = false, isPushing = false, unpushedParent = null, unpushedChildren = null, landedIn = null }: PushButtonContext = {},
): PushButtonState {
  // a blocked stacked branch keeps its last build, which lacks its parent's state: pushing it would mislead
  if (branch.status === "Blocked") {
    return { text: "Push", count: 0, color: "neutral", tooltip: formatBlockedReason(branch), dropped: null, disabled: true, confirm: false, pushesStack: false, action: null, childrenAction: null }
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
    const withCount = (label: string): string => commitsAhead > 0 ? `${label} (${commitsAhead})` : label
    tooltip = texts.tooltip
    if (pushAction !== "upToDate") {
      count = commitsAhead
    }
    if (pushAction === "create") {
      text = withCount("Create Remote")
    }
    else if (pushAction === "force") {
      text = withCount("Force Push")
      tooltip = buildRemoteStatusTexts(remoteStatus, parent, { forcePush: true }).tooltip
      const { commitsBehind, droppedCommits } = remoteStatus
      if (commitsBehind > 0 && droppedCommits.length > 0) {
        dropped = { commits: droppedCommits, more: Math.max(0, commitsBehind - droppedCommits.length) }
      }
    }
    else if (pushAction === "push") {
      text = withCount("Push")
    }
  }

  let action: PushButtonAction | null = null
  let childrenAction: PushButtonAction | null = null
  let color = getPushColor(branch) ?? "primary"
  let pushesStack = false
  if (landedIn !== null) {
    // the update archives the branch: a push is still possible, but there is nothing of the user's left to send
    tooltip = landedBranchDescription(landedIn)
    color = "neutral"
  }
  else if (pushAction !== "upToDate") {
    const stack = unpushedParent?.buttonStack ?? []
    if (unpushedParent !== null && stack.length > 1) {
      // the pull request of the branch is right only once its parents are on the remote: the button pushes them too
      pushesStack = true
      count = stack.reduce((sum, it) => sum + (it.remoteStatus?.commitsAhead ?? 0), 0)
      text = `Push Stack (${count})`
      tooltip = describeStackPush(stack)
      color = stackPushColor(branch, stack)
      action = {
        description: describeUnpushedParent(branch.name, unpushedParent),
        label: `Push ${branch.name} Only`,
        run: () => void pushBranch(branch.name),
      }
    }
    else if (unpushedParent !== null) {
      action = {
        description: describeUnpushedParent(branch.name, unpushedParent),
        label: formatPushStackLabel(unpushedParent.stackSize),
        run: () => void pushStack(branch.name),
      }
    }
    if (unpushedChildren !== null) {
      childrenAction = {
        description: describeUnpushedChildren(branch.name, unpushedChildren.names),
        label: formatPushWithChildrenLabel(unpushedChildren.pushCount),
        run: () => void pushWithChildren(branch.name),
      }
    }
  }

  return {
    text,
    count,
    color,
    tooltip,
    dropped,
    disabled: isPushing || isSyncing || branch.status === "Syncing" || pushAction === "upToDate",
    confirm: remoteStatus != null && dropsRemoteCommits(remoteStatus),
    pushesStack,
    action,
    childrenAction,
  }
}

/**
 * Whether the push button of the branch is enabled, for P in the branch preview and on the focused row of the branch
 * table: synced without an error, with commits, something to push, no sync or push running, no inline action on the row
 */
export function canPushNow(branch: Branch, { isSyncing, isPushing, processing }: { isSyncing: boolean, isPushing: boolean, processing: boolean }): boolean {
  return !branch.hasError && branch.commitCount > 0 && !processing && !getPushButtonState(branch, { isSyncing, isPushing }).disabled
}

/** The push button of a branch row, of the outgoing overview and of the branch preview */
export function usePushButton(branch: Branch, isSyncing: boolean, isPushing: boolean): PushButtonState {
  const unpushedParent = useUnpushedParent(branch)
  const unpushedChildren = useUnpushedChildren(branch.name)
  const landedIn = useLandedIn(branch.name)
  return useMemo(
    () => getPushButtonState(branch, { isSyncing, isPushing, unpushedParent, unpushedChildren, landedIn }),
    [branch, isSyncing, isPushing, unpushedParent, unpushedChildren, landedIn],
  )
}

/** The nearest branch a stacked branch is built on that has work for the remote, and the stack pushes of the branch */
export function useUnpushedParent(branch: Branch): UnpushedParent | null {
  const name = branch.name
  // each selector returns branch records (compared one by one) or a number, so the store notifies only when they change
  const parent = useBranchSync(state => stackAncestors(name, it => state.branches[it]).findLast(needsPush))
  const stackSize = useBranchSync(state => parent === undefined ? 1 : stackPushOrder(name, it => state.branches[it]).length)
  const buttonStack = useBranchSync(useShallow(state => buttonStackPushOrder(name, it => state.branches[it]).map(it => state.branches[it]!)))
  return useMemo(() => {
    if (parent === undefined) {
      return null
    }
    return { name: parent.name, onRemote: parent.remoteStatus?.exists === true, stackSize, buttonStack }
  }, [parent, stackSize, buttonStack])
}

/** The branches stacked on a branch, while a push of them has work for the remote */
function useUnpushedChildren(branchName: string): UnpushedChildren | null {
  // a string, so the store notifies only when the order changes
  const order = useBranchSync(state => stackChildrenPushOrder(branchName, stackDescendantsIn(state, branchName), name => state.branches[name]).join("\n"))
  return useMemo(() => {
    const pushOrder = order === "" ? [] : order.split("\n")
    const names = pushOrder.filter(name => name !== branchName)
    return names.length === 0 ? null : { names, pushCount: pushOrder.length }
  }, [order, branchName])
}
