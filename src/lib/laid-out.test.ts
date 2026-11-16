import { afterEach, describe, expect, it, vi } from "vite-plus/test"
import type { Layout } from "@/lib/laid-out"
import { laidOutBoxes, observedContentBox, sameBoxOf } from "@/lib/laid-out"

/** A scroller of 300 px, padding 4 and border 1 on each side, with a scroll bar `bar` px wide, in a box sizing */
function scroller(boxSizing: "border-box" | "content-box", bar: number): Element {
  // the computed width the engine gives: the border box, or the content box without the bar (WebKit and Chromium alike)
  const width = boxSizing === "border-box" ? 300 : 300 - bar
  const border = boxSizing === "border-box" ? 300 : 300 + 10
  vi.stubGlobal("getComputedStyle", () => ({
    display: "block",
    boxSizing,
    width: `${width}px`,
    height: "100px",
    paddingLeft: "4px",
    paddingRight: "4px",
    paddingTop: "4px",
    paddingBottom: "4px",
    borderLeftWidth: "1px",
    borderRightWidth: "1px",
    borderTopWidth: "1px",
    borderBottomWidth: "1px",
  }))
  return { isConnected: true, offsetWidth: border, clientWidth: border - 2 - bar, offsetHeight: 110, clientHeight: 108 } as unknown as Element
}

describe("the boxes of an element as laid out", () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  it("reads the content box of a scroller without its scroll bar, as its observer reports it, in either box sizing", () => {
    // what WebKit reported of a scroller with a bar of 12 px (::-webkit-scrollbar): content 278, border 300 in a border
    // box; content 288, border 310 in a content box
    expect(laidOutBoxes(scroller("border-box", 12))).toEqual({ content: { inline: 278, block: 90 }, border: { inline: 300, block: 100 } })
    expect(laidOutBoxes(scroller("content-box", 12))).toEqual({ content: { inline: 288, block: 100 }, border: { inline: 310, block: 110 } })
  })

  it("reads no scroll bar of an overlay bar or of none", () => {
    expect(laidOutBoxes(scroller("border-box", 0))).toEqual({ content: { inline: 290, block: 90 }, border: { inline: 300, block: 100 } })
  })

  it("reads no scroll bar of the pixel the rounding of the client box leaves", () => {
    // a border box of 300.5 px rounds to 301, its client box of 298.5 to 299: a pixel, no bar
    expect(laidOutBoxes(scroller("border-box", 1))).toEqual({ content: { inline: 290, block: 90 }, border: { inline: 300, block: 100 } })
  })
})

describe("two layouts of an element against each other", () => {
  /** A scroller of 300 px, padding 8 and borders 2 in all, its bar `bar` px wide */
  const layout = (width: number, bar: number): Layout => ({
    borderBox: true,
    size: { inline: width, block: 100 },
    padding: { inline: 8, block: 8 },
    borders: { inline: 1.8181, block: 1.8181 },
    bars: { inline: bar, block: 0 },
  })

  it("takes a bar the rounding of whole pixels estimates a pixel apart for the same bar", () => {
    // a move of the scroller by a fraction of a pixel rounds its border box and its client box anew
    expect(sameBoxOf(layout(300, 12), layout(300, 13), "content")).toBe(true)
    expect(sameBoxOf(layout(300, 12), layout(300, 13), "border")).toBe(true)
  })

  it("tells a bar that came or went, and a box of another size, in the box observed", () => {
    expect(sameBoxOf(layout(300, 12), layout(300, 0), "content")).toBe(false)
    // in a border box sizing a bar takes its room from the content box: the border box stays
    expect(sameBoxOf(layout(300, 0), layout(300, 12), "border")).toBe(true)
    expect(sameBoxOf(layout(300, 12), layout(300.5, 12), "content")).toBe(false)
    expect(sameBoxOf(layout(300, 12), layout(300.005, 12), "border")).toBe(true)
  })
})

describe("the content box of an element as its observer reports it", () => {
  afterEach(() => {
    vi.unstubAllGlobals()
  })

  /** The scroller of the diffs, no padding, border or bar, laid out as WebKit did at `zoom` (`html { zoom }`) */
  function zoomed(zoom: number, rect: { width: number; height: number }, computed: { width: string; height: string }): Element {
    vi.stubGlobal("getComputedStyle", () => ({
      display: "block",
      boxSizing: "border-box",
      ...computed,
      paddingLeft: "0px",
      paddingRight: "0px",
      paddingTop: "0px",
      paddingBottom: "0px",
      borderLeftWidth: "0px",
      borderRightWidth: "0px",
      borderTopWidth: "0px",
      borderBottomWidth: "0px",
    }))
    const client = { offsetWidth: 681, clientWidth: 681, offsetHeight: 624, clientHeight: 624 }
    return { isConnected: true, ...client, currentCSSZoom: Math.fround(zoom), getBoundingClientRect: () => rect } as unknown as Element
  }

  it("reads the box by the arithmetic of the report at a zoom, where the computed style says the size before its floor", () => {
    // what WebKit and Chromium reported at 110%, 90% and 100%: the border box over the zoom, floored to 1/64 px
    const at110 = zoomed(1.1, { width: 748.984375, height: 686.40625 }, { width: "680.894897px", height: "624.005676px" })
    expect(observedContentBox(at110)).toEqual({ inline: 680.890625, block: 624 })
    expect(laidOutBoxes(at110)?.content).toEqual({ inline: 680.894897, block: 624.005676 })
    const at90 = zoomed(0.9, { width: 831.015625, height: 561.625 }, { width: "923.350708px", height: "624.027771px" })
    expect(observedContentBox(at90)).toEqual({ inline: 923.34375, block: 624.015625 })
    expect(observedContentBox(zoomed(1, { width: 790, height: 624 }, { width: "790px", height: "624px" }))).toEqual({ inline: 790, block: 624 })
  })
})
