// @spec openspec/specs/commit/commit-review/spec.md
import { describe, expect, it } from "vite-plus/test"
import { fileMapBand, fileMapScale, fileMapSectionAt, fileMapThumb, fileMapTicks, PROLOGUE_SECTION } from "@/lib/diff/file-map"
import { DiffLayout } from "@/lib/diff/layout"
import type { LayoutFile } from "@/lib/diff/layout"

const CARDS = { rowHeight: 20, headerHeight: 48, cards: { gap: 10, padding: 10, borderBottom: 1, divider: 32 } }

/** A collapsed card (its header alone, 49 px) */
function card(key: string, extra: Partial<LayoutFile> = {}): LayoutFile {
  return { key, rows: null, notice: false, ...extra }
}

describe("the map of the files", () => {
  it("scales the document, never shorter than the view, to the height of the map, a tick at the start of each section", () => {
    // four collapsed cards and the opening of the step that starts with the third: 10 + 4 × 49 + 3 × 10 + 32 + 10 px
    const layout = new DiffLayout([card("a"), card("b"), card("c", { opening: 32 }), card("d")], CARDS)
    expect(layout.height).toBe(278)
    expect(fileMapScale(139, layout.height, 200)).toBe(0.5)
    // a review shorter than the view keeps its sections where its cards are
    expect(fileMapScale(100, layout.height, 556)).toBe(100 / 556)
    expect(fileMapScale(0, layout.height, 200)).toBe(0)
    // the start of a step is its opening, as `fileAt` counts it
    expect([layout.sectionStart(1), layout.sectionStart(2)]).toEqual([layout.offsets[1], layout.offsets[2]! - 32])
    const sections = [false, false, true, false].map(step => ({ step, muted: false }))
    // on a screen of 2 device pixels per CSS px: a file 2 device pixels, a step 4
    const ticks = fileMapTicks(layout, sections, 1, 2)
    expect(ticks).toEqual([
      { y: 10, height: 2, kind: "file", muted: false },
      { y: 69, height: 2, kind: "file", muted: false },
      { y: 128, height: 4, kind: "step", muted: false },
      { y: 219, height: 2, kind: "file", muted: false },
    ])
  })

  it("makes one tick of the sections whose pixels are too few, a step when any of them is, muted only when all are", () => {
    const layout = new DiffLayout([card("a"), card("b"), card("c", { opening: 32 }), card("d"), card("e")], CARDS)
    // a pixel of the map for every 100 px of the document: the sections start at 0.1, 0.69, 1.28, 2.19 and 2.78
    const muted = [true, true, true, false, true]
    const ticks = fileMapTicks(
      layout,
      muted.map((it, index) => ({ step: index === 2, muted: it })),
      0.01,
      1,
    )
    expect(ticks).toEqual([
      { y: 0, height: 1, kind: "file", muted: true },
      // the second card, the step and the fourth card (of the chip) in the 2 px of the step: one tick, a step, not muted
      { y: 1, height: 2, kind: "step", muted: false },
      { y: 3, height: 1, kind: "file", muted: true },
    ])
  })

  it("leaves the prologue above the first section a band without a tick, which the hover and a click tell from the sections and the map fills at the top", () => {
    const layout = new DiffLayout([card("a", { opening: 40 }), card("b")], CARDS, 200)
    const ticks = fileMapTicks(
      layout,
      [
        { step: true, muted: false },
        { step: false, muted: false },
      ],
      1,
      1,
    )
    // the first tick at the opening of the first step, below the space above the cards, the prologue and its gap
    expect(ticks.map(tick => tick.y)).toEqual([10 + 200 + 10, layout.offsets[1]])
    expect([layout.partAt(5), layout.partAt(219), layout.partAt(220)]).toEqual(["prologue", "prologue", "opening"])
    // the hover and a click: the prologue down to the first section, then the sections, nothing outside the document
    expect([5, 219, 220, layout.offsets[1]!, -1, layout.height + 1].map(y => fileMapSectionAt(layout, y))).toEqual([PROLOGUE_SECTION, PROLOGUE_SECTION, 0, 1, null, null])
    // the fill of the prologue at the top: from the top of the document to the first section
    expect(fileMapBand(layout, PROLOGUE_SECTION)).toEqual({ start: 0, end: 220 })
    expect(fileMapBand(layout, 0)).toEqual({ start: 220, end: layout.offsets[0]! + layout.fileHeight(0) })
    expect(fileMapBand(layout, 2)).toBeNull()
    // without a prologue its band is none, and a y in the space above the cards is the first section
    const plain = new DiffLayout([card("a"), card("b")], CARDS)
    expect([fileMapBand(plain, PROLOGUE_SECTION), fileMapSectionAt(plain, 5)]).toEqual([null, 0])
    expect(fileMapSectionAt(new DiffLayout([], CARDS), 0)).toBeNull()
  })

  it("draws the thumb at the window of the view, at least the least height centred on it, all of it inside the map", () => {
    // a window taller than the least height is the thumb itself
    expect(fileMapThumb(100, 40, 500, 16)).toEqual({ top: 100, height: 40 })
    // a thin window of a long review: a thumb of 16 px centred on it
    expect(fileMapThumb(100, 4, 500, 16)).toEqual({ top: 94, height: 16 })
    // at the top and at the end of the map it stays inside
    expect(fileMapThumb(2, 4, 500, 16)).toEqual({ top: 0, height: 16 })
    expect(fileMapThumb(495, 4, 500, 16)).toEqual({ top: 484, height: 16 })
    // a map lower than the least height: the whole map
    expect(fileMapThumb(0, 4, 10, 16)).toEqual({ top: 0, height: 10 })
  })
})
