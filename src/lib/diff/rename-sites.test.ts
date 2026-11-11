// @spec openspec/specs/commit/quiet-changes/spec.md
import { describe, expect, it } from "vite-plus/test"
import { RENAMED_RANGE } from "@/lib/diff/line-parts"
import { pairedWithSites, renamedRunOf, renameSites, runUnits, sameUnits, sitesOfLine, unitFrom, unitsIn, withSites } from "@/lib/diff/rename-sites"
import { block, rename, run, textDiff } from "@/test-support/diff-blocks"

const OLD = 0
const NEW = 1
const R = RENAMED_RANGE

// `var total = 0` / `total += total` become `var sum = 0` / `sum += sum`: three sites of one rename on two lines, and
// a second rename with one site on the first line (`count` → `n`, after the first name)
const RENAMED = textDiff(10, 12, [block(2, 3, 2, 3), block(5, 6, 7, 8)], {
  renames: [
    rename("total", "sum", [
      [2, 4, 2, 4],
      [5, 0, 7, 0],
      [5, 9, 7, 7],
    ]),
    rename("count", "n", [[2, 12, 2, 10]]),
  ],
})

describe("renameSites", () => {
  it("gives the sites of every rename by side and line, in the order of their columns, each with the same site of the other side", () => {
    const sites = renameSites(RENAMED)
    expect(sitesOfLine(sites, NEW, 7).map(site => [site.rename.new, site.place, site.start, site.end])).toEqual([
      ["sum", 1, 0, 3],
      ["sum", 2, 7, 10],
    ])
    expect(sitesOfLine(sites, OLD, 2).map(site => [site.rename.old, site.index, site.start, site.end])).toEqual([
      ["total", 0, 4, 9],
      ["count", 1, 12, 17],
    ])
    const [first] = sitesOfLine(sites, NEW, 2)
    expect([first!.twin.side, first!.twin.line, first!.twin.start, first!.twin.end, first!.twin.twin]).toEqual([OLD, 2, 4, 9, first])
    expect(sites.ofRename.map(places => places.length)).toEqual([3, 1])
    // no site on any other line, and the sites are read once per diff
    expect([sitesOfLine(sites, NEW, 3), sitesOfLine(sites, OLD, 7)]).toEqual([[], []])
    expect(renameSites(RENAMED)).toBe(sites)
    expect(renameSites(textDiff(3, 3, [block(1, 2, 1, 2)])).ofRename).toEqual([])
  })

  it("leaves out a place a rename names on one side alone, which no engine gives", () => {
    const lopsided = textDiff(4, 4, [block(1, 2, 1, 2)], { renames: [{ ...rename("a", "b", [[1, 0, 1, 0]]), oldSites: [1, 0, 1, 3, 0, 1] }] })
    expect(renameSites(lopsided).ofRename[0]!.length).toBe(1)
    expect(sitesOfLine(renameSites(lopsided), OLD, 3)).toEqual([])
  })
})

describe("withSites", () => {
  const sites = sitesOfLine(renameSites(RENAMED), NEW, 2)

  it("makes each renamed name a range of its own kind and leaves out what the engine marked inside it", () => {
    // the engine marked the two names as modified: the names alone are left, as renamed
    expect(withSites([4, 7, 2, 10, 11, 2], sites)).toEqual([4, 7, R, 10, 11, R])
    // a line of no range of the engine (a quiet one) has its names all the same
    expect(withSites(null, sites)).toEqual([4, 7, R, 10, 11, R])
    // a part of a name (the letters the two spellings do not share) goes with the name
    expect(withSites([5, 6, 2], sites)).toEqual([4, 7, R, 10, 11, R])
    // a line without a site keeps its ranges, the same object
    const own = [1, 2, 0]
    expect(withSites(own, [])).toBe(own)
    expect(withSites(null, [])).toBeNull()
  })

  it("keeps what a range of the engine holds outside a name, before it, after it and between two names", () => {
    // one range over both names and the text around them: `x sum + n y`
    expect(withSites([2, 13, 2], sites)).toEqual([2, 4, 2, 4, 7, R, 7, 10, 2, 10, 11, R, 11, 13, 2])
    // another change of the line beside the names stays what it is
    expect(withSites([0, 3, 0, 4, 7, 2, 14, 16, 1], sites)).toEqual([0, 3, 0, 4, 7, R, 10, 11, R, 14, 16, 1])
  })

  it("takes the marker of an empty side that touches a name with the name, and keeps one apart from it", () => {
    // `total` → `totals` is an insertion at the end of the old name: no bar after the name, which is marked as a whole
    expect(withSites([7, 7, 0], sites)).toEqual([4, 7, R, 10, 11, R])
    expect(withSites([4, 4, 1], sites)).toEqual([4, 7, R, 10, 11, R])
    expect(withSites([8, 8, 1], sites)).toEqual([4, 7, R, 8, 8, 1, 10, 11, R])
  })
})

