import type { DroppedCommit, RemoteStatusUpdate } from "@/lib/bindings"
import { formatPushCount } from "@/lib/push-summary"
import { cutList, joinNames } from "@/lib/text"
import { formatTimestamp } from "@/lib/time"
import type { UIColor } from "@/lib/ui-types"
import type { RemoteStatus } from "@/stores/branch-sync"

export interface StatusTexts {
  /**
   * Compact display text, e.g. "3↑" | "1 (3)↑ · 2↓" | "0 (3)↑" | "2↓" | "1↑ · 1 amended" | "4↑ · restacked" |
   * "restacked" | "1 reworded" | "1↑ · 7 landed" | "up to date" | "not pushed"
   */
  text: string
  /** Full descriptive text */
  tooltip: string
  color: UIColor
  /** An exceptional state (error, conflict) shown as a badge; a routine one is muted text with a colour dot */
  badge?: boolean
  /**
   * The state needs no action (pushed, the remote as the branch, "up to date"): the row shows nothing, so the states
   * that do stand out; the branch preview says it
   */
  quiet?: boolean
}

/** The texts of a remote status; its colour is the push colour of the branch (`getPushColor`). */
export type RemoteStatusTexts = Omit<StatusTexts, "color" | "badge" | "quiet">

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

/** "The remote has the old message of 1 commit reworded here." */
function formatReworded(count: number): string {
  return `The remote has the old ${count === 1 ? "message" : "messages"} of ${plural(count, "commit", "commits")} reworded here.`
}

/** "The remote has older versions of 7 commits that landed." */
function formatLanded(count: number): string {
  return `The remote has ${olderVersions(count, plural(count, "commit", "commits"))} that landed.`
}

/** "Rebuilt on the new auth: the remote lacks 1 of its commits, which a Safe Push of the branch lands with it." */
function formatAncestorsAhead(count: number, parent: string | null): string {
  return `Rebuilt on the new ${parent ?? "parent"}: the remote lacks ${count} of its commits, which a Safe Push of the branch lands with it.`
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
  const { exists, commitsAhead, commitsBehind, commitsAmended, commitsRestacked, commitsLanded, commitsReworded, ancestorCommitsAhead, lastPushTime, fastForward, action } = remote
  const mine = Math.min(remote.myCommitsAhead ?? 0, commitsAhead)

  const withPushTime = (lines: readonly string[]): string => {
    const tooltip = lines.join("\n")
    return lastPushTime > 0 ? `${tooltip}\n\nPushed ${formatTimestamp(lastPushTime)}` : tooltip
  }

  // by content: a branch only rebased onto a newer baseline is up to date, the remote has its commits
  if (action.kind === "upToDate") {
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
  if (commitsReworded > 0 && !olderVersionsNamed) {
    lines.push(formatReworded(commitsReworded))
  }
  if (commitsRestacked > 0 && !olderVersionsNamed) {
    lines.push(formatRestacked(commitsRestacked, parent))
  }
  if (ancestorCommitsAhead > 0 && !olderVersionsNamed) {
    lines.push(formatAncestorsAhead(ancestorCommitsAhead, parent))
  }
  if (commitsLanded > 0 && !olderVersionsNamed) {
    lines.push(formatLanded(commitsLanded))
  }
  if (commitsBehind > 0) {
    lines.push(formatRemoteHas(remote))
  } else if (action.kind === "force" && !action.replaces) {
    // a force push that drops nothing and replaces no older version: the branch was only rebased
    lines.push(rebasedText)
  }
  const text = formatPushCount({
    mine,
    count: commitsAhead,
    amended: commitsAmended,
    // rebuilt on a parent with new commits: restacked as well
    restacked: commitsRestacked + ancestorCommitsAhead,
    landed: commitsLanded,
    reworded: commitsReworded,
    behind: commitsBehind,
  })
  return { text, tooltip: withPushTime(lines) }
}

/**
 * The older versions a force push that drops nothing replaces: "1 commit amended here", "the old message of 1 commit
 * reworded here", "the older versions of auth's commits", "7 commits that landed", joined by "and"; null when there are
 * none or the push drops remote commits
 */
function formatReplacedVersions(remote: RemoteStatus, parent: string | null): string | null {
  if (remote.commitsBehind > 0) {
    return null
  }
  const replaced: string[] = []
  if (remote.commitsAmended > 0) {
    replaced.push(`${plural(remote.commitsAmended, "commit", "commits")} amended here`)
  }
  if (remote.commitsReworded > 0) {
    replaced.push(`the old ${remote.commitsReworded === 1 ? "message" : "messages"} of ${plural(remote.commitsReworded, "commit", "commits")} reworded here`)
  }
  if (remote.commitsRestacked > 0) {
    replaced.push(`the older versions of ${parent === null ? "the parent" : parent}'s commits`)
  }
  if (remote.ancestorCommitsAhead > 0) {
    replaced.push(`the remote branch without ${plural(remote.ancestorCommitsAhead, "new commit", "new commits")} of ${parent === null ? "the parent" : parent}`)
  }
  if (remote.commitsLanded > 0) {
    replaced.push(`${plural(remote.commitsLanded, "commit", "commits")} that landed`)
  }
  return replaced.length === 0 ? null : joinNames(replaced)
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
  const pushAction = remote.action
  const action = pushAction.kind === "force" ? "Force Push" : pushAction.kind === "upToDate" ? null : "Push"
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
  if (pushAction.kind === "force" && pushAction.replaces) {
    return { title, description: "The remote has older versions of the branch's commits. Nothing was pushed.", action }
  }
  return { title, description: "The remote has no commits the branch lacks. Nothing was pushed.", action }
}

/** The fields of a remote status of Rust the page reads */
type RemoteStatusFields = Pick<
  RemoteStatusUpdate,
  | "remoteExists"
  | "unpushedCommits"
  | "commitsBehind"
  | "othersBehind"
  | "commitsAmended"
  | "commitsRestacked"
  | "commitsLanded"
  | "commitsReworded"
  | "ancestorCommitsAhead"
  | "myUnpushedCount"
  | "fastForward"
  | "lastPushTime"
  | "remoteTip"
  | "droppedCommits"
  | "action"
>

/** The remote status of Rust (`RemoteStatusUpdate`) as the page reads it */
export function toRemoteStatus(update: RemoteStatusFields): RemoteStatus {
  return {
    exists: update.remoteExists,
    unpushedCommits: update.unpushedCommits,
    commitsAhead: update.unpushedCommits.length,
    commitsBehind: update.commitsBehind,
    othersBehind: update.othersBehind,
    commitsAmended: update.commitsAmended,
    commitsRestacked: update.commitsRestacked,
    commitsLanded: update.commitsLanded,
    commitsReworded: update.commitsReworded,
    ancestorCommitsAhead: update.ancestorCommitsAhead,
    myCommitsAhead: update.myUnpushedCount ?? 0,
    fastForward: update.fastForward,
    lastPushTime: update.lastPushTime ?? 0,
    remoteTip: update.remoteTip,
    droppedCommits: update.droppedCommits,
    action: update.action,
  }
}
