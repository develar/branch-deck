import type { ReactiveBranch, RemoteStatus } from "~/composables/branchSyncProvider"
import { buildRemoteStatusTexts, type StatusTexts } from "./remoteStatus"

export function useStatusBadge(branch: Ref<ReactiveBranch>) {
  // Unified status and tooltip computation
  const status = computed((): StatusTexts => {
    const branchValue = branch.value

    // Priority 1: Show syncing progress
    if (branchValue.status === "Syncing") {
      return markRaw({
        text: `syncing ${branchValue.processedCount}/${branchValue.commitCount}…`,
        tooltip: `Syncing ${branchValue.processedCount}/${branchValue.commitCount}…`,
        color: "neutral",
      })
    }

    // Priority 2: Show errors and conflicts (always highest priority)
    if (branchValue.status === "Error" || branchValue.status === "MergeConflict") {
      const text = branchValue.statusText
      return markRaw({
        text,
        tooltip: text || "Error",
        color: "error",
      })
    }
    if (branchValue.status === "AnalyzingConflict") {
      const text = branchValue.statusText
      return markRaw({
        text,
        tooltip: text || "Analyzing conflict…",
        color: "warning",
      })
    }

    // Priority 3: Show remote status if available (most actionable)
    const remoteStatus = branchValue.remoteStatus as RemoteStatus | null | undefined
    if (remoteStatus != null) {
      return markRaw(buildRemoteStatusTexts(remoteStatus))
    }

    // Priority 4: Fall back to sync status
    switch (branchValue.status) {
      case "Created":
        return markRaw({ text: "created", tooltip: "Branch created", color: "success" })
      case "Updated":
        return markRaw({ text: "updated", tooltip: "Branch updated", color: "primary" })
      case "Unchanged":
        return markRaw({ text: "unchanged", tooltip: "No changes", color: "neutral" })
      default: {
        const text = branchValue.statusText || "unknown"
        return markRaw({ text, tooltip: branchValue.statusText || "Unknown", color: "neutral" })
      }
    }
  })

  return status
}
