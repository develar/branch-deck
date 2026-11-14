import { describe, expect, it } from "vite-plus/test"
import type { ApiChange, Concern, ConcernName, ConcernPart, FileFacts, GeneratedBy, ReviewUnit, UnitBlock, UnitKind } from "@/lib/bindings"
import { headingText } from "@/lib/diff/rows"
import { concernChapters } from "@/presenters/review-steps"
import { formatPart, formatUnitHeading, presentNarrative, presentStepBar } from "@/presenters/review-units"
import type { NarrativeFile } from "@/presenters/review-units"

/** Block `index` of a file: one line changed at `line` on each side */
function block(index: number, line: number): UnitBlock {
  return { index, oldStart: line, oldEnd: line + 1, newStart: line, newEnd: line + 1, additions: 1, deletions: 1 }
}

function unit(kind: UnitKind, name: string | null, slices: { path: string; blocks: UnitBlock[]; whole?: boolean }[]): ReviewUnit {
  return {
    id: `${kind.kind}:${name ?? slices[0]?.path}`,
    kind,
    name,
    key: name,
    signature: null,
    api: true,
    test: false,
    line: null,
    slices: slices.map(slice => ({ path: slice.path, oldPath: null, whole: slice.whole ?? false, blocks: slice.blocks, fingerprint: "f" })),
    fingerprint: "f",
  }
}

/** The same edit of the first line of A.kt and of B.kt */
function sameEditOfAB(): ReviewUnit {
  return unit({ kind: "repeatedEdit", group: 0 }, null, [
    { path: "src/A.kt", blocks: [block(0, 1)] },
    { path: "src/B.kt", blocks: [block(0, 1)] },
  ])
}

/** The parameters of `open` of Api.kt changed, `outside` uses of it left outside, of which those at `lines` of Use.kt */
function openChanged(lines: number[], outside: number): ApiChange {
  const usages = { inside: 0, outside, outsideSites: lines.map(line => ({ path: "src/Use.kt", line })), stillBound: 0, byName: 0, complete: true, unbroken: false }
  return { path: "src/Api.kt", line: 3, name: "open", key: "open", kind: "changed", widened: false, changes: ["parameters"], before: null, after: null, usages }
}

/** A file whose diff has blocks at `lines` */
function file(path: string, lines: number[] | null): NarrativeFile {
  return { path, blocks: lines === null ? null : lines.map((line, index) => block(index, line)) }
}

