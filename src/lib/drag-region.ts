/**
 * The empty parts of a drag region of the title bar (`data-tauri-drag-region`), by the rule of Tauri's drag script
 * (`drag.js`). It reads no title bar, so the page of the IDE imports it before its bootstrap sets `__BD_TITLE_BAR__`.
 */

/** The tags and roles that are controls in a drag region, those of Tauri's drag script (`drag.js`) */
const CLICKABLE_TAGS = new Set(["A", "BUTTON", "INPUT", "SELECT", "TEXTAREA", "LABEL", "SUMMARY"])
const INTERACTIVE_ROLES = new Set(["button", "link", "menuitem", "tab", "checkbox", "radio", "switch", "option"])

/** The event is on the empty parts of a drag region, by the rule of Tauri's drag script */
export function isOnDragRegion(path: readonly EventTarget[]): boolean {
  for (const target of path) {
    if (!(target instanceof HTMLElement)) {
      continue
    }
    const region = target.getAttribute("data-tauri-drag-region")
    const tabIndex = target.getAttribute("tabindex")
    const editable = target.getAttribute("contenteditable")
    const clickable =
      CLICKABLE_TAGS.has(target.tagName) ||
      (editable !== null && editable !== "false") ||
      (tabIndex !== null && tabIndex !== "-1") ||
      INTERACTIVE_ROLES.has(target.getAttribute("role") ?? "")
    if (clickable && region === null) {
      return false
    }
    if (region === null) {
      continue
    }
    if (region === "false") {
      return false
    }
    if (region === "deep") {
      return true
    }
    return target === path[0]
  }
  return false
}
