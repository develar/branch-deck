import { z } from "zod"
import type { PatronusRobot } from "@/lib/bindings"
import { isRobotActive } from "@/lib/patronus"
import type { RobotSample } from "@/lib/robot-eta"
import { isMainWindow } from "@/stores/settings/app-store"
import { createPersistentStore } from "@/stores/settings/persistent-store"

/*
 * The finished robots the app saw, with the merge profile when an answer named it: what the estimate of a running robot
 * learns from (`robot-eta.ts`), also after a restart and beyond the 7 days a list of Patronus reaches. Persisted by the
 * main window, at most `MAX_ROBOT_HISTORY` entries and `MAX_ROBOT_HISTORY_AGE_MS`. No request is made for it.
 */

/** At most this many robots over all repositories; the ones that finished first go first */
export const MAX_ROBOT_HISTORY = 200
/** Robots that finished longer ago go */
export const MAX_ROBOT_HISTORY_AGE_MS = 30 * 24 * 60 * 60 * 1000

const RobotHistoryEntrySchema = z.object({
  id: z.string(),
  repositoryPath: z.string(),
  /** The merge profile id; null: no profile; absent: not known (a list doesn't name it) */
  profileId: z.string().nullable().optional(),
  pushMode: z.string(),
  targetBranch: z.string(),
  /** Unix seconds */
  startTime: z.number(),
  finishTime: z.number(),
  status: z.string(),
})

export type RobotHistoryEntry = z.infer<typeof RobotHistoryEntrySchema>

const RobotHistorySchema = z.array(RobotHistoryEntrySchema).default([])

export const robotHistoryStore = createPersistentStore("robotHistory", { schema: RobotHistorySchema, isArray: true, isMainOnly: true })

/** The entries within the age and the count limits, the newest finish first */
export function pruneRobotHistory(items: readonly RobotHistoryEntry[], now: number): RobotHistoryEntry[] {
  return [...items]
    .filter(entry => now - entry.finishTime * 1000 < MAX_ROBOT_HISTORY_AGE_MS)
    .sort((a, b) => b.finishTime - a.finishTime)
    .slice(0, MAX_ROBOT_HISTORY)
}

function toEntry(repositoryPath: string, robot: PatronusRobot): RobotHistoryEntry | null {
  if (isRobotActive(robot) || robot.finishTime === null || robot.startTime <= 0) {
    return null
  }
  return {
    id: robot.id,
    repositoryPath,
    ...(robot.profile != null && { profileId: robot.profile.id }),
    pushMode: robot.pushMode,
    targetBranch: robot.targetBranch,
    startTime: robot.startTime,
    finishTime: robot.finishTime,
    status: robot.status,
  }
}

/**
 * The history with the finished robots of an answer: a new robot is added, a known one gets the profile an answer named
 * since; the same array when nothing changed
 */
export function mergeRobotHistory(items: readonly RobotHistoryEntry[], repositoryPath: string, robots: Iterable<PatronusRobot>, now: number): readonly RobotHistoryEntry[] {
  const byId = new Map(items.map(entry => [entry.id, entry]))
  let changed = false
  for (const robot of robots) {
    const entry = toEntry(repositoryPath, robot)
    if (entry === null) {
      continue
    }
    const known = byId.get(entry.id)
    if (known === undefined || (known.profileId === undefined && entry.profileId !== undefined) || known.status !== entry.status) {
      byId.set(entry.id, known === undefined || entry.profileId !== undefined ? entry : { ...entry, profileId: known.profileId })
      changed = true
    }
  }
  return changed ? pruneRobotHistory([...byId.values()], now) : items
}

/** Records the finished robots of an answer for the estimates; the main window only, which owns the history */
export function recordFinishedRobots(repositoryPath: string, robots: Iterable<PatronusRobot>): void {
  if (!isMainWindow || repositoryPath === "") {
    return
  }
  const { items } = robotHistoryStore.getState()
  const next = mergeRobotHistory(items, repositoryPath, robots, Date.now())
  if (next !== items) {
    robotHistoryStore.setState({ items: [...next] })
  }
}

/** The history of a repository as samples of the estimate */
export function historySamples(items: readonly RobotHistoryEntry[], repositoryPath: string): RobotSample[] {
  return items
    .filter(entry => entry.repositoryPath === repositoryPath)
    .map(entry => ({
      id: entry.id,
      status: entry.status,
      pushMode: entry.pushMode,
      targetBranch: entry.targetBranch,
      startTime: entry.startTime,
      finishTime: entry.finishTime,
      profileId: entry.profileId,
    }))
}
