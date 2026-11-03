// @spec spec/branch/branch-row.spec.md
import type { Locator } from "@playwright/test"
import { testWithIssueLinks as test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getBranchCells, getMoreButton, verifyBranchState } from "../helpers/branch-helpers"

/** The outline an element draws now: the focus token (`bd-focus`) is a solid 2px one, `none` without a focus to show */
function outlineOf(element: Locator) {
  return element.evaluate(it => {
    const style = getComputedStyle(it)
    return { style: style.outlineStyle, width: style.outlineWidth, offset: style.outlineOffset }
  })
}

const NO_OUTLINE = expect.objectContaining({ style: "none" })
const INSIDE = { style: "solid", width: "2px", offset: "-2px" }
const OUTSIDE = { style: "solid", width: "2px", offset: "2px" }

test.describe("The focus outline of a branch row", () => {
  test("should outline the row the arrows focused inside it, a button of the row outside it, and nothing a click focused", { tag: "@webkit" }, async ({ page }) => {
    const first = findBranchRow(page, "JIRA-456")
    const second = findBranchRow(page, "ABC-123")

    // the arrows move the focus from the table down the rows; a row draws it inside its box, the rows around in place
    await page.getByRole("columnheader", { name: "Branch Name" }).click()
    await page.keyboard.press("ArrowDown")
    await expect(first).toBeFocused()
    await expect.poll(() => outlineOf(first)).toEqual(INSIDE)
    await page.keyboard.press("ArrowDown")
    await expect(second).toBeFocused()
    await expect.poll(() => outlineOf(second)).toEqual(INSIDE)
    await expect.poll(() => outlineOf(first)).toEqual(NO_OUTLINE)

    // a button of the focused row keeps its outline outside: the inset of the row is not inherited
    const more = getMoreButton(second)
    await more.focus()
    await expect(more).toBeFocused()
    await expect.poll(() => outlineOf(more)).toEqual(OUTSIDE)
    await expect.poll(() => outlineOf(second)).toEqual(NO_OUTLINE)

    // a click shows no focus: the pointer is where the user looks (WebKit focuses no button it clicks, Chromium does)
    const folder = getBranchCells(first).branchName.locator("button").first()
    await folder.click()
    await verifyBranchState(first, "open")
    await expect.poll(() => outlineOf(folder)).toEqual(NO_OUTLINE)
    await expect.poll(() => outlineOf(more)).toEqual(NO_OUTLINE)
  })
})
