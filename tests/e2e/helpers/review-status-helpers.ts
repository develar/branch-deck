import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"
import { findBranchRow, pushFromRowMenu } from "./branch-helpers"
import type { PushResult } from "./push-helpers"
import { expectPushedToast, routePush } from "./push-helpers"

/** The commit the review took (a robot's source, a pull request's head) that the local branch no longer is at */
export const EARLIER_COMMIT = "0123456789abcdef0123456789abcdef01234567"

/**
 * Pushes the branch with the push of its row's menu and waits for it; the remote answers with `result` (`routePush`),
 * e.g. the remote status of a branch whose remote is at `EARLIER_COMMIT`. The menu, not the push button: the first push
 * of a branch whose landing pushes it has no button on the row (`isFirstPushInLanding`).
 */
export async function pushBranchFromRow(page: Page, branchName: string, result: PushResult = {}): Promise<void> {
  await routePush(page, { [branchName]: result })
  await pushFromRowMenu(page, branchName)
  await expectPushedToast(page, branchName)
  // a branch with a push button on the row shows the progress, then the result, in its description until Escape or a
  // click elsewhere; left open, it competes with the next hover of a review chip
  await page.keyboard.press("Escape")
  await expect(page.getByTestId("push-description")).toBeHidden()
  await page.mouse.move(0, 0)
}

/**
 * Hovers a status chip (pull request, Patronus robot) and returns its details popover. The row hover gives the text of
 * a finished robot in the Safe Push slot to "Safe Push…": its shield, the chip now, moves from under the pointer, so it
 * is hovered once more.
 */
export async function hoverStatusChip(page: Page, chip: Locator, popoverTestId: string): Promise<Locator> {
  await chip.hover()
  await chip.hover()
  const popover = page.getByTestId(popoverTestId)
  await expect(popover).toBeVisible()
  return popover
}

/**
 * Clicks a status chip that opens its item a second after its hover opened (`hoverStatusChip`), later than the 500 ms
 * within which Base UI would pin the details: they stay open, as the pointer does (`HoverDescription`)
 */
export async function clickStatusChipAfterHover(page: Page, chip: Locator, popoverTestId: string): Promise<void> {
  const popover = await hoverStatusChip(page, chip, popoverTestId)
  await page.clock.fastForward(1000)
  await chip.click()
  await expect(popover).toHaveAttribute("data-open", "")
}

/** Hovers the landing indicator of the header and returns its overview */
export async function openLandingOverview(page: Page): Promise<Locator> {
  await page.getByTestId("landing-indicator").hover()
  const popover = page.getByTestId("landing-popover")
  await expect(popover).toBeVisible()
  return popover
}

/** The row of `name` in the landing overview: a branch, the root of a stack */
export function landingRow(popover: Locator, name: string): Locator {
  return popover.locator(`[data-testid="landing-row"][data-branch-name="${name}"]`)
}

/** Opens the preview of a branch with Space; its keys work once its body has the focus */
export async function openBranchPreview(page: Page, branchName: string): Promise<Locator> {
  await findBranchRow(page, branchName).hover()
  await page.keyboard.press("Space")
  const preview = page.getByTestId("branch-preview")
  await expect(preview.getByTestId("branch-preview-body")).toBeFocused()
  return preview
}

/** Closes the preview with Space and opens the one of another branch */
export async function switchBranchPreview(page: Page, preview: Locator, branchName: string): Promise<void> {
  await page.keyboard.press("Space")
  await expect(preview).toHaveCount(0)
  await findBranchRow(page, branchName).hover()
  await page.keyboard.press("Space")
  await expect(preview.getByRole("heading")).toHaveText(branchName)
}