describe("the story of a review", () => {
  it("words a change by its kind", () => {
    // the name of the declaration set apart from the words of what changed: the code font for the name alone
    expect(formatUnitHeading(unit({ kind: "shape", changes: ["parameters", "reach"] }, "open", []))).toEqual({ name: "open", words: "parameters and visibility changed" })
    expect(headingText([formatUnitHeading(unit({ kind: "behaviour" }, "close", []))])).toBe("close — body changed")
    expect(formatUnitHeading(unit({ kind: "other", reason: "outside" }, "Store.kt", []))).toEqual({ name: null, words: "Changes outside a declaration" })
  })

  it("words a body that changed in renamed names alone as the renames, not proven or checking, as Rust decided", () => {
    const balance = unit({ kind: "behaviour" }, "balance", [])
    const renamed = (wait: "checking" | "notProven", ...names: [string, string][]): ReviewUnit => ({
      ...balance,
      renamed: { names: names.map(([old, renamedTo]) => ({ old, new: renamedTo })), wait },
    })
    expect(headingText([formatUnitHeading(renamed("notProven", ["sumOf", "totalOf"]))])).toBe("balance — only renamed sumOf → totalOf, not proven")
    expect(headingText([formatUnitHeading(renamed("checking", ["sumOf", "totalOf"]))])).toBe("balance — only renamed sumOf → totalOf, checking")
    expect(formatUnitHeading(renamed("notProven", ["sumOf", "totalOf"], ["count", "size"]))).toEqual({
      name: "balance",
      words: "only renamed sumOf → totalOf and count → size, not proven",
    })
    // no renames Rust told of: the body changed
    expect(headingText([formatUnitHeading({ ...balance, renamed: null })])).toBe("balance — body changed")
  })

  it("words a declaration renamed where the index has not proven it as renamed from its old name, after what else of its form changed", () => {
    const totalOf = unit({ kind: "behaviour" }, "totalOf", [])
    expect(headingText([formatUnitHeading({ ...totalOf, renamedFrom: { old: "sumOf", wait: "notProven" } })])).toBe("totalOf — renamed from sumOf, not proven")
    expect(headingText([formatUnitHeading({ ...totalOf, renamedFrom: { old: "sumOf", wait: "checking" } })])).toBe("totalOf — renamed from sumOf, checking")
    const shape = unit({ kind: "shape", changes: ["parameters"] }, "totalOf", [])
    expect(headingText([formatUnitHeading({ ...shape, renamedFrom: { old: "sumOf", wait: "notProven" } })])).toBe("totalOf — parameters changed, renamed from sumOf, not proven")
    // no rename Rust told of: a body that changed
    expect(headingText([formatUnitHeading({ ...totalOf, renamedFrom: null })])).toBe("totalOf — body changed")
  })

  it("makes a run of changes of one file one section with a heading per change, and a group a section per file", () => {
    const units = [
      unit({ kind: "shape", changes: ["parameters"] }, "open", [{ path: "src/Api.kt", blocks: [block(1, 10)] }]),
      unit({ kind: "behaviour" }, "close", [{ path: "src/Api.kt", blocks: [block(2, 20)] }]),
      unit({ kind: "removed" }, "gone", [{ path: "src/Api.kt", blocks: [] }]),
      unit({ kind: "behaviour" }, "use", [{ path: "src/Use.kt", blocks: [block(0, 3)] }]),
      sameEditOfAB(),
      unit({ kind: "mechanical", mechanical: "importsOnly" }, null, [{ path: "src/Api.kt", blocks: [block(0, 2)] }]),
    ]
    const sections = presentNarrative(units, [file("src/A.kt", [1]), file("src/Api.kt", [2, 10, 20]), file("src/B.kt", [1]), file("src/Use.kt", [3])], [], [], [])
    expect(sections.map(section => [section.key, section.heading, section.folded])).toEqual([
      ["file:src/Api.kt", null, null],
      ["file:src/Use.kt", null, null],
      ["group:repeatedEdit:src/A.kt:src/A.kt", "Same edit in 2 files", null],
      ["group:repeatedEdit:src/A.kt:src/B.kt", null, "Same edit as A.kt"],
      ["group:mechanical:src/Api.kt:src/Api.kt", "Imports only (1)", "Imports only"],
    ])
    const api = sections[0]!
    // the removed declaration whose block another unit holds joins the heading before it
    expect([...api.units!.marks].map(([block, heading]) => [block, headingText(heading)])).toEqual([
      [1, "open — parameters changed"],
      [2, "close — body changed · gone — removed"],
    ])
    // the units each section shows, a mention of a declaration marks its section by them
    expect(sections.map(section => [...section.unitIds])).toEqual([
      ["shape:open", "behaviour:close", "removed:gone"],
      ["behaviour:use"],
      ["repeatedEdit:src/A.kt"],
      ["repeatedEdit:src/A.kt"],
      ["mechanical:src/Api.kt"],
    ])
    // the block of the imports is one row here, which opens the group: named by the group, never by its count
    expect([...api.units!.elsewhere]).toEqual([[0, "Shown in Imports only"]])
    expect(api.targets.get(0)).toBe("group:mechanical:src/Api.kt:src/Api.kt")
  })

  it("folds the declarations that moved as one group over their files, headed Moved with their count, each file saying Moved only", () => {
    const units = [
      unit({ kind: "behaviour" }, "open", [{ path: "src/Api.kt", blocks: [block(2, 30)] }]),
      unit({ kind: "mechanical", mechanical: "moved" }, null, [
        { path: "src/Api.kt", blocks: [block(0, 4), block(1, 20)] },
        { path: "src/Store.kt", blocks: [block(0, 2), block(1, 9)] },
      ]),
    ]
    const sections = presentNarrative(units, [file("src/Api.kt", [4, 20, 30]), file("src/Store.kt", [2, 9])], [], [], [])
    expect(sections.map(section => [section.key, section.heading, section.groupName, section.folded])).toEqual([
      ["file:src/Api.kt", null, null, null],
      ["group:mechanical:src/Api.kt:src/Api.kt", "Moved (2)", "Moved", "Moved only"],
      ["group:mechanical:src/Api.kt:src/Store.kt", null, "Moved", "Moved only"],
    ])
    // the body that changed names the two blocks the group shows: where the declaration left and where it stands
    expect([...sections[0]!.units!.elsewhere]).toEqual([
      [0, "Shown in Moved"],
      [1, "Shown in Moved"],
    ])
  })

  it("names a block a group shows by the name of the group on each of its files, never by the count of its heading", () => {
    // B.kt: a line of the same edit as A.kt, an import and the body of open
    const units = [
      unit({ kind: "behaviour" }, "open", [{ path: "src/B.kt", blocks: [block(2, 10)] }]),
      sameEditOfAB(),
      unit({ kind: "mechanical", mechanical: "importsOnly" }, null, [{ path: "src/B.kt", blocks: [block(1, 2)] }]),
    ]
    const sections = presentNarrative(units, [file("src/A.kt", [1]), file("src/B.kt", [1, 2, 10])], [], [], [])
    expect(sections.map(section => [section.key, section.heading, section.groupName])).toEqual([
      ["file:src/B.kt", null, null],
      ["group:repeatedEdit:src/A.kt:src/A.kt", "Same edit in 2 files", "Same edit in 2 files"],
      ["group:repeatedEdit:src/A.kt:src/B.kt", null, "Same edit in 2 files"],
      ["group:mechanical:src/B.kt:src/B.kt", "Imports only (1)", "Imports only"],
    ])
    // the rows and the folds of the body of open name the group of each block it does not show, of a file after the
    // first of the group too; those of the imports the same edit by its group, the body by its part
    expect([...sections[0]!.units!.elsewhere]).toEqual([
      [0, "Shown in Same edit in 2 files"],
      [1, "Shown in Imports only"],
    ])
    expect(new Map(sections[3]!.units!.elsewhere)).toEqual(
      new Map([
        [0, "Shown in Same edit in 2 files"],
        [2, "Shown in part 1"],
      ]),
    )
  })

  it("numbers the parts of a file the story shows in several sections, each with the lines of its own blocks, a part of deletions alone too", () => {
    // Loader.kt between the changes of Config.kt three times: the second part removes two lines and adds none
    const removal: UnitBlock = { index: 1, oldStart: 20, oldEnd: 22, newStart: 20, newEnd: 20, additions: 0, deletions: 2 }
    const wide: UnitBlock = { index: 2, oldStart: 40, oldEnd: 40, newStart: 38, newEnd: 43, additions: 5, deletions: 0 }
    const units = [
      unit({ kind: "added" }, "load", [{ path: "src/Loader.kt", blocks: [block(0, 3)] }]),
      unit({ kind: "behaviour" }, "configure", [{ path: "src/Config.kt", blocks: [block(0, 5)] }]),
      unit({ kind: "removed" }, "legacy", [{ path: "src/Loader.kt", blocks: [removal] }]),
      unit({ kind: "behaviour" }, "apply", [{ path: "src/Config.kt", blocks: [block(1, 9)] }]),
      unit({ kind: "added" }, "probe", [{ path: "src/Loader.kt", blocks: [wide] }]),
    ]
    const files: NarrativeFile[] = [
      { path: "src/Config.kt", blocks: [block(0, 5), block(1, 9)] },
      { path: "src/Loader.kt", blocks: [block(0, 3), removal, wide] },
    ]
    const sections = presentNarrative(units, files, [], [], [])
    expect(sections.map(section => [section.path, section.part === null ? null : formatPart(section.part), section.part?.additions, section.part?.deletions])).toEqual([
      ["src/Loader.kt", "part 1 of 3", 1, 1],
      ["src/Config.kt", "part 1 of 2", 1, 1],
      ["src/Loader.kt", "part 2 of 3", 0, 2],
      ["src/Config.kt", "part 2 of 2", 1, 1],
      ["src/Loader.kt", "part 3 of 3", 5, 0],
    ])
    // a block another part of the file shows says which part
    expect([...sections[0]!.units!.elsewhere]).toEqual([
      [1, "Shown in part 2"],
      [2, "Shown in part 3"],
    ])
    expect(sections[0]!.targets.get(2)).toBe(sections[4]!.key)
  })

  it("names no part of a file the story shows in one section", () => {
    const units = [
      unit({ kind: "added" }, "load", [{ path: "src/Loader.kt", blocks: [block(0, 3)] }]),
      unit({ kind: "behaviour" }, "configure", [{ path: "src/Loader.kt", blocks: [block(1, 5)] }]),
    ]
    const sections = presentNarrative(units, [file("src/Loader.kt", [3, 5])], [], [], [])
    expect(sections.map(section => section.part)).toEqual([null])
  })

  it("shows a deleted file whole in its place, and the files deleted from one directory as a group, each collapsed", () => {
    expect(headingText([formatUnitHeading(unit({ kind: "deletedFile" }, "config.toml", []))])).toBe("config.toml — deleted file")
    const units = [
      unit({ kind: "deletedFile" }, "config.toml", [{ path: "dotnet/.codex/config.toml", blocks: [], whole: true }]),
      unit({ kind: "deletedFiles", directory: "old/a" }, null, [
        { path: "old/a/A.kt", blocks: [], whole: true },
        { path: "old/a/B.kt", blocks: [], whole: true },
      ]),
    ]
    const files = [file("dotnet/.codex/config.toml", null), file("old/a/A.kt", null), file("old/a/B.kt", null)]
    expect(presentNarrative(units, files, [], [], []).map(section => [section.key, section.heading, section.folded])).toEqual([
      ["dotnet/.codex/config.toml", null, null],
      ["group:deletedFiles:old/a/A.kt:old/a/A.kt", "Deleted from old/a/ (2)", "Deleted"],
      ["group:deletedFiles:old/a/A.kt:old/a/B.kt", null, "Deleted"],
    ])
  })

  it("shows a file whole where its blocks are not those of the units, and a file no unit names after the story", () => {
    const units = [
      unit({ kind: "behaviour" }, "open", [{ path: "src/Api.kt", blocks: [block(0, 10)] }]),
      unit({ kind: "mechanical", mechanical: "whitespace" }, null, [{ path: "src/Api.kt", blocks: [block(1, 20)] }]),
    ]
    // another whitespace policy: the window has one block where the units have two
    const sections = presentNarrative(units, [file("src/Api.kt", [10]), file("README.md", [0])], [], [], [])
    expect(sections.map(section => [section.key, section.units])).toEqual([
      ["src/Api.kt", null],
      ["README.md", null],
    ])
  })
})

