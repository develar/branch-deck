import { useCallback, useState } from "react"
import { flushSync } from "react-dom"
import { observeSize } from "@/lib/observe-size"

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
 * heights, never their sizes. A change in the rendering update of a report that resizes a block after it (that render
 * takes in every update pending then, a text that arrived in the same frame included; WebKit renders a commit that waited
 * in a microtask there) lets the block leave the observer, whose next report a frame later gives its height
 * (`observeSize`): an observation resized in its own delivery would be a ResizeObserver loop
 */
export function useMeasuredHeights(): MeasuredHeights {
  const [heights, setHeights] = useState<ReadonlyMap<string, number>>(() => new Map())
  const [observer] = useState(() => observeSize(entries => flushSync(() => setHeights(previous => merged(previous, blockHeights(entries))))))
  // one ref callback for the life of the view: another function would detach and attach every block it measures
  const measure = useCallback(
    (element: HTMLElement | null) => {
      if (element === null) {
        return undefined
      }
      observer.observe(element)
      return () => observer.unobserve(element)
    },
    [observer],
  )
  return { heights, measure }
}

/**
 * The height of each measured block of `entries` by its key: the size as laid out, the border box the observer reports
 * (else `offsetHeight`), which a transform of an ancestor (a dialog that zooms in) leaves as it is, never the box drawn
 * on the screen (`getBoundingClientRect`)
 */
export function blockHeights(entries: readonly ResizeObserverEntry[]): (readonly [string, number])[] {
  return entries.flatMap(entry => {
    const block = entry.target as HTMLElement
    const key = block.dataset?.measure
    const box = entry.borderBoxSize?.[0]
    return key === undefined || !block.isConnected ? [] : [[key, box === undefined ? block.offsetHeight : box.blockSize] as const]
  })
}

/** The heights with the measured ones, the same map when none differs by more than the tolerance */
function merged(previous: ReadonlyMap<string, number>, measured: readonly (readonly [string, number])[]): ReadonlyMap<string, number> {
  const changed = measured.filter(([key, height]) => {
    const known = previous.get(key)
    return known === undefined || Math.abs(known - height) > TOLERANCE
  })
  return changed.length === 0 ? previous : new Map([...previous, ...changed])
}
