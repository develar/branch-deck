import { useCallback, useEffect, useEffectEvent, useRef } from "react"
import type { KeyboardEvent, MouseEvent, ReactElement, ReactNode } from "react"
import { cn } from "cn"
import { PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { HoverDescription } from "@/components/shared/HoverDescription"

/*
 * The card of a mark of a row: details on demand, one way in every list (the facts of a file of a review,
 * `FileRowMark`; a commit, `CommitRowMark`). The hover of the mark opens it after a move of the pointer; `I` on the
 * row opens it from the keyboard with the focus inside; Escape closes it and gives the focus back to the row, and so
 * does any other close that would drop the focus to the page (the hover of another mark, the pointer leaving, the row
 * going). One card is open at a time: its owner (the tree of a review; in the main window the `commit-card` store
 * across its lists) holds `open` and whether the key opened it.
 */

/** A click on the mark or in its card acts there only: the row under it does not select or toggle */
function keepFromRow(event: MouseEvent) {
  event.stopPropagation()
}

/** What takes the focus as a card opened by the key mounts: its first button (else the card), or the card */
export type RowCardFocus = "firstButton" | "card"

function focusOnOpen(popup: HTMLElement, focus: RowCardFocus) {
  const target = (focus === "firstButton" ? popup.querySelector<HTMLElement>("button") : null) ?? popup
  // no scroll while the card is not placed yet
  target.focus({ preventScroll: true })
}

/** What a list tells the card of one of its rows */
export interface RowCardState {
  /** One card is open at a time: by the hover of the mark, or by `I` on the row */
  open: boolean
  /** Opened by `I` on the row: the focus moves into the card, and goes back to the row as the card closes */
  openedByKey: boolean
  onOpenChange: (open: boolean) => void
  /** Gives the focus back to the row of the mark */
  focusRow: () => void
}

export interface RowCardProps extends RowCardState {
  /** The name of the mark, which the row's description says too: the mark is no tab stop */
  label: string
  /** The element of the mark (a `span` with its classes and test id); a click on it acts there only, never on the row */
  mark: ReactElement
  /** The icon inside the mark */
  icon: ReactNode
  /** The classes of the card: its width and padding */
  className: string
  "data-testid": string
  /** The content of the card; `close` closes it, giving the focus back to the row when the key opened it */
  children: (close: () => void) => ReactNode
  /** Where the focus goes as the key opens the card: its first button by default (a file to show), else the card */
  focus?: RowCardFocus
}

/** The mark of a row and the card its hover or `I` opens (the module's header) */
export function RowCard({ open, openedByKey, onOpenChange, focusRow, label, mark, icon, className, "data-testid": testId, children, focus = "firstButton" }: RowCardProps) {
  // the popup of the card opened by the key, kept as the ref lets it go (before the effect below asks where the focus
  // is); stable, so a render does not move the focus again
  const keyPopup = useRef<HTMLElement | null>(null)
  const mountKeyPopup = useCallback(
    (popup: HTMLElement | null) => {
      if (popup !== null) {
        keyPopup.current = popup
        focusOnOpen(popup, focus)
      }
    },
    [focus],
  )
  const focusBackIfLost = useEffectEvent(() => {
    const focused = document.activeElement
    // still in the card that goes, or fallen to the page as it went: a focus that went elsewhere on purpose stays
    if (focused === null || focused === document.body || keyPopup.current?.contains(focused) === true) {
      focusRow()
    }
  })
  // a card opened by the key that closes otherwise than by Escape or an action of it (the hover of another row's mark
  // replaces it, the pointer leaves its mark): the focus goes back to the row, else the next Escape would close the window
  useEffect(() => {
    if (!openedByKey) {
      return
    }
    return () => focusBackIfLost()
  }, [openedByKey])
  // the row goes (a search, a new view) or loses its mark: its card goes too, and does not open again as it comes back
  const closeAsMarkGoes = useEffectEvent(() => {
    if (open) {
      onOpenChange(false)
    }
  })
  useEffect(() => () => closeAsMarkGoes(), [])
  const close = (giveFocusBack: boolean) => {
    onOpenChange(false)
    if (giveFocusBack && openedByKey) {
      focusRow()
    }
  }
  // the Escape of the card is not the window's (Overview, Close)
  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault()
      close(true)
    }
  }
  return (
    <HoverDescription
      lazy
      open={open}
      onOpenChange={(next, eventDetails) => {
        if (next) {
          onOpenChange(true)
        } else {
          // a click or the focus elsewhere keeps it where it went
          close(eventDetails.reason === "escape-key")
        }
      }}
    >
      <PopoverTrigger openOnHover delay={300} nativeButton={false} tabIndex={-1} aria-label={label} render={mark} onClick={keepFromRow}>
        {icon}
      </PopoverTrigger>
      <PopoverContent
        align="end"
        className={cn("gap-3", className)}
        data-testid={testId}
        // the card moves the focus itself: by the key into it and back to the row, by the hover never
        initialFocus={false}
        finalFocus={false}
        ref={openedByKey ? mountKeyPopup : undefined}
        // the popup is in a portal, but React bubbles its events through the row: a click here must not select or toggle it
        onClick={keepFromRow}
        onContextMenu={keepFromRow}
        onKeyDown={handleKeyDown}
      >
        {children(() => close(true))}
      </PopoverContent>
    </HoverDescription>
  )
}