describe("the generated files in the story", () => {
  it("fold each with why it is generated: its name, its attributes, the line its tool wrote", () => {
    const paths = ["AGENTS.md", "config/mcp.json", "pnpm-lock.yaml"]
    const units = [
      unit(
        { kind: "mechanical", mechanical: "generated" },
        null,
        paths.map(path => ({ path, blocks: [block(0, 1)] })),
      ),
    ]
    const facts = (path: string, generated: GeneratedBy) => ({ path, generated }) as FileFacts
    const sections = presentNarrative(
      units,
      paths.map(path => file(path, [1])),
      [],
      [
        facts("AGENTS.md", { kind: "header", line: "Generated by tools/render.mjs; edit src/guide.md" }),
        facts("config/mcp.json", { kind: "attribute" }),
        facts("pnpm-lock.yaml", { kind: "name", name: "lockFile" }),
      ],
      [],
    )
    expect(sections.map(section => [section.path, section.heading, section.folded])).toEqual([
      ["AGENTS.md", "Generated (3)", "Generated by tools/render.mjs; edit src/guide.md"],
      ["config/mcp.json", null, "Generated (.gitattributes)"],
      ["pnpm-lock.yaml", null, "Lock file"],
    ])
  })
})

describe("the uses a change left outside in the story", () => {
  it("follow the section of the change: each its file around the use, at most five", () => {
    const change = openChanged([1, 2, 3, 4, 5, 6], 7)
    const units = [unit({ kind: "shape", changes: ["parameters"] }, "open", [{ path: "src/Api.kt", blocks: [block(0, 3)] }])]
    const sections = presentNarrative(units, [file("src/Api.kt", [3])], [change], [], [])
    expect(sections.map(section => section.key)).toEqual(["file:src/Api.kt", ...[1, 2, 3, 4, 5].map(line => `ghost:shape:open:src/Use.kt:${line}`)])
    expect(sections[1]!.ghost).toEqual({ line: 1, name: "open", heading: { name: "open", words: "left outside: src/Use.kt:1" } })
  })
})

