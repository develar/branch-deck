import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/base-test"
import { nextAnswer } from "./sync-helpers"
import type { TestRepositoryBuilder } from "./test-repository"
import { expectClosed } from "./wait-helpers"

/**
 * Closes `toast` with its close button, as the user does with a toast read, and waits until it is gone. The toasts lie
 * above the menus and the dialogs (`z-[60]` of `src/components/ui/toast.tsx`): one at the bottom right covers a menu
 * item or a row's button there, and a click under it waits until it times out (5 s, 10 s with an action)
 */
export async function closeToast(toast: Locator): Promise<void> {
  // Base UI hides the close button from the accessibility tree
  await toast.locator("[data-slot=toast-close]").click()
  // its exit slides for 500 ms, which ends only as the window paints: closed by its state, the slide ended for it
  await expectClosed(toast)
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

/**
 * The open toast whose title is `title`: a toast of an event that timed out while its operation ran on stays in its exit
 * (`data-ending-style`) until the page paints, which a loaded machine may not do for seconds, and the next state of the
 * event shows a new one beside it, the toast the user reads
 */
export function openToast(page: Page, title: string | RegExp): Locator {
  return page.locator("[data-slot=toast]:not([data-ending-style])").filter({ has: page.locator("[data-slot=toast-title]", { hasText: title }) })
}

/**
 * Starts a landing into the target by `start` (a push or a merge, answered by `execute`) while the sync of the update
 * after it waits: the toasts of `title` say `running` and stay past the 5 s of a toast without an action
 * (`TOAST_TIMEOUT_MS`), one toast; then the update runs to its end and `execute` answers. Every toast of the title, a
 * closing one too, so that an end of the update in a second toast counts
 */
export async function landPastTheTimeOfAToast(page: Page, repo: TestRepositoryBuilder, start: () => Promise<unknown>, title: string, running: string): Promise<Locator> {
  await repo.holdRemoteAnswers({ hold: ["sync"] })
  const answered = nextAnswer(page, "execute")
  await start()
  const toasts = page.locator("[data-slot=toast]", { hasText: title })
  await expect(toasts).toContainText(running)
  await repo.arrival("sync")
  await page.clock.fastForward(6000)
  await expect(toasts).toHaveCount(1)
  await expect(toasts).not.toHaveAttribute("data-ending-style")
  await repo.holdRemoteAnswers({ release: ["sync"] })
  await answered
  return toasts
}
