import { useMemo } from "react"
import { getPushColor } from "@/lib/push-summary"
import { buildRemoteStatusTexts } from "@/lib/remote-status"
import { blockingBranchOf } from "@/lib/stack"
import type { StatusTexts } from "@/lib/remote-status"
import type { Branch } from "@/stores/branch-sync"

/** Status badge text, tooltip and color of a branch: sync progress, then errors, then remote status, then sync result. */
export function getStatusBadge(branch: Branch): StatusTexts {
  if (branch.status === "Syncing") {
    return {
      text: `syncing ${branch.processedCount}/${branch.commitCount}…`,
      tooltip: `Syncing ${branch.processedCount}/${branch.commitCount}…`,
      color: "neutral",
    }
  }

  // errors and conflicts always have the highest priority
  if (branch.status === "Error" || branch.status === "MergeConflict") {
    return { text: branch.statusText, tooltip: branch.statusText || "Error", color: "error", badge: true }
  }
  if (branch.status === "AnalyzingConflict") {
    return { text: branch.statusText, tooltip: branch.statusText || "Analyzing conflict…", color: "warning", badge: true }
  }
  // a stacked branch whose parent can't be built on: not an error of its own, so muted; its ref is left as it was
  if (branch.status === "Blocked") {
    const blocker = blockingBranchOf(branch)
    return { text: blocker === null ? "blocked" : `blocked by ${blocker}`, tooltip: branch.statusText || "Blocked by its parent branch", color: "warning" }
  }

  // remote status is the most actionable; it has the colour of the push button
  if (branch.remoteStatus != null) {
    return { ...buildRemoteStatusTexts(branch.remoteStatus), color: getPushColor(branch) ?? "neutral" }
  }

  switch (branch.status) {
    case "Created":
      return { text: "created", tooltip: "Branch created", color: "success" }
    case "Updated":
      return { text: "updated", tooltip: "Branch updated", color: "primary" }
    case "Unchanged":
      return { text: "unchanged", tooltip: "No changes", color: "neutral" }
  }
}

export function useStatusBadge(branch: Branch): StatusTexts {
  return useMemo(() => getStatusBadge(branch), [branch])
}
