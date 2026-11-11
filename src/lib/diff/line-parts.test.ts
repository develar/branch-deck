// @spec openspec/specs/commit/commit-files/spec.md
import { describe, expect, it } from "vite-plus/test"
import { inlineOffset, inlinePair, inlineParts, isOwnOldHit, lineParts, ownOldHits, sameTextOffset } from "@/lib/diff/line-parts"
import type { InlinePair, InlinePart, LinePart } from "@/lib/diff/line-parts"

/** The parts as `text:style`, `+` for a changed range, `-` for a deleted part */
function describeParts(text: (part: LinePart) => string, parts: readonly LinePart[]): string[] {
  return parts.map(part => `${text(part)}:${part.style}${part.change >= 0 ? "+" : ""}${part.deleted ? "-" : ""}`)
}

describe("lineParts", () => {
  it("cuts a line at each run of the tokens and each bound of a changed range", () => {
    const line = "val count = 42"
    // a keyword, the default foreground, a number
    const runs = Uint32Array.from([0, 1, 3, 0, 12, 4])
    const parts = lineParts(line.length, runs, [12, 14, 2])
    expect(describeParts(part => line.slice(part.start, part.end), parts)).toEqual(["val:1", " count = :0", "42:4+"])
    expect(describeParts(part => line.slice(part.start, part.end), lineParts(line.length, null, [4, 9, 2]))).toEqual(["val :0", "count:0+", " = 42:0"])
    expect(lineParts(0, null, null)).toEqual([])
  })
})

describe("lineParts with the hits of the find", () => {
  it("cuts a line with more hits than a call takes arguments", () => {
    const hits = Array.from({ length: 400_000 }, (_, index) => index)
    const parts = lineParts(400_000, null, null, { ranges: hits, current: 1 })
    expect(parts.length).toBe(400_000)
    expect(parts.slice(0, 4).map(part => [part.start, part.end, part.hit])).toEqual([
      [0, 1, 1],
      [1, 2, undefined],
      [2, 3, 2],
      [3, 4, undefined],
    ])
  })
})

describe("lineParts with the kinds of the engine", () => {
  it("types each changed part, and puts a marker part where the empty side of a change stands", () => {
    // `call(x)` to `call(x, y)`: the old line marks the insertion after `x`
    const parts = lineParts(7, null, [6, 6, 0])
    expect(parts.map(part => [part.start, part.end, part.change, part.marker === true])).toEqual([
      [0, 6, -1, false],
      [6, 6, 0, true],
      [6, 7, -1, false],
    ])
    expect(lineParts(10, null, [6, 9, 0]).map(part => part.change)).toEqual([-1, 0, -1])
    // a marker at the end of a line
    expect(lineParts(3, null, [3, 3, 1]).at(-1)).toMatchObject({ start: 3, end: 3, change: 1, marker: true })
  })
})

/** The text of the parts of an Inline row, `-` after a deleted part, `+` after a changed one, `!` after a hit, `!!` the current one */
function describeRow(pair: InlinePair, parts: readonly InlinePart[]): string[] {
  return parts.map(part => {
    const text = (part.side === 0 ? pair.oldText : pair.newText).slice(part.start, part.end)
    return `${text}${part.deleted ? "-" : part.change >= 0 ? "+" : ""}${part.hit === undefined ? "" : part.hit === 2 ? "!!" : "!"}`
  })
}

/** The text of the row of a pair */
function rowText(pair: InlinePair): string {
  return inlineParts(pair, null, null)
    .map(part => (part.side === 0 ? pair.oldText : pair.newText).slice(part.start, part.end))
    .join("")
}

/** The parts of the row of a pair that the hits `[start, end, ...]` of its old line mark, the first one current */
function markedBy(pair: InlinePair, oldHits: number[], current = 0): string {
  return inlineParts(pair, null, null, null, { ranges: oldHits, current })
    .filter(part => part.hit !== undefined)
    .map(part => (part.side === 0 ? pair.oldText : pair.newText).slice(part.start, part.end))
    .join("")
}

