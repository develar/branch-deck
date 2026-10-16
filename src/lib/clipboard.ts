import { writeText } from "@tauri-apps/plugin-clipboard-manager"
import { notifyError } from "@/lib/errors"

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
