// @spec spec/branch/patronus.spec.md
import { describe, expect, it, vi } from "vitest"
import type { PatronusRobot } from "@/lib/bindings"
import type { RobotHistoryEntry } from "@/stores/robot-history"
import { historySamples, MAX_ROBOT_HISTORY, MAX_ROBOT_HISTORY_AGE_MS, mergeRobotHistory, recordFinishedRobots, robotHistoryStore } from "@/stores/robot-history"

// the window of a unit test is the main window, which owns the history (`isMainWindow`)
vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))

const NOW = 1_704_189_600_000
const NOW_SECONDS = NOW / 1000

function robot(id: string, patch: Partial<PatronusRobot> = {}): PatronusRobot {
  return {
    id,
    name: id,
    status: "success",
    pushMode: "rebaseAutosquash",
    branchName: id,
    sourceRevision: "tip",
    targetBranch: "master",
    startTime: NOW_SECONDS - 3600,
    finishTime: NOW_SECONDS - 600,
    cancellationReason: null,
    url: "u",
    ...patch,
  }
}

describe("robot history", () => {
  it("adds the finished robots of an answer, never a running one, and learns a profile an answer named since", () => {
    const listed = mergeRobotHistory([], "/a", [robot("r1"), robot("running", { status: "running", finishTime: null })], NOW)
    // a list doesn't name the profile: not known
    expect(listed).toEqual([expect.objectContaining({ id: "r1", repositoryPath: "/a", status: "success" })])
    expect(listed[0]).not.toHaveProperty("profileId")
    const polled = mergeRobotHistory(listed, "/a", [robot("r1", { profile: { id: "lsc", name: "Large Scale Change" } })], NOW)
    expect(polled[0]?.profileId).toBe("lsc")
    // a later list keeps it, and nothing changed: the same array
    expect(mergeRobotHistory(polled, "/a", [robot("r1")], NOW)).toBe(polled)
    // a robot without a profile is known as such
    expect(mergeRobotHistory([], "/a", [robot("r2", { profile: { id: null, name: null } })], NOW)[0]?.profileId).toBeNull()
  })

  it("keeps at most 200 robots and none that finished more than 30 days ago, the newest first", () => {
    const many = Array.from({ length: MAX_ROBOT_HISTORY + 5 }, (_, index) => robot(`r${index}`, { finishTime: NOW_SECONDS - index }))
    const history = mergeRobotHistory([], "/a", many, NOW)
    expect(history).toHaveLength(MAX_ROBOT_HISTORY)
    expect(history[0]?.id).toBe("r0")
    const old: RobotHistoryEntry = { id: "old", repositoryPath: "/a", pushMode: "rebaseAutosquash", targetBranch: "master", startTime: 1, finishTime: (NOW - MAX_ROBOT_HISTORY_AGE_MS) / 1000 - 1, status: "success" }
    expect(mergeRobotHistory([old], "/a", [robot("new")], NOW).map(it => it.id)).toEqual(["new"])
  })

  it("records the robots of a repository and gives them back as samples of its estimates", () => {
    vi.useFakeTimers({ now: NOW })
    recordFinishedRobots("/b", [robot("b1", { profile: { id: "lsc", name: "Large Scale Change" } })])
    vi.useRealTimers()
    const samples = historySamples(robotHistoryStore.getState().items, "/b")
    expect(samples).toEqual([expect.objectContaining({ id: "b1", profileId: "lsc", finishTime: NOW_SECONDS - 600 })])
    expect(historySamples(robotHistoryStore.getState().items, "/c")).toEqual([])
  })
})
