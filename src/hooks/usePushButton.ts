import { useMemo } from "react"
import { getPushAction, getPushColor } from "@/lib/push-summary"
import { buildRemoteStatusTexts } from "@/lib/remote-status"
import type { UIColor } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"

export interface PushButtonState {
  text: string
  color: UIColor
  tooltip: string
  disabled: boolean
}

export function getPushButtonState(branch: Branch, isSyncing: boolean, isPushing: boolean): PushButtonState {
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

  return {
    text,
    color: getPushColor(branch) ?? "primary",
    tooltip,
    disabled: isPushing || isSyncing || branch.status === "Syncing" || action === "upToDate",
  }
}

export function usePushButton(branch: Branch, isSyncing: boolean, isPushing: boolean): PushButtonState {
  return useMemo(() => getPushButtonState(branch, isSyncing, isPushing), [branch, isSyncing, isPushing])
}