describe("pairedWithSites", () => {
  const sites = sitesOfLine(renameSites(RENAMED), NEW, 2)

  it("makes the name of both lines one range of the pair, whatever the engine marked inside it", () => {
    // the engine marked `total` → `sum` and `count` → `n` as two modified ranges
    expect(pairedWithSites([4, 9, 2, 12, 17, 2], [4, 7, 2, 10, 11, 2], sites)).toEqual([
      [4, 9, R, 12, 17, R],
      [4, 7, R, 10, 11, R],
    ])
    // or only the letters that differ: three ranges inside the first name become one
    expect(pairedWithSites([4, 5, 2, 6, 6, 0, 8, 9, 1, 12, 17, 2], [4, 5, 2, 6, 7, 0, 7, 7, 1, 10, 11, 2], sites)).toEqual([
      [4, 9, R, 12, 17, R],
      [4, 7, R, 10, 11, R],
    ])
    // an insertion at the end of the old name (`total` → `totals`) is a part of the name
    const longer = sitesOfLine(renameSites(textDiff(3, 3, [block(1, 2, 1, 2)], { renames: [rename("total", "totals", [[1, 4, 1, 4]])] })), NEW, 1)
    expect(pairedWithSites([9, 9, 0], [9, 10, 0], longer)).toEqual([
      [4, 9, R],
      [4, 10, R],
    ])
  })

  it("cuts a range that reaches out of a name at the name: what is left is a range of its own, both lists as long as each other", () => {
    // one modified range from `total + 1` to `count`: `sum * 2` to `n` in the new line
    const [before, after] = pairedWithSites([4, 17, 2], [4, 11, 2], sites)
    expect(before).toEqual([4, 9, R, 9, 12, 2, 12, 17, R])
    expect(after).toEqual([4, 7, R, 7, 10, 2, 10, 11, R])
    // a range that starts before the name on the old side alone is a deletion before it; text inserted after it an insertion
    expect(pairedWithSites([2, 9, 2, 12, 17, 2], [4, 7, 2, 10, 14, 2], sites)).toEqual([
      [2, 4, 1, 4, 9, R, 12, 17, R, 17, 17, 0],
      [4, 4, 1, 4, 7, R, 10, 11, R, 11, 14, 0],
    ])
    // the other changes of the pair stay as they are, before, between and after the names
    expect(pairedWithSites([0, 2, 2, 4, 9, 2, 10, 10, 0, 12, 17, 2, 19, 20, 1], [0, 3, 2, 4, 7, 2, 8, 9, 0, 10, 11, 2, 13, 13, 1], sites)).toEqual([
      [0, 2, 2, 4, 9, R, 10, 10, 0, 12, 17, R, 19, 20, 1],
      [0, 3, 2, 4, 7, R, 8, 9, 0, 10, 11, R, 13, 13, 1],
    ])
  })

  it("leaves ranges that are no pair, and a pair without a site, as they are", () => {
    const [old, added] = [
      [0, 20, 2],
      [0, 5, 2, 7, 9, 2],
    ]
    expect(pairedWithSites(old, added, sites)).toEqual([old, added])
    expect(pairedWithSites([1, 2, 2], [1, 3, 2], [])).toEqual([
      [1, 2, 2],
      [1, 3, 2],
    ])
  })
})

