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
  document.addEventListener(
    "focusin",
    () => {
      pointerMovedSinceFocus = false
    },
    true,
  )
  document.addEventListener(
    "pointermove",
    () => {
      pointerMovedSinceFocus = true
    },
    { capture: true, passive: true },
  )
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

// an open menu (a row's context menu leaves the focus on the table) keeps the keys; a closing one (its exit animation)
// does not, so D right after Escape acts
const OPEN_MENU = "[role=menu][data-open]"

/** A focused row (the branch rows are focusable: ↑/↓ move the focus between them); null when the focus is elsewhere */
function focusedPreviewRow(): HTMLElement | null {
  const active = document.activeElement
  return active instanceof HTMLElement && active.matches("tr[data-row-id]") ? active : null
}

/**
 * D and W act on a row from anywhere but a field, a menu or a dialog: an inline form in the row is a dialog, its
 * buttons and checkboxes keep their letters. A focused button of the row (its expand toggle) has no D or W of its own.
 */
function isRowKeyTarget(target: EventTarget | null): boolean {
  return !(target instanceof Element) || target.closest(SPACE_OWNERS) === null
}

/** What a preview table does with the keys that look at a row: Space previews it, D reviews it, W opens it in a window */
export interface PreviewHotkeys {
  preview: (id: string) => void
  review?: (id: string) => void
  openInWindow?: (id: string) => void
}

/** Marks the tables that open a preview on Space, so the innermost one owns the focus inside it */
const PREVIEW_TABLE_ATTRIBUTE = "data-space-preview"

/**
 * The keys that look at a row, as Quick Look does: Space opens its preview, D its review, W its review in a window.
 * They act on a focused row while the pointer has not moved since the focus moved there (↑/↓ in the branch table), on
 * the row (`tr[data-row-id]`) under the pointer, otherwise on `getFallbackId()` (the last clicked row) while the focus
 * is in the table. The keys that act on a row (P, S, O, M) need its focus instead.
 *
 * Every table registers its own listener; a table acts only for its own rows, for the innermost hovered row and for
 * the focus that no nested preview table holds, so the branch table and a commit list nested in it (or a commit list
 * nested in a conflict view) never open two previews.
 */
export function usePreviewHotkeys(tableRef: RefObject<HTMLTableElement | null>, getFallbackId: () => string | null, hotkeys: PreviewHotkeys) {
  const latestRef = useRef({ getFallbackId, hotkeys })
  useEffect(() => {
    latestRef.current = { getFallbackId, hotkeys }
    // on every render: the table may mount after the hook (a card that renders nothing while empty)
    tableRef.current?.setAttribute(PREVIEW_TABLE_ATTRIBUTE, "")
  })

  useEffect(() => {
    trackPointerSinceFocus()
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.repeat || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) {
        return
      }
      const { hotkeys } = latestRef.current
      // the physical keys for the letters, in any keyboard layout
      const action = event.key === " " ? hotkeys.preview : event.code === "KeyD" ? hotkeys.review : event.code === "KeyW" ? hotkeys.openInWindow : undefined
      const table = tableRef.current
      if (action === undefined || table === null || (event.key === " " ? ownsSpace(event.target) : !isRowKeyTarget(event.target)) || document.querySelector(OPEN_MENU) !== null) {
        return
      }

      let id: string | null = null
      const hoveredRows = document.querySelectorAll<HTMLElement>("tr[data-row-id]:hover")
      const hoveredRow = hoveredRows[hoveredRows.length - 1]
      const focusedRow = pointerMovedSinceFocus ? null : focusedPreviewRow()
      if (focusedRow !== null) {
        id = focusedRow.closest("table") === table ? (focusedRow.dataset.rowId ?? null) : null
      } else if (hoveredRow !== undefined) {
        id = hoveredRow.closest("table") === table ? (hoveredRow.dataset.rowId ?? null) : null
      } else if (document.activeElement?.closest(`table[${PREVIEW_TABLE_ATTRIBUTE}]`) === table) {
        id = latestRef.current.getFallbackId()
      }

      if (id !== null) {
        // no page scroll
        event.preventDefault()
        action(id)
      }
    }
    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [tableRef])
}
