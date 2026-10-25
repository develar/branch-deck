import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { Fact, FactsChanges, FactsView, PatronusRobotDetails, PatronusRobotProgress } from "@/lib/bindings"
import { showRobotDetails } from "@/model/demand"
import { currentKey, repositoryKey } from "@/stores/patronus"
import { getEffectiveBranchPrefix } from "@/stores/repository"

/*
 * The details of the robots this window shows (the robot popover, a row of the landing overview, a toast that names the
 * failed checks): the checks, the problems, the merge profile and, of a running robot, the estimate of the CI. A fact of
 * the session of the repository (`facts.robotDetails`), which reads them while a surface holds its demand
 * (`showRobotDetails`): at once, again when the robot's status changes and every minute while it runs
 * (`remote_facts::scheduler`), and drops them once no surface does. What they name beyond the list goes onto the robots
 * of the session (the profile, a robot that will fail), so the page only shows them. The estimate of the CI is kept
 * with the time the page got it (`progress`), for the slot of its row while the details are shown.
 */

/** An estimate of the CI the details brought, and when (`Date.now()`) the page got it */
export interface ReadRobotProgress {
  readonly progress: PatronusRobotProgress
  readonly readAt: number
}

interface RobotDetailsState {
  /** By `repositoryKey`, by robot id, as the view of the session has them */
  readonly repositories: Readonly<Record<string, Readonly<Record<string, Fact<PatronusRobotDetails>>>>>
  /** By robot id, the estimate of the CI of each running robot whose details are shown */
  readonly progress: Readonly<Record<string, ReadRobotProgress>>
}

export const robotDetailsStore = createStore<RobotDetailsState>(() => ({ repositories: {}, progress: {} }))

const NONE: Readonly<Record<string, Fact<PatronusRobotDetails>>> = {}

/**
 * The details of the session of `repositoryPath` as its view has them now (`facts.robotDetails`), with the patch that
 * brought them (`change`; null for a snapshot): an entry the patch doesn't name keeps its object
 */
export function applyRobotDetailsFacts(repositoryPath: string, facts: FactsView, change: FactsChanges | null): void {
  if (change !== null && change.robotDetails == null && change.removedRobotDetails == null) {
    return
  }
  const key = repositoryKey(repositoryPath, getEffectiveBranchPrefix())
  const state = robotDetailsStore.getState()
  const before = state.repositories[key] ?? NONE
  const details = facts.robotDetails
  if (before === details) {
    return
  }
  // the robots of the other repositories keep theirs; of these, the details that came now are read now
  const progress: Record<string, ReadRobotProgress> = Object.fromEntries(Object.entries(state.progress).filter(([id]) => before[id] === undefined))
  const now = Date.now()
  for (const [id, fact] of Object.entries(details)) {
    const read = fact.value?.progress
    if (read != null) {
      const kept = fact.value === before[id]?.value ? state.progress[id] : undefined
      progress[id] = kept ?? { progress: read, readAt: now }
    }
  }
  robotDetailsStore.setState({ repositories: { ...state.repositories, [key]: details }, progress })
}

/** The details of a robot of the selected repository the session has now; null while it has none */
function detailsNow(robotId: string): Fact<PatronusRobotDetails> | null {
  const key = currentKey()
  return key === null ? null : (robotDetailsStore.getState().repositories[key]?.[robotId] ?? null)
}

/** The details of a robot of the selected repository as its view has them; null while it has none (no answer yet) */
export function useRobotDetailsFact(robotId: string): PatronusRobotDetails | null {
  return useStore(robotDetailsStore, () => detailsNow(robotId)?.value ?? null)
}

/**
 * The details of the robot `robotId` of the selected repository: demanded until the session answered (or couldn't),
 * then let go; null when it has none to give (Patronus failed, the window shows another repository meanwhile). The
 * details an open popover has already are taken as they are (the service keeps them a minute, as it did for a read)
 */
export function readRobotDetails(robotId: string): Promise<PatronusRobotDetails | null> {
  const key = currentKey()
  if (key === null) {
    return Promise.resolve(null)
  }
  const answered = (): PatronusRobotDetails | null | undefined => {
    if (currentKey() !== key) {
      return null
    }
    const fact = detailsNow(robotId)
    if (fact === null || fact.state.kind === "loading" || (fact.state.kind === "stale" && fact.state.reason == null)) {
      return undefined
    }
    return fact.value
  }
  const release = showRobotDetails(robotId)
  return new Promise<PatronusRobotDetails | null>(resolve => {
    const settle = (): boolean => {
      const details = answered()
      if (details === undefined) {
        return false
      }
      resolve(details)
      return true
    }
    if (settle()) {
      return
    }
    const unsubscribe = robotDetailsStore.subscribe(() => {
      if (settle()) {
        unsubscribe()
      }
    })
  }).finally(release)
}

/** The estimate of the CI of a robot while its details are shown; undefined otherwise */
export function useReadRobotProgress(robotId: string): ReadRobotProgress | undefined {
  return useStore(robotDetailsStore, state => state.progress[robotId])
}

/** Forgets the details (tests) */
export function resetRobotDetails(): void {
  robotDetailsStore.setState({ repositories: {}, progress: {} })
}
