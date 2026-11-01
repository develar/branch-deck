import type { Page } from "@playwright/test"
import type { BranchReviewWindowData, ReviewWindowData } from "../../../src/types/sub-window"
import { expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "./branch-helpers"
import { getOpenedSubWindows } from "./sub-window-helpers"
import type { OpenedSubWindow } from "./sub-window-helpers"

/*
 * The one review window (spec/review/review-window.spec.md): a branch with its stack, a whole stack, a kept branch, a
 * list of commits, each an open of the window `review` on `/review`
 */

/** The `count`-th window the page opened (`open_sub_window`), once it is recorded: the review window */
export async function openedReview<T extends ReviewWindowData = BranchReviewWindowData>(page: Page, count: number): Promise<OpenedSubWindow<T>> {
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(count)
  const opened = (await getOpenedSubWindows<T>(page))[count - 1]!
  expect(opened).toMatchObject({ windowId: "review", url: "/review" })
  return opened
}

/** "Review Changes" of the menu of the row of `branch`: the review window it opens, titled "Review <branch>" */
export async function openReviewWindow(page: Page, branch: string): Promise<OpenedSubWindow<BranchReviewWindowData>> {
  const count = (await getOpenedSubWindows(page)).length
  await findBranchRow(page, branch).click({ button: "right" })
  await page.getByRole("menuitem", { name: "Review Changes" }).click()
  const opened = await openedReview(page, count + 1)
  expect(opened.title).toBe(`Review ${branch}`)
  return opened
}
