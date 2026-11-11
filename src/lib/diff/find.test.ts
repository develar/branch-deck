// @spec openspec/specs/commit/commit-review/spec.md
import { describe, expect, it } from "vite-plus/test"
import { findHits, findInSegments, hitsByLine, lineKey, mergeHits } from "@/lib/diff/find"
import type { DiffFindHit, FindFile, TextFindHit } from "@/lib/diff/find"
import { buildLineTable } from "@/lib/diff/line-table"
import { diffCells } from "@/lib/diff/row-cells"
import { buildFileRows, NEW, OLD, rowOfLine } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import { block, run, textDiff } from "@/test-support/diff-blocks"

const OLD_TEXT = Array.from({ length: 30 }, (_, index) => (index === 20 ? "val Total = 1" : `val total${index} = ${index}`)).join("\n")
const NEW_TEXT = OLD_TEXT.replace("val Total = 1", "val total = 2")

function file(mode: DiffViewMode): FindFile {
  const diff = textDiff(30, 30, [block(20, 21, 20, 21)])
  return { rows: buildFileRows(diff, mode), tables: [buildLineTable(OLD_TEXT), buildLineTable(NEW_TEXT)], ranges: diffCells(diff, mode).ranges }
}

/** One Inline row of a similar pair, with the ranges of the engine: `[start, end, kind]` of each line */
function inlineFile(oldLine: string, newLine: string, oldRanges: number[], newRanges: number[]): FindFile {
  const diff = textDiff(1, 1, [block(0, 1, 0, 1, { inlinePairs: true, oldRanges: [oldRanges], newRanges: [newRanges] })])
  return { rows: buildFileRows(diff, "inline"), tables: [buildLineTable(oldLine), buildLineTable(newLine)], ranges: diffCells(diff, "inline").ranges }
}

/** The hits as `-old` or `+new` with their offsets, in the order the find steps through them */
function places(hits: readonly DiffFindHit[]): string[] {
  return hits.map(hit => `${hit.side === OLD ? "-" : "+"}${hit.start}..${hit.end}`)
}

