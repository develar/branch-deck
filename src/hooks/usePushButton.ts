import { useMemo } from "react"
import { useLandedIn } from "@/hooks/useLandedIn"
import type { DroppedCommit } from "@/lib/bindings"
import { pushStack } from "@/lib/git/push"
import { landedBranchDescription } from "@/lib/landed"
import { dropsRemoteCommits, getPushAction, getPushColor } from "@/lib/push-summary"
import { buildRemoteStatusTexts } from "@/lib/remote-status"
import { blockingBranchOf, formatPushStackLabel, needsPush, stackPushOrder } from "@/lib/stack"
import type { UIColor } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"
import { useBranchSync } from "@/stores/branch-sync"

export interface PushButtonState {
  text: string
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
  /** Another push the description offers after its text: Push Stack while the parent has work for the remote */
  action: PushButtonAction | null
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

/** The branch a stacked branch is built on, while a push of it has work for the remote */
export interface UnpushedParent {
  name: string
  /** The parent has a remote branch, only without some of its commits */
  onRemote: boolean
  /** The branches a Push Stack of the branch pushes, the branch included */
  stackSize: number
}

export interface PushButtonContext {
  isSyncing?: boolean
  isPushing?: boolean
  /** The pull request of the branch shows the parent's commits until the parent is pushed */
  unpushedParent?: UnpushedParent | null
  /** The baseline that has every commit of the branch ("origin/master"): the update archives it, nothing is left to push */
  landedIn?: string | null
}

/** "login isn't on the remote yet. Pushing oauth alone works; …": the choice between this push and Push Stack */
function describeUnpushedParent(branchName: string, parent: UnpushedParent): string {
  const alone = `Pushing ${branchName} alone works; a pull request includes`
  return parent.onRemote
    ? `${parent.name} has commits the remote lacks. ${alone} them until ${parent.name} is pushed.`
    : `${parent.name} isn't on the remote yet. ${alone} ${parent.name}'s commits until ${parent.name} is pushed.`
}

export function getPushButtonState(branch: Branch, { isSyncing = false, isPushing = false, unpushedParent = null, landedIn = null }: PushButtonContext = {}): PushButtonState {
  // a blocked stacked branch keeps its last build, which lacks its parent's state: pushing it would mislead
  if (branch.status === "Blocked") {
    const blocker = blockingBranchOf(branch)
    return {
      text: "Push",
      color: "neutral",
      tooltip: blocker === null ? "Blocked: fix the parent branch first" : `Blocked by ${blocker}: fix it first, then sync`,
      dropped: null,
      disabled: true,
      confirm: false,
      action: null,
    }
  }
  const remoteStatus = branch.remoteStatus
  const texts = remoteStatus != null ? buildRemoteStatusTexts(remoteStatus) : null
  const pushAction = getPushAction(branch)

  let text = "Push"
  let tooltip = "Push branch to remote"
  let dropped: DroppedCommits | null = null
  if (remoteStatus != null && texts != null) {
    // a label counts the commits the push sends, whoever authored them; the description says how many are the user's
    const { commitsAhead } = remoteStatus
    const withCount = (label: string): string => commitsAhead > 0 ? `${label} (${commitsAhead})` : label
    tooltip = texts.tooltip
    if (pushAction === "create") {
      text = withCount("Create Remote")
    }
    else if (pushAction === "force") {
      text = withCount("Force Push")
      tooltip = `Force push replaces the remote branch.\n${texts.tooltip}`
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
  if (landedIn !== null) {
    // the update archives the branch: a push is still possible, but there is nothing of the user's left to send
    tooltip = landedBranchDescription(landedIn)
  }
  else if (unpushedParent !== null && pushAction !== "upToDate") {
    action = {
      description: describeUnpushedParent(branch.name, unpushedParent),
      label: formatPushStackLabel(unpushedParent.stackSize),
      run: () => void pushStack(branch.name),
    }
  }

  return {
    text,
    color: landedIn === null ? getPushColor(branch) ?? "primary" : "neutral",
    tooltip,
    dropped,
    disabled: isPushing || isSyncing || branch.status === "Syncing" || pushAction === "upToDate",
    confirm: remoteStatus != null && dropsRemoteCommits(remoteStatus),
    action,
  }
}

/** The push button of a branch row, of the outgoing overview and of the branch preview */
export function usePushButton(branch: Branch, isSyncing: boolean, isPushing: boolean): PushButtonState {
  const unpushedParent = useUnpushedParent(branch)
  const landedIn = useLandedIn(branch.name)
  return useMemo(() => getPushButtonState(branch, { isSyncing, isPushing, unpushedParent, landedIn }), [branch, isSyncing, isPushing, unpushedParent, landedIn])
}

/** The branch a stacked branch is built on, while a push of it has work for the remote */
export function useUnpushedParent(branch: Branch): UnpushedParent | null {
  const parentName = branch.stack?.effectiveBase ?? null
  // each selector returns a branch record or a number, so the store notifies only when they change
  const parent = useBranchSync(state => parentName === null ? undefined : state.branches[parentName])
  const stackSize = useBranchSync(state => parentName === null ? 1 : stackPushOrder(branch.name, name => state.branches[name]).length)
  return useMemo(() => {
    if (parent === undefined || !needsPush(parent)) {
      return null
    }
    return { name: parent.name, onRemote: parent.remoteStatus?.exists === true, stackSize }
  }, [parent, stackSize])
}
