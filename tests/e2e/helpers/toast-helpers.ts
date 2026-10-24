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