describe("inlinePair", () => {
  it("takes the ranges of the engine, which align, and shows any other two lines whole", () => {
    const pair = inlinePair("val a = string", "val a = name", [8, 14, 2], [8, 12, 2])
    expect([pair.deletions, pair.insertions]).toEqual([
      [8, 14, 2],
      [8, 12, 2],
    ])
    // no engine pairs lines whose ranges do not align: the row hides nothing of them
    const apart = inlinePair("abcde", "abXde", [0, 1, 2, 2, 3, 2], [2, 3, 2])
    expect(describeRow(apart, inlineParts(apart, null, null))).toEqual(["abcde-", "abXde+"])
    expect(rowText(inlinePair("call(x, y)", "call(x)", [6, 9, 1], []))).toBe("call(x, y)call(x)")
  })
})

describe("inlineParts", () => {
  it("puts the deleted part of the old line before the inserted part, each with the runs of its side", () => {
    const oldLine = "val a = string"
    const newLine = "val a = name"
    const parts = inlineParts(inlinePair(oldLine, newLine, [8, 14, 2], [8, 12, 2]), Uint32Array.from([8, 2]), Uint32Array.from([8, 6]))
    expect(parts.map(part => `${(part.side === 0 ? oldLine : newLine).slice(part.start, part.end)}:${part.style}${part.deleted ? "-" : part.change >= 0 ? "+" : ""}`)).toEqual([
      "val a = :0",
      "string:2-",
      "name:6+",
    ])
  })

  it("shows a pure insertion as the new line: the engine marks the empty old side, which the row needs no marker for", () => {
    const pair = inlinePair("val a = state", "val a = state + 1", [13, 13, 0], [13, 17, 0])
    expect(describeRow(pair, inlineParts(pair, null, null))).toEqual(["val a = state", " + 1+"])
    // a pure deletion: the deleted part where the new line has its marker
    const deleted = inlinePair("call(x, y)", "call(x)", [6, 9, 1], [6, 6, 1])
    expect(describeRow(deleted, inlineParts(deleted, null, null))).toEqual(["call(x", ", y-", ")"])
  })

  it("marks a hit of the old line in its deleted part and on in the text the row shows from the new line", () => {
    const pair = inlinePair("val a = string", "val a = name", [8, 14, 2], [8, 12, 2])
    const marks = (oldHits: number[], current: number, newHits: number[] = []) =>
      describeRow(pair, inlineParts(pair, null, null, { ranges: newHits, current: -1 }, { ranges: oldHits, current })).filter(part => part.endsWith("!"))
    // `string`, the deleted part alone
    expect(marks([8, 14], -1)).toEqual(["string-!"])
    // `a = str`, the current hit: the shared `a = ` is the text of the new line
    expect(marks([4, 11], 0)).toEqual(["a = !!", "str-!!"])
    // with a hit of the new line on the same shared text, the part both hold is marked once, the current hit over the other
    expect(marks([4, 11], 0, [0, 5])).toEqual(["val !", "a!!", " = !!", "str-!!"])
    expect(marks([4, 11], -1, [0, 5])).toEqual(["val !", "a!", " = !", "str-!"])
  })

  it("marks a hit of the old line around the text the new line inserts into it", () => {
    // `call(x)` to `call(x, y)`: the hit `x)` of the old line stands on both sides of the inserted `, y`
    const pair = inlinePair("call(x)", "call(x, y)", [6, 6, 0], [6, 9, 0])
    expect(describeRow(pair, inlineParts(pair, null, null, null, { ranges: [5, 7], current: 0 }))).toEqual(["call(", "x!!", ", y+", ")!!"])
  })

  it("marks a hit of the old line where the new line has the shared text, when a whitespace policy pairs lines of another indent", () => {
    // Trim Whitespace: `    foo(bar)` indented again and edited; the text before the range is 8 units old and 12 new
    const pair = inlinePair("    foo(bar)", "        foo(baz)", [8, 11, 2], [12, 15, 2])
    expect(rowText(pair)).toBe("        foo(barbaz)")
    const marks = (oldHits: number[]) => describeRow(pair, inlineParts(pair, null, null, null, { ranges: oldHits, current: 0 })).filter(part => part.endsWith("!"))
    // `foo(bar`: the name and the bracket of the new line, not its indent
    expect(marks([4, 11])).toEqual(["foo(!!", "bar-!!"])
    // `bar)`: the bracket after the inserted part
    expect(marks([8, 12])).toEqual(["bar-!!", ")!!"])
    // a hit that starts in the indent takes the indent of the new line
    expect(marks([2, 11])).toEqual(["        foo(!!", "bar-!!"])
    // Ignore Whitespace: other spaces inside the line and at its end
    const spaced = inlinePair("call( a,  b ) // old", "call(a, b)  // new", [17, 20, 2], [15, 18, 2])
    expect(markedBy(spaced, [6, 20])).toBe("a, b)  // old")
    expect(markedBy(spaced, [10, 20])).toBe("b)  // old")
    // a hit that starts in whitespace takes the whitespace the new line has there
    expect(markedBy(spaced, [9, 20])).toBe(" b)  // old")
  })

  it("keeps of the hits of the old line of a pair its own alone, the current one among them, for the row of an edited line of a move", () => {
    // `sum(first)` to `sum(second)`: `first` is struck through in the row, `sum` is text both lines have
    const pair = inlinePair("  sum(first)", "  sum(second)", [6, 11, 2], [6, 12, 2])
    // the hits `sum`, `first` and `)` of the old line, the second the current one: `first` alone is the row's own
    expect(ownOldHits(pair, { ranges: [2, 5, 6, 11, 11, 12], current: 1 })).toEqual({ ranges: [6, 11], current: 0 })
    // the current hit in the shared text: the row marks none as current, its new line has that text
    expect(ownOldHits(pair, { ranges: [2, 5, 6, 11], current: 0 })).toEqual({ ranges: [6, 11], current: -1 })
    expect(ownOldHits(pair, { ranges: [2, 5], current: 0 })).toEqual({ ranges: [], current: -1 })
    // the row marks the struck-through word and leaves the shared text to the hits of the new line
    const marked = (hits: { ranges: number[]; current: number }) =>
      describeRow(pair, inlineParts(pair, null, null, null, ownOldHits(pair, hits))).filter(part => part.endsWith("!"))
    expect(marked({ ranges: [2, 5, 6, 11], current: 0 })).toEqual(["first-!"])
    expect(marked({ ranges: [2, 5, 6, 11], current: 1 })).toEqual(["first-!!"])
  })
})

