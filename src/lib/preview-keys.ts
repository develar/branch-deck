import type { KeyboardEvent } from "react"

export function hasKeyModifier(event: KeyboardEvent): boolean {
  return event.metaKey || event.ctrlKey || event.altKey || event.shiftKey
}

/**
 * ↑/↓ show the previous or next item, Space closes the preview. Returns true if the key was handled.
 * Call it for a key without modifiers.
 */
export function handlePreviewNavigationKey(event: KeyboardEvent, index: number, count: number, onIndexChange: (index: number) => void, onClose: () => void): boolean {
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
