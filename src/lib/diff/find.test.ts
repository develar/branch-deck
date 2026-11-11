// @spec openspec/specs/commit/commit-review/spec.md
import { describe, expect, it } from "vite-plus/test"
import { findHits, findInSegments, hitsByLine, lineKey, mergeHits, rowOfHit } from "@/lib/diff/find"
import type { DiffFindHit, FindFile, TextFindHit } from "@/lib/diff/find"
import { buildLineTable } from "@/lib/diff/line-table"
import { diffCells } from "@/lib/diff/row-cells"
import { buildFileRows, NEW, OLD, rowOfLine } from "@/lib/diff/rows"
import type { DiffViewMode } from "@/stores/settings/diff-view-settings"
import { block, move, rename, run, textDiff } from "@/test-support/diff-blocks"

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

/** A diff with marks as a mode shows it, by its texts; `open`: the notes opened, by the first old line each hides */
const marked =
  (diff: ReturnType<typeof textDiff>, tables: FindFile["tables"]) =>
  (mode: DiffViewMode, open: number[] = []): FindFile => ({
    rows: buildFileRows(diff, mode, [], false, null, new Set(open)),
    tables,
    ranges: diffCells(diff, mode).ranges,
  })
/** The hits as `-line:start` of the old side or `+line:start` of the new one */
const lines = (hits: readonly DiffFindHit[]) => hits.map(hit => `${hit.side === OLD ? "-" : "+"}${hit.line}:${hit.start}`)

