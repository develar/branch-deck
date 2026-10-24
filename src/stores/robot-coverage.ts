import { useMemo } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { PatronusRobot, RobotCommitMap, RobotCommitState } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { LandingDescendant } from "@/lib/patronus"
import { findBranchRobot } from "@/lib/patronus"
import type { RobotCoverage } from "@/lib/robot-coverage"
import { branchCoverageOf, robotCoverageNeed } from "@/lib/robot-coverage"
import { robotAncestors } from "@/lib/robot-prompt-copy"
import type { BranchSyncState, RemoteStatus } from "@/stores/branch-sync"
import { branchSyncStore, useBranchSync } from "@/stores/branch-sync"
import { currentKey, getLandingSpan, patronusRequest, patronusStore, useLandingSpan, useRepositoryRobots } from "@/stores/patronus"

/**
 * By `coverageKey`, the commit map of a running robot (`get_robot_commit_map`), asked once the branch it tests changed
 * since it started (`watchRobotCoverage`); null: no answer, the rows keep their plain dots
 */
interface RobotCoverageState {
  readonly maps: Readonly<Record<string, RobotCommitMap | null>>
}

export const robotCoverageStore = createStore<RobotCoverageState>(() => ({ maps: {} }))

/** The running robot that tests a branch, with the tip and the remote status of the branch it took (its subject) */
export interface CoveringRobot {
  readonly robot: PatronusRobot
  /** The local tip of the virtual branch the robot tests, or of the stack branch of a whole stack */
  readonly tip: string | null | undefined
  readonly remote: RemoteStatus | null
}

/**
 * The running robot that tests the commits of the branch: its own, else the one that lands it with a branch built on
 * it or with the whole stack (`span`, `useLandingSpan`), as the ring and the landing span of the graph show; null while
 * none runs
 */
export function coveringRobotIn(robots: Readonly<Record<string, PatronusRobot>>, span: LandingDescendant | null, state: BranchSyncState, branchName: string): CoveringRobot | null {
  const branch = state.branches[branchName]
  if (branch === undefined) {
    return null
  }
  const own = findBranchRobot(robots, branchName, branch.commits[0]?.hash, branch.remoteStatus?.remoteTip ?? null)
  if (own?.status === "running") {
    return { robot: own, tip: branch.commits[0]?.hash, remote: branch.remoteStatus }
  }
  if (span === null || span.robot.status !== "running") {
    return null
  }
  if (span.wholeStack === true) {
    const stack = state.stackBranches[span.branch]
    return stack === undefined ? null : { robot: span.robot, tip: stack.tip, remote: stack.remoteStatus }
  }
  const subject = state.branches[span.branch]
  return subject === undefined ? null : { robot: span.robot, tip: subject.commits[0]?.hash, remote: subject.remoteStatus }
}

/** The map of a robot for the tip its branch has now: a sync that changes the branch asks again */
function coverageKey(covering: CoveringRobot): string {
  return `${covering.robot.id}\n${covering.robot.sourceRevision ?? ""}\n${covering.tip ?? ""}`
}

/** The coverage of a branch as one string, so that a row renders again only when it changes */
function coverageLine(
  robots: Readonly<Record<string, PatronusRobot>> | undefined,
  span: LandingDescendant | null,
  maps: RobotCoverageState["maps"],
  state: BranchSyncState,
  branchName: string,
): string {
  const covering = robots === undefined ? null : coveringRobotIn(robots, span, state, branchName)
  const need = covering === null ? null : robotCoverageNeed(covering.robot, covering.tip, covering.remote)
  if (covering === null || need === null) {
    return ""
  }
  if (need === "all") {
    return "all"
  }
  const map = maps[coverageKey(covering)]
  if (map == null) {
    return ""
  }
  return [...branchCoverageOf(map, branchName)].map(([hash, state]) => `${hash}\t${state}`).join("\n")
}

