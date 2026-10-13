import { useEffect, useRef } from "react"
import type { RefObject } from "react"

// Space keeps its own meaning in these: typing, a menu or another dialog
const SPACE_OWNERS = "input, textarea, select, [contenteditable]:not([contenteditable=false]), [role=dialog], [role=menu]"

// whether the pointer moved after the focus last changed: then the pointer, not the focused button, says what Space is for
let pointerMovedSinceFocus = false
let isTrackingPointer = false

function trackPointerSinceFocus() {
  if (isTrackingPointer) {
    return
  }
  isTrackingPointer = true
  document.addEventListener("focusin", () => {
    pointerMovedSinceFocus = false
  }, true)
  document.addEventListener("pointermove", () => {
    pointerMovedSinceFocus = true
  }, { capture: true, passive: true })
}

/**
 * A focused button or link keeps Space, unless the pointer moved since it got the focus: a click leaves the focus on the
 * clicked button (the folder icon that expanded the branch) while the user goes on to point at a commit.
 */
function ownsSpace(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false
  }
  return target.closest(SPACE_OWNERS) !== null || (!pointerMovedSinceFocus && target.matches("button, a, [role=button]"))
}

/**
 * Space opens the commit preview, as Quick Look does: for the commit row under the pointer, otherwise for
 * `getFallbackId()` (the last clicked row) while the table has the focus.
 *
 * Every commit list registers its own listener; a list acts only for its own rows, and for the innermost hovered row,
 * so a list nested in a conflict view does not open a second preview.
 */
export function useCommitPreviewHotkey(
  tableRef: RefObject<HTMLTableElement | null>,
  getFallbackId: () => string | null,
  open: (id: string) => void,
) {
  const latestRef = useRef({ getFallbackId, open })
  useEffect(() => {
    latestRef.current = { getFallbackId, open }
  })

  useEffect(() => {
    trackPointerSinceFocus()
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== " " || event.repeat || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) {
        return
      }
      const table = tableRef.current
      if (table === null || ownsSpace(event.target)) {
        return
      }

      let id: string | null = null
      const hoveredRows = document.querySelectorAll<HTMLElement>("tr[data-row-id]:hover")
      const hoveredRow = hoveredRows[hoveredRows.length - 1]
      if (hoveredRow !== undefined) {
        id = hoveredRow.closest("table") === table ? hoveredRow.dataset.rowId ?? null : null
      }
      else if (table.contains(document.activeElement)) {
        id = latestRef.current.getFallbackId()
      }

      if (id !== null) {
        // no page scroll
        event.preventDefault()
        latestRef.current.open(id)
      }
    }
    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [tableRef])
}
