import { formatCommitCount } from "@/lib/commit"
import { joinNames } from "@/lib/text"
import type { UIColor } from "@/lib/ui-types"
import type { Branch, RemoteStatus } from "@/stores/branch-sync"

/** What a push of a branch does: a plain push, the first push that creates the remote branch, a force push. */
export type OutgoingState = "push" | "create" | "force"

/** What the push button of a branch does; `upToDate` when there is nothing to push. */
export type PushAction = OutgoingState | "upToDate"

type RemoteStatusInput = Pick<Branch, "remoteStatus">

/** The older versions a push replaces */
type OlderVersions = Pick<RemoteStatus, "commitsAmended" | "commitsRestacked" | "commitsLanded" | "commitsReworded">

type RemoteCounts = Pick<RemoteStatus, "exists" | "commitsAhead" | "commitsBehind"> & OlderVersions

/**
 * The remote branch has the branch as it is, by content: nothing to push, and no remote commit the branch lacks, as an
 * older version (amended, restacked, landed, reworded) or as work a push drops. A branch only rebased onto a newer
 * baseline is up to date; one reworded since is not, a robot would land the remote's messages.
 */
export function isRemoteUpToDate(remote: RemoteCounts): boolean {
  return remote.exists && remote.commitsAhead === 0 && remote.commitsBehind === 0 && !replacesOlderVersions(remote)
}

/**
 * The remote has older versions of commits of the branch (amended here, or reworded: the same changes with another
 * message or author), of its stack ancestors (restacked) or of the baseline (landed)
 */
export function replacesOlderVersions(remote: OlderVersions): boolean {
  return remote.commitsAmended > 0 || remote.commitsRestacked > 0 || remote.commitsLanded > 0 || remote.commitsReworded > 0
}

/**
 * The action of the push button of a branch, null while its remote status is not loaded. The remote status compares
 * content: a branch only rebased onto a newer baseline is up to date, the remote has its work. A push that is not a
 * fast-forward (the remote has commits the branch lacks, older versions of amended or restacked commits, or the branch
 * was rebased) is a force push.
 */
export function getPushAction(branch: RemoteStatusInput): PushAction | null {
  const remoteStatus = branch.remoteStatus
  if (remoteStatus === null) {
    return null
  }
  if (!remoteStatus.exists) {
    return "create"
  }
  if (isRemoteUpToDate(remoteStatus)) {
    return "upToDate"
  }
  if (remoteStatus.commitsBehind > 0 || replacesOlderVersions(remoteStatus) || !remoteStatus.fastForward) {
    return "force"
  }
  return "push"
}

/** Hue of a push: the push buttons, the status texts and the outgoing bars take it from here. */
export type PushHue = Extract<UIColor, "primary" | "warning">

/**
 * Hue of a push, by its risk: `warning` when it drops remote commits the branch lacks, `primary` otherwise. A force push
 * of a rebased branch only replaces the remote commits with their copies, one of an amended or restacked branch their
 * older versions: it is safe.
 */
export function getPushHue(commitsBehind: number): PushHue {
  return commitsBehind > 0 ? "warning" : "primary"
}

/**
 * A push that drops remote commits the branch lacks (the `warning` hue), someone else's work: a click opens its
 * description with the dropped commits first, and the Force Push button there pushes
 */