describe("inlineOffset", () => {
  it("gives the place of an offset of each line in the text of the row", () => {
    // the row: `val a = ` + struck `string` + inserted `name`
    const pair = inlinePair("val a = string;", "val a = name;", [8, 14, 2], [8, 12, 2])
    expect(rowText(pair)).toBe("val a = stringname;")
    expect([0, 7, 8, 13, 14].map(offset => inlineOffset(pair, 0, offset))).toEqual([0, 7, 8, 13, 18])
    expect([0, 7, 8, 11, 12].map(offset => inlineOffset(pair, 1, offset))).toEqual([0, 7, 14, 17, 18])
    // the end of a range stands after its last character: the end of the deleted part, not the text after the inserted one
    expect(inlineOffset(pair, 0, 14, true)).toBe(14)
    expect(inlineOffset(pair, 0, 15, true)).toBe(19)
    expect(inlineOffset(pair, 1, 8, true)).toBe(8)
    expect(inlineOffset(pair, 1, 12, true)).toBe(18)
  })

  it("places an offset of the old line by the text the lines share, whatever whitespace a policy ignored in it", () => {
    const pair = inlinePair("    foo(bar)", "        foo(baz)", [8, 11, 2], [12, 15, 2])
    const row = rowText(pair)
    // each character of the old line that is no whitespace stands in the row where `inlineOffset` says
    for (let offset = 0; offset < pair.oldText.length; offset++) {
      if (pair.oldText[offset] !== " ") {
        expect(row[inlineOffset(pair, 0, offset)], `offset ${offset}`).toBe(pair.oldText[offset])
      }
    }
    // the old name `bar` and the old `foo`, as a reveal selects them
    expect(row.slice(inlineOffset(pair, 0, 8), inlineOffset(pair, 0, 11, true))).toBe("bar")
    expect(row.slice(inlineOffset(pair, 0, 4), inlineOffset(pair, 0, 7, true))).toBe("foo")
    expect(row.slice(inlineOffset(pair, 0, 4), inlineOffset(pair, 0, 12, true))).toBe("foo(barbaz)")
  })

  // a fixed seed: the same pairs in every run
  it("places every character of both lines of random pairs, and marks every hit of the old line as its text", () => {
    let seed = 0x7e57d1ff
    const random = (limit: number) => {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0
      return Math.floor((seed / 0x100000000) * limit)
    }
    const word = (alphabet: string, max: number) => Array.from({ length: 1 + random(max) }, () => alphabet[random(alphabet.length)]).join("")
    // the shared text of the new line: the old one with other whitespace, as under Ignore Whitespace
    const respaced = (text: string) => text.replace(/ +/g, spaces => (random(3) === 0 ? " ".repeat(1 + random(3)) : spaces))
    const withoutSpaces = (text: string) => text.replaceAll(" ", "")
    for (let round = 0; round < 3000; round++) {
      const ignored = round % 2 === 1
      let oldText = word("ab ", 5)
      let newText = ignored ? respaced(oldText) : oldText
      const oldRanges: number[] = []
      const newRanges: number[] = []
      for (let range = 1 + random(4); range > 0; range--) {
        const kind = random(3)
        const deleted = kind === 0 ? "" : word("abc", 4)
        const inserted = kind === 1 ? "" : word("abd", 4)
        oldRanges.push(oldText.length, oldText.length + deleted.length, kind)
        newRanges.push(newText.length, newText.length + inserted.length, kind)
        // text between two ranges: a change never borders whitespace a policy ignores on one side only
        const shared = `a${word("ab ", 5)}b`
        oldText += deleted + shared
        newText += inserted + (ignored ? respaced(shared) : shared)
      }
      const pair = inlinePair(oldText, newText, oldRanges, newRanges)
      const row = rowText(pair)
      const tag = JSON.stringify({ oldText, newText, oldRanges, newRanges, row })
      for (const [side, text] of [oldText, newText].entries()) {
        for (let offset = 0; offset < text.length; offset++) {
          if (text[offset] !== " ") {
            expect(row[inlineOffset(pair, side, offset)], `side ${side} offset ${offset} of ${tag}`).toBe(text[offset])
          }
        }
      }
      // a hit of the old line that holds a deleted part: the row marks its text, but for the whitespace of the new line
      const start = random(oldText.length)
      const end = Math.min(oldText.length, start + 1 + random(6))
      if (isOwnOldHit(pair, start, end) && oldText[start] !== " " && oldText[end - 1] !== " ") {
        expect(withoutSpaces(markedBy(pair, [start, end])), `hit ${start}..${end} of ${tag}`).toBe(withoutSpaces(oldText.slice(start, end)))
        // and a reveal selects it from its first character to its last
        const selected = row.slice(inlineOffset(pair, 0, start), inlineOffset(pair, 0, end, true))
        expect([selected[0], selected.at(-1)], `selection ${start}..${end} of ${tag}`).toEqual([oldText[start], oldText[end - 1]])
      }
    }
  })
})

describe("sameTextOffset", () => {
  it("places a name of the old line in the new line of a row the whitespace policy ignores, by the characters that are no whitespace", () => {
    // a line indented again from 2 to 6 columns, a space gone inside it: one column shows the new line alone
    const oldText = "  val entry = make(a,  b)"
    const newText = "      val entry = make(a, b)"
    const select = (start: number, end: number) => newText.slice(sameTextOffset(oldText, newText, start), sameTextOffset(oldText, newText, end, true))
    // each name of the old line is the same name in the new one, four columns to the right
    expect([select(2, 5), select(6, 11), select(14, 18), select(19, 20), select(23, 24)]).toEqual(["val", "entry", "make", "a", "b"])
    expect(sameTextOffset(oldText, newText, 14)).toBe(18)
    // a range over the whitespace that differs is the same text but for it
    expect(select(14, 25)).toBe("make(a, b)")
    // an unchanged line is the same text: every offset is its own
    expect([0, 3, 9].map(offset => sameTextOffset("val a = 1", "val a = 1", offset))).toEqual([0, 3, 9])
  })
})
