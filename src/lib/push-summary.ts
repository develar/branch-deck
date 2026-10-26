import type { PushAction } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { joinNames } from "@/lib/text"
import type { UIColor } from "@/lib/ui-types"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"

type RemoteStatusInput = Pick<Branch, "remoteStatus">

/** The older versions a push replaces */
type OlderVersions = Pick<RemoteStatus, "commitsAmended" | "commitsRestacked" | "commitsLanded" | "commitsReworded" | "ancestorCommitsAhead">

/** Hue of a push: the push buttons and the status texts take it from here. */
export type PushHue = Extract<UIColor, "primary" | "warning">

/**
 * Hue of a push, by its risk: `warning` when it drops remote commits the branch lacks, `primary` otherwise. A force push
 * of a rebased branch only replaces the remote commits with their copies, one of an amended or restacked branch their
 * older versions: it is safe.
 */
export function getPushHue(action: PushAction): PushHue {
  return action.kind === "force" && action.drops ? "warning" : "primary"
}

/** What a push did, as its toast and its Activity entry say it */
export interface PushNews {
  /** "Pushed feature-auth", "Force pushed feature-auth" */
  title: string
  /** "3 commits · created the remote branch", "1 commit · dropped 2 remote commits" */
  description?: string
  /** The push dropped remote commits the branch lacked */
  warning?: true
}

/** What a push did, from the remote status it was made against (`null`: not loaded, nothing to say but the push) */
export function describePush(branchName: string, before: RemoteStatus | null): PushNews {
  const action = before?.action.kind
  if (before === null || action === undefined || action === "upToDate") {
    return { title: `Pushed ${branchName}` }
  }
  // a force push may only replace or drop remote commits
  const parts = before.commitsAhead === 0 ? [] : [formatCommitCount(before.commitsAhead)]
  if (before.ancestorCommitsAhead > 0) {
    parts.push(plural(before.ancestorCommitsAhead, "new commit of the parent", "new commits of the parent"))
  }
  if (action === "create") {
    return { title: `Pushed ${branchName}`, description: [...parts, "created the remote branch"].join(" · ") }
  }
  if (action === "push") {
    return { title: `Pushed ${branchName}`, description: parts[0] }
  }
  const behind = before.commitsBehind
  const replaced = formatReplaced(before)
  if (behind === 0) {
    const rest = replaced ?? (parts.length > 0 && before.ancestorCommitsAhead > 0 ? null : "replaced the rebased copies")
    return { title: `Force pushed ${branchName}`, description: [...parts, ...(rest === null ? [] : [rest])].join(" · ") }
  }
  if (replaced !== null) {
    parts.push(replaced)
  }
  return {
    title: `Force pushed ${branchName}`,
    description: [...parts, `dropped ${behind === 1 ? "1 remote commit" : `${behind} remote commits`}`].join(" · "),
    warning: true,
  }
}

/**
 * "replaced 1 amended commit", "replaced 2 reworded commits", "replaced the parent's older commits", "replaced 3 landed
 * commits", joined by "and"; null when none
 */
function formatReplaced({ commitsAmended, commitsRestacked, commitsLanded, commitsReworded }: OlderVersions): string | null {
  const replaced: string[] = []
  if (commitsAmended > 0) {
    replaced.push(plural(commitsAmended, "amended commit", "amended commits"))
  }
  if (commitsReworded > 0) {
    replaced.push(plural(commitsReworded, "reworded commit", "reworded commits"))
  }
  if (commitsRestacked > 0) {
    replaced.push("the parent's older commits")
  }
  if (commitsLanded > 0) {
    replaced.push(plural(commitsLanded, "landed commit", "landed commits"))
  }
  return replaced.length === 0 ? null : `replaced ${joinNames(replaced)}`
}

/** A push of a branch, by the remote status it was made against */
export interface BranchPush {
  name: string
  before: RemoteStatus | null
}

