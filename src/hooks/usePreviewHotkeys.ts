import { useEffect, useRef } from "react"
import type { RefObject } from "react"

// Space keeps its own meaning in these: typing, a menu or another dialog
const SPACE_OWNERS = "input, textarea, select, [contenteditable]:not([contenteditable=false]), [role=dialog], [role=menu]"

// an open menu keeps the keys, also where the focus stays on the table (a row's context menu opened with the mouse); a
// closing one (its exit animation, which keeps the focus until it ends) does not, so D right after Escape acts
const OPEN_MENU = "[role=menu][data-open]"
const CLOSING_MENU = "[role=menu][data-closed]"

/** The field, menu or dialog the key goes to; null for none, or for a closing menu */
function keyOwner(target: Element): Element | null {
  const owner = target.closest(SPACE_OWNERS)
  return owner?.matches(CLOSING_MENU) === true ? null : owner
}

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
  return keyOwner(target) !== null || (!pointerMovedSinceFocus && target.matches("button, a, [role=button]"))
}

/** A focused row (the branch rows are focusable: ↑/↓ move the focus between them); null when the focus is elsewhere */
function focusedPreviewRow(): HTMLElement | null {
  const active = document.activeElement
  return active instanceof HTMLElement && active.matches("tr[data-row-id]") ? active : null
}

/**
 * D acts on a row from anywhere but a field, a menu or a dialog: an inline form in the row is a dialog, its buttons
 * and checkboxes keep their letters. A focused button of the row (its expand toggle) has no D of its own.
 */
function isRowKeyTarget(target: EventTarget | null): boolean {
  return !(target instanceof Element) || keyOwner(target) === null
}

/** What a preview table does with the keys that look at a row: Space previews it, D reviews it (in its window) */
export interface PreviewHotkeys {
  preview: (id: string) => void
  review?: (id: string) => void
  /**
   * The user may review the row next: the pointer rested on it (`INTENT_DWELL_MS`) or it got the focus. A table reads
   * ahead what its review shows first, so D finds it read
   */
  intent?: (id: string) => void
}

/** How long the pointer rests on a row before it counts as the intent to look at it: passing over a row does not */
const INTENT_DWELL_MS = 200

/** Marks the tables that open a preview on Space, so the innermost one owns the focus inside it */
const PREVIEW_TABLE_ATTRIBUTE = "data-space-preview"

/**
 * The keys that look at a row, as Quick Look and ⌘O do: Space opens its preview, D its review in its window.
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
      const action = event.key === " " ? hotkeys.preview : event.code === "KeyD" ? hotkeys.review : undefined
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

  // the intent: a row of this table (not of a table nested in it) the pointer rests on, or that gets the focus; on the
  // document, as the keys: the table may mount after the hook
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined
    let pointed: string | null = null
    const ownRowId = (target: EventTarget | null): string | null => {
      const table = tableRef.current
      const row = target instanceof Element ? target.closest<HTMLElement>("tr[data-row-id]") : null
      return table !== null && row !== null && row.closest("table") === table ? (row.dataset.rowId ?? null) : null
    }
    const tell = (id: string) => latestRef.current.hotkeys.intent?.(id)
    const handlePointerOver = (event: PointerEvent) => {
      const id = ownRowId(event.target)
      if (id === pointed) {
        return
      }
      clearTimeout(timer)
      pointed = id
      if (id !== null) {
        timer = setTimeout(() => tell(id), INTENT_DWELL_MS)
      }
    }
    const handleFocusIn = (event: FocusEvent) => {
      const id = ownRowId(event.target)
      if (id !== null) {
        tell(id)
      }
    }
    document.addEventListener("pointerover", handlePointerOver, { passive: true })
    document.addEventListener("focusin", handleFocusIn)
    return () => {
      clearTimeout(timer)
      document.removeEventListener("pointerover", handlePointerOver)
      document.removeEventListener("focusin", handleFocusIn)
    }
  }, [tableRef])
}
