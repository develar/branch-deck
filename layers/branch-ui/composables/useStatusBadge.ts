import type { ReactiveBranch, RemoteStatus } from "~/composables/branchSyncProvider"
import { buildRemoteStatusTexts } from "~/composables/remoteStatusText"
import type { UIColor } from "~/utils/uiTypes"

interface StatusInfo {
  text: string
  color: UIColor
}

export function useStatusBadge(branch: Ref<ReactiveBranch>) {
  const status = computed((): StatusInfo => {
    const branchValue = branch.value

    // Priority 1: Show syncing progress
    if (branchValue.status === "Syncing") {
      return markRaw({
        text: `syncing ${branchValue.processedCount}/${branchValue.commitCount}…`,
        color: "neutral",
      })
    }

    // Priority 2: Show errors and conflicts (always highest priority)
    if (branchValue.status === "Error" || branchValue.status === "MergeConflict") {
      return markRaw({
        text: branchValue.statusText,
        color: "error",
      })
    }
    if (branchValue.status === "AnalyzingConflict") {
      return markRaw({
        text: branchValue.statusText,
        color: "warning",
      })
    }

    // Priority 3: Show remote status if available (most actionable)
    const remoteStatus = branchValue.remoteStatus as RemoteStatus | null | undefined
    if (remoteStatus != null) {
      const texts = buildRemoteStatusTexts(remoteStatus)
      // Choose color by state
      if (remoteStatus.exists && remoteStatus.commitsAhead === 0 && remoteStatus.commitsBehind === 0) {
        return markRaw({ text: texts.label, color: "neutral" })
      }
      if (!remoteStatus.exists) {
        return markRaw({ text: texts.label, color: "primary" })
      }
      // ahead-only = success, behind or both = info
      if ((remoteStatus.commitsAhead ?? 0) > 0 && (remoteStatus?.commitsBehind ?? 0) === 0) {
        return markRaw({ text: texts.label, color: "success" })
      }
      if ((remoteStatus.commitsBehind ?? 0) > 0 || (remoteStatus?.commitsAhead ?? 0) > 0) {
        return markRaw({ text: texts.label, color: "info" })
      }
    }

    // Priority 4: Fall back to sync status
    switch (branchValue.status) {
      case "Created":
        return markRaw({ text: "created", color: "success" })
      case "Updated":
        return markRaw({ text: "updated", color: "primary" })
      case "Unchanged":
        return markRaw({ text: "unchanged", color: "neutral" })
      default:
        return markRaw({ text: branchValue.statusText || "unknown", color: "neutral" })
    }
  })

  const tooltip = computed((): string => {
    const branchValue = branch.value

    // Syncing tooltip: show progress
    if (branchValue.status === "Syncing") {
      return `Syncing ${branchValue.processedCount}/${branchValue.commitCount}…`
    }

    // Errors/conflicts
    if (branchValue.status === "Error" || branchValue.status === "MergeConflict") {
      return branchValue.statusText || "Error"
    }
    if (branchValue.status === "AnalyzingConflict") {
      return branchValue.statusText || "Analyzing conflict…"
    }

    const remoteStatus = branch.value.remoteStatus
    if (remoteStatus != null) {
      return buildRemoteStatusTexts(remoteStatus).tooltip
    }

    // Fallback to sync status
    switch (branchValue.status) {
      case "Created":
        return "Branch created"
      case "Updated":
        return "Branch updated"
      case "Unchanged":
        return "No changes"
      default:
        return branchValue.statusText || "Unknown"
    }
  })

  return { status, tooltip }
}
