// @spec spec/branch/unapply.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getMoreButton } from "../helpers/branch-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"
import { routeCommandError } from "../helpers/wait-helpers"

// the `branch_filter` template: the stack core → api → web with cli forked off core, and seven branches on their own,
// each with a file of its own
const BRANCH_COUNT = 11

function toast(page: Page, title: string): Locator {
  return page.locator("[data-slot=toast]").filter({ hasText: title })
}

/** Opens Unapply… from the menu that `open` opens and waits for its confirmation, its button focused */
async function openUnapply(page: Page, open: () => Promise<void>): Promise<Locator> {
  await open()
  await page.getByRole("menuitem", { name: "Unapply…" }).click()
  const confirm = page.getByRole("button", { name: "Unapply Branch" })
  await expect(confirm).toBeFocused()
  return confirm
}

test.describe("Unapplied branches", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("branch_filter")
    await syncAndWaitForBranches()
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT)
  })

  test("should unapply a branch from its menus: its commits and their files leave HEAD, the branch is kept", async ({ page }) => {
    // the right-click menu, confirmed with Enter
    await openUnapply(page, () => findBranchRow(page, "docs").click({ button: "right" }))
    await expect(page.getByText("This removes 1 commit and their changes from HEAD")).toBeVisible()
    await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
    await expect(toast(page, "Unapplied docs")).toContainText("kept in user-name/unapplied/docs")
    await expect(findBranchRow(page, "docs")).toHaveCount(0)
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT - 1)
    // the files followed: nothing is left as an uncommitted change
    await expect(page.getByTestId("uncommitted-changes-card")).toHaveCount(0)

    // the "⋯" menu, confirmed with the button
    const ci = findBranchRow(page, "ci")
    await ci.hover()
    const confirm = await openUnapply(page, () => getMoreButton(ci).click())
    await runAndWaitForSync(page, () => confirm.click())
    await expect(toast(page, "Unapplied ci")).toContainText("kept in user-name/unapplied/ci")
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT - 2)
    await expect(page.getByTestId("uncommitted-changes-card")).toHaveCount(0)
  })

  test("should take the unapply back with the Undo of its toast: the commits, their files and the branch return", async ({ page }) => {
    await openUnapply(page, () => findBranchRow(page, "docs").click({ button: "right" }))
    await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
    const unapplied = toast(page, "Unapplied docs")
    await expect(findBranchRow(page, "docs")).toHaveCount(0)

    await runAndWaitForSync(page, () => unapplied.getByRole("button", { name: "Undo" }).click())
    await expect(findBranchRow(page, "docs")).toHaveCount(1)
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT)
    await expect(page.getByTestId("uncommitted-changes-card")).toHaveCount(0)
    await expect(findBranchRow(page, "docs")).toContainText("1 commit")
  })

  test("should keep the branch and say why when the unapply fails, and close the confirmation with Escape", async ({ page }) => {
    await openUnapply(page, () => findBranchRow(page, "docs").click({ button: "right" }))
    await page.keyboard.press("Escape")
    await expect(page.getByRole("button", { name: "Unapply Branch" })).toHaveCount(0)

    await routeCommandError(page, "unapply_branch", "Uncommitted, staged or untracked files sit on paths the unapply writes: docs.md")
    await openUnapply(page, () => findBranchRow(page, "docs").click({ button: "right" }))
    await page.keyboard.press("Enter")
    await expect(toast(page, "Cannot unapply docs")).toContainText("docs.md")
    await expect(findBranchRow(page, "docs")).toHaveCount(1)
  })
})
