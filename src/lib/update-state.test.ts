// @spec spec/repository/update-project.spec.md
import { describe, expect, it } from "vitest"
import type { FetchProgress, UpdateCheck } from "@/lib/bindings"
import { describeUpdateState, fetchTransfer, lastFetchTime, runningUpdateSummary, updatePhaseLabel } from "@/lib/update-state"

describe("describeUpdateState", () => {
  it("names the state, never a count of upstream commits", () => {
    expect(describeUpdateState({ status: "upToDate" }, "origin/master")).toEqual({ tone: "muted", summary: "Up to date with origin/master", short: "nothing new" })
    expect(describeUpdateState({ status: "clean", kind: "rebase" }, "origin/master")).toEqual({ tone: "success", summary: "Rebases cleanly onto origin/master", short: "rebases cleanly" })
    expect(describeUpdateState({ status: "clean", kind: "fastForward" }, "origin/main").summary).toBe("Fast-forwards to origin/main")
  })

  it("names at most three files", () => {
    const one = describeUpdateState({ status: "conflict", files: ["a.txt"], commits: [], upstreamCommits: [] }, "origin/master")
    expect(one).toEqual({ tone: "warning", summary: "Conflicts in 1 file: a.txt", short: "conflicts in 1 file" })
    const five = describeUpdateState({ status: "conflict", files: ["a", "b", "c", "d", "e"], commits: [], upstreamCommits: [] }, "origin/master")
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

describe("updatePhaseLabel", () => {
  const receiving: FetchProgress = { percent: 51, received: "2.61 MiB", speed: "5.20 MiB/s", line: "Receiving objects:  45% (136/302), 2.61 MiB | 5.20 MiB/s" }

  it("shows the percent of the fetch once git reports a stage", () => {
    expect(updatePhaseLabel("fetching", null)).toBe("Fetching…")
    expect(updatePhaseLabel("fetching", receiving)).toBe("Fetching 51%")
    expect(updatePhaseLabel("checking", null)).toBe("Checking…")
  })

  it("shows the amount and the rate as git prints them", () => {
    expect(fetchTransfer(null)).toBeNull()
    expect(fetchTransfer({ ...receiving, received: null, speed: null })).toBeNull()
    expect(fetchTransfer(receiving)).toBe("2.61 MiB · 5.20 MiB/s")
  })

  it("names the running step in the tooltip, never the last state", () => {
    expect(runningUpdateSummary("fetching", null, "origin/master")).toBe("Fetching origin/master…")
    expect(runningUpdateSummary("fetching", receiving, "origin/master")).toBe(receiving.line)
    expect(runningUpdateSummary("checking", null, "origin/master")).toBe("Checking the update onto origin/master…")
    expect(runningUpdateSummary("updating", null, "origin/master")).toBe("Updating to origin/master…")
  })
})
