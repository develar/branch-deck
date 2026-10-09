import { useMemo } from "react"
import { buildRemoteStatusTexts } from "@/lib/remote-status"
import type { UIColor } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"

export interface PushButtonState {
  text: string
  color: UIColor
  tooltip: string
  disabled: boolean
}

function isRemoteUpToDate(branch: Branch): boolean {
  const remoteStatus = branch.remoteStatus
  return remoteStatus != null && remoteStatus.exists && remoteStatus.commitsAhead === 0 && remoteStatus.commitsBehind === 0
}

function needsForcePush(branch: Branch): boolean {
  return branch.remoteStatus != null && branch.remoteStatus.commitsBehind > 0
}

function isFirstPush(branch: Branch): boolean {
  return branch.remoteStatus != null && !branch.remoteStatus.exists
}

export function getPushButtonState(branch: Branch, isSyncing: boolean): PushButtonState {
  const remoteStatus = branch.remoteStatus
  const texts = remoteStatus != null ? buildRemoteStatusTexts(remoteStatus) : null
  const upToDate = isRemoteUpToDate(branch)

  let text = "Push"
  let tooltip = "Push branch to remote"
  if (remoteStatus != null && texts != null) {
    const { commitsAhead } = remoteStatus
    if (upToDate) {
      tooltip = "All commits are already pushed"
    }
    else if (isFirstPush(branch)) {
      text = commitsAhead > 0 ? `Create Remote (${commitsAhead})` : "Create Remote"
      tooltip = texts.tooltip
    }
    else if (needsForcePush(branch) || branch.status === "Updated") {
      const mine = remoteStatus.myCommitsAhead ?? 0
      const count = mine > 0 ? mine : commitsAhead
      text = count > 0 ? `Force Push (${count})` : "Force Push"
      tooltip = mine > 0
        ? `Force push ${mine} of your commits.\n${texts.tooltip}`
        : `Force push (no local authored changes).\n${texts.tooltip}; histories diverged after sync`
    }
    else if (commitsAhead > 0) {
      text = `Push (${commitsAhead})`
      tooltip = texts.tooltip
    }
  }

  return {
    text,
    color: texts != null ? texts.color : "primary",
    tooltip,
    disabled: branch.isPushing || isSyncing || branch.status === "Syncing" || upToDate,
  }
}

export function usePushButton(branch: Branch, isSyncing: boolean): PushButtonState {
  return useMemo(() => getPushButtonState(branch, isSyncing), [branch, isSyncing])
}
