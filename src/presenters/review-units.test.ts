import { describe, expect, it } from "vite-plus/test"
import type { ApiChange, Concern, ConcernName, ConcernPart, FileFacts, GeneratedBy, ReviewUnit, UnitBlock, UnitKind } from "@/lib/bindings"
import { formatUnitHeading, presentNarrative } from "@/presenters/review-units"
import type { NarrativeFile } from "@/presenters/review-units"

/** Block `index` of a file: one line changed at `line` on each side */
function block(index: number, line: number): UnitBlock {
  return { index, oldStart: line, oldEnd: line + 1, newStart: line, newEnd: line + 1 }
}

function unit(kind: UnitKind, name: string | null, slices: { path: string; blocks: UnitBlock[]; whole?: boolean }[]): ReviewUnit {
  return {
    id: `${kind.kind}:${name ?? slices[0]?.path}`,
    kind,
    name,
    key: name,
    signature: null,
    public: true,
    line: null,
    slices: slices.map(slice => ({ path: slice.path, oldPath: null, whole: slice.whole ?? false, blocks: slice.blocks, fingerprint: "f" })),
    fingerprint: "f",
  }
}

/** A file whose diff has blocks at `lines` */
function file(path: string, lines: number[] | null): NarrativeFile {
  return { path, blocks: lines === null ? null : lines.map((line, index) => block(index, line)) }
}

describe("the story of a review", () => {
  it("words a change by its kind", () => {
    expect(formatUnitHeading(unit({ kind: "shape", changes: ["parameters", "reach"] }, "open", []))).toBe("open — form changed: parameters, visibility")
    expect(formatUnitHeading(unit({ kind: "behaviour" }, "close", []))).toBe("close — body changed")
    expect(formatUnitHeading(unit({ kind: "other", reason: "outside" }, "Store.kt", []))).toBe("Changes outside a declaration")
  })

  it("makes a run of changes of one file one section with a heading per change, and a group a section per file", () => {
    const units = [
      unit({ kind: "shape", changes: ["parameters"] }, "open", [{ path: "src/Api.kt", blocks: [block(1, 10)] }]),
      unit({ kind: "behaviour" }, "close", [{ path: "src/Api.kt", blocks: [block(2, 20)] }]),
      unit({ kind: "removed" }, "gone", [{ path: "src/Api.kt", blocks: [] }]),
      unit({ kind: "behaviour" }, "use", [{ path: "src/Use.kt", blocks: [block(0, 3)] }]),
      unit({ kind: "repeatedEdit", group: 0 }, null, [
        { path: "src/A.kt", blocks: [block(0, 1)] },
        { path: "src/B.kt", blocks: [block(0, 1)] },
      ]),
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
    expect([...api.units!.marks]).toEqual([
      [1, "open — form changed: parameters"],
      [2, "close — body changed · gone — removed"],
    ])
    // the block of the imports is one row here, which opens the group
    expect([...api.units!.elsewhere]).toEqual([[0, "Shown in Imports only (1)"]])
    expect(api.targets.get(0)).toBe("group:mechanical:src/Api.kt:src/Api.kt")
  })

  it("shows a deleted file whole in its place, and the files deleted from one directory as a group, each collapsed", () => {
    expect(formatUnitHeading(unit({ kind: "deletedFile" }, "config.toml", []))).toBe("config.toml — deleted file")
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
    const usages = { inside: 0, outside: 7, outsideSites: [1, 2, 3, 4, 5, 6].map(line => ({ path: "src/Use.kt", line })), complete: true, updated: false }
    const change: ApiChange = { path: "src/Api.kt", line: 3, name: "open", key: "open", kind: "changed", changes: ["parameters"], before: null, after: null, usages }
    const units = [unit({ kind: "shape", changes: ["parameters"] }, "open", [{ path: "src/Api.kt", blocks: [block(0, 3)] }])]
    const sections = presentNarrative(units, [file("src/Api.kt", [3])], [change], [], [])
    expect(sections.map(section => section.key)).toEqual(["file:src/Api.kt", ...[1, 2, 3, 4, 5].map(line => `ghost:shape:open:src/Use.kt:${line}`)])
    expect(sections[1]!.ghost).toEqual({ line: 1, name: "open", heading: "open — left outside: src/Use.kt:1" })
  })
})

describe("the concerns in the story", () => {
  const concern = (id: string, name: ConcernName, parts: ConcernPart[]): Concern => ({
    id,
    name,
    parts,
    paths: [...new Set(parts.map(part => part.path))],
    elsewhere: [],
    evidence: [],
    fingerprint: id,
  })
  const part = (unit: string, path: string, blocks: number[] | null = null): ConcernPart => ({ unit, path, blocks })

  it("lays the parts of each concern out together under its name, a unit cut by the concerns in each, its generated file collapsed at its end, the mechanical groups after", () => {
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
    const sections = presentNarrative(units, files, [], facts, concerns)
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
    // the guide shows the block of each concern, the other one a row that opens its section
    const [, ofProxy, , ofSearch] = sections
    expect([[...ofProxy!.units!.marks.keys()], [...ofProxy!.units!.elsewhere]]).toEqual([[1], [[0, "Shown in grep → accio"]]])
    expect([[...ofSearch!.units!.marks.keys()], ofSearch!.targets.get(1)]).toEqual([[0], "file:docs/guide.md"])
  })

  it("keeps the order of the units while the plan has no concern but the rest", () => {
    const units = [
      unit({ kind: "behaviour" }, "open", [{ path: "src/Api.kt", blocks: [block(0, 10)] }]),
      unit({ kind: "behaviour" }, "use", [{ path: "src/Use.kt", blocks: [block(0, 3)] }]),
    ]
    const rest = concern("r", { source: "rest" }, [part("behaviour:use", "src/Use.kt"), part("behaviour:open", "src/Api.kt")])
    const sections = presentNarrative(units, [file("src/Api.kt", [10]), file("src/Use.kt", [3])], [], [], [rest])
    expect(sections.map(section => [section.key, section.concern, section.concernHeading])).toEqual([
      ["file:src/Api.kt", null, null],
      ["file:src/Use.kt", null, null],
    ])
  })
})
