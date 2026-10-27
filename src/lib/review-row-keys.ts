import type { KeyboardEvent } from "react"
import { neighbourRow } from "@/lib/branch-row-keys"

/** What the keys of a row of pull requests do, by the number of the row */
export interface ReviewRowKeys {
  /** Enter: review it (or what the row is of) */
  readonly onEnter: (number: number) => void
  /** D: review its changes */
  readonly onReview: (number: number) => void
  /** O: open it on GitHub */
  readonly onOpen: (number: number) => void
}

/**
 * The keys of a list of pull requests (Your Turn, a section of Reviews): ↑/↓ move the focus between its rows (one tab
 * stop, `setFocused`), Enter, D and O by `keys` (the physical keys, in any keyboard layout), the context menu key and
 * Shift+F10 the menu of the row at its start. Space does nothing: it is Quick Look elsewhere, and a pull request has none
 */
export function handleReviewRowKeys(event: KeyboardEvent<HTMLElement>, numbers: readonly number[], keys: ReviewRowKeys, setFocused: (number: number) => void): void {
  if (event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey) {
    return
  }
  const row = (event.target as HTMLElement).closest<HTMLElement>("[data-review-number]")
  const shown = row?.dataset.reviewNumber ?? null
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault()
    const next = neighbourRow(numbers.map(String), shown, event.key === "ArrowDown" ? 1 : -1)
    const target = next === null ? null : event.currentTarget.querySelector<HTMLElement>(`[data-review-number="${next}"]`)
    if (target != null) {
      setFocused(Number(next))
      target.focus()
    }
    return
  }
  if (row == null || shown === null || event.target !== row || !numbers.includes(Number(shown))) {
    return
  }
  const number = Number(shown)
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault()
    keys.onEnter(number)
  } else if (event.code === "KeyD" && !event.shiftKey) {
    event.preventDefault()
    keys.onReview(number)
  } else if (event.code === "KeyO" && !event.shiftKey) {
    event.preventDefault()
    keys.onOpen(number)
  } else if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
    event.preventDefault()
    const rect = row.getBoundingClientRect()
    row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: rect.left + 24, clientY: rect.bottom, button: 2 }))
  }
}
