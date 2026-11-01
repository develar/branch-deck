import { describe, expect, it } from "vite-plus/test"
import type { ApiChange, DependencyChange, FileFacts, FileLink, Interdiff, PairedCommit, Usages } from "@/lib/bindings"
import {
  formatDependencyChange,
  formatMarkedReviewed,
  formatGenerated,
  formatMechanicalGroup,
  formatPairState,
  formatRewrittenChanges,
  formatSinceReview,
  formatOutsideUses,
  presentBreakage,
  presentDependencyBadge,
  presentFileBadges,
  presentRowFacts,
} from "@/presenters/review-plan"

function paired(state: PairedCommit["state"], id: string): PairedCommit {
  return { id: state === "dropped" ? null : id, reviewedId: state === "added" ? null : `r${id}`, subject: `subject ${id}`, state, changes: null }
}

function change(patch: Partial<ApiChange>): ApiChange {
  return { path: "src/Store.kt", line: 34, name: "helper", key: "helper", kind: "changed", changes: [], before: null, after: null, usages: null, ...patch }
}

function usages(inside: number, outside: number): Usages {
  // a complete count: every use is updated with none left outside, as Rust sends it
  return { inside, outside, outsideSites: [], stillBound: 0, byName: 0, complete: true, unbroken: outside === 0 }
}

function facts(patch: Partial<FileFacts>): FileFacts {
  return {
    path: "src/Store.kt",
    oldPath: null,
    mechanical: null,
    generated: null,
    uses: 0,
    usedBy: 0,
    newLinks: 0,
    breaks: 0,
    breaksComplete: true,
    untested: false,
    repeatedEdit: null,
    repeatedEditOnly: false,
    ...patch,
  }
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

describe("the possible breakage of the overview", () => {
  it("words the uses left outside, a cut or partial count as a lower bound, an unknown count as unknown", () => {
    expect(formatOutsideUses(usages(1, 2), false)).toBe("2 uses left outside")
    expect(formatOutsideUses(usages(0, 1), false)).toBe("1 use left outside")
    expect(formatOutsideUses({ ...usages(0, 100), complete: false, unbroken: false }, false)).toBe("At least 100 uses left outside")
    expect(formatOutsideUses({ ...usages(3, 0), complete: false, unbroken: false }, false)).toBe("No use found outside, the count is incomplete")
    expect(formatOutsideUses(null, false)).toBe("The uses are unknown")
    expect(formatOutsideUses(null, true)).toBe("Finding the uses…")
  })

  it("lists a removed or changed API with uses left outside in Rust's order, with its place, the parts that changed, the forms apart and the first sites", () => {
    const { entries, unbroken } = presentBreakage(
      [
        change({
          kind: "removed",
          name: "open",
          key: "a.open",
          path: "src/a/Api.kt",
          line: 3,
          before: "fun open(path:String)",
          usages: {
            inside: 0,
            outside: 7,
            outsideSites: [1, 2, 3, 4, 5].map(line => ({ path: "src/b/Client.kt", line })),
            stillBound: 0,
            byName: 0,
            complete: true,
            unbroken: false,
          },
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
      ],
      false,
    )
    expect(unbroken).toBeNull()
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
        stillBound: null,
        byName: null,
        // each at its place at the head, the name of the API selected there
        sites: [1, 2, 3, 4, 5].map(line => ({ label: `src/b/Client.kt:${line}`, place: { path: "src/b/Client.kt", side: "new", line, name: "open" } })),
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
        stillBound: null,
        byName: null,
        sites: [
          { label: "src/Base.kt:1", place: { path: "src/Base.kt", side: "new", line: 1, name: "helper" } },
          { label: "src/Uses.kt:4", place: { path: "src/Uses.kt", side: "new", line: 4, name: "helper" } },
        ],
        moreSites: null,
      },
    ])
  })

  it("folds only the API none of whose uses left outside breaks, and keeps an unknown, incomplete or by-name count listed, also at 0", () => {
    const { entries, unbroken } = presentBreakage(
      [
        change({ name: "unknown", key: "unknown", usages: null }),
        change({ name: "constructor", key: "constructor", changes: ["constructors"], usages: { ...usages(2, 0), complete: false, unbroken: false } }),
        change({ name: "isEmpty", key: "isEmpty", usages: { ...usages(1, 0), byName: 3, unbroken: false } }),
        change({ name: "renamed", key: "renamed", kind: "removed", usages: usages(3, 0) }),
        change({ name: "policy", key: "policy", changes: ["constructors"], usages: { ...usages(0, 0), stillBound: 4 } }),
      ],
      false,
    )
    expect(entries.map(entry => [entry.name, entry.outside, entry.byName])).toEqual([
      ["unknown", "The uses are unknown", null],
      ["constructor", "No use found outside, the count is incomplete", null],
      ["isEmpty", "0 uses left outside", "3 uses by name only, unknown"],
    ])
    expect(unbroken?.label).toBe("2 API changes, no use left outside breaks")
    expect(unbroken?.entries.map(entry => [entry.name, entry.inside, entry.stillBound])).toEqual([
      ["renamed", "3 uses updated in this change", null],
      ["policy", null, "4 calls left outside still bind"],
    ])
    expect(presentBreakage([change({ usages: usages(1, 0) })], false).unbroken?.label).toBe("1 API change, no use left outside breaks")
  })

  it("says the uses are being found while Rust asks the index", () => {
    const { entries } = presentBreakage([change({ name: "open", key: "open", usages: null })], true)
    expect(entries.map(entry => entry.outside)).toEqual(["Finding the uses…"])
  })
})

