import type { UIColor } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"

/** What a push of a branch does: a plain push, the first push that creates the remote branch, a force push. */
export type OutgoingState = "push" | "create" | "force"

/** What the push button of a branch does; `upToDate` when there is nothing to push. */
export type PushAction = OutgoingState | "upToDate"

type RemoteStatusInput = Pick<Branch, "remoteStatus">

/**
 * The action of the push button of a branch, null while its remote status is not loaded. The remote status compares
 * content: a branch only rebased onto a newer baseline is up to date, the remote has its work. A push that is not a
 * fast-forward (the remote has commits the branch lacks, or the branch was rebased) is a force push.
 */
export function getPushAction(branch: RemoteStatusInput): PushAction | null {
  const remoteStatus = branch.remoteStatus
  if (remoteStatus === null) {
    return null
  }
  if (!remoteStatus.exists) {
    return "create"
  }
  if (remoteStatus.commitsAhead === 0 && remoteStatus.commitsBehind === 0) {
    return "upToDate"
  }
  if (remoteStatus.commitsBehind > 0 || !remoteStatus.fastForward) {
    return "force"
  }
  return "push"
}

/** Hue of a push: the push buttons, the status texts and the outgoing bars take it from here. */
export type PushHue = Extract<UIColor, "primary" | "warning">

/**
 * Hue of a push, by its risk: `warning` when it drops remote commits the branch lacks, `primary` otherwise. A force push
 * of a rebased branch only replaces the remote commits with their copies: it is safe.
 */
export function getPushHue(commitsBehind: number): PushHue {
  return commitsBehind > 0 ? "warning" : "primary"
}

/**
 * Colour of the push button and the status text of a branch; null while its remote status is not loaded.
 * Neutral when up to date, and for a safe push of no commit of the user: there is nothing of theirs to push.
 */
export function getPushColor(branch: RemoteStatusInput): UIColor | null {
  const action = getPushAction(branch)
  if (action === null) {
    return null
  }
  const remoteStatus = branch.remoteStatus!
  const hue = getPushHue(remoteStatus.commitsBehind)
  if (action === "upToDate" || (action !== "create" && hue === "primary" && remoteStatus.myCommitsAhead === 0)) {
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
}

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
}

type SummaryInput = Pick<Branch, "name" | "remoteStatus" | "hasError" | "commitCount"> & { status?: Branch["status"] }

/**
 * What is waiting to be pushed: only the branches whose row offers a push (synced without an error, with commits, not
 * blocked by their stack parent).
 */
export function summarizePushState(branches: readonly SummaryInput[]): PushSummary {
  const summary: PushSummary = {
    total: 0,
    mine: 0,
    branches: [],
    byState: { push: 0, create: 0, force: 0 },
    byHue: { primary: 0, warning: 0 },
    notLoaded: 0,
  }
  for (const branch of branches) {
    if (branch.hasError || branch.commitCount === 0 || branch.status === "Blocked") {
      continue
    }
    const action = getPushAction(branch)
    if (action === null) {
      summary.notLoaded++
      continue
    }
    if (action === "upToDate") {
      continue
    }
    const { commitsAhead: count, commitsBehind, myCommitsAhead } = branch.remoteStatus!
    const mine = Math.min(myCommitsAhead, count)
    summary.branches.push({ name: branch.name, count, mine, state: action, behind: commitsBehind })
    summary.total += count
    summary.mine += mine
    summary.byState[action]++
    summary.byHue[getPushHue(commitsBehind)]++
  }
  return summary
}

/**
 * The size of a push, as the status text of a branch row: `3↑` when the 3 commits are the user's, `1 (3)↑` when some
 * are others' (`0 (3)↑`: none is theirs), then `· 2↓` for the remote commits the push drops; `2↓` alone when it sends
 * no commit.
 */
export function formatPushCount(mine: number, count: number, behind: number): string {
  const ahead = mine === count ? `${count}↑` : `${mine} (${count})↑`
  if (behind === 0) {
    return ahead
  }
  return count === 0 ? `${behind}↓` : `${ahead} · ${behind}↓`
}

/** The size of an outgoing push, `formatPushCount` */
export function formatOutgoingCount(branch: OutgoingBranch): string {
  return formatPushCount(branch.mine, branch.count, branch.behind)
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`
}

/** "12 commits to push in 4 branches" */
export function formatPushSummaryTitle(summary: PushSummary): string {
  return `${plural(summary.total, "commit", "commits")} to push in ${plural(summary.branches.length, "branch", "branches")}`
}

/** The push summary title; with nothing to push, "Nothing to push, 2 pull requests open" */
export function formatOutgoingTitle(summary: PushSummary, openPullRequests: number): string {
  if (summary.branches.length > 0) {
    return formatPushSummaryTitle(summary)
  }
  return `Nothing to push, ${plural(openPullRequests, "pull request", "pull requests")} open`
}

/** Fills of an outgoing bar: the hue of the push, solid for the user's commits, light for the others'. */
export interface OutgoingBarFill {
  mine: string
  others: string
}

const barFill: Readonly<Record<PushHue, OutgoingBarFill>> = {
  primary: { mine: "bg-primary", others: "bg-primary/40" },
  warning: { mine: "bg-warning", others: "bg-warning/40" },
}

export function outgoingBarFill(hue: PushHue): OutgoingBarFill {
  return barFill[hue]
}

export interface OutgoingLegendEntry {
  hue: PushHue
  label: string
}

/** A swatch per hue present: the state words of the rows ("new", "force") say how each push goes. */
export function getOutgoingLegend(summary: PushSummary): OutgoingLegendEntry[] {
  const entries: OutgoingLegendEntry[] = []
  if (summary.byHue.primary > 0) {
    entries.push({ hue: "primary", label: "push" })
  }
  if (summary.byHue.warning > 0) {
    entries.push({ hue: "warning", label: "drops remote commits" })
  }
  return entries
}

/** One segment of the header bar */
export interface OutgoingBarSegment {
  key: string
  hue: PushHue
  /** Relative width: the commits, at least 1 so a force push that only drops remote commits stays visible */
  weight: number
  /** Of the weight, the solid part of the user's commits; all of it for a force push that sends no commit */
  mine: number
}

/** More branches than this make segments too thin to tell apart: the bar then has one segment per hue. */
export const MAX_BAR_SEGMENTS = 8

const hueOrder: readonly PushHue[] = ["primary", "warning"]

/** The segments of the header bar: one per branch, in the order of the rows, or one per hue for many branches. */
export function getOutgoingBarSegments(summary: PushSummary): OutgoingBarSegment[] {
  if (summary.branches.length <= MAX_BAR_SEGMENTS) {
    return summary.branches.map(branch => ({ key: branch.name, hue: getPushHue(branch.behind), ...barWeight(branch) }))
  }
  return hueOrder
    .filter(hue => summary.byHue[hue] > 0)
    .map((hue) => {
      const segment: OutgoingBarSegment = { key: hue, hue, weight: 0, mine: 0 }
      for (const branch of summary.branches) {
        if (getPushHue(branch.behind) === hue) {
          const { weight, mine } = barWeight(branch)
          segment.weight += weight
          segment.mine += mine
        }
      }
      return segment
    })
}

function barWeight(branch: OutgoingBranch): { weight: number, mine: number } {
  return branch.count === 0 ? { weight: 1, mine: 1 } : { weight: branch.count, mine: branch.mine }
}
