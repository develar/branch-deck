import { useMemo } from "react"
import type { DroppedCommit } from "@/lib/bindings"
import { getPushAction, getPushColor } from "@/lib/push-summary"
import { buildRemoteStatusTexts } from "@/lib/remote-status"
import { blockingBranchOf, needsPush } from "@/lib/stack"
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
}

export interface DroppedCommits {
  /** The newest ones, newest first */
  commits: readonly DroppedCommit[]
  /** How many more the push drops than listed */
  more: number
}

/**
 * `unpushedParent`: the branch a stacked branch is built on, if a push of it has work for the remote; the pull request
 * of the branch would show the parent's commits until it is pushed
 */
export function getPushButtonState(branch: Branch, isSyncing: boolean, isPushing: boolean, unpushedParent: string | null = null): PushButtonState {
  // a blocked stacked branch keeps its last build, which lacks its parent's state: pushing it would mislead
  if (branch.status === "Blocked") {
    const blocker = blockingBranchOf(branch)
    return { text: "Push", color: "neutral", tooltip: blocker === null ? "Blocked: fix the parent branch first" : `Blocked by ${blocker}: fix it first, then sync`, dropped: null, disabled: true }
  }
  const remoteStatus = branch.remoteStatus
  const texts = remoteStatus != null ? buildRemoteStatusTexts(remoteStatus) : null
  const action = getPushAction(branch)

  let text = "Push"
  let tooltip = "Push branch to remote"
  let dropped: DroppedCommits | null = null
  if (remoteStatus != null && texts != null) {
    // a label counts the commits the push sends, whoever authored them; the description says how many are the user's
    const { commitsAhead } = remoteStatus
    const withCount = (label: string): string => commitsAhead > 0 ? `${label} (${commitsAhead})` : label
    tooltip = texts.tooltip
    if (action === "create") {
      text = withCount("Create Remote")
    }
    else if (action === "force") {
      text = withCount("Force Push")
      tooltip = `Force push replaces the remote branch.\n${texts.tooltip}`
      const { commitsBehind, droppedCommits } = remoteStatus
      if (commitsBehind > 0 && droppedCommits.length > 0) {
        dropped = { commits: droppedCommits, more: Math.max(0, commitsBehind - droppedCommits.length) }
      }
    }
    else if (action === "push") {
      text = withCount("Push")
    }
  }

  if (unpushedParent !== null && action !== "upToDate") {
    tooltip = `Push ${unpushedParent} first: its commits aren't on the remote yet.\n${tooltip}`
  }

  return {
    text,
    color: getPushColor(branch) ?? "primary",
    tooltip,
    dropped,
    disabled: isPushing || isSyncing || branch.status === "Syncing" || action === "upToDate",
  }
}

export function usePushButton(branch: Branch, isSyncing: boolean, isPushing: boolean): PushButtonState {
  const unpushedParent = useUnpushedParent(branch)
  return useMemo(() => getPushButtonState(branch, isSyncing, isPushing, unpushedParent), [branch, isSyncing, isPushing, unpushedParent])
}

/** The branch a stacked branch is built on, while a push of it has work for the remote */
export function useUnpushedParent(branch: Branch | undefined): string | null {
  const parent = branch?.stack?.effectiveBase ?? null
  return useBranchSync((state) => {
    const parentBranch = parent === null ? undefined : state.branches[parent]
    return parentBranch !== undefined && needsPush(parentBranch) ? parentBranch.name : null
  })
}
