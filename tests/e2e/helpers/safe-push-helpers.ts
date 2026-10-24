import type { Locator, Page } from "@playwright/test"
import type { PatronusStartOptions } from "../../../src/lib/bindings"
import { expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "./branch-helpers"
import { closeMenus, openSubmenu } from "./selection-helpers"
import { animationsEnded } from "./wait-helpers"

/** The merge profiles of master without any: a start runs the default checks */
export const DEFAULT_CHECKS_ONLY: PatronusStartOptions = { targetBranch: "master", supported: true, profiles: [], emergencyProfiles: [], defaultProfileId: null }

/** The popover of a start (`SafePushPopover`): it opens at the slot of the row, or of the island header */
export function safePushPopover(page: Page): Locator {
  return page.getByTestId("safe-push-popover")
}

/** The start button of the popover ("Safe Push", "Dry Run", "Safe Push Without Review") */
export function popoverStart(page: Page): Locator {
  return safePushPopover(page).getByTestId("safe-push-start")
}

/** The Safe Push button of the row's slot ("Safe Push", "Safe Push · 3"; "Checking…", "45%" (named "Pushing 45%"), "Starting…" while it starts) */
export function slotSafePush(page: Page, branchName: string): Locator {
  return findBranchRow(page, branchName).getByTestId("safe-push-button")
}

/**
 * S on the row: the popover of its Safe Push, never a start by itself; resolves once its start took the focus (it can
 * start), so Enter starts the robot. Pressed once the facts of its step are in: before them S says what the step waits
 * for (`Checking…`)
 */
export async function openSafePushWithKey(page: Page, branchName: string): Promise<Locator> {
  const row = findBranchRow(page, branchName)
  await expect(row.getByTestId("next-step-slot")).not.toHaveAttribute("data-step", "pending")
  await row.focus()
  await page.keyboard.press("s")
  const popover = safePushPopover(page)
  await expect(popover).toBeVisible()
  return popover
}

/** "Safe Push With Profile…" of the branch menu: the popover, its profile to choose */
export async function openSafePushWithProfile(page: Page, branchName: string): Promise<Locator> {
  await findBranchRow(page, branchName).click({ button: "right" })
  await openSubmenu(page, "Patronus")
  await page.getByRole("menuitem", { name: "Safe Push With Profile…" }).click()
  const popover = safePushPopover(page)
  await expect(popover).toBeVisible()
  return popover
}

/** Chooses a merge profile in the popover's Select */
export async function chooseProfile(page: Page, name: string): Promise<void> {
  await safePushPopover(page).getByTestId("safe-push-profile").click()
  await page.getByRole("option", { name, exact: true }).click()
}

/** Closes the sticky error toast of a start Patronus refused: it covers no menu of the next surface */
export async function closeRefusal(page: Page): Promise<void> {
  const refused = page.locator("[data-slot=toast]", { hasText: "Cannot start the Safe Push" })
  // Base UI hides the close button from the accessibility tree
  await refused.locator("[data-slot=toast-close]").click()
  await expect(refused).toHaveCount(0)
}

/** The rows the path to the root is drawn through (`stack-path-lanes`) */
export async function pathRows(page: Page): Promise<string[]> {
  return page
    .getByTestId("branch-row")
    .evaluateAll(rows => rows.filter(row => row.querySelector("[data-testid=stack-path-lanes]") !== null).map(row => row.getAttribute("data-branch-name") ?? ""))
}

/**
 * The popover settles at `slot`, above or below it: the slide of its opening moves it (a canceled one ends it too), and
 * so does a line that arrives later (the trial build alone): above the slot it grows upwards a frame after it grew
 */
export async function expectPopoverAtSlot(popover: Locator, slot: Locator): Promise<void> {
  await expect
    .poll(async () => {
      await animationsEnded(popover)
      const box = (await popover.boundingBox())!
      const anchor = (await slot.boundingBox())!
      return Math.min(Math.abs(box.y - (anchor.y + anchor.height)), Math.abs(box.y + box.height - anchor.y))
    })
    .toBeLessThan(24)
}

/**
 * The popover of a Safe Push without review, opened from the pull request dialog: no profile chosen for the user, the
 * approvals of `groups` it goes without named
 */
export async function expectPopoverWithoutReview(page: Page, groups: string): Promise<Locator> {
  const popover = safePushPopover(page)
  await expect(popover).toHaveAttribute("data-decision", "choose")
  await expect(popover.getByTestId("safe-push-status")).toHaveText("Choose the profile that skips the code-owner review")
  await expect(popover.getByTestId("safe-push-review")).toHaveText(`Starts without the approvals of ${groups}: the profile you choose has to skip the code-owner review.`)
  return popover
}

/**
 * A branch a merged Safe Push landed (`reason`: "Landed in master with oauth"): its menu disables Safe Push, Dry Run and
 * Push, each saying why (the push that nothing of it is left to push), and S on the row says it and opens nothing
 */
export async function expectLandedActions(page: Page, branchName: string, reason: string): Promise<void> {
  const row = findBranchRow(page, branchName)
  await row.click({ button: "right" })
  await expectDisabledItems(page, [
    // "Safe Push · 2" while a branch it is built on has commits after the landed ones
    [/^Safe Push( · \d+)? S/, reason],
    [/^Push P$/, `${reason}: nothing of it is left to push`],
  ])
  await openSubmenu(page, "Patronus")
  await expectDisabledItems(page, [[/^Dry Run/, reason]])
  await closeMenus(page)
  await row.focus()
  await page.keyboard.press("s")
  await expect(page.locator("[data-slot=toast]", { hasText: `Safe Push of ${branchName}` }).locator("[data-slot=toast-description]")).toHaveText(reason)
  await expect(safePushPopover(page)).toHaveCount(0)
}

/**
 * Each item of the open menu, by its name (a string is the exact name), is disabled and says why on hover: the reason
 * in its tooltip
 */
export async function expectDisabledItems(page: Page, items: readonly (readonly [string | RegExp, string])[]): Promise<void> {
  for (const [name, expected] of items) {
    const item = page.getByRole("menuitem", typeof name === "string" ? { name, exact: true } : { name })
    await expect(item).toHaveAttribute("aria-disabled", "true")
    await item.hover()
    // the description of the item before may still be fading out
    await expect(page.getByRole("tooltip").filter({ hasText: expected }).last()).toHaveText(expected)
  }
}
