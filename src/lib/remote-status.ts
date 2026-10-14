import type { DroppedCommit } from "@/lib/bindings"
import { getPushAction } from "@/lib/push-summary"
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

/** A rebased branch is not a fast-forward of its remote: the push replaces the remote commits, nothing is lost */
const rebasedText = "The branch was rebased onto the baseline: the push replaces the remote commits with their rebased copies."

export function buildRemoteStatusTexts(remote: RemoteStatus): RemoteStatusTexts {
  const { exists, commitsAhead, commitsBehind, lastPushTime, myCommitsAhead, fastForward } = remote
  const my = myCommitsAhead ?? 0

  const addPushTime = (tooltip: string): string => lastPushTime > 0
    ? `${tooltip}\n\nLast pushed: ${formatTimestamp(lastPushTime)}`
    : tooltip

  // by content: a branch only rebased onto a newer baseline is up to date, the remote has its commits
  if (exists && commitsAhead === 0 && commitsBehind === 0) {
    const tooltip = fastForward
      ? "Everything is in sync with the remote."
      : "The remote has all the commits; the branch was only rebased onto the baseline."
    return { text: "up to date", tooltip: addPushTime(tooltip) }
  }

  // not pushed / first push
  if (!exists) {
    const text = "not pushed"
    if (commitsAhead > 0) {
      const tooltip = my > 0
        ? `You have ${my} of your commits not on remote (${commitsAhead} total). Push to create the remote branch.`
        : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes). Push to create the remote branch.`
      return { text, tooltip: addPushTime(tooltip) }
    }
    return { text, tooltip: addPushTime("This branch isn't on the remote yet. Push to create it.") }
  }

  // diverged (both ahead and behind)
  if (commitsAhead > 0 && commitsBehind > 0) {
    const tooltip = my > 0
      ? `You have ${my} of your commits not on remote (${commitsAhead} total).\nRemote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`
      : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes).\nRemote includes ${plural(commitsBehind, "commit", "commits")} ${remoteDiffText}.`
    return { text: `${my} (${commitsAhead})↑ · ${commitsBehind}↓`, tooltip: addPushTime(tooltip) }
  }

  // ahead only
  if (commitsAhead > 0) {
    const ahead = my > 0
      ? `You have ${my} of your commits not on remote (${commitsAhead} total).`
      : `${plural(commitsAhead, "commit", "commits")} to push (no local authored changes).`
    const tooltip = fastForward ? ahead : `${ahead}\n${rebasedText}`
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

/** "Ann", "Ann and Bob", "Ann, Bob and Cy", "Ann, Bob and 2 others": the distinct authors, in the order of the commits */
export function formatAuthors(commits: readonly DroppedCommit[]): string {
  const authors = [...new Set(commits.map(commit => commit.author))]
  if (authors.length <= 1) {
    return authors[0] ?? ""
  }
  if (authors.length <= 3) {
    return `${authors.slice(0, -1).join(", ")} and ${authors.at(-1)}`
  }
  return `${authors.slice(0, 2).join(", ")} and ${authors.length - 2} others`
}

export interface RemoteMovedTexts {
  title: string
  description: string
  /** The label of the push that replaces what the remote has now, as the push button names it; null when up to date */
  action: string | null
}

/**
 * The toast of a push that found the remote branch moved since its status was shown: nothing was pushed, and `remote`
 * is the status of the branch as fetched now
 */
export function buildRemoteMovedTexts(branchName: string, remote: RemoteStatus): RemoteMovedTexts {
  const pushAction = getPushAction({ remoteStatus: remote })
  const action = pushAction === "create" ? "Create Remote" : pushAction === "force" ? "Force Push" : pushAction === "push" ? "Push" : null
  if (!remote.exists) {
    return { title: `${branchName} was deleted on the remote`, description: "Nothing was pushed.", action }
  }
  const title = `${branchName} changed on the remote`
  const { commitsBehind, droppedCommits } = remote
  if (commitsBehind > 0) {
    const authors = formatAuthors(droppedCommits)
    const by = authors === "" ? "" : ` (by ${authors})`
    return { title, description: `${plural(commitsBehind, "commit", "commits")} you don't have${by}. Nothing was pushed.`, action }
  }
  if (action === null) {
    return { title, description: "The remote has the branch's commits now. Nothing was pushed.", action }
  }
  return { title, description: "The remote has no commits the branch lacks. Nothing was pushed.", action }
}
