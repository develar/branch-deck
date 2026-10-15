import { useMemo } from "react"
import { useLandedIn } from "@/hooks/useLandedIn"
import { landedBranchDescription } from "@/lib/landed"
import { getPushAction, getPushColor } from "@/lib/push-summary"
import { buildRemoteStatusTexts } from "@/lib/remote-status"
import { blockingBranchOf } from "@/lib/stack"
import type { StatusTexts } from "@/lib/remote-status"
import type { UIColor } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"

/**
 * Status badge text, tooltip and color of a branch: sync progress, then errors, then landed, then remote status, then
 * sync result. `landedIn`: the baseline that has every commit of the branch before the update ("origin/master"), the
 * branch is "landed" and has nothing to push; null otherwise.
 */
export function getStatusBadge(branch: Branch, landedIn: string | null = null): StatusTexts {
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

  // the update archives a branch the baseline has all of: its push state no longer matters; `info` is the merged hue
  if (landedIn !== null) {
    return { text: "landed", tooltip: landedBranchDescription(landedIn), color: "info" }
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
  const landedIn = useLandedIn(branch.name)
  return useMemo(() => getStatusBadge(branch, landedIn), [branch, landedIn])
}

/** The node of a branch in the stack graph: the colour of its status, filled while it needs something */
export interface LaneNode {
  color: UIColor
  /** The branch has work for the remote, or an error or a conflict; hollow when up to date or landed */
  filled: boolean
}

/** The node of a branch in the stack graph: it replaces the dot of the status text */
export function getLaneNode(branch: Branch, landedIn: string | null = null): LaneNode {
  const color = getStatusBadge(branch, landedIn).color
  const action = landedIn === null ? getPushAction(branch) : null
  return { color, filled: color === "error" || (action !== null && action !== "upToDate") }
}

export function useLaneNode(branch: Branch): LaneNode {
  const landedIn = useLandedIn(branch.name)
  return useMemo(() => getLaneNode(branch, landedIn), [branch, landedIn])
}
