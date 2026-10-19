import type { PatronusRobot } from "@/lib/bindings"
import { findBranchRobot, findStackRobot, isRobotActive, needsBranchRobotAttention, needsStackRobotAttention } from "@/lib/patronus"
import { summarizePushState } from "@/lib/push-summary"
import type { Branch, StackBranch } from "@/stores/branch-sync"

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
  /** A running Safe Push holds the branch (`usePushHolds`): its row offers no push */
  readonly isHeld?: (name: string) => boolean
}

/** A state of `StackState` a branch is in, but the push */
export type StackStateKey = Exclude<keyof StackState, "toPush">

/**
 * The stack branch of a stack that forks (`StackBranch`): the robot of the whole stack runs on it (`findStackRobot`),
 * no branch's, and lands every branch of the stack; its failure counts only while the sync built it (`state`)
 */
export type WholeStackSource = Pick<StackBranch, "root" | "state" | "tip" | "remoteStatus">

/** The state of a robot: failing, running, or failed on what it is matched by as it is (`needsAttention`) */
function robotState(robot: PatronusRobot | undefined, needsAttention: (robot: PatronusRobot) => boolean, sources: StackStateSources): StackStateKey | null {
  if (robot === undefined) {
    return null
  }
  if (robot.status === "running" && sources.failing[robot.id] === true) {
    return "failing"
  }
  if (isRobotActive(robot)) {
    return "running"
  }
  if (needsAttention(robot)) {
    return sources.needsReview[robot.id] === true ? "needsReview" : "failed"
  }
  return null
}

/** Another robot holds or landed the branch, or the baseline has it (`needsBranchRobotAttention`) */
function isCarried(name: string, sources: StackStateSources): boolean {
  return sources.isHeld?.(name) === true || sources.isLanded(name)
}

/** The states of `StackState` the branch is in, but the push: a sync failure, a block, its robot */
function branchStates(branch: Branch, sources: StackStateSources): StackStateKey[] {
  const states: StackStateKey[] = []
  if (branch.errorDetails?.MergeConflict != null) {
    states.push("conflicts")
  } else if (branch.hasError) {
    states.push("errors")
  }
  if (branch.status === "Blocked") {
    states.push("blocked")
  }
  const localTip = branch.commits[0]?.hash
  const robot = robotState(
    findBranchRobot(sources.robots, branch.name, localTip, branch.remoteStatus?.remoteTip ?? null),
    it => needsBranchRobotAttention(it, localTip, branch.remoteStatus, isCarried(branch.name, sources)),
    sources,
  )
  if (robot !== null) {
    states.push(robot)
  }
  return states
}

/**
 * The state of the robot of the whole stack, on its stack branch as it is, its failure only on one the sync built
 * (`needsStackRobotAttention`); none for a stack without a stack branch
 */
function wholeStackState(stack: WholeStackSource | undefined, sources: StackStateSources): StackStateKey | null {
  if (stack === undefined) {
    return null
  }
  const robot = findStackRobot(sources.robots, stack.root, stack.tip, stack.remoteStatus?.remoteTip ?? null)
  return robotState(robot, it => needsStackRobotAttention(it, stack), sources)
}

/**
 * What `branches` hold, and the robot of the whole stack on its stack branch (`stack`), which runs on no branch: "1
 * running" while it runs, as the island's header has it
 */
export function summarizeStackState(branches: readonly Branch[], sources: StackStateSources, stack?: WholeStackSource): StackState {
  const counts: Record<StackStateKey, number> = { conflicts: 0, errors: 0, blocked: 0, failed: 0, needsReview: 0, failing: 0, running: 0 }
  for (const branch of branches) {
    for (const state of branchStates(branch, sources)) {
      counts[state]++
    }
  }
  const whole = wholeStackState(stack, sources)
  if (whole !== null) {
    counts[whole]++
  }
  const toPush = summarizePushState(branches, sources.isLanded, sources.isHeld).branches.length
  return { ...counts, toPush }
}

/**
 * The first of `branches` (in their order) in each state of `StackState`: where a click on its count goes; the root for
 * the robot of the whole stack (`stack`), as the landing overview shows it
 */
export function firstBranchOfEachState(branches: readonly Branch[], sources: StackStateSources, stack?: WholeStackSource): Partial<Record<keyof StackState, string>> {
  const first: Partial<Record<keyof StackState, string>> = {}
  const whole = wholeStackState(stack, sources)
  if (whole !== null && stack !== undefined) {
    first[whole] = stack.root
  }
  for (const branch of branches) {
    for (const state of branchStates(branch, sources)) {
      first[state] ??= branch.name
    }
  }
  const toPush = summarizePushState(branches, sources.isLanded, sources.isHeld).branches[0]
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
