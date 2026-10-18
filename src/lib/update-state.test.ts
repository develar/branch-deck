// @spec spec/repository/update-project.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { TransferProgress, UpdateCheck } from "@/lib/bindings"
import { advanceRun, startRun } from "@/lib/update-progress"
import {
  describeAsFetched,
  describeUpdateState,
  fetchAndUpdateLine,
  lastFetchTime,
  runFooterWords,
  runLabel,
  runningStepSentence,
  runStepLines,
  updateFooterWords,
} from "@/lib/update-state"

describe("describeUpdateState", () => {
  it("names the state, never a count of upstream commits", () => {
    expect(describeUpdateState({ status: "upToDate" }, "origin/master")).toEqual({ tone: "muted", summary: "Up to date with origin/master", short: "nothing new" })
    expect(describeUpdateState({ status: "clean", kind: "rebase" }, "origin/master")).toEqual({
      tone: "success",
      summary: "Rebases cleanly onto origin/master",
      short: "rebases cleanly",
    })
    expect(describeUpdateState({ status: "clean", kind: "fastForward" }, "origin/main").summary).toBe("Fast-forwards to origin/main")
  })

  it("names at most three files, four of four: never 'and 1 more'", () => {
    const one = describeUpdateState({ status: "conflict", files: ["a.txt"], fileCount: 1, commits: [], upstreamCommits: [] }, "origin/master")
    expect(one).toEqual({ tone: "warning", summary: "Conflicts in 1 file: a.txt", short: "conflicts in 1 file" })
    const five = describeUpdateState({ status: "conflict", files: ["a", "b", "c", "d", "e"], fileCount: 5, commits: [], upstreamCommits: [] }, "origin/master")
    expect(five.summary).toBe("Conflicts in 5 files: a, b, c and 2 more")
    const four = describeUpdateState({ status: "conflict", files: ["a", "b", "c", "d"], fileCount: 4, commits: [], upstreamCommits: [] }, "origin/master")
    expect(four.summary).toBe("Conflicts in 4 files: a, b, c, d")
    expect(describeUpdateState({ status: "blocked", paths: ["src/app.txt"], pathCount: 1 }, "origin/master")).toEqual({
      tone: "warning",
      summary: "Your uncommitted changes in src/app.txt overlap origin/master",
      short: "blocked by your uncommitted changes",
    })
  })

  it("counts the files the backend left out of its list", () => {
    const names = Array.from({ length: 50 }, (_, i) => `f${i}`)
    const wide = describeUpdateState({ status: "conflict", files: names, fileCount: 300, commits: [], upstreamCommits: [] }, "origin/master")
    expect(wide).toEqual({ tone: "warning", summary: "Conflicts in 300 files: f0, f1, f2 and 297 more", short: "conflicts in 300 files" })
    const blocked = describeUpdateState({ status: "blocked", paths: names, pathCount: 120 }, "origin/master")
    expect(blocked.summary).toBe("Your uncommitted changes in f0, f1, f2 and 117 more overlap origin/master")
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

describe("describeAsFetched", () => {
  const upToDate = describeUpdateState({ status: "upToDate" }, "origin/master")
  const clean = describeUpdateState({ status: "clean", kind: "rebase" }, "origin/master")
  const conflict = describeUpdateState({ status: "conflict", files: ["a.txt"], fileCount: 1, commits: [], upstreamCommits: [] }, "origin/master")

  it("says the baseline was not fetched yet instead of a state nobody asked the remote for", () => {
    const fetched = Date.now()
    expect(describeAsFetched(clean, "origin/master", fetched)).toBe(clean)
    expect(fetchAndUpdateLine(fetched)).toBe("Fetched just now · fetch and update")

    const notFetched = { tone: "muted", summary: "origin/master not fetched yet", short: "not fetched yet" }
    expect(describeAsFetched(upToDate, "origin/master", null)).toEqual(notFetched)
    expect(describeAsFetched(clean, "origin/master", null)).toEqual(notFetched)
    // a warning keeps its words: its icon shows
    expect(describeAsFetched(conflict, "origin/master", null)).toEqual({
      tone: "warning",
      summary: "Conflicts in 1 file: a.txt · origin/master not fetched yet",
      short: "conflicts in 1 file · not fetched yet",
    })
    expect(fetchAndUpdateLine(null)).toBe("fetch and update")
  })

  it("says the same in the footer of the landing overview", () => {
    expect(updateFooterWords(clean, Date.now(), "your wsm landed")).toBe("rebases cleanly · your wsm landed · fetched just now")
    expect(updateFooterWords(describeAsFetched(upToDate, "origin/master", null), null, null)).toBe("not fetched yet")
  })
})

describe("the labels of a running update", () => {
  const receiving: TransferProgress = { percent: 51, transferred: "2.61 MiB", speed: "5.20 MiB/s", line: "Receiving objects:  45% (136/302), 2.61 MiB | 5.20 MiB/s" }
  const fetching = startRun("update", ["fetch", "check", "apply", "sync"], undefined, 0)

  it("names the whole run with one word and its percent, the toast's Fetch as a fetch", () => {
    expect(runLabel(fetching, 0)).toBe("Updating…")
    expect(runLabel(fetching, 0.624)).toBe("Updating 62%")
    // 100 only once it ended: the bar shows that
    expect(runLabel(fetching, 0.999)).toBe("Updating 99%")
    expect(runLabel(startRun("fetch", ["fetch"], undefined, 0), 0.3)).toBe("Fetching 30%")
  })

  it("names the running step in the tooltip, never the last state", () => {
    expect(runningStepSentence(fetching, "origin/master")).toBe("Fetching origin/master…")
    expect(runningStepSentence({ ...fetching, fetch: receiving }, "origin/master")).toBe(receiving.line)
    const waiting = advanceRun(fetching, "waiting", fetching.plan, 1000)
    expect(runningStepSentence(waiting, "origin/master")).toBe("Waiting for Branch Deck or bd to finish…")
    expect(runningStepSentence(advanceRun(waiting, "check", fetching.plan, 2000), "origin/master")).toBe("Checking the update onto origin/master…")
  })

  it("lists the steps done, the running one and those left, without a wait that ended", () => {
    const received = { ...fetching, fetch: receiving }
    const waiting = advanceRun(received, "waiting", received.plan, 1000, "2.61 MiB")
    const checking = advanceRun(waiting, "check", ["fetch", "check"], 2000)
    expect(runStepLines(checking, "origin/master")).toEqual([
      { step: "fetch", state: "done", text: "Fetched origin/master · 2.61 MiB" },
      { step: "check", state: "running", text: "Checking the update onto origin/master…" },
    ])
    expect(runStepLines(fetching, "origin/master").map(it => `${it.state} ${it.text}`)).toEqual([
      "running Fetching origin/master…",
      "next Check the update",
      "next Update to origin/master",
      "next Sync branches",
    ])
  })

  it("says the step in the footer, the amount and the rate while the fetch receives", () => {
    expect(runFooterWords(fetching)).toBe("fetching…")
    expect(runFooterWords({ ...fetching, fetch: receiving })).toBe("2.61 MiB · 5.20 MiB/s")
    expect(runFooterWords(advanceRun(fetching, "check", fetching.plan, 1000))).toBe("checking the update…")
  })
})
