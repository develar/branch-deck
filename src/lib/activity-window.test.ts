// @spec spec/repository/activity.spec.md
import { describe, expect, it } from "vite-plus/test"
import { activityDiagnostics, activityFilterOf, countActivityFilters, filterActivity } from "@/lib/activity-window"
import type { ActivityFilter } from "@/lib/activity-window"
import type { ActivityEntry } from "@/stores/activity"

function entry(id: string, kind: ActivityEntry["kind"], title: string, description?: string, agent = false): ActivityEntry {
  return {
    id,
    repositoryPath: "/repo",
    time: 0,
    kind,
    title,
    ...(description !== undefined && { description }),
    ...(agent && { agent: { worktree: "/pool/repo-1/1/repo" }, operationId: `op-${id}` }),
  }
}

const entries = [
  entry("1", "patronus", "auth merged into master", "Safe Push"),
  entry("2", "push", "Pushed ABC-123", "4 commits"),
  entry("3", "landed", "2 branches landed in origin/master", "auth and search"),
  entry("4", "update", "Didn't update master"),
  entry("5", "error", "Cannot push login"),
]

/** The entries of the chip `filter` with the text `query`, without the agents' */
const shownBy = (filter: ActivityFilter | null, query: string) => ({ filter, query, agents: false })

describe("filterActivity", () => {
  it("keeps the entries of the chip whose title or description has the text, ignoring case", () => {
    expect(filterActivity(entries, shownBy(null, "")).map(it => it.id)).toEqual(["1", "2", "3", "4", "5"])
    // an update and what it landed are one chip
    expect(filterActivity(entries, shownBy("updates", "")).map(it => it.id)).toEqual(["3", "4"])
    expect(filterActivity(entries, shownBy(null, " AUTH ")).map(it => it.id)).toEqual(["1", "3"])
    expect(filterActivity(entries, shownBy("robots", "abc"))).toEqual([])
    expect(activityFilterOf({ kind: "rewrite" })).toBe("rewrites")
  })

  it("counts the entries of each chip that have the text, and of all", () => {
    expect(countActivityFilters(entries, shownBy(null, ""))).toEqual({ all: 5, robots: 1, pushes: 1, updates: 2, reviews: 0, rewrites: 0, errors: 1, agents: 0 })
    expect(countActivityFilters(entries, shownBy(null, "auth"))).toEqual({ all: 2, robots: 1, pushes: 0, updates: 1, reviews: 0, rewrites: 0, errors: 0, agents: 0 })
  })

  it("shows the operations of the agents only behind their chip, which counts what its press adds", () => {
    const withAgents = [...entries, entry("6", "push", "Pushed docs", "1 commit", true), entry("7", "error", "Cannot push auth", undefined, true)]
    expect(filterActivity(withAgents, shownBy(null, "")).map(it => it.id)).toEqual(["1", "2", "3", "4", "5"])
    expect(filterActivity(withAgents, { filter: null, query: "", agents: true }).map(it => it.id)).toEqual(["1", "2", "3", "4", "5", "6", "7"])
    expect(filterActivity(withAgents, { filter: "pushes", query: "", agents: true }).map(it => it.id)).toEqual(["2", "6"])
    // the kinds count the agents' only while they show; Agents counts the agents' of the chosen chip and the text
    expect(countActivityFilters(withAgents, shownBy(null, ""))).toEqual({ all: 5, robots: 1, pushes: 1, updates: 2, reviews: 0, rewrites: 0, errors: 1, agents: 2 })
    expect(countActivityFilters(withAgents, { filter: null, query: "", agents: true })).toEqual({
      all: 7,
      robots: 1,
      pushes: 2,
      updates: 2,
      reviews: 0,
      rewrites: 0,
      errors: 2,
      agents: 2,
    })
    expect(countActivityFilters(withAgents, shownBy("pushes", "")).agents).toBe(1)
    expect(countActivityFilters(withAgents, shownBy(null, "auth")).agents).toBe(1)
  })
})

describe("activityDiagnostics", () => {
  it("copies the words of an entry, when it ended and its operation as the lines of the log name it, with the worktree of an agent", () => {
    expect(activityDiagnostics({ ...entry("6", "push", "Pushed docs", "1 commit", true), time: Date.UTC(2026, 9, 27, 10, 30) })).toBe(
      "Pushed docs\n1 commit\n2026-10-27T10:30:00.000Z operation_id=op-6\nagent worktree=/pool/repo-1/1/repo",
    )
    expect(activityDiagnostics({ ...entry("5", "error", "Cannot push login"), operationId: "op-5" })).toBe("Cannot push login\n1970-01-01T00:00:00.000Z operation_id=op-5")
    // an entry of no operation has none
    expect(activityDiagnostics(entry("4", "update", "Didn't update master"))).toBeNull()
  })
})
