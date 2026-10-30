import { describe, expect, it } from "vite-plus/test"
import type { ApiChange, DependencyChange, FileFacts, FileLink, Interdiff, PairedCommit, Usages } from "@/lib/bindings"
import {
  formatDependencyChange,
  formatMarkedReviewed,
  formatMechanicalGroup,
  formatPairState,
  formatPlanSummary,
  formatRewrittenChanges,
  formatSinceReview,
  formatUsages,
  presentApiChanges,
  presentDependencyBadge,
  presentFileBadges,
  presentRiskyFiles,
} from "@/presenters/review-plan"

function paired(state: PairedCommit["state"], id: string): PairedCommit {
  return { id: state === "dropped" ? null : id, reviewedId: state === "added" ? null : `r${id}`, subject: `subject ${id}`, state, changes: null }
}

function change(patch: Partial<ApiChange>): ApiChange {
  return { path: "src/Store.kt", line: 34, name: "helper", key: "helper", kind: "changed", changes: [], before: null, after: null, usages: null, ...patch }
}

function usages(inside: number, outside: number): Usages {
  return { inside, outside, outsideSites: [], complete: true }
}

function facts(patch: Partial<FileFacts>): FileFacts {
  return { path: "src/Store.kt", oldPath: null, mechanical: null, uses: 0, usedBy: 0, newLinks: 0, breaks: 0, breaksComplete: true, untested: false, repeatedEdit: null, ...patch }
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
  it("words the usages inside and outside the change, a cut answer as more, none when unknown", () => {
    expect(formatUsages(usages(1, 2))).toBe("3 usages")
    expect(formatUsages(usages(0, 1))).toBe("1 usage")
    expect(formatUsages({ ...usages(0, 100), complete: false })).toBe("100+ usages")
    expect(formatUsages(null)).toBeNull()
  })

  it("keeps the order Rust ranked, with the kind, the place, the parts that changed and the forms of a removal or a change", () => {
    const entries = presentApiChanges([
      change({ kind: "removed", name: "open", key: "a.open", path: "src/a/Api.kt", line: 3, before: "fun open(path:String)", usages: usages(0, 12) }),
      change({ changes: ["parameters", "reach"], before: "fun helper(value:Int)", after: "fun helper(value:Int, scale:Int)", usages: usages(1, 2) }),
      change({ kind: "added", name: "Cache", key: "b.Cache", path: "src/b/Cache.kt", line: 3, after: "class Cache" }),
    ])
    expect(entries.map(({ key: _key, ...entry }) => entry)).toEqual([
      { name: "open", kind: "Removed", tone: "destructive", location: "src/a/Api.kt:3", changed: null, before: "fun open(path:String)", after: null, usages: "12 usages" },
      {
        name: "helper",
        kind: "Changed",
        tone: "warning",
        location: "src/Store.kt:34",
        changed: "parameters, visibility",
        before: "fun helper(value:Int)",
        after: "fun helper(value:Int, scale:Int)",
        usages: "3 usages",
      },
      { name: "Cache", kind: "Added", tone: "info", location: "src/b/Cache.kt:3", changed: null, before: null, after: null, usages: null },
    ])
    expect(new Set(entries.map(entry => entry.key)).size).toBe(3)
  })
})

describe("the risky files of the card", () => {
  it("raises only the risky files, in Rust's order, with the usages and the missing test", () => {
    expect(
      presentRiskyFiles([
        facts({ path: "src/Store.kt", breaks: 3, untested: true }),
        facts({ path: "src/a/Api.kt", breaks: 1 }),
        facts({ path: "src/b/Cache.kt", untested: true }),
        facts({ path: "src/a/Reader.kt" }),
      ]),
    ).toEqual([
      { path: "src/Store.kt", reasons: "An API used in 3 places · No test changed" },
      { path: "src/a/Api.kt", reasons: "An API used in 1 place" },
      { path: "src/b/Cache.kt", reasons: "No test changed" },
    ])
  })

  it("sums the card up in its title", () => {
    expect(formatPlanSummary({ api: [change({})], files: [facts({ breaks: 2 }), facts({ path: "b", untested: true }), facts({ path: "c" })] })).toBe("1 API change · 2 risky files")
    expect(formatPlanSummary({ api: [], files: [facts({})] })).toBe("No API changes")
    expect(formatPlanSummary({ api: [change({}), change({})], files: [facts({ untested: true })] })).toBe("2 API changes · 1 risky file")
  })
})

function link(from: string, to: string, state: FileLink["state"], symbols: string[] = ["Cache"]): FileLink {
  return { from, to, symbols: symbols.map((name, index) => ({ name, line: 3 + index })), state, crossModule: false }
}

function dependency(patch: Partial<DependencyChange>): DependencyChange {
  return {
    manifest: "src/b/b.iml",
    module: "b",
    kind: "module",
    name: "intellij.platform.x",
    change: "added",
    versionBefore: null,
    versionAfter: null,
    scope: "main",
    scopeBefore: null,
    ...patch,
  }
}

