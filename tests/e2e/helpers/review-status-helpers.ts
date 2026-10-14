import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"
import { findBranchRow, getPushButton } from "./branch-helpers"
import type { PushResult } from "./push-helpers"
import { routePush } from "./push-helpers"

/** The commit the review took (a robot's source, a pull request's head) that the local branch no longer is at */
export const EARLIER_COMMIT = "0123456789abcdef0123456789abcdef01234567"

/**
 * Pushes the branch with its push button and waits for it; the remote answers with `result` (`routePush`), e.g. the
 * remote status of a branch whose remote is at `EARLIER_COMMIT`
 */
export async function pushBranchFromRow(page: Page, branchName: string, result: PushResult = {}): Promise<void> {
  await routePush(page, { [branchName]: result })
  await getPushButton(findBranchRow(page, branchName)).click()
  await expect(page.getByText(`Branch ${branchName} pushed successfully`)).toBeVisible()
  // the click opened the description of the push button, which stays open (the progress, then the result) until Escape
  // or a click elsewhere; left open, it competes with the next hover of a review chip
  await page.keyboard.press("Escape")
  await expect(page.getByTestId("push-description")).toBeHidden()
  await page.mouse.move(0, 0)
}

/** Hovers a status chip (pull request, Patronus robot) and returns its details popover */
export async function hoverStatusChip(page: Page, chip: Locator, popoverTestId: string): Promise<Locator> {
  await chip.hover()
  const popover = page.getByTestId(popoverTestId)
  await expect(popover).toBeVisible()
  return popover
}

/** Hovers the outgoing indicator and returns the overview */
export async function openOutgoingOverview(page: Page): Promise<Locator> {
  await page.getByTestId("outgoing-indicator").hover()
  const popover = page.getByTestId("outgoing-popover")
  await expect(popover).toBeVisible()
  return popover
}

/**
 * Checks that the outgoing indicator has the attention dot and names it ("…, 1 needs attention"), opens the overview and
 * checks that its line says why ("ABC-123 needs attention: failed Safe Push"); returns the overview
 */
export async function openOverviewWithAttention(page: Page, indicatorTitle: string, line: string): Promise<Locator> {
  const indicator = page.getByTestId("outgoing-indicator")
  await expect(indicator.getByTestId("outgoing-attention-dot")).toBeVisible()
  await expect(indicator).toHaveAccessibleName(`${indicatorTitle}, 1 needs attention`)
  const popover = await openOutgoingOverview(page)
  await expect(popover.getByTestId("outgoing-attention")).toHaveText(line)
  return popover
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
