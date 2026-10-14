import { useMemo } from "react"
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
  disabled: boolean
}

/**
 * `unpushedParent`: the branch a stacked branch is built on, if a push of it has work for the remote; the pull request
 * of the branch would show the parent's commits until it is pushed
 */
export function getPushButtonState(branch: Branch, isSyncing: boolean, isPushing: boolean, unpushedParent: string | null = null): PushButtonState {
  // a blocked stacked branch keeps its last build, which lacks its parent's state: pushing it would mislead
  if (branch.status === "Blocked") {
    const blocker = blockingBranchOf(branch)
    return { text: "Push", color: "neutral", tooltip: blocker === null ? "Blocked: fix the parent branch first" : `Blocked by ${blocker}: fix it first, then sync`, disabled: true }
  }
  const remoteStatus = branch.remoteStatus
  const texts = remoteStatus != null ? buildRemoteStatusTexts(remoteStatus) : null
  const action = getPushAction(branch)

  let text = "Push"
  let tooltip = "Push branch to remote"
  if (remoteStatus != null && texts != null) {
    const { commitsAhead } = remoteStatus
    if (action === "upToDate") {
      tooltip = "All commits are already pushed"
    }
    else if (action === "create") {
      text = commitsAhead > 0 ? `Create Remote (${commitsAhead})` : "Create Remote"
      tooltip = texts.tooltip
    }
    else if (action === "force") {
      const mine = remoteStatus.myCommitsAhead ?? 0
      const count = mine > 0 ? mine : commitsAhead
      text = count > 0 ? `Force Push (${count})` : "Force Push"
      tooltip = mine > 0
        ? `Force push ${mine} of your commits.\n${texts.tooltip}`
        : `Force push (no local authored changes).\n${texts.tooltip}`
    }
    else if (commitsAhead > 0) {
      text = `Push (${commitsAhead})`
      tooltip = texts.tooltip
    }
  }

  if (unpushedParent !== null && action !== "upToDate") {
    tooltip = `Push ${unpushedParent} first: its commits aren't on the remote yet.\n${tooltip}`
  }

  return {
    text,
    color: getPushColor(branch) ?? "primary",
    tooltip,
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
