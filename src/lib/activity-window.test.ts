// @spec spec/repository/activity.spec.md
import { describe, expect, it } from "vite-plus/test"
import { activityFilterOf, countActivityFilters, filterActivity } from "@/lib/activity-window"
import type { ActivityEntry } from "@/stores/activity"

function entry(id: string, kind: ActivityEntry["kind"], title: string, description?: string): ActivityEntry {
  return { id, repositoryPath: "/repo", time: 0, kind, title, ...(description !== undefined && { description }) }
}

const entries = [
  entry("1", "patronus", "auth merged into master", "Safe Push"),
  entry("2", "push", "Pushed ABC-123", "4 commits"),
  entry("3", "landed", "2 branches landed in origin/master", "auth and search"),
  entry("4", "update", "Didn't update master"),
  entry("5", "error", "Cannot push login"),
]

describe("filterActivity", () => {
  it("keeps the entries of the chip whose title or description has the text, ignoring case", () => {
    expect(filterActivity(entries, null, "").map(it => it.id)).toEqual(["1", "2", "3", "4", "5"])
    // an update and what it landed are one chip
    expect(filterActivity(entries, "updates", "").map(it => it.id)).toEqual(["3", "4"])
    expect(filterActivity(entries, null, " AUTH ").map(it => it.id)).toEqual(["1", "3"])
    expect(filterActivity(entries, "robots", "abc")).toEqual([])
    expect(activityFilterOf({ kind: "rewrite" })).toBe("rewrites")
  })

  it("counts the entries of each chip that have the text, and of all", () => {
    expect(countActivityFilters(entries, "")).toEqual({ all: 5, robots: 1, pushes: 1, updates: 2, reviews: 0, rewrites: 0, errors: 1 })
    expect(countActivityFilters(entries, "auth")).toEqual({ all: 2, robots: 1, pushes: 0, updates: 1, reviews: 0, rewrites: 0, errors: 0 })
  })
})
