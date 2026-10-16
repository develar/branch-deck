import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { PatronusRobotProgress } from "@/lib/bindings"

/*
 * The estimate of the CI for the running robots whose details are open (`usePatronusRobotDetails`): while one is, the
 * slot of its row and its progress line may use it; once the last of its details closes it is gone, and they fall
 * back to the estimate from history. In memory only, never a request of its own.
 */

/** A progress the details read, and when (`Date.now()`) */
export interface ReadRobotProgress {
  readonly progress: PatronusRobotProgress
  readonly readAt: number
}

interface RobotProgressState {
  /** By robot id */
  readonly byRobot: Readonly<Record<string, ReadRobotProgress>>
}

export const robotProgressStore = createStore<RobotProgressState>(() => ({ byRobot: {} }))

/** By robot id: how many open details want its progress */
const holders = new Map<string, number>()

/**
 * An open details of a running robot wants its progress; the returned function lets it go, the last one forgets the
 * progress
 */
export function holdRobotProgress(robotId: string): () => void {
  holders.set(robotId, (holders.get(robotId) ?? 0) + 1)
  return () => {
    const count = (holders.get(robotId) ?? 1) - 1
    if (count > 0) {
      holders.set(robotId, count)
      return
    }
    holders.delete(robotId)
    robotProgressStore.setState(state => {
      const { [robotId]: _, ...rest } = state.byRobot
      return { byRobot: rest }
    })
  }
}

/** The progress an answer of the details had, kept while they are open */
export function setRobotProgress(robotId: string, progress: PatronusRobotProgress | null): void {
  if (!holders.has(robotId)) {
    return
  }
  robotProgressStore.setState(state => {
    if (progress === null) {
      const { [robotId]: _, ...rest } = state.byRobot
      return { byRobot: rest }
    }
    return { byRobot: { ...state.byRobot, [robotId]: { progress, readAt: Date.now() } } }
  })
}

/** The progress of a robot while one of its details is open; undefined otherwise */
export function useReadRobotProgress(robotId: string): ReadRobotProgress | undefined {
  return useStore(robotProgressStore, state => state.byRobot[robotId])
}
