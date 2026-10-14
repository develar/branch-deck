import type { UIColor } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"

/** What a push of a branch does: a plain push, the first push that creates the remote branch, a force push. */
export type OutgoingState = "push" | "create" | "force"

/** What the push button of a branch does; `upToDate` when there is nothing to push. */
export type PushAction = OutgoingState | "upToDate"

/**
 * The action of the push button of a branch, null while its remote status is not loaded.
 * A rewritten branch (`Updated`) needs a force push even if the remote has no commit the branch lacks.
 */
export function getPushAction(branch: Pick<Branch, "remoteStatus" | "status">): PushAction | null {
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
  if (remoteStatus.commitsBehind > 0 || branch.status === "Updated") {
    return "force"
  }
  return "push"
}

/**
 * Colour of a push, by its risk: `primary` for the safe pushes (push, create remote), `warning` for a force push, which
 * overwrites remote commits. The push buttons, the status texts and the outgoing bars take their hue from here.
 */
export const pushStateColor: Readonly<Record<OutgoingState, Extract<UIColor, "primary" | "warning">>> = {
  push: "primary",
  create: "primary",
  force: "warning",
}

/**
 * Colour of the push button and the status text of a branch; null while its remote status is not loaded.
 * Neutral when up to date, and for a plain push of no commit of the user: there is nothing of theirs to push.
 */
export function getPushColor(branch: Pick<Branch, "remoteStatus" | "status">): UIColor | null {
  const action = getPushAction(branch)
  if (action === null) {
    return null
  }
  if (action === "upToDate" || (action === "push" && branch.remoteStatus!.myCommitsAhead === 0)) {
    return "neutral"
  }
  return pushStateColor[action]
}

/** A branch with something to push. */
export interface OutgoingBranch {
  name: string
  /** Commits the push sends */
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
  /** Branches that offer a push but whose remote status is not loaded yet */
  notLoaded: number
}

type SummaryInput = Pick<Branch, "name" | "remoteStatus" | "status" | "hasError" | "commitCount">

/** What is waiting to be pushed: only the branches whose row offers a push (synced without an error, with commits). */
export function summarizePushState(branches: readonly SummaryInput[]): PushSummary {
  const summary: PushSummary = { total: 0, mine: 0, branches: [], byState: { push: 0, create: 0, force: 0 }, notLoaded: 0 }
  for (const branch of branches) {
    if (branch.hasError || branch.commitCount === 0) {
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
    const { commitsAhead, commitsBehind, myCommitsAhead } = branch.remoteStatus!
    // a branch not on the remote reports no commits ahead: the push sends all its commits
    const count = action === "create" && commitsAhead === 0 ? branch.commitCount : commitsAhead
    const mine = Math.min(myCommitsAhead, count)
    summary.branches.push({ name: branch.name, count, mine, state: action, behind: commitsBehind })
    summary.total += count
    summary.mine += mine
    summary.byState[action]++
  }
  return summary
}

/** `3 (5)↑`, then `· 2↓` for the remote commits a force push drops, as the status text of a branch row. */
export function formatOutgoingCount(branch: OutgoingBranch): string {
  const ahead = `${branch.mine} (${branch.count})↑`
  return branch.behind > 0 ? `${ahead} · ${branch.behind}↓` : ahead
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`
}

/** "12 commits to push in 4 branches" */
export function formatPushSummaryTitle(summary: PushSummary): string {
  return `${plural(summary.total, "commit", "commits")} to push in ${plural(summary.branches.length, "branch", "branches")}`
}

/** Fills of an outgoing bar: the hue of the state, solid for the user's commits, light for the others'. */
export interface OutgoingBarFill {
  mine: string
  others: string
}

const barFill: Readonly<Record<Extract<UIColor, "primary" | "warning">, OutgoingBarFill>> = {
  primary: { mine: "bg-primary", others: "bg-primary/40" },
  warning: { mine: "bg-warning", others: "bg-warning/40" },
}

export function outgoingBarFill(state: OutgoingState): OutgoingBarFill {
  return barFill[pushStateColor[state]]
}

export interface OutgoingLegendEntry {
  state: OutgoingState
  label: string
}

/** A swatch per colour of the states present: a first push is a push, its row says "new". */
export function getOutgoingLegend(summary: PushSummary): OutgoingLegendEntry[] {
  const entries: OutgoingLegendEntry[] = []
  if (summary.byState.push > 0 || summary.byState.create > 0) {
    entries.push({ state: summary.byState.push > 0 ? "push" : "create", label: "push" })
  }
  if (summary.byState.force > 0) {
    entries.push({ state: "force", label: "force push" })
  }
  return entries
}

/** One segment of the header bar */
export interface OutgoingBarSegment {
  key: string
  state: OutgoingState
  /** Relative width: the commits, at least 1 so a force push that only drops remote commits stays visible */
  weight: number
  /** Of the weight, the solid part of the user's commits; all of it for a force push that sends no commit */
  mine: number
}

/** More branches than this make segments too thin to tell apart: the bar then has one segment per state. */
export const MAX_BAR_SEGMENTS = 8

const stateOrder: readonly OutgoingState[] = ["push", "create", "force"]

/** The segments of the header bar: one per branch, in the order of the rows, or one per state for many branches. */
export function getOutgoingBarSegments(summary: PushSummary): OutgoingBarSegment[] {
  if (summary.branches.length <= MAX_BAR_SEGMENTS) {
    return summary.branches.map(branch => ({ key: branch.name, state: branch.state, ...barWeight(branch) }))
  }
  return stateOrder
    .filter(state => summary.byState[state] > 0)
    .map((state) => {
      const segment: OutgoingBarSegment = { key: state, state, weight: 0, mine: 0 }
      for (const branch of summary.branches) {
        if (branch.state === state) {
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
