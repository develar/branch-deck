import { useEffect, useMemo, useRef } from "react"
import type { DOMAttributes, FocusEvent, SyntheticEvent } from "react"
import type { StackPathTip } from "@/stores/stack-view"
import { focusStackPath, hoverStackPath, stackPathStore } from "@/stores/stack-view"

/** The pointer rests this long on a branch row before its path shows: moving over the rows doesn't flash the graph */
const HOVER_INTENT_MS = 100

interface PointedTip {
  tip: StackPathTip
  /** The push button or the Safe Push slot: what it sends is the question, the path shows at once */
  control: boolean
}

/**
 * The branch of the row an event of the table body comes from (`data-branch-name` of a branch row, `data-branch-of` of
 * the rows below it); null outside the rows of a branch, such as an event bubbling from a portal (a menu, a popover)
 */
function pointedTip(event: SyntheticEvent<HTMLElement>): PointedTip | null {
  const target = event.target
  if (!(target instanceof Element) || !event.currentTarget.contains(target)) {
    return null
  }
  const row = target.closest("tr[data-branch-name], tr[data-branch-of]")
  const name = row?.getAttribute("data-branch-name") ?? row?.getAttribute("data-branch-of") ?? null
  if (name === null) {
    return null
  }
  const control = target.closest("[data-stack-path]")?.getAttribute("data-stack-path") ?? null
  return { tip: { name, push: control === "push" }, control: control !== null }
}

/**
 * The handlers of the body of the branch table that point the highlighted stack path (`stack-view.ts`) at a branch:
 * its row after a short rest of the pointer, its push button or Safe Push slot at once, the row or the control the
 * keyboard focused. Leaving the body or starting a drag clears the pointer's path.
 */
export function useStackPathPointer(): Pick<DOMAttributes<HTMLTableSectionElement>, "onPointerOver" | "onPointerLeave" | "onFocus" | "onBlur" | "onDragStart"> {
  const timer = useRef<number | undefined>(undefined)
  useEffect(
    () => () => {
      window.clearTimeout(timer.current)
      hoverStackPath(null)
      focusStackPath(null)
    },
    [],
  )
  return useMemo(() => {
    const clearHover = () => {
      window.clearTimeout(timer.current)
      hoverStackPath(null)
    }
    return {
      onPointerOver: event => {
        const pointed = pointedTip(event)
        if (pointed === null) {
          return
        }
        window.clearTimeout(timer.current)
        if (pointed.control) {
          hoverStackPath(pointed.tip)
          return
        }
        const hovered = stackPathStore.getState().hovered
        if (hovered?.name !== pointed.tip.name || hovered.push) {
          timer.current = window.setTimeout(() => hoverStackPath(pointed.tip), HOVER_INTENT_MS)
        }
      },
      onPointerLeave: clearHover,
      onDragStart: clearHover,
      onFocus: event => {
        // a click focuses the row too: only the keyboard's focus is a pointer of its own
        const pointed = pointedTip(event)
        if (pointed !== null && event.target.matches(":focus-visible")) {
          focusStackPath(pointed.tip)
        }
      },
      onBlur: (event: FocusEvent<HTMLTableSectionElement>) => {
        if (!(event.relatedTarget instanceof Node) || !event.currentTarget.contains(event.relatedTarget)) {
          focusStackPath(null)
        }
      },
    }
  }, [])
}
