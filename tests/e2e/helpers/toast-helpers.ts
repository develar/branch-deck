import type { Locator } from "@playwright/test"
import { expect } from "@playwright/test"

/**
 * Closes `toast` with its close button, as the user does with a toast read, and waits until it is gone. The toasts lie
 * above the menus and the dialogs (`z-[60]` of `src/components/ui/toast.tsx`): one at the bottom right covers a menu
 * item or a row's button there, and a click under it waits until it times out (5 s, 10 s with an action)
 */
export async function closeToast(toast: Locator): Promise<void> {
  // Base UI hides the close button from the accessibility tree
  await toast.locator("[data-slot=toast-close]").click()
  await expect(toast).toHaveCount(0)
}

let held = 0

/**
 * The toast `toast` finds now, held by its element: a later state of its event replaces it in place
 * (`src/lib/event-toast.ts`), with words `toast` no longer finds, and this locator finds the same element wherever it
 * is. The element is marked (`data-held`), so a new toast in its place is not it.
 */
export async function holdToast(toast: Locator): Promise<Locator> {
  const mark = `held-${++held}`
  await toast.evaluate((element, value) => element.setAttribute("data-held", value), mark)
  return toast.page().locator(`[data-slot=toast][data-held="${mark}"]`)
}
