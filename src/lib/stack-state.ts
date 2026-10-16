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
  /** A running robot will fail: a required check failed ("failing" of the header) */
  readonly failing: number
  /** Another robot runs or cancels: Running of the filter */
  readonly running: number
  /** A push has work for the remote, by the rule of the outgoing counts and To Push (`summarizePushState`) */
  readonly toPush: number
}

/** What the counts read besides the branches, from the stores */
export interface StackStateSources {
  readonly robots: Readonly<Record<string, PatronusRobot>> | undefined
  /** The robots that failed for missing approvals (`useRobotsNeedingReview`) */
  readonly needsReview: Readonly<Record<string, boolean>>
  /** The running robots that will fail (`useFailingRobots`) */
  readonly failing: Readonly<Record<string, true>>
  /** The fetched baseline has every commit of the branch (`useIsFullyLanded`) */
  readonly isLanded: (name: string) => boolean
}

/** A state of `StackState` a branch is in, but the push */
export type StackStateKey = Exclude<keyof StackState, "toPush">

/** The states of `StackState` the branch is in, but the push: a sync failure, a block, its robot */
function branchStates(branch: Branch, sources: StackStateSources): StackStateKey[] {
  const states: StackStateKey[] = []
  if (branch.errorDetails?.MergeConflict != null) {
    states.push("conflicts")
  }
  else if (branch.hasError) {
    states.push("errors")
  }
  if (branch.status === "Blocked") {
    states.push("blocked")
  }
  const localTip = branch.commits[0]?.hash
  const robot = findBranchRobot(sources.robots, branch.name, localTip, branch.remoteStatus?.remoteTip ?? null)
  if (robot !== undefined) {
    if (robot.status === "running" && sources.failing[robot.id] === true) {
      states.push("failing")
    }
    else if (isRobotActive(robot)) {
      states.push("running")
    }
    else if (needsRobotAttention(robot, localTip, branch.remoteStatus)) {
      states.push(sources.needsReview[robot.id] === true ? "needsReview" : "failed")
    }
  }
  return states
}

export function summarizeStackState(branches: readonly Branch[], sources: StackStateSources): StackState {
  const counts: Record<StackStateKey, number> = { conflicts: 0, errors: 0, blocked: 0, failed: 0, needsReview: 0, failing: 0, running: 0 }
  for (const branch of branches) {
    for (const state of branchStates(branch, sources)) {
      counts[state]++
    }
  }
  const toPush = summarizePushState(branches, sources.isLanded).branches.length
  return { ...counts, toPush }
}

/** The first of `branches` (in their order) in each state of `StackState`: where a click on its count goes */
export function firstBranchOfEachState(branches: readonly Branch[], sources: StackStateSources): Partial<Record<keyof StackState, string>> {
  const first: Partial<Record<keyof StackState, string>> = {}
  for (const branch of branches) {
    for (const state of branchStates(branch, sources)) {
      first[state] ??= branch.name
    }
  }
  const toPush = summarizePushState(branches, sources.isLanded).branches[0]
  if (toPush !== undefined) {
    first.toPush = toPush.name
  }
  return first
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

/** A state of a stack with its count, for its chip: "1 failed" */
export interface StackStateChip extends StackAttention {
  readonly state: keyof StackState
}

/**
 * The states the branches hold, with their counts, the strongest first: first what waits for the user (a conflict, an
 * error, a block, a failed robot, one that needs review, a running one that will fail), then what runs, then the push
 */
export function getStackStateChips(state: StackState): StackStateChip[] {
  const chips: StackStateChip[] = []
  const add = (key: keyof StackState, text: string, color: StackAttention["color"], ring = false) => {
    if (state[key] > 0) {
      chips.push({ state: key, text, color, ring })
    }
  }
  add("conflicts", plural(state.conflicts, "conflict", "conflicts"), "error")
  add("errors", plural(state.errors, "error", "errors"), "error")
  add("blocked", `${state.blocked} blocked`, "warning")
  add("failed", `${state.failed} failed`, "error")
  add("needsReview", `${state.needsReview} ${state.needsReview === 1 ? "needs" : "need"} review`, "warning")
  add("failing", `${state.failing} failing`, "error", true)
  add("running", `${state.running} running`, "primary", true)
  add("toPush", `${state.toPush} to push`, "primary")
  return chips
}

/** The strongest state, with its count (`getStackStateChips`); null when the rows hold none of them */
export function getStackAttention(state: StackState): StackAttention | null {
  const strongest = getStackStateChips(state)[0]
  return strongest === undefined ? null : { text: strongest.text, color: strongest.color, ring: strongest.ring }
}
