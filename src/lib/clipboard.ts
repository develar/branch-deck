import type { KeyboardEvent as ReactKeyboardEvent } from "react"
import { writeText } from "@tauri-apps/plugin-clipboard-manager"
import { notifyError } from "@/lib/errors"
import { isMac } from "@/lib/platform"

/**
 * Writes text to the clipboard; a failure is an error toast. Resolves to whether it was copied, so a button can say
 * "Copied" in place (`useCopyToClipboard`); a menu item just fires it.
 */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch (error) {
    notifyError("Cannot copy to the clipboard", error)
    return false
  }
}

/**
 * Writes text that is still being built (a backend call, the network) to the clipboard. WebKit lets the page write the
 * clipboard only within the click, so it goes through the clipboard plugin of the app instead. A failure of `text` or
 * of the write is thrown to the caller, which names what it could not copy.
 */
export async function copyTextLater(text: Promise<string>): Promise<void> {
  await writeText(await text)
}

/**
 * ⌘C (Ctrl+C elsewhere) on a focused row copies what its menu's copy item does (Copy Path of a file, Copy Full Branch
 * Name of a branch); not while text is selected, which the native copy takes. The key of the layout, as the system's
 * copy: not the physical key of the review's letters.
 */
export function isCopyKey(event: KeyboardEvent | ReactKeyboardEvent): boolean {
  const selection = window.getSelection()
  return (
    event.key.toLowerCase() === "c" &&
    (isMac ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey) &&
    !event.altKey &&
    !event.shiftKey &&
    (selection === null || selection.isCollapsed)
  )
}

/** ⌘C on the focused row of a table of branches (`tr[data-branch-name]`) copies its full name, as Copy Full Branch Name of its menu */
export function copyFocusedBranchRow(event: ReactKeyboardEvent<HTMLElement>): void {
  const name = (event.target as HTMLElement).closest<HTMLElement>("tr[data-branch-name]")?.dataset.branchName
  if (name !== undefined && isCopyKey(event)) {
    event.preventDefault()
    void copyText(name)
  }
}
