import type { Page } from "@playwright/test"
import type { BranchReviewWindowData, ReviewWindowData } from "../../../src/types/sub-window"
import { expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "./branch-helpers"
import { getOpenedSubWindows, pressAndExpectOpened } from "./sub-window-helpers"
import type { OpenedSubWindow } from "./sub-window-helpers"

/*
 * The one review window (openspec/specs/review/review-window/spec.md): a branch with its stack, a whole stack, a kept branch, a
 * list of commits, each an open of the window `review` on `/review`
 */

/**
 * Runs `press`, which makes `page` open the review window, and returns that open, the `count`-th window `page` opened
 * (`pressAndExpectOpened`: read once the open answered, never by a poll after the press)
 */
export async function pressAndExpectReviewOpened<T extends ReviewWindowData = BranchReviewWindowData>(
  page: Page,
  press: () => Promise<unknown>,
  count: number,
): Promise<OpenedSubWindow<T>> {
  const opened = await pressAndExpectOpened<T>(page, press, "review", count)
  expect(opened.url).toBe("/review")
  return opened
}

/** "Review Changes" of the menu of the row of `branch`: the review window it opens, titled "Review <branch>" */
export async function openReviewWindow(page: Page, branch: string): Promise<OpenedSubWindow<BranchReviewWindowData>> {
  const count = (await getOpenedSubWindows(page)).length
  await findBranchRow(page, branch).click({ button: "right" })
  const opened = await pressAndExpectReviewOpened(page, () => page.getByRole("menuitem", { name: "Review Changes" }).click(), count + 1)
  expect(opened.title).toBe(`Review ${branch}`)
  return opened
}
