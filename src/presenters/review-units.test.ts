import { describe, expect, it } from "vite-plus/test"
import type { ApiChange, ReviewUnit, UnitBlock, UnitKind } from "@/lib/bindings"
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
    const sections = presentNarrative(units, [file("src/A.kt", [1]), file("src/Api.kt", [2, 10, 20]), file("src/B.kt", [1]), file("src/Use.kt", [3])], [])
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

  it("shows a file whole where its blocks are not those of the units, and a file no unit names after the story", () => {
    const units = [
      unit({ kind: "behaviour" }, "open", [{ path: "src/Api.kt", blocks: [block(0, 10)] }]),
      unit({ kind: "mechanical", mechanical: "whitespace" }, null, [{ path: "src/Api.kt", blocks: [block(1, 20)] }]),
    ]
    // another whitespace policy: the window has one block where the units have two
    const sections = presentNarrative(units, [file("src/Api.kt", [10]), file("README.md", [0])], [])
    expect(sections.map(section => [section.key, section.units])).toEqual([
      ["src/Api.kt", null],
      ["README.md", null],
    ])
  })
})

describe("the uses a change left outside in the story", () => {
  it("follow the section of the change: each its file around the use, at most five", () => {
    const usages = { inside: 0, outside: 7, outsideSites: [1, 2, 3, 4, 5, 6].map(line => ({ path: "src/Use.kt", line })), complete: true, updated: false }
    const change: ApiChange = { path: "src/Api.kt", line: 3, name: "open", key: "open", kind: "changed", changes: ["parameters"], before: null, after: null, usages }
    const units = [unit({ kind: "shape", changes: ["parameters"] }, "open", [{ path: "src/Api.kt", blocks: [block(0, 3)] }])]
    const sections = presentNarrative(units, [file("src/Api.kt", [3])], [change])
    expect(sections.map(section => section.key)).toEqual(["file:src/Api.kt", ...[1, 2, 3, 4, 5].map(line => `ghost:shape:open:src/Use.kt:${line}`)])
    expect(sections[1]!.ghost).toEqual({ line: 1, name: "open", heading: "open — left outside: src/Use.kt:1" })
  })
})