function link(from: string, to: string, state: FileLink["state"], symbols: string[] = ["Cache"]): FileLink {
  return { from, to, symbols: symbols.map((name, index) => ({ name, line: 3 + index })), state, crossModule: false, newBetweenModules: false }
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
    expect([reader.uses, reader.usedBy, reader.newLinks, reader.breaks]).toEqual(["uses 1", null, "+1 new", null])
    expect(reader.description).toBe("uses 1, +1 new")
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

  it("merges the facts of a row into one mark, in the warning tone only when it breaks uses outside, and its card sections what can break first", () => {
    const badges = presentFileBadges({
      files: [facts({ path: "src/a/Reader.kt", uses: 1, newLinks: 1, breaks: 3, repeatedEdit: 0 }), facts({ path: "src/b/Cache.kt", usedBy: 1, repeatedEdit: 0 })],
      links: [link("src/a/Reader.kt", "src/b/Cache.kt", "added")],
      dependencies: [],
      repeatedEdits: [{ files: ["src/a/Reader.kt", "src/b/Cache.kt"], removed: [], added: ["// read"] }],
    })
    const reader = presentRowFacts(badges.get("src/a/Reader.kt"))!
    expect(reader.tone).toBe("warning")
    expect(reader.description).toBe("uses 1, +1 new, breaks 3, same edit ×2")
    expect(reader.sections.map(section => [section.kind, section.title])).toEqual([
      ["breaks", "Breaks"],
      ["uses", "Uses 1 · +1 new"],
      ["sameEdit", "Same edit ×2"],
    ])
    expect(reader.sections[0]).toMatchObject({ label: "breaks 3", why: "3 uses left outside the change", tone: "warning" })
    expect(reader.sections[2]).toMatchObject({ edit: "+ // read", files: ["src/b/Cache.kt"] })

    // no break: a muted mark, the links that use it
    const cache = presentRowFacts(badges.get("src/b/Cache.kt"))!
    expect(cache.tone).toBe("muted")
    expect(cache.sections.map(section => section.title)).toEqual(["Used by 1", "Same edit ×2"])
    // a lower bound at 0 is no safe 0, but no warning either
    const unknown = presentFileBadges({ files: [facts({ breaksComplete: false })], links: [], dependencies: [], repeatedEdits: [] }).get("src/Store.kt")
    expect(presentRowFacts(unknown)).toMatchObject({ tone: "muted", description: "breaks ?" })
    const lowerBound = presentFileBadges({ files: [facts({ breaks: 2, breaksComplete: false })], links: [], dependencies: [], repeatedEdits: [] }).get("src/Store.kt")
    expect(presentRowFacts(lowerBound)?.tone).toBe("warning")
  })

  it("puts the dependency changes a row carries into the same mark, after the facts of its file", () => {
    const dependencyBadge = presentDependencyBadge([dependency({ kind: "newModule", name: "b" })])!
    const module = presentRowFacts(undefined, dependencyBadge)!
    expect(module).toEqual({ tone: "muted", description: "new module", sections: [{ kind: "dependencies", title: "Dependencies · new module", lines: ["New module b"] }] })
    // the build file at the root: its own facts and the changes of its module, one mark
    const file = presentFileBadges({ files: [facts({ breaks: 1 })], links: [], dependencies: [], repeatedEdits: [] }).get("src/Store.kt")
    const both = presentRowFacts(file, dependencyBadge)!
    expect([both.tone, both.description, both.sections.map(section => section.kind)]).toEqual(["warning", "breaks 1, new module", ["breaks", "dependencies"]])
    expect(presentRowFacts(undefined, undefined)).toBeNull()
  })

  it("names the Mechanical group and its groups by kind", () => {
    expect(formatMechanicalGroup(null, 5)).toBe("Mechanical (5)")
    expect(formatMechanicalGroup("importsOnly", 2)).toBe("Imports only (2)")
    expect(formatMechanicalGroup("generated", 3)).toBe("Generated (3)")
  })

  it("says why a generated file is folded: its name, its attributes, the line its tool wrote, a long one cut", () => {
    expect(formatGenerated({ kind: "name", name: "lockFile" })).toBe("Lock file")
    expect(formatGenerated({ kind: "attribute" })).toBe("Generated (.gitattributes)")
    expect(formatGenerated({ kind: "header", line: "Generated by tools/render.mjs; edit src/guide.md" })).toBe("Generated by tools/render.mjs; edit src/guide.md")
    const long = formatGenerated({ kind: "header", line: `Generated by ${"x".repeat(200)}` })
    expect([long.length, long.endsWith("…")]).toEqual([100, true])
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
