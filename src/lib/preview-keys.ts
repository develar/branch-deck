import type { KeyboardEvent } from "react"

/** Where a key of the window belongs to its target ("/", F2): a text field, a dialog or a menu */
export const KEY_OWNERS = "input, textarea, select, [contenteditable]:not([contenteditable=false]), [role=dialog], [role=menu]"

export function hasKeyModifier(event: KeyboardEvent): boolean {
  return event.metaKey || event.ctrlKey || event.altKey || event.shiftKey
}

/** The key is typed into a text field (the comment of a review, a commit message): it is no key of the preview */
export function isTypingTarget(event: KeyboardEvent): boolean {
  return event.target instanceof HTMLElement && (event.target.isContentEditable || event.target.closest("input, textarea, select") !== null)
}

/**
 * ↑/↓ show the previous or next item, Space closes the preview. Returns true if the key was handled.
 * Call it for a key without modifiers.
 */
export function handlePreviewNavigationKey(event: KeyboardEvent, index: number, count: number, onIndexChange: (index: number) => void, onClose: () => void): boolean {
  if (isTypingTarget(event)) {
    return false
  }
  if (event.key === "ArrowUp" || event.key === "ArrowDown") {
    moveIndex(event, index, count, onIndexChange)
    return true
  }
  // Space on a button or a link presses it
  if (event.key === " " && !(event.target instanceof Element && event.target.closest("button, a") !== null)) {
    event.preventDefault()
    onClose()
    return true
  }
  return false
}

/**
 * ⌥↑/⌥↓ (Alt+↑/↓) show the previous or next item also where ↑/↓ belong to the content: the file tree, a diff. Returns
 * true if the key was handled.
 */
export function handlePreviewItemSwitchKey(event: KeyboardEvent, index: number, count: number, onIndexChange: (index: number) => void): boolean {
  if (!event.altKey || event.metaKey || event.ctrlKey || event.shiftKey || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) {
    return false
  }
  moveIndex(event, index, count, onIndexChange)
  return true
}

function moveIndex(event: KeyboardEvent, index: number, count: number, onIndexChange: (index: number) => void): void {
  event.preventDefault()
  const next = index + (event.key === "ArrowUp" ? -1 : 1)
  if (next >= 0 && next < count) {
    onIndexChange(next)
  }
}