describe("the concerns in the story", () => {
  const concern = (id: string, name: ConcernName, parts: ConcernPart[]): Concern => ({
    id,
    name,
    change: { kind: "mixed" },
    parts,
    paths: [...new Set(parts.map(part => part.path))],
    elsewhere: [],
    evidence: [],
    fingerprint: id,
  })
  const part = (unit: string, path: string, blocks: number[] | null = null): ConcernPart => ({ unit, path, blocks })

  /**
   * ijproxy and grep → accio, each a told step, the version of App.kt in Other changes, AGENTS.md generated in grep →
   * accio and pnpm-lock.yaml generated after the story
   */
  function concernsStory() {
    const units = [
      unit({ kind: "other", reason: "noLanguage" }, ".codex/config.toml", [{ path: ".codex/config.toml", blocks: [block(0, 3)] }]),
      // a file of no language the story shows whole, cut by the concerns
      unit({ kind: "other", reason: "noLanguage" }, "guide.md", [{ path: "docs/guide.md", blocks: [block(0, 2), block(1, 8)], whole: true }]),
      unit({ kind: "other", reason: "noLanguage" }, "tools.md", [{ path: "docs/tools.md", blocks: [block(0, 2)] }]),
      unit({ kind: "behaviour" }, "version", [{ path: "src/App.kt", blocks: [block(0, 1)] }]),
      unit({ kind: "other", reason: "noLanguage" }, ".mcp.json", [{ path: ".mcp.json", blocks: [block(0, 5)] }]),
      unit({ kind: "mechanical", mechanical: "generated" }, null, [
        { path: "AGENTS.md", blocks: [block(0, 2)] },
        { path: "pnpm-lock.yaml", blocks: [block(0, 3)] },
      ]),
    ]
    const concerns = [
      concern("p", { source: "token", token: "ijproxy" }, [
        part("other:.codex/config.toml", ".codex/config.toml"),
        part("other:guide.md", "docs/guide.md", [1]),
        part("other:.mcp.json", ".mcp.json"),
      ]),
      concern("g", { source: "replacement", removed: "grep", added: "accio" }, [
        part("other:guide.md", "docs/guide.md", [0]),
        part("other:tools.md", "docs/tools.md"),
        part("mechanical:AGENTS.md", "AGENTS.md"),
      ]),
      concern("r", { source: "rest" }, [part("behaviour:version", "src/App.kt")]),
    ]
    const facts = [{ path: "AGENTS.md", generated: { kind: "header", line: "Generated from docs/guide.md by tools/render.mjs" } } as FileFacts]
    // the blocks of each file as the units have them
    const files = units
      .flatMap(unit => unit.slices)
      .map(slice =>
        file(
          slice.path,
          slice.blocks.map(block => block.oldStart),
        ),
      )
    const chapters = concernChapters(concerns)
    return { chapters, sections: presentNarrative(units, files, [], facts, chapters) }
  }

  it("lays the parts of each concern out together under its name, a unit cut by the concerns in each, its generated file collapsed at its end, the mechanical groups after", () => {
    const { sections } = concernsStory()
    expect(sections.map(section => [section.key, section.concern, section.concernHeading, section.heading, section.folded])).toEqual([
      ["file:.codex/config.toml", "p", "ijproxy", null, null],
      ["file:docs/guide.md", "p", null, null, null],
      ["file:.mcp.json", "p", null, null, null],
      ["file:docs/guide.md#1", "g", "grep → accio", null, null],
      ["file:docs/tools.md", "g", null, null, null],
      ["group:mechanical:AGENTS.md:AGENTS.md", "g", null, null, "Generated from docs/guide.md by tools/render.mjs"],
      ["file:src/App.kt", "r", "Other changes", null, null],
      ["group:mechanical:AGENTS.md:pnpm-lock.yaml", null, null, "Generated (1)", "Generated"],
    ])
    // the guide shows the block of each concern, the other one a row that opens its section; a file of no language the
    // review parses gets no heading, its header names it
    const [, ofProxy, , ofSearch] = sections
    expect([[...ofProxy!.units!.marks.keys()], [...ofProxy!.units!.elsewhere]]).toEqual([[], [[0, "Shown in grep → accio"]]])
    expect([[...ofSearch!.units!.marks.keys()], ofSearch!.targets.get(1)]).toEqual([[], "file:docs/guide.md"])
  })

  it("names in the bar above the diffs a step by its place among the told steps and its title, Other changes and a group after the story by their name alone", () => {
    const { chapters, sections } = concernsStory()
    const bar = (opening: string | null, section: string | null) => {
      const view = presentStepBar(chapters, sections, { opening, section })
      return view === null ? null : [view.position, view.label, view.chapter?.id ?? null, view.section?.key ?? null]
    }
    // before the diffs report a section the first step; "Overview · 2 steps" counts the told steps alone
    expect(presentStepBar(chapters, sections, { opening: null, section: null })?.steps).toBe(2)
    expect(bar(null, null)).toEqual(["Step 1 / 2", "ijproxy", "p", null])
    expect(bar("g", null)).toEqual(["Step 2 / 2", "grep → accio", "g", null])
    expect(bar(null, "group:mechanical:AGENTS.md:AGENTS.md")).toEqual(["Step 2 / 2", "grep → accio", "g", null])
    // Other changes, at its opening and at its card: no place, as the overview and its opening give it no number
    expect(bar("r", null)).toEqual([null, "Other changes", "r", null])
    expect(bar(null, "file:src/App.kt")).toEqual([null, "Other changes", "r", null])
    // the generated file after the story is of no step: its group by its name, a click showing its first file
    expect(bar(null, "group:mechanical:AGENTS.md:pnpm-lock.yaml")).toEqual([null, "Generated", null, "group:mechanical:AGENTS.md:pnpm-lock.yaml"])
  })

  it("names in the bar a use left outside by the step of the section before it, a group after the story from any of its files, a file of no unit by Other changes, and has no bar in a story that tells no step", () => {
    const change = openChanged([4], 1)
    const units = [
      unit({ kind: "shape", changes: ["parameters"] }, "open", [{ path: "src/Api.kt", blocks: [block(0, 3)] }]),
      unit({ kind: "mechanical", mechanical: "importsOnly" }, null, [
        { path: "src/A.kt", blocks: [block(0, 1)] },
        { path: "src/B.kt", blocks: [block(0, 1)] },
      ]),
    ]
    const chapters = concernChapters([concern("p", { source: "token", token: "open" }, [part("shape:open", "src/Api.kt")])])
    const files = [file("src/Api.kt", [3]), file("src/A.kt", [1]), file("src/B.kt", [1]), file("README.md", null)]
    const sections = presentNarrative(units, files, [change], [], chapters)
    expect(sections.map(section => [section.key, section.concern, section.groupName])).toEqual([
      ["file:src/Api.kt", "p", null],
      ["ghost:shape:open:src/Use.kt:4", null, null],
      ["group:mechanical:src/A.kt:src/A.kt", null, "Imports only"],
      ["group:mechanical:src/A.kt:src/B.kt", null, "Imports only"],
      ["README.md", null, null],
    ])
    const bar = (section: string) => {
      const view = presentStepBar(chapters, sections, { opening: null, section })
      return view === null ? null : [view.position, view.label, view.chapter?.id ?? null, view.section?.key ?? null]
    }
    expect(bar("ghost:shape:open:src/Use.kt:4")).toEqual(["Step 1 / 1", "open", "p", null])
    expect(bar("group:mechanical:src/A.kt:src/B.kt")).toEqual([null, "Imports only", null, "group:mechanical:src/A.kt:src/A.kt"])
    // a file no unit names, as the tree lists it; with the rest, Other changes is its chapter
    expect(bar("README.md")).toEqual([null, "Other changes", null, "README.md"])
    const withRest = [...chapters, ...concernChapters([concern("r", { source: "rest" }, [])]).map(rest => ({ ...rest, number: 2 }))]
    expect(presentStepBar(withRest, sections, { opening: null, section: "README.md" })?.chapter?.id).toBe("r")
    // a story of no told step has no bar
    expect(
      presentStepBar(concernChapters([concern("r", { source: "rest" }, [part("shape:open", "src/Api.kt")])]), sections, { opening: null, section: "file:src/Api.kt" }),
    ).toBeNull()
  })

  it("keeps the order of the units while the plan has no concern but the rest", () => {
    const units = [
      unit({ kind: "behaviour" }, "open", [{ path: "src/Api.kt", blocks: [block(0, 10)] }]),
      unit({ kind: "behaviour" }, "use", [{ path: "src/Use.kt", blocks: [block(0, 3)] }]),
    ]
    const rest = concern("r", { source: "rest" }, [part("behaviour:use", "src/Use.kt"), part("behaviour:open", "src/Api.kt")])
    const sections = presentNarrative(units, [file("src/Api.kt", [10]), file("src/Use.kt", [3])], [], [], concernChapters([rest]))
    expect(sections.map(section => [section.key, section.concern, section.concernHeading])).toEqual([
      ["file:src/Api.kt", null, null],
      ["file:src/Use.kt", null, null],
    ])
  })
})
