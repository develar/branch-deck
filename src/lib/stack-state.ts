import type { PatronusRobot } from "@/lib/bindings"
import { findBranchRobot, isRobotActive, needsRobotAttention } from "@/lib/patronus"
import { summarizePushState } from "@/lib/push-summary"
import type { Branch } from "@/stores/branch-sync"

/** What the branches of a stack hold, each counted by the rule of the surface that tells it for all branches */
export interface StackState {
  readonly conflicts: number
  /** Sync errors other than a conflict */
  readonly errors: number
  /** Blocked by their stack parent */
  readonly blocked: number
  /** A robot failed on the branch as it is (`needsRobotAttention`), not for missing approvals: "failed" of the header */
  readonly failed: number
  /** A robot failed on the branch as it is for missing code-owner approvals: "needs review" of the header */
  readonly needsReview: number
  /** A robot runs or cancels: Running of the filter */
  readonly running: number
  /** A push has work for the remote, by the rule of the outgoing counts and To Push (`summarizePushState`) */
  readonly toPush: number
}

/** What the counts read besides the branches, from the stores */
export interface StackStateSources {
  readonly robots: Readonly<Record<string, PatronusRobot>> | undefined
  /** The robots that failed for missing approvals (`useRobotsNeedingReview`) */
  readonly needsReview: Readonly<Record<string, boolean>>
  /** The fetched baseline has every commit of the branch (`useIsFullyLanded`) */
  readonly isLanded: (name: string) => boolean
}

export function summarizeStackState(branches: readonly Branch[], sources: StackStateSources): StackState {
  let conflicts = 0
  let errors = 0
  let blocked = 0
  let failed = 0
  let needsReview = 0
  let running = 0
  for (const branch of branches) {
    if (branch.errorDetails?.MergeConflict != null) {
      conflicts++
    }
    else if (branch.hasError) {
      errors++
    }
    if (branch.status === "Blocked") {
      blocked++
    }
    const localTip = branch.commits[0]?.hash
    const robot = findBranchRobot(sources.robots, branch.name, localTip, branch.remoteStatus?.remoteTip ?? null)
    if (robot === undefined) {
      continue
    }
    if (isRobotActive(robot)) {
      running++
    }
    else if (needsRobotAttention(robot, localTip, branch.remoteStatus)) {
      if (sources.needsReview[robot.id] === true) {
        needsReview++
      }
      else {
        failed++
      }
    }
  }
  const toPush = summarizePushState(branches, sources.isLanded).branches.length
  return { conflicts, errors, blocked, failed, needsReview, running, toPush }
}

/** The strongest state of a collapsed stack's hidden rows, for its chip: "· 1 failed" */
export interface StackAttention {
  readonly text: string
  /** `error` a failure, `warning` attention, `primary` a robot that runs or work to push */
  readonly color: "error" | "warning" | "primary"
  /** A robot runs: the chip shows its ring */
  readonly ring: boolean
}

function plural(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`
}

/**
 * The strongest state, with its count: first what waits for the user (a conflict, an error, a block, a failed robot,
 * one that needs review), then what runs, then the push; null when the rows hold none of them
 */
export function getStackAttention(state: StackState): StackAttention | null {
  if (state.conflicts > 0) {
    return { text: plural(state.conflicts, "conflict", "conflicts"), color: "error", ring: false }
  }
  if (state.errors > 0) {
    return { text: plural(state.errors, "error", "errors"), color: "error", ring: false }
  }
  if (state.blocked > 0) {
    return { text: `${state.blocked} blocked`, color: "warning", ring: false }
  }
  if (state.failed > 0) {
    return { text: `${state.failed} failed`, color: "error", ring: false }
  }
  if (state.needsReview > 0) {
    return { text: `${state.needsReview} ${state.needsReview === 1 ? "needs" : "need"} review`, color: "warning", ring: false }
  }
  if (state.running > 0) {
    return { text: `${state.running} running`, color: "primary", ring: true }
  }
  if (state.toPush > 0) {
    return { text: `${state.toPush} to push`, color: "primary", ring: false }
  }
  return null
}
