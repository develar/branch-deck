import type { RemoteStatus } from "~/composables/branchSyncProvider"
import { formatTimestamp } from "#layers/shared-ui/utils/time"

export interface RemoteStatusTexts {
  label: string // compact badge text, e.g. "3↑ (1) 2↓" | "3↑" | "2↓" | "up to date" | "not pushed"
  tooltip: string // full descriptive text
}

function plural(n: number, one: string, many: string) {
  return `${n} ${n === 1 ? one : many}`
}

export function buildRemoteStatusTexts(remote: RemoteStatus): RemoteStatusTexts {
  const { exists, commitsAhead, commitsBehind, lastPushTime } = remote
  const my = remote.myCommitsAhead ?? 0

  // Helper function to add push time to tooltip if available
  const addPushTime = (tooltip: string): string => {
    if (lastPushTime > 0) {
      return `${tooltip}\n\nLast pushed: ${formatTimestamp(lastPushTime)}`
    }
    return tooltip
  }

  if (exists && commitsAhead === 0 && commitsBehind === 0) {
    return { label: "up to date", tooltip: addPushTime("Everything is in sync with the remote.") }
  }
  if (!exists) {
    // First push / remote missing – my-first phrasing
    const label = "not pushed"
    if (commitsAhead > 0) {
      if (my > 0) {
        return { label, tooltip: addPushTime(`You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total). Push to create the remote branch.`) }
      }
      return { label, tooltip: addPushTime(`This branch has ${plural(commitsAhead, "local commit", "local commits")} not on remote. Push to create the remote branch.`) }
    }
    return { label, tooltip: addPushTime("This branch isn't on the remote yet. Push to create it.") }
  }

  const remoteDiffText = "you removed or changed locally"

  if (commitsAhead > 0 && commitsBehind > 0) {
    // Show my first with total using parentheses: (m/N)↑
    const aheadLabel = my > 0 ? `${my} (${commitsAhead})↑` : `${commitsAhead}↑`
    return {
      label: `${aheadLabel} · ${commitsBehind}↓`,
      tooltip: addPushTime(my > 0
        ? `You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total).\nRemote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`
        : `This branch has ${plural(commitsAhead, "local commit", "local commits")} not on remote.\nRemote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`),
    }
  }

  if (commitsAhead > 0) {
    const aheadLabel = my > 0 ? `${my} (${commitsAhead})↑` : `${commitsAhead}↑`
    return {
      label: aheadLabel,
      tooltip: addPushTime(my > 0
        ? `You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total).`
        : `This branch has ${plural(commitsAhead, "local commit", "local commits")} not on remote.`),
    }
  }

  if (commitsBehind > 0) {
    return {
      label: `${commitsBehind}↓`,
      tooltip: addPushTime(`Remote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`),
    }
  }

  return { label: "unknown", tooltip: addPushTime("Unknown") }
}
