import { createContext, useContext } from "react"

/** Closes the branch preview around the component; null outside it */
export const ClosePreviewContext = createContext<(() => void) | null>(null)

/**
 * The close of the branch preview around the component; null outside it. An action of the row asked from inside the
 * modal preview (the start of a Safe Push, whose popover opens at the row's slot) closes the preview first
 */
export function useClosePreview(): (() => void) | null {
  return useContext(ClosePreviewContext)
}
