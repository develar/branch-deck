import { createContext, useContext } from "react"
import type { InlineActionType } from "@/stores/inline-row-action"
import { openInline } from "@/stores/inline-row-action"

/** Closes the branch preview around the component; null outside it */
export const ClosePreviewContext = createContext<(() => void) | null>(null)

/**
 * Opens the inline form of a branch row. The form is below the row, behind the modal branch preview: from inside the
 * preview (`closePreview`), the preview closes first.
 */
export function openRowForm(type: InlineActionType, branchName: string, closePreview: (() => void) | null): void {
  closePreview?.()
  openInline(type, branchName)
}

/** `openRowForm` for a component that may be inside the branch preview (the Patronus details, the chip popover) */
export function useOpenRowForm(): (type: InlineActionType, branchName: string) => void {
  const closePreview = useContext(ClosePreviewContext)
  return (type, branchName) => openRowForm(type, branchName, closePreview)
}