describe("findHits", () => {
  it("finds the query in the lines the rows show, ignoring case, in the order of the rows", () => {
    // Unified: the context 16-19, -20, +20, 21-24; the folds are not searched
    const hits = findHits([file("unified")], "TOTAL", { sideBySide: false, folded: false })
    expect(hits.map(hit => `${hit.side === OLD ? "-" : "+"}${hit.line}`)).toEqual(["+16", "+17", "+18", "+19", "-20", "+20", "+21", "+22", "+23", "+24"])
    expect(hits[4]).toEqual({ kind: "diff", file: 0, side: OLD, line: 20, start: 4, end: 9, gap: -1 })
  })

  it("searches both sides of a change in Split and the new side of an unchanged line, and the folds on request", () => {
    const split = findHits([file("split")], "total", { sideBySide: true, folded: false })
    expect(split.filter(hit => hit.line === 20).map(hit => hit.side)).toEqual([OLD, NEW])
    expect(split.filter(hit => hit.line === 19).map(hit => hit.side)).toEqual([NEW])
    const folded = findHits([file("split")], "total3 ", { sideBySide: true, folded: true })
    // line 3 lies in the first fold, the gap before the change
    expect(folded).toEqual([{ kind: "diff", file: 0, side: NEW, line: 3, start: 4, end: 11, gap: 0 }])
    expect(findHits([file("split")], "", { sideBySide: true, folded: true })).toEqual([])
  })

  it("searches the deleted rows of Unified and of Inline by their old line, and the lines of a deleted file", () => {
    // a block without a similar pair is a deleted and an added row in Inline, as in Unified
    for (const mode of ["unified", "inline"] as const) {
      const hits = findHits([file(mode)], "Total = 1", { sideBySide: false, folded: false })
      expect(places(hits), mode).toEqual(["-4..13"])
      expect(hits[0]).toMatchObject({ side: OLD, line: 20, gap: -1 })
    }
    // a deleted file is its old side alone, in one column in every layout
    const deleted = textDiff(2, 0, [block(0, 2, 0, 0)])
    const gone: FindFile = {
      rows: buildFileRows(deleted, "unified"),
      tables: [buildLineTable("val gone = 1\nval left = 2\n"), null],
      ranges: diffCells(deleted, "inline", OLD).ranges,
    }
    expect(places(findHits([gone], "val", { sideBySide: false, folded: false }))).toEqual(["-0..3", "-0..3"])
  })

  it("searches an Inline row in its new line and in the deleted parts of its old line, in the order of the text of the row", () => {
    // the row shows `val a = ` + struck `string` + inserted `name`
    const pair = inlineFile("val a = string", "val a = name", [8, 14, 2], [8, 12, 2])
    const search = (query: string) => places(findHits([pair], query, { sideBySide: false, folded: false }))
    // a deleted part alone, and an inserted one
    expect(search("string")).toEqual(["-8..14"])
    expect(search("name")).toEqual(["+8..12"])
    // the text both lines share is the text of the new line: one hit, not one of each line
    expect(search("val a")).toEqual(["+0..5"])
    // a hit that reaches from the shared text into the deleted part is one of the old line; the deleted part stands
    // before the inserted one, so the old line comes first
    expect(search("a = ")).toEqual(["+4..8"])
    expect(search("= s")).toEqual(["-6..9"])
    expect(search("= ")).toEqual(["+6..8"])
    expect(search("a = string")).toEqual(["-4..14"])
    // one letter of each part: the shared `a` of `val` and of the name, then the deleted part, then the inserted one
    expect(search("a")).toEqual(["+1..2", "+4..5", "+9..10"])
    expect(search("n")).toEqual(["-12..13", "+8..9"])
  })

  it("finds in an Inline row a text of the old line that the new line cuts with an insertion, or has lost with a deletion", () => {
    // `state` to `state + 1`: the engine marks the empty old side; the old line has no hit the new line lacks
    const inserted = inlineFile("val a = state", "val a = state + 1", [13, 13, 0], [13, 17, 0])
    const search = (target: FindFile, query: string) => places(findHits([target], query, { sideBySide: false, folded: false }))
    expect(search(inserted, "state")).toEqual(["+8..13"])
    // `call(x)` to `call(x, y)`: `x)` stood in the old line, and the row shows it around the inserted `, y`
    const cut = inlineFile("call(x)", "call(x, y)", [6, 6, 0], [6, 9, 0])
    expect(search(cut, "x)")).toEqual(["-5..7"])
    expect(search(cut, "call(x")).toEqual(["+0..6"])
    // `call(x, y)` to `call(x)`: the engine marks the empty new side where `, y` stood
    const deleted = inlineFile("call(x, y)", "call(x)", [6, 9, 1], [6, 6, 1])
    expect(search(deleted, ", y")).toEqual(["-6..9"])
    expect(search(deleted, "x)")).toEqual(["+5..7"])
    // ranges that do not align (no engine pairs such lines): the row is the whole old line before the whole new one
    const apart = inlineFile("abcde", "abXde", [0, 1, 2, 2, 3, 2], [2, 3, 2])
    expect(search(apart, "c")).toEqual(["-2..3"])
    expect(search(apart, "x")).toEqual(["+2..3"])
    expect(search(apart, "ab")).toEqual(["-0..2", "+0..2"])
  })

  it("finds in an Inline row of a whitespace policy the old line by its own text, in the order of the text of the row", () => {
    // Trim Whitespace pairs `    foo(bar)` with the line indented again and edited: the row is the new indent,
    // `foo(`, struck `bar`, inserted `baz`, `)`
    const pair = inlineFile("    foo(bar)", "        foo(baz)", [8, 11, 2], [12, 15, 2])
    const search = (query: string) => places(findHits([pair], query, { sideBySide: false, folded: false }))
    expect(search("foo(bar")).toEqual(["-4..11"])
    expect(search("bar)")).toEqual(["-8..12"])
    // the shared text is the new line's: one hit
    expect(search("foo")).toEqual(["+8..11"])
    // `a` of the deleted part before `a` of the inserted one, both after the shared text
    expect(search("a")).toEqual(["-9..10", "+13..14"])
    expect(search("(")).toEqual(["+11..12"])
    expect(search(")")).toEqual(["+15..16"])
    // the old line after its deleted part is the text after the inserted part of the row
    expect(search("r)")).toEqual(["-10..12"])
    expect(search("z)")).toEqual(["+14..16"])
  })

  it("finds in a line more hits than a call takes arguments", () => {
    // a minified file, one letter typed: every character of its one line is a hit
    const line = "a".repeat(200_000)
    for (const mode of ["unified", "inline", "split"] as const) {
      const diff = textDiff(1, 1, [block(0, 1, 0, 1, { inlinePairs: true })])
      const long: FindFile = { rows: buildFileRows(diff, mode), tables: [buildLineTable(`${line}b`), buildLineTable(line)], ranges: diffCells(diff, mode).ranges }
      const hits = findHits([long], "a", { sideBySide: mode === "split", folded: false })
      // Unified and Split: the old and the new line; Inline: the row of the pair, the text the lines share once
      expect(hits.length, mode).toBe(mode === "inline" ? 200_000 : 400_000)
      expect(hits.at(-1), mode).toMatchObject({ side: NEW, start: 199_999 })
    }
  })

  it("groups the hits by file and line for the painter, the current one marked", () => {
    const hits = findHits([file("unified")], "t", { sideBySide: false, folded: false }).filter(hit => hit.line === 16)
    const byLine = hitsByLine(hits, 1)
    expect(byLine.get(0)?.get(lineKey(NEW, 16))).toEqual({ ranges: [4, 5, 6, 7], current: 1 })
  })
})

