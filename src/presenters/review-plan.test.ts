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
  formatOutsideUses,
  presentBreakage,
  presentDependencyBadge,
  presentDependencyChanges,
  presentFileBadges,
  presentNewApi,
  presentRepeatedEdits,
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

describe("the possible breakage of the card", () => {
  it("words the uses left outside, a cut or partial count as a lower bound, an unknown count as unknown", () => {
    expect(formatOutsideUses(usages(1, 2))).toBe("2 uses left outside")
    expect(formatOutsideUses(usages(0, 1))).toBe("1 use left outside")
    expect(formatOutsideUses({ ...usages(0, 100), complete: false })).toBe("At least 100 uses left outside")
    expect(formatOutsideUses({ ...usages(3, 0), complete: false })).toBe("No use found outside, the count is incomplete")
    expect(formatOutsideUses(null)).toBe("The uses are unknown")
  })

  it("lists a removed or changed API with uses left outside in Rust's order, with its place, the parts that changed, the forms apart and the first sites", () => {
    const { entries, updated } = presentBreakage([
      change({
        kind: "removed",
        name: "open",
        key: "a.open",
        path: "src/a/Api.kt",
        line: 3,
        before: "fun open(path:String)",
        usages: { inside: 0, outside: 7, outsideSites: [1, 2, 3, 4, 5].map(line => ({ path: "src/b/Client.kt", line })), complete: true },
      }),
      change({
        changes: ["parameters", "reach"],
        before: "fun helper(value:Int)",
        after: "fun helper(value:Int, scale:Int)",
        usages: {
          ...usages(1, 2),
          outsideSites: [
            { path: "src/Base.kt", line: 1 },
            { path: "src/Uses.kt", line: 4 },
          ],
        },
      }),
      change({ kind: "added", name: "Cache", key: "b.Cache", path: "src/b/Cache.kt", line: 3, after: "class Cache" }),
    ])
    expect(updated).toBeNull()
    expect(entries.map(({ key: _key, ...entry }) => entry)).toEqual([
      {
        path: "src/a/Api.kt",
        // a removal shows its declaration before the commits
        place: { path: "src/a/Api.kt", side: "old", line: 3, name: "open" },
        name: "open",
        kind: "Removed",
        tone: "destructive",
        location: "src/a/Api.kt:3",
        changed: null,
        before: "fun open(path:String)",
        after: null,
        outside: "7 uses left outside",
        inside: null,
        sites: ["src/b/Client.kt:1", "src/b/Client.kt:2", "src/b/Client.kt:3", "src/b/Client.kt:4", "src/b/Client.kt:5"],
        moreSites: "+2 more",
      },
      {
        path: "src/Store.kt",
        place: { path: "src/Store.kt", side: "new", line: 34, name: "helper" },
        name: "helper",
        kind: "Changed",
        tone: "warning",
        location: "src/Store.kt:34",
        changed: "parameters, visibility",
        before: "fun helper(value:Int)",
        after: "fun helper(value:Int, scale:Int)",
        outside: "2 uses left outside",
        inside: "1 use updated in this change",
        sites: ["src/Base.kt:1", "src/Uses.kt:4"],
        moreSites: null,
      },
    ])
  })

  it("folds only the API whose every use is updated, and keeps an unknown or incomplete count listed, also at 0", () => {
    const { entries, updated } = presentBreakage([
      change({ name: "unknown", key: "unknown", usages: null }),
      change({ name: "constructor", key: "constructor", changes: ["constructors"], usages: { ...usages(2, 0), complete: false } }),
      change({ name: "renamed", key: "renamed", kind: "removed", usages: usages(3, 0) }),
      change({ name: "unused", key: "unused", usages: usages(0, 0) }),
    ])
    expect(entries.map(entry => [entry.name, entry.outside])).toEqual([
      ["unknown", "The uses are unknown"],
      ["constructor", "No use found outside, the count is incomplete"],
    ])
    expect(updated?.label).toBe("2 API changes, every use updated in this change")
    expect(updated?.entries.map(entry => entry.name)).toEqual(["renamed", "unused"])
    expect(presentBreakage([change({ usages: usages(1, 0) })]).updated?.label).toBe("1 API change, every use updated in this change")
  })

  it("sums the card up in its title, what can break first", () => {
    const plan = { api: [], dependencies: [], links: [], repeatedEdits: [] }
    expect(formatPlanSummary(plan)).toBe("No possible break")
    expect(
      formatPlanSummary({
        api: [change({ usages: null }), change({ usages: usages(1, 0) }), change({ kind: "added", after: "class Cache" }), change({ kind: "added", name: "get" })],
        dependencies: [dependency({}), dependency({ name: "b" })],
        links: [
          { ...link("src/a/Reader.kt", "src/b/Cache.kt", "added"), crossModule: true },
          { ...link("src/a/A.kt", "src/b/B.kt", "kept"), crossModule: true },
        ],
        repeatedEdits: [{ files: ["a", "b", "c"], removed: [], added: ["// a line of the edit"] }],
      }),
    ).toBe("1 possible break · 2 dependency changes · 1 new link between modules · 1 repeated edit · 2 new declarations")
  })
})

