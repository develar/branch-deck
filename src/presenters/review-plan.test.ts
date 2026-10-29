import { describe, expect, it } from "vite-plus/test"
import type { ApiChange, FileRisk, Interdiff, PairedCommit } from "@/lib/bindings"
import {
  formatFanIn,
  formatMarkedReviewed,
  formatPairState,
  formatPlanSummary,
  formatRewrittenChanges,
  formatSinceReview,
  presentApiChanges,
  presentRiskyFiles,
} from "@/presenters/review-plan"

function paired(state: PairedCommit["state"], id: string): PairedCommit {
  return { id: state === "dropped" ? null : id, reviewedId: state === "added" ? null : `r${id}`, subject: `subject ${id}`, state, changes: null }
}

function change(patch: Partial<ApiChange>): ApiChange {
  return { path: "src/Store.kt", line: 34, name: "helper", key: "helper", kind: "changed", changes: [], before: null, after: null, fanIn: null, ...patch }
}

function risk(patch: Partial<FileRisk>): FileRisk {
  return { path: "src/Store.kt", maxFanIn: null, untested: false, risky: false, ...patch }
}

describe("the words of the commits since the review", () => {
  it("says New and Rewritten in the column, nothing of an unchanged or a dropped commit", () => {
    expect(formatPairState("added")).toBe("New")
    expect(formatPairState("rewritten")).toBe("Rewritten")
    expect(formatPairState("unchanged")).toBeNull()
    expect(formatPairState("dropped")).toBeNull()
  })

  it("counts what changed since the review, and nothing when only the baseline moved", () => {
    const interdiff = (commits: PairedCommit[]): Interdiff => ({ reviewedBase: "b", reviewedTip: "t", commits, scope: null })
    expect(formatSinceReview(interdiff([paired("unchanged", "1"), paired("rewritten", "2"), paired("added", "3"), paired("added", "4"), paired("dropped", "5")]))).toBe(
      "2 new · 1 rewritten · 1 dropped",
    )
    expect(formatSinceReview(interdiff([paired("unchanged", "1")]))).toBeNull()
  })

  it("says per path what a rewritten commit changes that its reviewed version did not, and the reverse", () => {
    expect(
      formatRewrittenChanges({
        newOnly: [{ path: "src/Store.kt", lines: ["+fun helper(value: Int, scale: Int)", "+  = value * scale"] }],
        reviewedOnly: [
          { path: "src/Store.kt", lines: ["-fun helper(value: Int) = value"] },
          { path: "README.md", lines: ["-old"] },
        ],
      }),
    ).toBe("src/Store.kt +2 −1, README.md +0 −1")
    expect(formatRewrittenChanges({ newOnly: [], reviewedOnly: [] })).toBe("Only its message changed")
  })
})

describe("the API of the card", () => {
  it("words the usages, a cut answer as more, none without a fan-in", () => {
    expect(formatFanIn({ usages: 3, complete: true })).toBe("3 usages")
    expect(formatFanIn({ usages: 1, complete: true })).toBe("1 usage")
    expect(formatFanIn({ usages: 100, complete: false })).toBe("100+ usages")
    expect(formatFanIn(null)).toBeNull()
  })

  it("keeps the order Rust ranked, with the kind, the place, the parts that changed and the forms of a removal or a change", () => {
    const entries = presentApiChanges([
      change({ kind: "removed", name: "open", key: "a.open", path: "src/a/Api.kt", line: 3, before: "fun open(path:String)", fanIn: { usages: 12, complete: true } }),
      change({ changes: ["parameters", "reach"], before: "fun helper(value:Int)", after: "fun helper(value:Int, scale:Int)", fanIn: { usages: 3, complete: true } }),
      change({ kind: "added", name: "Cache", key: "b.Cache", path: "src/b/Cache.kt", line: 3, after: "class Cache" }),
    ])
    expect(entries.map(({ key: _key, ...entry }) => entry)).toEqual([
      { name: "open", kind: "Removed", tone: "destructive", location: "src/a/Api.kt:3", changed: null, before: "fun open(path:String)", after: null, fanIn: "12 usages" },
      {
        name: "helper",
        kind: "Changed",
        tone: "warning",
        location: "src/Store.kt:34",
        changed: "parameters, visibility",
        before: "fun helper(value:Int)",
        after: "fun helper(value:Int, scale:Int)",
        fanIn: "3 usages",
      },
      { name: "Cache", kind: "Added", tone: "info", location: "src/b/Cache.kt:3", changed: null, before: null, after: null, fanIn: null },
    ])
    expect(new Set(entries.map(entry => entry.key)).size).toBe(3)
  })
})

describe("the risky files of the card", () => {
  it("raises only the risky files, in Rust's order, with the usages and the missing test", () => {
    expect(
      presentRiskyFiles([
        risk({ path: "src/Store.kt", maxFanIn: 3, untested: true, risky: true }),
        risk({ path: "src/a/Api.kt", maxFanIn: 1, risky: true }),
        risk({ path: "src/b/Cache.kt", untested: true, risky: true }),
        risk({ path: "src/a/Reader.kt", maxFanIn: 0 }),
      ]),
    ).toEqual([
      { path: "src/Store.kt", reasons: "An API used in 3 places · No test changed" },
      { path: "src/a/Api.kt", reasons: "An API used in 1 place" },
      { path: "src/b/Cache.kt", reasons: "No test changed" },
    ])
  })

  it("sums the card up in its title", () => {
    expect(formatPlanSummary({ api: [change({})], risk: [risk({ risky: true }), risk({ path: "b", risky: true }), risk({ path: "c" })] })).toBe("1 API change · 2 risky files")
    expect(formatPlanSummary({ api: [], risk: [risk({})] })).toBe("No API changes")
    expect(formatPlanSummary({ api: [change({}), change({})], risk: [risk({ risky: true })] })).toBe("2 API changes · 1 risky file")
  })
})

describe("Mark Reviewed", () => {
  it("says the branch is marked, or that it was at this commit already", () => {
    expect(formatMarkedReviewed("plan", true)).toBe("Marked plan reviewed")
    expect(formatMarkedReviewed("plan", false)).toBe("plan was already marked reviewed at this commit")
  })
})