describe("findInSegments", () => {
  it("matches ignoring case and finds several hits in one segment", () => {
    expect(findInSegments(["Total total TOTAL"], "tOtAl")).toEqual([
      { segment: 0, start: 0, end: 5 },
      { segment: 0, start: 6, end: 11 },
      { segment: 0, start: 12, end: 17 },
    ])
  })

  it("does not match across segments and returns no hits for an empty query", () => {
    expect(findInSegments(["over", "view"], "overview")).toEqual([])
    expect(findInSegments(["overview"], "")).toEqual([])
  })
})

describe("the find in a quiet run", () => {
  // `call(first, second)` wrapped over three lines, a quiet run of formatting
  const diff = textDiff(3, 5, [block(1, 2, 1, 4)], { runs: [run(1, 2, 1, 4)] })
  const tables = [buildLineTable("a\ncall(first, second)\nz\n"), buildLineTable("a\ncall(\n  first, second\n)\nz\n")]
  const shown = (mode: DiffViewMode, open: number[] = []): FindFile => ({
    rows: buildFileRows(diff, mode, [], false, null, new Set(open)),
    tables,
    ranges: diffCells(diff, mode).ranges,
  })
  const lines = (hits: readonly DiffFindHit[]) => hits.map(hit => `${hit.side === OLD ? "-" : "+"}${hit.line}:${hit.start}`)

  it("searches the old lines that Inline shows once, at the note that stands for them, before the new lines", () => {
    const inline = shown("inline")
    const hits = findHits([inline], "first", { sideBySide: false, folded: false })
    expect(lines(hits)).toEqual(["-1:5", "+2:2"])
    // the hit of the old line is of the row of its note; the hit of the new line of its own row
    expect(hits.map(hit => rowOfLine(inline.rows!, hit.side, hit.line))).toEqual([1, 3])
    // text only the old line has in one piece is found there alone
    expect(lines(findHits([inline], "call(first", { sideBySide: false, folded: false }))).toEqual(["-1:0"])
  })

  it("finds the same hits by the rows of the old lines once the note is opened, and where every line stays", () => {
    const open = shown("inline", [1])
    const hits = findHits([open], "first", { sideBySide: false, folded: false })
    expect(lines(hits)).toEqual(["-1:5", "+2:2"])
    expect(hits.map(hit => rowOfLine(open.rows!, hit.side, hit.line))).toEqual([2, 4])
    expect(lines(findHits([shown("unified")], "first", { sideBySide: false, folded: false }))).toEqual(["-1:5", "+2:2"])
    expect(lines(findHits([shown("split")], "first", { sideBySide: true, folded: false }))).toEqual(["-1:5", "+2:2"])
  })
})

describe("mergeHits", () => {
  it("puts text before diff hits for each file and keeps each list's order", () => {
    const textHit = (file: number): TextFindHit => ({ kind: "text", file, range: {} as unknown as Range, folds: [] })
    const diffHit = (file: number, line: number): DiffFindHit => ({ kind: "diff", file, side: NEW, line, start: 0, end: 1, gap: -1 })
    const prologue = textHit(-1)
    const opening = [textHit(0), textHit(0)]
    const epilogue = textHit(2)
    const rows = [diffHit(0, 1), diffHit(0, 2), diffHit(1, 3)]
    const hits = mergeHits([prologue, ...opening, epilogue], rows)

    expect(hits).toEqual([prologue, ...opening, ...rows, epilogue])
  })
})