/**
 * What a push of several branches of a stack did, as one toast says it: "Pushed auth, login and oauth" / "7 commits ·
 * created 2 remote branches · force pushed auth"; a push that dropped remote commits says how many. `withStackBranch`:
 * the stack branch of the whole stack went along (Push with Children of the root), "· pushed the whole stack branch"
 */
export function describeStackPush(pushes: readonly BranchPush[], withStackBranch = false): PushNews {
  let commits = 0
  let created = 0
  let dropped = 0
  const forced: string[] = []
  for (const { name, before } of pushes) {
    const action = before?.action.kind
    if (before === null || action === undefined || action === "upToDate") {
      continue
    }
    commits += before.commitsAhead
    if (action === "create") {
      created++
    } else if (action === "force") {
      forced.push(name)
      dropped += before.commitsBehind
    }
  }
  const parts: string[] = []
  if (commits > 0) {
    parts.push(formatCommitCount(commits))
  }
  if (created > 0) {
    parts.push(`created ${plural(created, "remote branch", "remote branches")}`)
  }
  if (forced.length > 0) {
    parts.push(`force pushed ${joinNames(forced)}`)
  }
  if (dropped > 0) {
    parts.push(`dropped ${plural(dropped, "remote commit", "remote commits")}`)
  }
  if (withStackBranch) {
    parts.push("pushed the whole stack branch")
  }
  return {
    title: `Pushed ${joinNames(pushes.map(push => push.name))}`,
    ...(parts.length > 0 && { description: parts.join(" · ") }),
    ...(dropped > 0 && { warning: true as const }),
  }
}

/**
 * Colour of the push button and the status text of a branch; null while its remote status is not loaded.
 * Neutral when up to date, and for a safe push of no commit of the user: there is nothing of theirs to push. A push that
 * replaces older versions of amended or restacked commits keeps the hue: the remote branch is behind the user's work.
 */
export function getPushColor(branch: RemoteStatusInput): UIColor | null {
  const remoteStatus = branch.remoteStatus
  if (remoteStatus === null) {
    return null
  }
  const action = remoteStatus.action
  // a push of the user's commits, a first push and a push that replaces older versions keep the hue
  const quiet = action.kind === "push" || (action.kind === "force" && !action.drops && !action.replaces)
  if (action.kind === "upToDate" || (quiet && remoteStatus.myCommitsAhead === 0)) {
    return "neutral"
  }
  return getPushHue(action)
}

/** What a push sends and what it replaces on the remote, `formatPushCount` writes it */
export interface PushCount {
  /** Of the commits to push, the user's */
  mine: number
  /** The commits to push */
  count: number
  /** Older versions of commits amended here */
  amended: number
  /** Older versions of the commits of the branches it is built on, or new commits of them the remote lacks */
  restacked: number
  /** Older versions of commits the baseline has */
  landed: number
  /** Commits whose remote copies have another message or author */
  reworded: number
  /** Remote commits the push drops */
  behind: number
}

/**
 * The size of a push, as the status text of a branch row: `3↑` when the 3 commits are the user's, `1 (3)↑` when some
 * are others' (`0 (3)↑`: none is theirs), then `· 1 amended` for the older versions of commits amended here,
 * `· 1 reworded` for the remote copies of commits reworded here, `· 2 landed` for older versions of commits the baseline
 * has, `· restacked` for those of the branches it is built on, `· 2↓` for the remote commits the push drops. A part
 * without commits to push starts the text: `2↓`, `restacked`, `1 reworded`.
 */
export function formatPushCount({ mine, count, amended, restacked, landed, reworded, behind }: PushCount): string {
  const ahead = mine === count ? `${count}↑` : `${mine} (${count})↑`
  const parts: string[] = count === 0 ? [] : [ahead]
  if (amended > 0) {
    parts.push(`${amended} amended`)
  }
  if (reworded > 0) {
    parts.push(`${reworded} reworded`)
  }
  if (landed > 0) {
    parts.push(`${landed} landed`)
  }
  if (restacked > 0) {
    parts.push("restacked")
  }
  if (behind > 0) {
    parts.push(`${behind}↓`)
  }
  return parts.length === 0 ? ahead : parts.join(" · ")
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`
}