describe("the badges of the files in the tree", () => {
  it("counts the links among the changed files, the new ones apart, and lists each with its names, its line and its state", () => {
    const badges = presentFileBadges({
      files: [facts({ path: "src/a/Reader.kt", uses: 1, newLinks: 1, untested: true }), facts({ path: "src/b/Cache.kt", usedBy: 1 })],
      links: [link("src/a/Reader.kt", "src/b/Cache.kt", "added", ["Cache", "get", "put", "size"]), link("src/a/Reader.kt", "src/Old.kt", "removed")],
      dependencies: [],
      repeatedEdits: [],
    })
    const reader = badges.get("src/a/Reader.kt")!
    expect([reader.uses, reader.usedBy, reader.newLinks, reader.untested, reader.breaks]).toEqual(["uses 1", null, "+1 new", "no test", null])
    expect(reader.description).toBe("uses 1, +1 new, no test")
    expect(reader.usesLinks.map(({ key: _key, ...entry }) => entry)).toEqual([
      { path: "src/b/Cache.kt", name: "Cache.kt", symbols: "Cache, get, put +1 · Reader.kt:3", state: "added", stateWord: "Added" },
      { path: "src/Old.kt", name: "Old.kt", symbols: "Cache · Reader.kt:3", state: "removed", stateWord: "Removed" },
    ])
    const cache = badges.get("src/b/Cache.kt")!
    expect([cache.uses, cache.usedBy, cache.newLinks]).toEqual([null, "used by 1", null])
    expect(cache.usedByLinks.map(entry => [entry.path, entry.stateWord])).toEqual([["src/a/Reader.kt", "Added"]])
  })

  it("says the uses an API leaves outside, as a lower bound where Rust says the count is, never a safe 0", () => {
    const breaks = (patch: Partial<FileFacts>) => presentFileBadges({ files: [facts(patch)], links: [], dependencies: [], repeatedEdits: [] }).get("src/Store.kt")?.breaks ?? null
    expect(breaks({ breaks: 3 })).toMatchObject({ label: "breaks 3", tone: "warning" })
    expect(breaks({ breaks: 3, breaksComplete: false })).toMatchObject({ label: "breaks 3+", tone: "warning" })
    expect(breaks({ breaks: 0, breaksComplete: false })).toMatchObject({ label: "breaks ?", tone: "muted" })
    expect(breaks({ breaks: 0 })).toBeNull()
  })

  it("names the same edit of a group of files and the other files that get it", () => {
    const group = { files: ["src/A.kt", "src/B.kt", "src/C.kt"], removed: ["ProjectManager.getInstance()"], added: ["ProjectManagerEx.getInstanceEx()"] }
    const badges = presentFileBadges({ files: group.files.map(path => facts({ path, repeatedEdit: 0 })), links: [], dependencies: [], repeatedEdits: [group] })
    expect(badges.get("src/B.kt")!.sameEdit).toEqual({
      label: "same edit ×3",
      edit: "ProjectManager.getInstance() → ProjectManagerEx.getInstanceEx()",
      files: ["src/A.kt", "src/C.kt"],
    })
    const added = presentFileBadges({ files: [facts({ repeatedEdit: 0 })], links: [], dependencies: [], repeatedEdits: [{ ...group, removed: [] }] })
    expect(added.get("src/Store.kt")!.sameEdit!.edit).toBe("+ ProjectManagerEx.getInstanceEx()")
  })

  it("lists no file without a badge", () => {
    expect(presentFileBadges({ files: [facts({})], links: [], dependencies: [], repeatedEdits: [] }).size).toBe(0)
  })

  it("names the Mechanical group and its groups by kind", () => {
    expect(formatMechanicalGroup(null, 5)).toBe("Mechanical (5)")
    expect(formatMechanicalGroup("importsOnly", 2)).toBe("Imports only (2)")
  })

  it("counts the dependency changes of a module, a version change as one changed entry with both versions", () => {
    const badge = presentDependencyBadge([
      dependency({}),
      dependency({ kind: "library", name: "kotlinx-coroutines" }),
      dependency({ change: "removed", name: "intellij.platform.y", scope: "test" }),
      dependency({ kind: "crate", name: "serde", change: "changed", versionBefore: "1.2", versionAfter: "1.3" }),
    ])
    expect(badge).toEqual({
      label: "+2 deps −1 ~1",
      lines: ["Added module intellij.platform.x", "Added library kotlinx-coroutines", "Removed module intellij.platform.y (test)", "Changed crate serde 1.2 → 1.3"],
    })
    expect(presentDependencyBadge([dependency({ change: "removed" })])!.label).toBe("−1 dep")
    expect(presentDependencyBadge([dependency({ kind: "newModule", name: "b" }), dependency({})])!.label).toBe("new module · +1 dep")
    expect(presentDependencyBadge([])).toBeNull()
  })

  it("words a version, a scope and a new module", () => {
    expect(formatDependencyChange(dependency({ kind: "package", name: "vitest", versionAfter: "^3.0.0", scope: "dev" }))).toBe("Added package vitest ^3.0.0 (dev)")
    expect(formatDependencyChange(dependency({ change: "changed", scopeBefore: "test", scope: "main" }))).toBe("Changed module intellij.platform.x (test → main)")
    expect(formatDependencyChange(dependency({ kind: "newModule", name: "b" }))).toBe("New module b")
  })
})

describe("Mark Reviewed", () => {
  it("says the branch is marked, or that it was at this commit already", () => {
    expect(formatMarkedReviewed("plan", true)).toBe("Marked plan reviewed")
    expect(formatMarkedReviewed("plan", false)).toBe("plan was already marked reviewed at this commit")
  })
})
