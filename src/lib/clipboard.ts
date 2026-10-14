import { notifyError } from "@/lib/errors"

/**
 * Writes text to the clipboard; a failure is an error toast. Resolves to whether it was copied, so a button can say
 * "Copied" in place (`useCopyToClipboard`); a menu item just fires it.
 */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  }
  catch (error) {
    notifyError("Copy Failed", error)
    return false
  }
}
