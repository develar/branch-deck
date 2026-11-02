import { useEffect, useRef } from "react"
import type { PointerEvent } from "react"
import type { RowCardState } from "@/components/shared/RowCard"
import { pointerMoved } from "@/lib/pointer-moves"

/** The rest of the pointer on the text of a row that opens its card: longer than on its mark, the text is read */
const TEXT_DELAY_MS = 600
/** The pointer that leaves the text of a row has this long to reach its card */
const TEXT_CLOSE_MS = 150

/** The handlers of the text of a row and of its card that open the card on the hover of the text (`useRowCardHover`) */
export interface RowCardHover {
  readonly text: { onPointerMove: (event: PointerEvent) => void; onPointerLeave: () => void }
  readonly popup: { onPointerEnter: () => void; onPointerLeave: () => void }
}

/**
 * The hover of the text of a row (the subject of a commit): a rest of the pointer after a move over it
 * (`TEXT_DELAY_MS`, a move as `hoverOpensOnMove` asks, never content that came under a still pointer) opens the card of
 * the row, and the pointer that leaves the text and does not reach the card closes it; the card the mark or `I` opened
 * stays theirs
 */
export function useRowCardHover({ open, onOpenChange }: Pick<RowCardState, "open" | "onOpenChange">): RowCardHover {
  const opening = useRef<ReturnType<typeof setTimeout> | null>(null)
  const closing = useRef<ReturnType<typeof setTimeout> | null>(null)
  // the text opened the card: its leave closes it
  const byText = useRef(false)
  useEffect(() => {
    if (!open) {
      byText.current = false
    }
  }, [open])
  useEffect(
    () => () => {
      stopTimer(opening)
      stopTimer(closing)
    },
    [],
  )
  const closeSoon = () => {
    if (byText.current && closing.current === null) {
      closing.current = setTimeout(() => {
        closing.current = null
        if (byText.current) {
          onOpenChange(false)
        }
      }, TEXT_CLOSE_MS)
    }
  }
  return {
    text: {
      onPointerMove: event => {
        stopTimer(closing)
        if (open || opening.current !== null || !pointerMoved(event.nativeEvent)) {
          return
        }
        opening.current = setTimeout(() => {
          opening.current = null
          byText.current = true
          onOpenChange(true)
        }, TEXT_DELAY_MS)
      },
      onPointerLeave: () => {
        stopTimer(opening)
        closeSoon()
      },
    },
    popup: {
      onPointerEnter: () => stopTimer(closing),
      onPointerLeave: closeSoon,
    },
  }
}

function stopTimer(timer: { current: ReturnType<typeof setTimeout> | null }) {
  if (timer.current !== null) {
    clearTimeout(timer.current)
    timer.current = null
  }
}
