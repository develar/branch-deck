import { useCallback, useState } from "react"
import { flushSync } from "react-dom"
import { nextTask, observedAbove, ReportGuard } from "@/lib/report-guard"

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
 * heights, never their sizes. But a change in the rendering update of a report can resize a block after it (that render
 * takes in every update pending then, a text that arrived in the same frame included; WebKit renders a commit that waited
 * in a microtask there): such a block leaves the observer at once and is measured, rendered and observed anew in the
 * next task (`ReportGuard`), because an observation resized in its own delivery is a ResizeObserver loop (an uncaught
 * error)
 */
export function useMeasuredHeights(): MeasuredHeights {
  const [heights, setHeights] = useState<ReadonlyMap<string, number>>(() => new Map())
  const [observer] = useState(() => {
    const blocks = new Set<HTMLElement>()
    // the height each block was rendered with
    const rendered = new Map<HTMLElement, number>()
    const render = (measured: readonly (readonly [HTMLElement, number])[]) => {
      for (const [block, height] of measured) {
        rendered.set(block, height)
      }
      flushSync(() =>
        setHeights(previous =>
          merged(
            previous,
            measured.map(([block, height]) => [block.dataset.measure ?? "", height] as const),
          ),
        ),
      )
    }
    const resize = new ResizeObserver(entries => {
      render(
        entries.flatMap(entry => {
          const block = entry.target
          const box = entry.borderBoxSize[0]
          return block instanceof HTMLElement && block.dataset.measure !== undefined && block.isConnected
            ? [[block, box === undefined ? block.getBoundingClientRect().height : box.blockSize] as const]
            : []
        }),
      )
      guard.reported()
    })
    // a block a change resized after its report leaves the observer at once (no observation of it is left to skip), and
    // is measured, rendered and observed anew in the next task: a render in the rendering update would take in the
    // updates pending then, and resize other observed elements in their own delivery
    const guard = new ReportGuard(touched => {
      const resized = [...observedAbove(touched, element => blocks.has(element as HTMLElement))] as HTMLElement[]
      for (const block of resized) {
        resize.unobserve(block)
      }
      nextTask(() => {
        const present = resized.filter(block => blocks.has(block) && block.isConnected)
        render(present.map(block => [block, block.getBoundingClientRect().height] as const))
        for (const block of present) {
          resize.observe(block)
        }
      })
    })
    return { blocks, rendered, resize, guard }
  })
  // one ref callback for the life of the view: another function would detach and attach every block it measures
  const measure = useCallback(
    (element: HTMLElement | null) => {
      if (element === null) {
        return undefined
      }
      observer.blocks.add(element)
      observer.resize.observe(element)
      observer.guard.watch(element)
      return () => {
        observer.blocks.delete(element)
        observer.rendered.delete(element)
        observer.resize.unobserve(element)
      }
    },
    [observer],
  )
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
