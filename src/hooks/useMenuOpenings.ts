import { createContext, useState } from "react"

/**
 * How often the menu root around it opened: an open menu shows its items as they were at its opening (`MenuItems`,
 * `frozenMenuGroups`), and a menu reopened while its popup still fades out keeps its elements, so the opening, not the
 * mount, starts the list anew
 */
export const MenuOpeningContext = createContext(0)

/** The openings of a menu root (`MenuOpeningContext`), counted by its `onOpenChange`: an open nothing canceled is one */
export function useMenuOpenings<Details extends { readonly isCanceled: boolean }>(
  onOpenChange: ((open: boolean, details: Details) => void) | undefined,
): { readonly openings: number; readonly onOpenChange: (open: boolean, details: Details) => void } {
  const [openings, setOpenings] = useState(0)
  return {
    openings,
    onOpenChange: (next, details) => {
      onOpenChange?.(next, details)
      if (next && !details.isCanceled) {
        setOpenings(count => count + 1)
      }
    },
  }
}
