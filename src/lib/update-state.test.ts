// @spec spec/repository/update-project.spec.md
import { describe, expect, it } from "vitest"
import type { UpdateCheck } from "@/lib/bindings"
import { describeUpdateState, lastFetchTime } from "@/lib/update-state"

describe("describeUpdateState", () => {
  it("names the state, never a count of upstream commits", () => {
    expect(describeUpdateState({ status: "upToDate" }, "origin/master")).toEqual({ tone: "muted", summary: "Up to date with origin/master", short: "up to date" })
    expect(describeUpdateState({ status: "clean", kind: "rebase" }, "origin/master")).toEqual({ tone: "success", summary: "Rebases cleanly onto origin/master", short: "rebases cleanly" })
    expect(describeUpdateState({ status: "clean", kind: "fastForward" }, "origin/main").summary).toBe("Fast-forwards to origin/main")
  })

  it("names at most three files", () => {
    const one = describeUpdateState({ status: "conflict", files: ["a.txt"], commits: [] }, "origin/master")
    expect(one).toEqual({ tone: "warning", summary: "Conflicts in 1 file: a.txt", short: "conflicts in 1 file" })
    const five = describeUpdateState({ status: "conflict", files: ["a", "b", "c", "d", "e"], commits: [] }, "origin/master")
    expect(five.summary).toBe("Conflicts in 5 files: a, b, c and 2 more")
    expect(describeUpdateState({ status: "blocked", paths: ["src/app.txt"] }, "origin/master")).toEqual({
      tone: "warning",
      summary: "Your uncommitted changes in src/app.txt overlap origin/master",
      short: "blocked by your uncommitted changes",
    })
  })
})

describe("lastFetchTime", () => {
  const update = (fetchedAt: number | null): UpdateCheck => ({ branch: "m", upstream: "o/m", head: "", upstreamHead: "", base: "", state: { status: "upToDate" }, fetchedAt })

  it("takes the newer of Branch Deck's fetch and one elsewhere", () => {
    expect(lastFetchTime(update(100), 50_000)).toBe(100_000)
    expect(lastFetchTime(update(100), 150_000)).toBe(150_000)
    expect(lastFetchTime(update(null), 150_000)).toBe(150_000)
    expect(lastFetchTime(null, undefined)).toBeNull()
  })
})
