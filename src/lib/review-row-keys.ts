import type { KeyboardEvent } from "react"
import { neighbourRow } from "@/lib/branch-row-keys"
import { isMenuKey, openMenuByKey } from "@/lib/menu-key"

/** What the keys of a row of pull requests do, by the number of the row */
export interface ReviewRowKeys {
  /** Enter: review it (or what the row is of) */
  readonly onEnter: (number: number) => void
  /** D: review its changes */
  readonly onReview: (number: number) => void
  /** O: open it on GitHub */
  readonly onOpen: (number: number) => void
  /** The header of a stack (`data-review-stack`, its key): Enter shows or hides its layers, → shows them, ← hides them */
  readonly onStack?: (stack: string, open: boolean | "toggle") => void
  /** D on the header of a stack: Review Stack, its changes */
  readonly onReviewStack?: (stack: string) => void
}

/**
 * The keys of a list of pull requests (Your Turn, a section of Reviews), its rows by their keys (`data-review-row`: the
 * number of a pull request, the key of the header of a stack): ↑/↓ move the focus between its rows (one tab stop,
 * `setFocused`), Enter, D and O by `keys` (the physical keys, in any keyboard layout; on the header of a stack Enter, →
 * and ← its layers, D Review Stack), the context menu key and Shift+F10 the menu of the row at its start
 * (`openMenuByKey`). Space does nothing: it is Quick Look elsewhere, and a
 * pull request has none
 */
export function handleReviewRowKeys(event: KeyboardEvent<HTMLElement>, rows: readonly string[], keys: ReviewRowKeys, setFocused: (row: string) => void): void {
  if (event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey) {
    return
  }
  const row = (event.target as HTMLElement).closest<HTMLElement>("[data-review-row]")
  const shown = row?.dataset.reviewRow ?? null
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault()
    const next = neighbourRow(rows, shown, event.key === "ArrowDown" ? 1 : -1)
    const target = next === null ? null : event.currentTarget.querySelector<HTMLElement>(`[data-review-row="${CSS.escape(next)}"]`)
    if (target != null) {
      setFocused(next!)
      target.focus()
    }
    return
  }
  if (row == null || shown === null || event.target !== row || !rows.includes(shown)) {
    return
  }
  const stack = row.dataset.reviewStack
  if (stack !== undefined) {
    const open = event.key === "Enter" ? "toggle" : event.key === "ArrowRight" ? true : event.key === "ArrowLeft" ? false : null
    if (open !== null && !event.shiftKey) {
      event.preventDefault()
      keys.onStack?.(stack, open)
    } else if (event.code === "KeyD" && !event.shiftKey) {
      event.preventDefault()
      keys.onReviewStack?.(stack)
    } else if (isMenuKey(event)) {
      openRowMenu(event, row)
    }
    return
  }
  const number = Number(row.dataset.reviewNumber)
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault()
    keys.onEnter(number)
  } else if (event.code === "KeyD" && !event.shiftKey) {
    event.preventDefault()
    keys.onReview(number)
  } else if (event.code === "KeyO" && !event.shiftKey) {
    event.preventDefault()
    keys.onOpen(number)
  } else if (isMenuKey(event)) {
    openRowMenu(event, row)
  }
}

/** The menu of `row` at its start, as a right click there opens it */
function openRowMenu(event: KeyboardEvent<HTMLElement>, row: HTMLElement): void {
  const rect = row.getBoundingClientRect()
  openMenuByKey(event, row, rect.left + 24, rect.bottom)
}
