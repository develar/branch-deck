import type { RemoteStatus } from "~/composables/branchSyncProvider"
import { formatTimestamp } from "#layers/shared-ui/utils/time"
import type { UIColor } from "~/utils/uiTypes"

export interface StatusTexts {
  text: string // compact display text, e.g. "1 (3)↑ · 2↓" | "0 (3)↑" | "2↓" | "up to date" | "not pushed"
  tooltip: string // full descriptive text
  color: UIColor // appropriate UI color for this status
}

function plural(n: number, one: string, many: string) {
  return `${n} ${n === 1 ? one : many}`
}

export function buildRemoteStatusTexts(remote: RemoteStatus): StatusTexts {
  const { exists, commitsAhead, commitsBehind, lastPushTime, myCommitsAhead } = remote
  const my = myCommitsAhead ?? 0

  // Helper function to add push time to tooltip if available
  const addPushTime = (tooltip: string): string => {
    if (lastPushTime > 0) {
      return `${tooltip}\n\nLast pushed: ${formatTimestamp(lastPushTime)}`
    }
    return tooltip
  }

  // Up to date
  if (exists && commitsAhead === 0 && commitsBehind === 0) {
    return {
      text: "up to date",
      tooltip: addPushTime("Everything is in sync with the remote."),
      color: "neutral",
    }
  }

  // Not pushed / first push
  if (!exists) {
    const text = "not pushed"
    if (commitsAhead > 0) {
      if (my > 0) {
        return {
          text,
          tooltip: addPushTime(`You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total). Push to create the remote branch.`),
          color: "primary",
        }
      }
      return {
        text,
        tooltip: addPushTime(`${plural(commitsAhead, "commit", "commits")} to push (no local authored changes). Push to create the remote branch.`),
        color: "primary",
      }
    }
    return {
      text,
      tooltip: addPushTime("This branch isn't on the remote yet. Push to create it."),
      color: "primary",
    }
  }

  const remoteDiffText = "you removed or changed locally"

  // Diverged (both ahead and behind)
  if (commitsAhead > 0 && commitsBehind > 0) {
    const aheadText = `${my} (${commitsAhead})↑`
    return {
      text: `${aheadText} · ${commitsBehind}↓`,
      tooltip: addPushTime(my > 0
        ? `You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total).\nRemote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`
        : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes).\nRemote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`),
      color: "info",
    }
  }

  // Ahead only
  if (commitsAhead > 0) {
    const aheadText = `${my} (${commitsAhead})↑`
    const color = my > 0 ? "primary" : "neutral" // Only actionable if user has commits
    return {
      text: aheadText,
      tooltip: addPushTime(my > 0
        ? `You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total).`
        : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes).`),
      color,
    }
  }

  // Behind only
  if (commitsBehind > 0) {
    return {
      text: `${commitsBehind}↓`,
      tooltip: addPushTime(`Remote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`),
      color: "info",
    }
  }

  // Fallback (shouldn't happen)
  return {
    text: "unknown",
    tooltip: addPushTime("Unknown"),
    color: "neutral",
  }
}
