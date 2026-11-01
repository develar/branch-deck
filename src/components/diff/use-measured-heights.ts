import { useState } from "react"
import { flushSync } from "react-dom"

/** A height within this of the one the layout has is the same: a subpixel of a resize lays out nothing again */
const TOLERANCE = 0.5

export interface MeasuredHeights {
  /** The height of each block measured so far, by its `data-measure` key */
  heights: ReadonlyMap<string, number>
  /** The ref of a block to measure, which names its key in `data-measure` */
  measure: (element: HTMLElement | null) => (() => void) | undefined
}

/**
 * The heights of the few blocks of words of a document that the layout cannot count (docs/design/diff-view.md, "Layout
 * and virtualization"): always mounted and measured in place. One `ResizeObserver` measures a block as it mounts and
 * after every change of its height (the width of the view, the load of a font, a disclosure, a text that arrives). It
 * reports after the layout of the frame and before its paint, and the heights it reports are rendered at once
 * (`flushSync`), so no frame paints a block over the card below it. Only the positions of the blocks depend on the
 * heights, never their sizes, so a new height is never measured again in the same frame.
 */
export function useMeasuredHeights(): MeasuredHeights {
  const [heights, setHeights] = useState<ReadonlyMap<string, number>>(() => new Map())
  const [observer] = useState(
    () =>
      new ResizeObserver(entries => {
        const measured = entries.flatMap(entry => {
          const key = entry.target instanceof HTMLElement ? entry.target.dataset.measure : undefined
          const box = entry.borderBoxSize[0]
          return key === undefined || !entry.target.isConnected ? [] : [[key, box === undefined ? entry.target.getBoundingClientRect().height : box.blockSize] as const]
        })
        flushSync(() => setHeights(previous => merged(previous, measured)))
      }),
  )
  const measure = (element: HTMLElement | null) => {
    if (element === null) {
      return undefined
    }
    observer.observe(element)
    return () => observer.unobserve(element)
  }
  return { heights, measure }
}

/** The heights with the measured ones, the same map when none differs by more than the tolerance */
function merged(previous: ReadonlyMap<string, number>, measured: readonly (readonly [string, number])[]): ReadonlyMap<string, number> {
  const changed = measured.filter(([key, height]) => {
    const known = previous.get(key)
    return known === undefined || Math.abs(known - height) > TOLERANCE
  })
  return changed.length === 0 ? previous : new Map([...previous, ...changed])
}
