import type { DroppedCommit } from "@/lib/bindings"
import { formatPushCount, getPushAction, isRemoteUpToDate, replacesOlderVersions } from "@/lib/push-summary"
import { cutList } from "@/lib/text"
import { formatTimestamp } from "@/lib/time"
import type { UIColor } from "@/lib/ui-types"
import type { RemoteStatus } from "@/stores/branch-sync"

export interface StatusTexts {
  /**
   * Compact display text, e.g. "3↑" | "1 (3)↑ · 2↓" | "0 (3)↑" | "2↓" | "1↑ · 1 amended" | "4↑ · restacked" |
   * "restacked" | "up to date" | "not pushed"
   */
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

/** The one sentence of a branch the remote has as it is */
const upToDateText = "Everything is pushed."

/** A branch only rebased onto a newer baseline: the remote has its commits by content */
const onlyRebasedText = "The branch was only rebased onto the baseline: the remote has its commits."

/** A rebased branch is not a fast-forward of its remote: the push replaces the remote commits, nothing is lost */
const rebasedText = "The branch was rebased onto the baseline: the push replaces the remote commits with their rebased copies."

const createText = "The branch isn't on the remote yet: the push creates it."

/** "3 commits to push." when they are all the user's, "2 of 3 commits to push are yours." otherwise */
function formatCommitsToPush(mine: number, count: number): string {
  if (mine === count) {
    return `${plural(count, "commit", "commits")} to push.`
  }
  if (mine === 0) {
    return count === 1 ? "1 commit to push, not yours." : `${count} commits to push, none of them yours.`
  }
  return `${mine} of ${count} commits to push ${mine === 1 ? "is" : "are"} yours.`
}

/** "The remote has 2 commits your branch doesn't (by Ann and Bob)." */
function formatRemoteHas(remote: RemoteStatus): string {
  return `The remote has ${plural(remote.commitsBehind, "commit", "commits")} your branch doesn't${formatByAuthors(remote.droppedCommits)}.`
}

/** "an older version of 1 commit", "older versions of 2 commits": `what` counts `count` commits */
function olderVersions(count: number, what: string): string {
  return `${count === 1 ? "an older version" : "older versions"} of ${what}`
}

/** "The remote has an older version of 1 commit amended here." */
function formatAmended(count: number): string {
  return `The remote has ${olderVersions(count, plural(count, "commit", "commits"))} amended here.`
}

/** "Rebuilt on the new auth: the remote has older versions of 2 of its commits." */
function formatRestacked(count: number, parent: string | null): string {
  return `Rebuilt on the new ${parent ?? "parent"}: the remote has ${olderVersions(count, `${count} of its commits`)}.`
}

export interface RemoteStatusTextOptions {
  /**
   * The description of a Force Push: it starts with what the push replaces (`formatForcePushLine`), and the older
   * versions it names there are not repeated below
   */
  forcePush?: boolean
}

/**
 * The texts of a remote status. `parent`: the branch a stacked branch is built on (`effectiveBase`), which the remote
 * has older versions of commits of when the branch is restacked
 */
export function buildRemoteStatusTexts(remote: RemoteStatus, parent: string | null = null, { forcePush = false }: RemoteStatusTextOptions = {}): RemoteStatusTexts {
  const { exists, commitsAhead, commitsBehind, commitsAmended, commitsRestacked, lastPushTime, fastForward } = remote
  const mine = Math.min(remote.myCommitsAhead ?? 0, commitsAhead)

  const withPushTime = (lines: readonly string[]): string => {
    const tooltip = lines.join("\n")
    return lastPushTime > 0 ? `${tooltip}\n\nPushed ${formatTimestamp(lastPushTime)}` : tooltip
  }

  // by content: a branch only rebased onto a newer baseline is up to date, the remote has its commits
  if (isRemoteUpToDate(remote)) {
    return { text: "up to date", tooltip: withPushTime(fastForward ? [upToDateText] : [upToDateText, onlyRebasedText]) }
  }

  // not pushed / first push
  if (!exists) {
    const lines = commitsAhead > 0 ? [formatCommitsToPush(mine, commitsAhead), createText] : [createText]
    return { text: "not pushed", tooltip: withPushTime(lines) }
  }

  const lines: string[] = forcePush ? [formatForcePushLine(remote, parent)] : []
  // the first line of a force push that drops nothing names the older versions it replaces
  const olderVersionsNamed = forcePush && commitsBehind === 0
  if (commitsAhead > 0) {
    lines.push(formatCommitsToPush(mine, commitsAhead))
  }
  if (commitsAmended > 0 && !olderVersionsNamed) {
    lines.push(formatAmended(commitsAmended))
  }
  if (commitsRestacked > 0 && !olderVersionsNamed) {
    lines.push(formatRestacked(commitsRestacked, parent))
  }
  if (commitsBehind > 0) {
    lines.push(formatRemoteHas(remote))
  } else if (!fastForward && !replacesOlderVersions(remote)) {
    lines.push(rebasedText)
  }
  const text = formatPushCount({ mine, count: commitsAhead, amended: commitsAmended, restacked: commitsRestacked, behind: commitsBehind })
  return { text, tooltip: withPushTime(lines) }
}

/**
 * The older versions a force push that drops nothing replaces: "1 commit amended here", "the older versions of auth's
 * commits", both joined by "and"; null when there are none or the push drops remote commits
 */
function formatReplacedVersions(remote: RemoteStatus, parent: string | null): string | null {
  const replaced: string[] = []
  if (remote.commitsBehind === 0 && remote.commitsAmended > 0) {
    replaced.push(`${plural(remote.commitsAmended, "commit", "commits")} amended here`)
  }
  if (remote.commitsBehind === 0 && remote.commitsRestacked > 0) {
    replaced.push(`the older versions of ${parent === null ? "the parent" : parent}'s commits`)
  }
  return replaced.length === 0 ? null : replaced.join(" and ")
}

/**
 * The first line of the description of a force push, what it replaces: "Force push replaces 1 commit amended here.",
 * "Force push replaces the older versions of auth's commits."; a push that drops remote commits or replaces rebased
 * copies "replaces the remote branch."
 */
export function formatForcePushLine(remote: RemoteStatus, parent: string | null = null): string {
  return `Force push replaces ${formatReplacedVersions(remote, parent) ?? "the remote branch"}.`
}

/**
 * A force push in a line of a stack push, in the words of `formatForcePushLine`: "force push, replaces 1 commit amended
 * here", "force push, replaces the older versions of auth's commits", "force push, drops 1 remote commit"; "force
 * push" for a rebased branch
 */
export function formatForcePushPart(remote: RemoteStatus, parent: string | null = null): string {
  if (remote.commitsBehind > 0) {
    return `force push, drops ${plural(remote.commitsBehind, "remote commit", "remote commits")}`
  }
  const replaced = formatReplacedVersions(remote, parent)
  return replaced === null ? "force push" : `force push, replaces ${replaced}`
}

/** " (by Ann and Bob)" after a count of commits; nothing without authors */
function formatByAuthors(commits: readonly DroppedCommit[]): string {
  const authors = formatAuthors(commits)
  return authors === "" ? "" : ` (by ${authors})`
}

/**
 * "Ann", "Ann and Bob", "Ann, Bob and Cy", "Ann, Bob and 2 others": the distinct authors, in the order of the commits,
 * never "and 1 other" (`cutList`)
 */
export function formatAuthors(commits: readonly DroppedCommit[]): string {
  const authors = [...new Set(commits.map(commit => commit.author))]
  const { shown, more } = cutList(authors, 2)
  if (more > 0) {
    return `${shown.join(", ")} and ${more} others`
  }
  return shown.length <= 1 ? (shown[0] ?? "") : `${shown.slice(0, -1).join(", ")} and ${shown.at(-1)}`
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
    return { title, description: `${plural(commitsBehind, "commit", "commits")} you don't have${formatByAuthors(droppedCommits)}. Nothing was pushed.`, action }
  }
  if (action === null) {
    return { title, description: "The remote has the branch's commits now. Nothing was pushed.", action }
  }
  if (replacesOlderVersions(remote)) {
    return { title, description: "The remote has older versions of the branch's commits. Nothing was pushed.", action }
  }
  return { title, description: "The remote has no commits the branch lacks. Nothing was pushed.", action }
}