describe("the find in a quiet run", () => {
  // `call(first, second)` wrapped over three lines, a quiet run of formatting
  const diff = textDiff(3, 5, [block(1, 2, 1, 4)], { runs: [run(1, 2, 1, 4)] })
  const shown = marked(diff, [buildLineTable("a\ncall(first, second)\nz\n"), buildLineTable("a\ncall(\n  first, second\n)\nz\n")])

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

describe("the find in the lines of a move", () => {
  // `fun total()` and its two lines move from the end of the file to its start, the second line edited on the way
  const tables = [buildLineTable("a\nb\nfun total() {\n  sum(first)\n}\n"), buildLineTable("fun total() {\n  sum(second)\n}\na\nb\n")]
  const diff = textDiff(6, 6, [block(0, 0, 0, 3), block(2, 5, 5, 5)], {
    moves: [move(2, 0, 3, { edited: [1], oldRanges: [[], [6, 11, 2], []], newRanges: [[], [6, 12, 2], []] })],
  })
  const shown = marked(diff, tables)

  it("searches the lines that left at the note that stands for them in Inline, after the lines where they stand now", () => {
    const inline = shown("inline")
    // the lines once where they stand now, then the old lines behind the note of where they left
    const hits = findHits([inline], "total", { sideBySide: false, folded: false })
    expect(lines(hits)).toEqual(["+0:4", "-2:4"])
    expect(hits.map(hit => rowOfLine(inline.rows!, hit.side, hit.line))).toEqual([1, 6])
    // the edited line is the row of its two lines where the lines stand now, searched by its new line alone: the word
    // the old line had is one hit, of the old line where it left, at the note that stands for it, which counts it once
    const lost = findHits([inline], "first", { sideBySide: false, folded: false })
    expect(lines(lost)).toEqual(["-3:6"])
    expect(lost.map(hit => rowOfLine(inline.rows!, hit.side, hit.line))).toEqual([6])
    expect(hitsByLine(lost, 0).get(0)!.get(lineKey(OLD, 3))).toEqual({ ranges: [6, 11], current: 0 })
    expect(lines(findHits([inline], "second", { sideBySide: false, folded: false }))).toEqual(["+1:6"])
    // text both lines of the row have: the new line's in the row, the old line's behind the note; no hit twice
    const shared = findHits([inline], "sum", { sideBySide: false, folded: false })
    expect(lines(shared)).toEqual(["+1:2", "-3:2"])
    expect(new Set(shared.map(hit => `${hit.side}:${hit.line}:${hit.start}`)).size).toBe(shared.length)
  })

  it("finds the lines that left by their own rows once the note is opened, and where every line stays", () => {
    const open = shown("inline", [2])
    const hits = findHits([open], "total", { sideBySide: false, folded: false })
    expect(lines(hits)).toEqual(["+0:4", "-2:4"])
    expect(hits.map(hit => rowOfLine(open.rows!, hit.side, hit.line))).toEqual([1, 7])
    // the word the edited line lost: one hit, in the row of the old line where it left, not in the row of the pair
    const lost = findHits([open], "first", { sideBySide: false, folded: false })
    expect(lines(lost)).toEqual(["-3:6"])
    expect(lost.map(hit => rowOfLine(open.rows!, hit.side, hit.line))).toEqual([8])
    expect(lines(findHits([shown("unified")], "first", { sideBySide: false, folded: false }))).toEqual(["-3:6"])
    expect(lines(findHits([shown("split")], "total", { sideBySide: true, folded: false }))).toEqual(["+0:4", "-2:4"])
  })
})

describe("the find in the lines of a renamed name", () => {
  // the local `total` is `sum` on three lines, the name alone at each (a run of the kind `renamed` a block), twice on the last
  const OLD_LINES = ["fun balance(): Long {", "  var total = 0L", "  total += amount", "  return total + total", "}"]
  const NEW_LINES = OLD_LINES.map(line => line.replaceAll("total", "sum"))
  const renamedFile = (quiet: boolean, mode: DiffViewMode, open: number[] = []): FindFile => {
    const pair = (line: number, ...columns: number[]) =>
      block(line, line + 1, line, line + 1, {
        inlinePairs: true,
        oldRanges: [columns.flatMap(at => [at, at + 5, 2])],
        newRanges: [columns.flatMap((at, index) => [at - 2 * index, at - 2 * index + 3, 2])],
      })
    const diff = textDiff(5, 5, [pair(1, 6), pair(2, 2), pair(3, 9, 17)], {
      runs: [1, 2, 3].map(line => run(line, line + 1, line, line + 1, { kind: "renamed", quiet })),
      renames: [
        rename("total", "sum", [
          [1, 6, 1, 6],
          [2, 2, 2, 2],
          [3, 9, 3, 9],
          [3, 17, 3, 15],
        ]),
      ],
    })
    const cells = diffCells(diff, mode)
    return {
      rows: buildFileRows(diff, mode, [], false, null, new Set(open)),
      tables: [buildLineTable(OLD_LINES.join("\n")), buildLineTable(NEW_LINES.join("\n"))],
      ranges: cells.ranges,
      renamed: cells.renamed,
      renamedRun: cells.renamedRun,
    }
  }
  /** The hits as `-line:start` or `+line:start`, in the order the find steps through them */
  const lines = (hits: readonly DiffFindHit[]) => hits.map(hit => `${hit.side === OLD ? "-" : "+"}${hit.line}:${hit.start}`)
  const search = (file: FindFile, query: string, sideBySide = false) => findHits([file], query, { sideBySide, folded: false })

  it("finds the old name where the row of a pair of Inline strikes it through, and at the row of a quiet line shown once, whose new name stands for it", () => {
    for (const quiet of [false, true]) {
      const file = renamedFile(quiet, "inline")
      // one hit a site, of the old line, at the row that shows its new line (rows 1 to 4 after the note)
      const old = search(file, "total")
      expect(lines(old), String(quiet)).toEqual(["-1:6", "-2:2", "-3:9", "-3:17"])
      expect(old.map(hit => rowOfHit(file.rows!, hit))).toEqual([2, 3, 4, 4])
      // where no row shows the old line, each hit says the new name that stands for it: its line and its columns
      expect(old.map(hit => hit.stand)).toEqual(
        quiet
          ? [
              { line: 1, start: 6, end: 9 },
              { line: 2, start: 2, end: 5 },
              { line: 3, start: 9, end: 12 },
              { line: 3, start: 15, end: 18 },
            ]
          : [undefined, undefined, undefined, undefined],
      )
      // the new name is a hit of the new line, as any text of it
      expect(lines(search(file, "sum"))).toEqual(["+1:6", "+2:2", "+3:9", "+3:15"])
      // the text both lines have is found once, in the new line: the old line has no row
      expect(lines(search(file, "return"))).toEqual(["+3:2"])
      expect(lines(search(file, "balance"))).toEqual(["+0:4"])
      // a hit that reaches from that text into the old name is the old line's, placed where the name stands in the row
      expect(lines(search(file, "var total"))).toEqual(["-1:2"])
      // and one over two old names is one hit, at the first of them
      expect(lines(search(file, "total + total"))).toEqual(["-3:9"])
      // a search of the old and the new spelling at once steps through the row in the order of its text
      expect(lines(search(file, "t"))).toEqual(["-1:6", "-1:8", "-2:2", "-2:4", "+2:14", "+3:4", "-3:9", "-3:11", "-3:17", "-3:19"])
    }
  })

  it("finds the old name in its own lines where they show: under the opened note of Inline, in Unified and on the old side of Split", () => {
    const old = ["-1:6", "-2:2", "-3:9", "-3:17"]
    const opened = renamedFile(true, "inline", [1])
    expect(lines(search(opened, "total"))).toEqual(old)
    // each in the row of its old line, before the row of the new one
    expect(search(opened, "total").map(hit => rowOfHit(opened.rows!, hit))).toEqual([2, 4, 6, 6])
    expect(search(opened, "total").every(hit => hit.stand === undefined)).toBe(true)
    // the text both lines have is found in both, as wherever both show
    expect(lines(search(opened, "return"))).toEqual(["-3:2", "+3:2"])
    for (const quiet of [false, true]) {
      expect(lines(search(renamedFile(quiet, "unified"), "total")), String(quiet)).toEqual(old)
      expect(lines(search(renamedFile(quiet, "split"), "total", true))).toEqual(old)
      expect(lines(search(renamedFile(quiet, "split"), "sum", true))).toEqual(["+1:6", "+2:2", "+3:9", "+3:15"])
    }
  })

  it("gives the painter the hits of the hidden old lines as marks of the new lines that stand for them, the current one marked, two in one name as one", () => {
    const file = renamedFile(true, "inline")
    const hits = search(file, "total")
    const byLine = hitsByLine(hits, 3).get(0)!
    // a mark of the new line, on its new name; no hit of an old line that no row shows
    expect([byLine.get(lineKey(NEW, 1)), byLine.get(lineKey(NEW, 3)), byLine.get(lineKey(OLD, 1)), byLine.get(lineKey(OLD, 3))]).toEqual([
      { ranges: [], current: -1, stands: [6, 9, 1] },
      { ranges: [], current: -1, stands: [9, 12, 1, 15, 18, 2] },
      undefined,
      undefined,
    ])
    // no hit comes twice
    expect(new Set(hits.map(hit => `${hit.side}:${hit.line}:${hit.start}`)).size).toBe(hits.length)
    // two hits in one old name are two hits and one mark of its new name, the current hit where either is; the hits
    // of the new line itself stay its own
    const letters = search(file, "t")
    expect(lines(letters).slice(0, 5)).toEqual(["-1:6", "-1:8", "-2:2", "-2:4", "+2:14"])
    expect([hitsByLine(letters, 1).get(0)!.get(lineKey(NEW, 1)), hitsByLine(letters, 4).get(0)!.get(lineKey(NEW, 2))]).toEqual([
      { ranges: [], current: -1, stands: [6, 9, 2] },
      { ranges: [14, 15], current: 0, stands: [2, 5, 1] },
    ])
    // where the old lines show, the hits are their own: no line stands for another
    const shown = hitsByLine(search(renamedFile(false, "inline"), "total"), 0).get(0)!
    expect([shown.get(lineKey(OLD, 1)), shown.get(lineKey(NEW, 1))]).toEqual([{ ranges: [6, 11], current: 0 }, undefined])
  })
})

describe("the find in a run of a renamed name whose lines were wrapped otherwise", () => {
  // `  foo(a, total)` is `  foo(a,` and `    sum)`: a quiet run of one old line and two new ones, the name on the second
  const OLD_LINES = ["fun f() {", "  foo(a, total)", "}"]
  const NEW_LINES = ["fun f() {", "  foo(a,", "    sum)", "}"]
  const wrappedFile = (open: number[] = [], oldLines = OLD_LINES): FindFile => {
    const diff = textDiff(3, 4, [block(1, 2, 1, 3)], { runs: [run(1, 2, 1, 3, { kind: "renamed" })], renames: [rename("total", "sum", [[1, 9, 2, 4]])] })
    const cells = diffCells(diff, "inline")
    return {
      rows: buildFileRows(diff, "inline", [], false, null, new Set(open)),
      tables: [buildLineTable(oldLines.join("\n")), buildLineTable(NEW_LINES.join("\n"))],
      ranges: cells.ranges,
      renamed: cells.renamed,
      renamedRun: cells.renamedRun,
    }
  }
  const search = (file: FindFile, query: string) => findHits([file], query, { sideBySide: false, folded: false })
  /** The hits with where each stands and the row it is revealed at: `-1:9 > 2:4..7 @3` */
  const stood = (file: FindFile, query: string) =>
    search(file, query).map(hit => `${lines([hit])[0]}${hit.stand === undefined ? "" : ` > ${hit.stand.line}:${hit.stand.start}..${hit.stand.end}`} @${rowOfHit(file.rows!, hit)}`)

  it("finds the old name at the row whose new line holds the new name, not at the first row of the run", () => {
    // the rows: an unchanged line, the first new line, the note at the line of the name, the second new line
    const file = wrappedFile()
    expect(stood(file, "total")).toEqual(["-1:9 > 2:4..7 @3"])
    expect(stood(file, "a, total")).toEqual(["-1:6 > 2:4..7 @3"])
    expect(hitsByLine(search(file, "total"), 0).get(0)!.get(lineKey(NEW, 2))).toEqual({ ranges: [], current: -1, stands: [4, 7, 2] })
  })

  it("finds text that only the hidden old line holds in one piece at the code of the new lines that stands for it, and text both sides hold once", () => {
    const file = wrappedFile()
    // `(a, ` ends with a space the new line has not: found, at `(a,` of the first new line, marked there
    expect(stood(file, "(a, ")).toEqual(["-1:5 > 1:5..8 @1"])
    expect(hitsByLine(search(file, "(a, "), 0).get(0)!.get(lineKey(NEW, 1))).toEqual({ ranges: [], current: -1, stands: [5, 8, 2] })
    // `, ` likewise; a text that spans the place of the wrap stands as far as its first line holds it
    expect(stood(file, ", ")).toEqual(["-1:7 > 1:7..8 @1"])
    // text the new lines hold at the same place is the hit of the new line alone
    expect([stood(file, "foo(a"), stood(file, ")"), stood(file, "(a,")]).toEqual([["+1:2 @1"], ["+0:6 @0", "+2:7 @3"], ["+1:5 @1"]])
    // whitespace alone before code that the new lines have whitespace before too (the indent, the space the wrap
    // turned into an indent) is the new line's
    expect(stood(file, "  ")).toEqual(["+1:0 @1", "+2:0 @3", "+2:2 @3"])
    expect(stood(file, " ").filter(hit => hit.startsWith("-"))).toEqual([])
    // a space the new lines have none for, before `)`: found where that code stands, with nothing to mark
    const spaced = wrappedFile([], ["fun f() {", "  foo(a, total )", "}"])
    expect(stood(spaced, " ").filter(hit => hit.startsWith("-"))).toEqual(["-1:14 > 2:7..7 @3"])
    expect(hitsByLine(search(spaced, " "), 0).get(0)!.get(lineKey(NEW, 2))?.stands).toBeUndefined()
  })

  it("finds every hit of the old line in its own row once the note of the rename is opened", () => {
    const opened = wrappedFile([1])
    expect(stood(opened, "total")).toEqual(["-1:9 @3"])
    expect(stood(opened, "(a, ")).toEqual(["-1:5 @3"])
    expect(stood(opened, "foo(a")).toEqual(["+1:2 @1", "-1:2 @3"])
  })

  it("counts every hit of an old line whose code is not the code of the new lines, which no engine gives, at the first row of the run", () => {
    const other = wrappedFile([], ["fun f() {", "  bar(b, total)", "}"])
    expect(stood(other, "(b")).toEqual(["-1:5 > 1:0..0 @1"])
    expect(stood(other, "total")).toEqual(["-1:9 > 2:4..7 @3"])
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