/**
 * Which commits of the branch the running robot that tests it covers (`RobotCoverage`); null while none runs or nothing
 * is known yet
 */
export function useRobotCoverage(branchName: string): RobotCoverage | null {
  const robots = useRepositoryRobots()
  const span = useLandingSpan(branchName)
  const maps = useStore(robotCoverageStore, state => state.maps)
  const line = useBranchSync(state => coverageLine(robots, span, maps, state, branchName))
  return useMemo(() => {
    if (line === "") {
      return null
    }
    if (line === "all") {
      return "all"
    }
    return new Map(line.split("\n").map(it => it.split("\t") as [string, RobotCommitState]))
  }, [line])
}

/** The running robot that tests the commits of the branch (`coveringRobotIn`); re-renders only when it changes */
export function useCoveringRobot(branchName: string): PatronusRobot | null {
  const robots = useRepositoryRobots()
  const span = useLandingSpan(branchName)
  return useBranchSync(state => (robots === undefined ? null : (coveringRobotIn(robots, span, state, branchName)?.robot ?? null)))
}

/** The requests asked and not answered yet, by `coverageKey` */
const inFlight = new Set<string>()

/**
 * Asks `get_robot_commit_map` for each running robot whose branch changed since it started, once per tip of that
 * branch (`coverageKey`), never while a sync runs: the virtual branches are the sync's. The maps of robots no branch
 * needs any more go.
 */
async function askRobotCoverage(): Promise<void> {
  const key = currentKey()
  const state = branchSyncStore.getState()
  const repository = key === null ? undefined : patronusStore.getState().repositories[key]
  if (key === null || repository === undefined || state.isSyncing || state.baselineBranch === null) {
    return
  }
  const needed = new Map<string, CoveringRobot>()
  for (const name of Object.keys(state.branches)) {
    const covering = coveringRobotIn(repository.robots, getLandingSpan(name), state, name)
    if (covering !== null && robotCoverageNeed(covering.robot, covering.tip, covering.remote) === "map") {
      needed.set(coverageKey(covering), covering)
    }
  }
  const { maps } = robotCoverageStore.getState()
  if (Object.keys(maps).some(it => !needed.has(it))) {
    robotCoverageStore.setState({ maps: Object.fromEntries(Object.entries(maps).filter(([it]) => needed.has(it))) })
  }
  const { repositoryPath } = patronusRequest()
  const baselineBranch = state.baselineBranch
  await Promise.all(
    [...needed]
      .filter(([it]) => !(it in robotCoverageStore.getState().maps) && !inFlight.has(it))
      .map(async ([coverage, { robot }]) => {
        inFlight.add(coverage)
        try {
          const result = await commands
            .getRobotCommitMap({
              repositoryPath,
              branchName: robot.branchName,
              stackRoot: robot.stackRoot ?? null,
              sourceRevision: robot.sourceRevision ?? "",
              ancestors: robotAncestors(robot),
              baselineBranch,
            })
            .catch((error: unknown) => ({ status: "error" as const, error }))
          if (result.status === "error") {
            console.warn("Cannot read which commits the robot covers", result.error)
          }
          robotCoverageStore.setState(it => ({ maps: { ...it.maps, [coverage]: result.status === "ok" ? result.data : null } }))
        } finally {
          inFlight.delete(coverage)
        }
      }),
  )
}

/**
 * Keeps the commit maps of the running robots current (`askRobotCoverage`) while the main window shows the branches:
 * on every change of the robots and of the branches. Returns the unsubscribe.
 */
export function watchRobotCoverage(): () => void {
  let scheduled = false
  const schedule = () => {
    if (scheduled) {
      return
    }
    scheduled = true
    queueMicrotask(() => {
      scheduled = false
      void askRobotCoverage()
    })
  }
  const unsubscribes = [patronusStore, branchSyncStore].map(store => store.subscribe(schedule))
  schedule()
  return () => unsubscribes.forEach(unsubscribe => unsubscribe())
}