describe("the dependencies, the repeated edits and the new API of the card", () => {
  it("groups the dependency changes by build file in Rust's order, and lists only the links the commits added between modules", () => {
    const section = presentDependencyChanges({
      dependencies: [
        dependency({ manifest: "crates/x/Cargo.toml", module: "x", kind: "crate", name: "serde", change: "changed", versionBefore: "1.0.1", versionAfter: "1.0.2" }),
        dependency({ manifest: "crates/x/Cargo.toml", module: "x", kind: "crate", name: "toml", change: "added", versionAfter: "0.8" }),
        dependency({ manifest: "src/b/b.iml", module: "b", kind: "newModule", name: "b", change: "added" }),
      ],
      links: [
        { ...link("src/a/Reader.kt", "src/b/Cache.kt", "added", ["Cache", "get"]), crossModule: true },
        { ...link("src/a/A.kt", "src/b/B.kt", "removed"), crossModule: true },
        link("src/a/A.kt", "src/a/C.kt", "added"),
      ],
    })
    expect(section.modules.map(module => ({ ...module, lines: module.lines.map(line => [line.change, line.text]) }))).toEqual([
      {
        manifest: "crates/x/Cargo.toml",
        module: "x",
        lines: [
          ["changed", "Changed crate serde 1.0.1 → 1.0.2"],
          ["added", "Added crate toml 0.8"],
        ],
      },
      { manifest: "src/b/b.iml", module: "b", lines: [["added", "New module b"]] },
    ])
    expect(section.links.map(({ key: _key, ...entry }) => entry)).toEqual([
      {
        from: "src/a/Reader.kt",
        to: "src/b/Cache.kt",
        // the first use, in the file that uses
        place: { path: "src/a/Reader.kt", side: "new", line: 3, name: "Cache" },
        label: "Reader.kt → Cache.kt",
        symbols: "Cache, get · Reader.kt:3",
      },
    ])
  })

  it("words each repeated edit with its files, and the new API per file in the order of its first declaration", () => {
    expect(presentRepeatedEdits([{ files: ["src/A.kt", "src/B.kt", "src/C.kt"], removed: ["ProjectManager"], added: ["ProjectManagerEx"] }])).toEqual([
      { key: "0", label: "Same edit in 3 files", edit: "ProjectManager → ProjectManagerEx", files: ["src/A.kt", "src/B.kt", "src/C.kt"] },
    ])
    expect(
      presentNewApi([
        change({ kind: "added", name: "Cache", path: "src/b/Cache.kt", line: 3 }),
        change({ kind: "changed", name: "helper" }),
        change({ kind: "added", name: "Reader", path: "src/a/Reader.kt", line: 5 }),
        change({ kind: "added", name: "get", path: "src/b/Cache.kt", line: 4 }),
      ]),
    ).toEqual([
      // each at its first new declaration
      { path: "src/b/Cache.kt", place: { path: "src/b/Cache.kt", side: "new", line: 3, name: "Cache" }, names: "Cache, get" },
      { path: "src/a/Reader.kt", place: { path: "src/a/Reader.kt", side: "new", line: 5, name: "Reader" }, names: "Reader" },
    ])
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
  it("counts the links among the changed files, the new ones apart, and lists each with its names, its line, its state and its place", () => {
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
      // a file it uses at its header: the link names no line of it
      {
        path: "src/b/Cache.kt",
        place: { path: "src/b/Cache.kt", side: "new", line: null, name: null },
        name: "Cache.kt",
        symbols: "Cache, get, put +1 · Reader.kt:3",
        state: "added",
        stateWord: "Added",
      },
      {
        path: "src/Old.kt",
        place: { path: "src/Old.kt", side: "new", line: null, name: null },
        name: "Old.kt",
        symbols: "Cache · Reader.kt:3",
        state: "removed",
        stateWord: "Removed",
      },
    ])
    const cache = badges.get("src/b/Cache.kt")!
    expect([cache.uses, cache.usedBy, cache.newLinks]).toEqual([null, "used by 1", null])
    // a file that uses it at its first use
    expect(cache.usedByLinks.map(entry => [entry.path, entry.stateWord, entry.place])).toEqual([
      ["src/a/Reader.kt", "Added", { path: "src/a/Reader.kt", side: "new", line: 3, name: "Cache" }],
    ])
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
