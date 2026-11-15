import { afterEach, describe, expect, it, vi } from "vite-plus/test"
import { laidOutBoxes } from "@/lib/laid-out"

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
})