export function dropsRemoteCommits(remoteStatus: Pick<RemoteStatus, "commitsBehind">): boolean {
  return getPushHue(remoteStatus.commitsBehind) === "warning"
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
  const action = before === null ? null : getPushAction({ remoteStatus: before })
  if (before === null || action === null || action === "upToDate") {
    return { title: `Pushed ${branchName}` }
  }
  // a force push may only replace or drop remote commits
  const parts = before.commitsAhead === 0 ? [] : [formatCommitCount(before.commitsAhead)]
  if (action === "create") {
    return { title: `Pushed ${branchName}`, description: [...parts, "created the remote branch"].join(" · ") }
  }
  if (action === "push") {
    return { title: `Pushed ${branchName}`, description: parts[0] }
  }
  const behind = before.commitsBehind
  const replaced = formatReplaced(before)
  if (behind === 0) {
    return { title: `Force pushed ${branchName}`, description: [...parts, replaced ?? "replaced the rebased copies"].join(" · ") }
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
    const action = before === null ? null : getPushAction({ remoteStatus: before })
    if (before === null || action === null || action === "upToDate") {
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
  const action = getPushAction(branch)
  if (action === null) {
    return null
  }
  const remoteStatus = branch.remoteStatus!
  const hue = getPushHue(remoteStatus.commitsBehind)
  if (action === "upToDate" || (action !== "create" && hue === "primary" && remoteStatus.myCommitsAhead === 0 && !replacesOlderVersions(remoteStatus))) {
    return "neutral"
  }
  return hue
}

/** A branch with something to push. */
export interface OutgoingBranch {
  name: string
  /** Commits whose content the remote lacks: the baseline and the rebased copies of pushed commits don't count */
  count: number
  /** Of them, the commits authored by the user */
  mine: number
  state: OutgoingState
  /** Commits on the remote that a force push drops */
  behind: number
  /** Older versions of commits amended here on the remote, which the push replaces */
  amended: number
  /** Older versions of the commits of the branches it is built on on the remote, which the push replaces */
  restacked: number
}

/**
 * Why a branch with commits is not outgoing: its sync failed, its stack parent blocks it, a running Safe Push holds it,
 * the fetched baseline has all of it (landed), its remote status is not loaded yet, or the remote has it (up to date)
 */
export type PushSkip = "failed" | "blocked" | "held" | "landed" | "notLoaded" | "upToDate"

/** The skips that keep work for the remote from the overview: the branch has commits the remote lacks, or may have */
export type WithheldSkip = Extract<PushSkip, "failed" | "blocked" | "held">

export interface PushSummary {
  /** Commits to push over all branches */
  total: number
  mine: number
  /** In the order of the given branches */
  branches: OutgoingBranch[]
  /** Branch count per state */
  byState: Record<OutgoingState, number>
  /** Branch count per hue */
  byHue: Record<PushHue, number>
  /** Branches that offer a push but whose remote status is not loaded yet */
  notLoaded: number
  /** Why each branch with commits that is not outgoing is not: the overview names it in the row of its pull request */
  skipped: ReadonlyMap<string, PushSkip>
  /**
   * Of them, the branches whose work for the remote the overview leaves out, per reason: a failed sync (unknown), a
   * blocked or a held branch whose remote lacks some of its commits
   */
  withheld: Record<WithheldSkip, number>
}

type SummaryInput = Pick<Branch, "name" | "remoteStatus" | "hasError" | "commitCount"> & { status?: Branch["status"] }

/**
 * What is waiting to be pushed: only the branches whose row offers a push (synced without an error, with commits, not
 * blocked by their stack parent, not held by a running Safe Push, `isHeld`), and not the ones the fetched baseline has
 * all of (`isLanded`): the update archives them.
 */
export function summarizePushState(
  branches: readonly SummaryInput[],
  isLanded: (name: string) => boolean = () => false,
  isHeld: (name: string) => boolean = () => false,
): PushSummary {
  const skipped = new Map<string, PushSkip>()
  const summary: PushSummary = {
    total: 0,
    mine: 0,
    branches: [],
    byState: { push: 0, create: 0, force: 0 },
    byHue: { primary: 0, warning: 0 },
    notLoaded: 0,
    skipped,
    withheld: { failed: 0, blocked: 0, held: 0 },
  }
  for (const branch of branches) {
    if (branch.commitCount === 0) {
      continue
    }
    const action = branch.hasError ? null : getPushAction(branch)
    const skip = skipOf(branch, action, isLanded, isHeld)
    if (skip !== null) {
      skipped.set(branch.name, skip)
      if (skip === "notLoaded") {
        summary.notLoaded++
      } else if (skip === "failed" || ((skip === "blocked" || skip === "held") && action !== "upToDate")) {
        summary.withheld[skip]++
      }
      continue
    }
    if (action === null || action === "upToDate") {
      continue
    }
    const { commitsAhead: count, commitsBehind, commitsAmended, commitsRestacked, myCommitsAhead } = branch.remoteStatus!
    const mine = Math.min(myCommitsAhead, count)
    summary.branches.push({ name: branch.name, count, mine, state: action, behind: commitsBehind, amended: commitsAmended, restacked: commitsRestacked })
    summary.total += count
    summary.mine += mine
    summary.byState[action]++
    summary.byHue[getPushHue(commitsBehind)]++
  }
  return summary
}

/** Why the branch is not outgoing, null when it is: the reasons of a row that offers no push first, as it shows them */
function skipOf(branch: SummaryInput, action: PushAction | null, isLanded: (name: string) => boolean, isHeld: (name: string) => boolean): PushSkip | null {
  if (branch.hasError) {
    return "failed"
  }
  if (branch.status === "Blocked") {
    return "blocked"
  }
  if (isHeld(branch.name)) {
    return "held"
  }
  if (isLanded(branch.name)) {
    return "landed"
  }
  if (action === null) {
    return "notLoaded"
  }
  return action === "upToDate" ? "upToDate" : null
}

/** What a push sends and what it replaces on the remote, `formatPushCount` writes it */
export interface PushCount {
  /** Of the commits to push, the user's */
  mine: number
  /** The commits to push */
  count: number
  /** Older versions of commits amended here */
  amended: number
  /** Older versions of the commits of the branches it is built on */
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
