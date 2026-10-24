import { Channel } from "@tauri-apps/api/core"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { commands } from "@/lib/bindings"
import type { RobotStats, RobotStatsEvent, RobotStatsResult, StatsPeriod } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"

export interface RobotStatsProgress {
  readonly lists_read: number
  readonly lists_total: number
  readonly runs_read: number
  readonly runs_total: number
}

/** The Safe Push statistics of a repository as the Landing view shows them */
export interface RobotStatsEntry {
  /** The last answer; kept while the next read runs, so nothing moves until it is there */
  readonly result: RobotStatsResult | null
  /** The numbers of the lists of the read running, before the checks are read */
  readonly partial: RobotStats | null
  readonly progress: RobotStatsProgress | null
  readonly loading: boolean
  /** The command failed (not Patronus: the repository, its baseline) */
  readonly error: string | null
}

export const IDLE_ROBOT_STATS: RobotStatsEntry = { result: null, partial: null, progress: null, loading: false, error: null }

/** By repository path; what `get_robot_stats` answered. No timer reads: the view asks while it shows */
const robotStatsStore = createStore<{ byRepository: Readonly<Record<string, RobotStatsEntry>> }>(() => ({ byRepository: {} }))

/** The read whose answer counts, by repository: an older one that answers later is dropped */
const latestReads = new Map<string, number>()

function update(repositoryPath: string, change: (entry: RobotStatsEntry) => RobotStatsEntry): void {
  robotStatsStore.setState(state => ({ byRepository: { ...state.byRepository, [repositoryPath]: change(state.byRepository[repositoryPath] ?? IDLE_ROBOT_STATS) } }))
}

export interface RobotStatsSubject {
  readonly repositoryPath: string
  readonly baselineBranch: string
  readonly period: StatsPeriod
}

/**
 * Reads the statistics of `subject` (`get_robot_stats`): Rust decides whether Patronus is asked, its lists holding 5
 * minutes; `refresh` asks again whatever their age and tells a failure (the user asked), a read nobody asked for keeps
 * its failure in the entry
 */
export function readRobotStats(subject: RobotStatsSubject, refresh: boolean): void {
  const { repositoryPath } = subject
  const id = (latestReads.get(repositoryPath) ?? 0) + 1
  latestReads.set(repositoryPath, id)
  const isCurrent = () => latestReads.get(repositoryPath) === id
  update(repositoryPath, entry => ({ ...entry, partial: null, loading: true, error: null }))
  const events = new Channel<RobotStatsEvent>()
  events.onmessage = event => {
    if (isCurrent()) {
      update(repositoryPath, entry => (event.type === "progress" ? { ...entry, progress: event.data } : { ...entry, partial: event.data }))
    }
  }
  const params = { repository_path: repositoryPath, baseline_branch: subject.baselineBranch, period: subject.period, refresh }
  void commands.getRobotStats(params, events).then(answer => {
    if (!isCurrent()) {
      return
    }
    if (answer.status === "error") {
      update(repositoryPath, entry => ({ ...entry, loading: false, progress: null, error: answer.error.message }))
      if (refresh) {
        notifyError("Cannot refresh the Safe Push statistics", answer.error)
      }
      return
    }
    update(repositoryPath, () => ({ result: answer.data, partial: null, progress: null, loading: false, error: null }))
    if (refresh && answer.data.availability.status === "error") {
      notifyError("Cannot refresh the Safe Push statistics", answer.data.availability.message)
    }
  })
}

export function useRobotStatsEntry(repositoryPath: string | null): RobotStatsEntry {
  return useStore(robotStatsStore, state => (repositoryPath === null ? IDLE_ROBOT_STATS : (state.byRepository[repositoryPath] ?? IDLE_ROBOT_STATS)))
}
