import { describe, expect, it } from "vite-plus/test"
import { blockHeights } from "@/components/diff/use-measured-heights"

/** A block under an ancestor scaled by `scale`: laid out `height` tall, drawn `height * scale` tall on the screen */
function scaledBlock(key: string, height: number, scale: number, isConnected = true): HTMLElement {
  return {
    dataset: { measure: key },
    isConnected,
    offsetHeight: height,
    getBoundingClientRect: () => ({ height: height * scale }),
  } as unknown as HTMLElement
}

describe("blockHeights", () => {
  it("measures a block under a scaled ancestor by its size as laid out, never by the box drawn on the screen", () => {
    // a dialog that zooms in (`zoom-in-95`): its blocks are drawn at 95 % of their layout as it opens
    const prologue = scaledBlock("prologue", 200, 0.95)
    const opening = scaledBlock("opening:a", 80, 0.95)
    const entries = [
      { target: prologue, borderBoxSize: [{ blockSize: 200, inlineSize: 600 }] },
      // an engine without the border box of the entry: the layout of the element
      { target: opening },
    ] as unknown as ResizeObserverEntry[]
    expect(blockHeights(entries)).toEqual([
      ["prologue", 200],
      ["opening:a", 80],
    ])
  })

  it("measures no block that left the page or names no key", () => {
    const gone = scaledBlock("gone", 50, 1, false)
    const unnamed = { dataset: {}, isConnected: true, offsetHeight: 10 } as unknown as HTMLElement
    expect(blockHeights([{ target: gone }, { target: unnamed }] as unknown as ResizeObserverEntry[])).toEqual([])
  })
})
