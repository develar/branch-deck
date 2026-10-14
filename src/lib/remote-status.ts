import { formatTimestamp } from "@/lib/time"
import type { UIColor } from "@/lib/ui-types"
import type { RemoteStatus } from "@/stores/branch-sync"

export interface StatusTexts {
  /** Compact display text, e.g. "1 (3)↑ · 2↓" | "0 (3)↑" | "2↓" | "up to date" | "not pushed" */
  text: string
  /** Full descriptive text */
  tooltip: string
  color: UIColor
  /** An exceptional state (error, conflict) shown as a badge; a routine one is muted text with a colour dot */
  badge?: boolean
}

/** The texts of a remote status; its colour is the push colour of the branch (`getPushColor`). */
export type RemoteStatusTexts = Omit<StatusTexts, "color" | "badge">

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`
}

const remoteDiffText = "you removed or changed locally"

export function buildRemoteStatusTexts(remote: RemoteStatus): RemoteStatusTexts {
  const { exists, commitsAhead, commitsBehind, lastPushTime, myCommitsAhead } = remote
  const my = myCommitsAhead ?? 0

  const addPushTime = (tooltip: string): string => lastPushTime > 0
    ? `${tooltip}\n\nLast pushed: ${formatTimestamp(lastPushTime)}`
    : tooltip

  if (exists && commitsAhead === 0 && commitsBehind === 0) {
    return { text: "up to date", tooltip: addPushTime("Everything is in sync with the remote.") }
  }

  // not pushed / first push
  if (!exists) {
    const text = "not pushed"
    if (commitsAhead > 0) {
      const tooltip = my > 0
        ? `You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total). Push to create the remote branch.`
        : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes). Push to create the remote branch.`
      return { text, tooltip: addPushTime(tooltip) }
    }
    return { text, tooltip: addPushTime("This branch isn't on the remote yet. Push to create it.") }
  }

  // diverged (both ahead and behind)
  if (commitsAhead > 0 && commitsBehind > 0) {
    const tooltip = my > 0
      ? `You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total).\nRemote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`
      : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes).\nRemote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`
    return { text: `${my} (${commitsAhead})↑ · ${commitsBehind}↓`, tooltip: addPushTime(tooltip) }
  }

  // ahead only
  if (commitsAhead > 0) {
    const tooltip = my > 0
      ? `You have ${plural(my, "of your commit", "of your commits")} not on remote (${commitsAhead} total).`
      : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes).`
    return { text: `${my} (${commitsAhead})↑`, tooltip: addPushTime(tooltip) }
  }

  // behind only
  if (commitsBehind > 0) {
    return {
      text: `${commitsBehind}↓`,
      tooltip: addPushTime(`Remote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`),
    }
  }

  // shouldn't happen
  return { text: "unknown", tooltip: addPushTime("Unknown") }
}
