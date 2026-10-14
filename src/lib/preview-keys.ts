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
    event.preventDefault()
    const next = index + (event.key === "ArrowUp" ? -1 : 1)
    if (next >= 0 && next < count) {
      onIndexChange(next)
    }
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
