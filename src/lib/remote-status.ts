import { formatTimestamp } from "@/lib/time"
import type { UIColor } from "@/lib/ui-types"
import type { RemoteStatus } from "@/stores/branch-sync"

export interface StatusTexts {
  /** Compact display text, e.g. "1 (3)↑ · 2↓" | "0 (3)↑" | "2↓" | "up to date" | "not pushed" */
  text: string
  /** Full descriptive text */
  tooltip: string
  color: UIColor
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`
}

const remoteDiffText = "you removed or changed locally"

export function buildRemoteStatusTexts(remote: RemoteStatus): StatusTexts {
  const { exists, commitsAhead, commitsBehind, lastPushTime, myCommitsAhead } = remote
  const my = myCommitsAhead ?? 0

  const addPushTime = (tooltip: string): string => lastPushTime > 0
    ? `${tooltip}\n\nLast pushed: ${formatTimestamp(lastPushTime)}`
    : tooltip

  if (exists && commitsAhead === 0 && commitsBehind === 0) {
    return { text: "up to date", tooltip: addPushTime("Everything is in sync with the remote."), color: "neutral" }
  }

  // not pushed / first push
  if (!exists) {
    const text = "not pushed"
    if (commitsAhead > 0) {
      const tooltip = my > 0
        ? `You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total). Push to create the remote branch.`
        : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes). Push to create the remote branch.`
      return { text, tooltip: addPushTime(tooltip), color: "primary" }
    }
    return { text, tooltip: addPushTime("This branch isn't on the remote yet. Push to create it."), color: "primary" }
  }

  // diverged (both ahead and behind)
  if (commitsAhead > 0 && commitsBehind > 0) {
    const tooltip = my > 0
      ? `You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total).\nRemote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`
      : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes).\nRemote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`
    return { text: `${my} (${commitsAhead})↑ · ${commitsBehind}↓`, tooltip: addPushTime(tooltip), color: "info" }
  }

  // ahead only
  if (commitsAhead > 0) {
    const tooltip = my > 0
      ? `You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total).`
      : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes).`
    // only actionable if the user has commits
    return { text: `${my} (${commitsAhead})↑`, tooltip: addPushTime(tooltip), color: my > 0 ? "primary" : "neutral" }
  }

  // behind only
  if (commitsBehind > 0) {
    return {
      text: `${commitsBehind}↓`,
      tooltip: addPushTime(`Remote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`),
      color: "info",
    }
  }

  // shouldn't happen
  return { text: "unknown", tooltip: addPushTime("Unknown"), color: "neutral" }
}