describe("the text of a run of a renamed name, unit by unit", () => {
  // `  foo(a, total)` is `  foo(a,` and `    sum)`: one old line, two new ones
  const wrapped = textDiff(3, 4, [block(1, 2, 1, 3)], { runs: [run(1, 2, 1, 3, { kind: "renamed" })], renames: [rename("total", "sum", [[1, 9, 2, 4]])] })
  const OLD_LINES = ["fun f() {", "  foo(a, total)", "}"]
  const NEW_LINES = ["fun f() {", "  foo(a,", "    sum)", "}"]
  const sites = renameSites(wrapped)
  const old = runUnits(
    line => OLD_LINES[line]!,
    line => sitesOfLine(sites, OLD, line),
    1,
    2,
  )
  const now = runUnits(
    line => NEW_LINES[line]!,
    line => sitesOfLine(sites, NEW, line),
    1,
    3,
  )

  it("reads each side as its characters outside whitespace and each renamed name as one unit: the same units on both sides, however the lines are wrapped", () => {
    expect(old.codes.map(code => (code < 0 ? "<name>" : String.fromCharCode(code))).join("")).toBe("foo(a,<name>)")
    expect([old.count, old.lines, old.starts, old.ends, old.offsets]).toEqual([8, [1, 1, 1, 1, 1, 1, 1, 1], [2, 3, 4, 5, 6, 7, 9, 14], [3, 4, 5, 6, 7, 8, 14, 15], [0, 8]])
    expect([now.lines, now.starts, now.ends, now.offsets]).toEqual([
      [1, 1, 1, 1, 1, 1, 2, 2],
      [2, 3, 4, 5, 6, 7, 4, 7],
      [3, 4, 5, 6, 7, 8, 7, 8],
      [0, 6, 8],
    ])
    expect(sameUnits(old, now)).toBe(true)
    // another character, and a name where the other side has its letters, is another text
    const other = runUnits(
      () => "  foo(b, total)",
      line => sitesOfLine(sites, OLD, line),
      1,
      2,
    )
    const unnamed = runUnits(
      () => "  foo(a, total)",
      () => [],
      1,
      2,
    )
    expect([sameUnits(other, now), sameUnits(unnamed, now)]).toEqual([false, false])
  })

  it("gives the units a range of a line holds a part of, and the unit at or after a place", () => {
    // `(a, ` of the old line holds `(`, `a` and `,`; a part of the name is the name
    expect([unitsIn(old, 1, 5, 9), unitsIn(old, 1, 11, 12), unitsIn(now, 2, 0, 5)]).toEqual([
      [3, 5],
      [6, 6],
      [6, 6],
    ])
    // whitespace alone, and a line of no unit, hold none
    expect([unitsIn(old, 1, 0, 2), unitsIn(old, 1, 8, 9), unitsIn(old, 5, 0, 9)]).toEqual([null, null, null])
    // the unit at or after a column: on its line, else the first of the next line; the count after the last and for a line outside
    expect([unitFrom(old, 1, 0), unitFrom(old, 1, 8), unitFrom(now, 1, 8), unitFrom(now, 2, 8), unitFrom(now, 9, 0)]).toEqual([0, 6, 6, 8, 8])
  })

  it("finds the run of a renamed name that holds a new line, never a run of another kind", () => {
    const runs = [run(1, 2, 1, 3, { kind: "renamed" }), run(5, 6, 6, 7), run(9, 10, 10, 11, { kind: "renamed", quiet: false })]
    const diff = textDiff(20, 21, [block(1, 2, 1, 3), block(5, 6, 6, 7), block(9, 10, 10, 11)], { runs })
    expect([0, 1, 2, 3, 6, 10, 11].map(line => renamedRunOf(diff, line))).toEqual([null, runs[0], runs[0], null, null, runs[2], null])
    expect(renamedRunOf(textDiff(3, 3, []), 1)).toBeNull()
  })
})
