import type { ReactiveBranch, RemoteStatus } from "~/composables/branchSyncProvider"
import { buildRemoteStatusTexts } from "~/composables/remoteStatusText"
import type { UIColor } from "~/utils/uiTypes"

export function usePushButton(
  branch: Ref<ReactiveBranch>,
  isSyncing: Ref<boolean>,
) {
  // Helper functions for remote status checks
  const isRemoteUpToDate = (branch: ReactiveBranch): boolean => {
    const remoteStatus = branch.remoteStatus
    if (remoteStatus == null) {
      return false
    }
    const { exists, commitsAhead, commitsBehind } = remoteStatus
    return commitsAhead === 0 && commitsBehind === 0 && exists
  }

  const needsForcePush = (branch: ReactiveBranch): boolean => {
    return branch.remoteStatus === null ? false : branch.remoteStatus.commitsBehind > 0
  }

  const isFirstPush = (branch: ReactiveBranch): boolean => {
    return branch.remoteStatus === null ? false : !branch.remoteStatus.exists
  }

  // Push button properties computed directly
  const pushButtonText = computed(() => {
    const branchValue = branch.value
    // Check if remote status has been loaded
    const remoteStatus = branchValue.remoteStatus
    if (remoteStatus != null && !isRemoteUpToDate(branchValue)) {
      const { commitsAhead } = remoteStatus
      if (isFirstPush(branchValue)) {
        return commitsAhead > 0 ? `Create Remote (${commitsAhead})` : "Create Remote"
      }
      else if (needsForcePush(branchValue) || branchValue.status === "Updated") {
        const mine = (remoteStatus as RemoteStatus).myCommitsAhead ?? 0
        const count = mine > 0 ? mine : commitsAhead
        return count > 0 ? `Force Push (${count})` : "Force Push"
      }
      else if (commitsAhead > 0) {
        return `Push (${commitsAhead})`
      }
    }

    return "Push"
  })

  const pushButtonColor = computed((): UIColor => {
    const branchValue = branch.value
    const remoteStatus = branchValue.remoteStatus
    if (remoteStatus != null) {
      if (isRemoteUpToDate(branchValue)) {
        return "neutral"
      }
      else if (isFirstPush(branchValue)) {
        return "success"
      }
      else if (needsForcePush(branchValue) || branchValue.status === "Updated") {
        return "info"
      }
    }

    return "primary"
  })

  const pushButtonTooltip = computed(() => {
    const branchValue = branch.value

    const remoteStatus = branchValue.remoteStatus
    if (remoteStatus != null) {
      if (isRemoteUpToDate(branchValue)) {
        return "All commits are already pushed"
      }

      const texts = buildRemoteStatusTexts(remoteStatus)
      if (isFirstPush(branchValue)) {
        // Reuse shared summary for consistency
        return texts.tooltip
      }
      else if (needsForcePush(branchValue) || branchValue.status === "Updated") {
        const mine = (remoteStatus as RemoteStatus).myCommitsAhead ?? 0
        const base = texts.tooltip
        if (mine > 0) {
          return `Force push ${mine} of your commits.\n${base}`
        }
        return `Force push (no local authored changes).\n${base}; histories diverged after sync`
      }
      else if (remoteStatus.commitsAhead > 0) {
        return texts.tooltip
      }
    }

    return "Push branch to remote"
  })

  const isPushButtonDisabled = computed(() => {
    const branchValue = branch.value
    return branchValue.isPushing || isSyncing.value || branchValue.status === "Syncing" || isRemoteUpToDate(branchValue)
  })

  return {
    pushButtonText,
    pushButtonColor,
    pushButtonTooltip,
    isPushButtonDisabled,
  }
}