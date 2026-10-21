// @spec spec/branch/unapply.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getMoreButton } from "../helpers/branch-helpers"
import { inlineDeleteConfirmation, submitInlineForm } from "../helpers/inline-form-helpers"
import { routePushBranches } from "../helpers/push-helpers"
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

function unappliedCard(page: Page): Locator {
  return page.getByTestId("unapplied-branches-card")
}

function unappliedRow(page: Page, name: string): Locator {
  return unappliedCard(page).locator(`tr[data-branch-name="user-name/unapplied/${name}"]`)
}

/** Unapplies `name` from its right-click menu, confirmed with Enter; resolves once the sync showed it */
async function unapplyFromMenu(page: Page, name: string): Promise<void> {
  await openUnapply(page, () => findBranchRow(page, name).click({ button: "right" }))
  await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
  await expect(unappliedRow(page, name)).toBeVisible()
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

  test("should list an unapplied branch in its own card with its commits, out of the branch table", async ({ page }) => {
    await expect(unappliedCard(page)).toHaveCount(0)
    await unapplyFromMenu(page, "docs")
    await expect(unappliedCard(page)).toContainText("1 branch")
    const row = unappliedRow(page, "docs")
    await expect(row).toContainText("Document the API")
    await expect(row.getByTestId("unapplied-branch-remote")).toHaveText("Not pushed")
    await expect(findBranchRow(page, "docs")).toHaveCount(0)

    // the row expands to its commits
    await row.click()
    await expect(unappliedCard(page).getByText("Document the API").nth(1)).toBeVisible()

    await unappliedCard(page).getByTestId("unapplied-branches-help-icon").click()
    await expect(page.locator("[data-slot=popover-title]")).toHaveText("Unapplied")
  })

  test("should push an unapplied branch under its own name from both menus, and refuse its Safe Push with why", async ({ page }) => {
    const { stackPushes } = await routePushBranches(page)
    await unapplyFromMenu(page, "docs")
    const row = unappliedRow(page, "docs")

    // the right-click menu
    await row.click({ button: "right" })
    const safePush = page.getByRole("menuitem", { name: "Safe Push" })
    await expect(safePush).toHaveAttribute("aria-disabled", "true")
    await safePush.hover()
    await expect(page.getByRole("tooltip")).toHaveText("Not in master: Reapply it to land it")
    await runAndWaitForSync(page, () => page.getByRole("menuitem", { name: "Push", exact: true }).click())
    await expect(toast(page, "Pushed docs")).toContainText("The remote keeps it as user-name/unapplied/docs")
    expect(stackPushes).toEqual([[{ branchName: "docs", expectedRemote: null, unapplied: true }]])

    // the "⋯" menu
    await row.hover()
    await row.getByRole("button", { name: "More actions for docs" }).click()
    await runAndWaitForSync(page, () => page.getByRole("menuitem", { name: "Push", exact: true }).click())
    expect(stackPushes).toHaveLength(2)
  })

  test("should delete an unapplied branch after its name is typed", async ({ page }) => {
    await unapplyFromMenu(page, "docs")
    await unappliedRow(page, "docs").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Delete Unapplied Branch…" }).click()
    await inlineDeleteConfirmation.waitForVisible(page)
    await expect(inlineDeleteConfirmation.getPortal(page)).toContainText("only a copy on the remote keeps them")
    await runAndWaitForSync(page, () => submitInlineForm(inlineDeleteConfirmation.getInput(page), "docs"))
    await expect(toast(page, "Deleted the unapplied branch docs")).toBeVisible()
    await expect(unappliedCard(page)).toHaveCount(0)
  })

  test("should reapply an unapplied branch from both menus, the branch back in the table, and take it back with Undo", async ({ page }) => {
    await unapplyFromMenu(page, "docs")

    // the right-click menu
    await unappliedRow(page, "docs").click({ button: "right" })
    await runAndWaitForSync(page, () => page.getByRole("menuitem", { name: "Reapply" }).click())
    const reapplied = toast(page, "Reapplied docs")
    await expect(reapplied).toContainText("1 commit back on top of HEAD")
    await expect(findBranchRow(page, "docs")).toHaveCount(1)
    await expect(unappliedCard(page)).toHaveCount(0)
    await expect(page.getByTestId("uncommitted-changes-card")).toHaveCount(0)

    // its Undo takes it out again, and the unapplied branch is back
    await runAndWaitForSync(page, () => reapplied.getByRole("button", { name: "Undo" }).click())
    await expect(unappliedRow(page, "docs")).toBeVisible()
    await expect(findBranchRow(page, "docs")).toHaveCount(0)

    // the "⋯" menu
    const row = unappliedRow(page, "docs")
    await row.hover()
    await row.getByRole("button", { name: "More actions for docs" }).click()
    await runAndWaitForSync(page, () => page.getByRole("menuitem", { name: "Reapply" }).click())
    await expect(findBranchRow(page, "docs")).toHaveCount(1)
  })

  test("should keep an unapplied branch whose reapply conflicts and say why", async ({ page }) => {
    await unapplyFromMenu(page, "docs")
    await routeCommandError(page, "reapply_branch", "Document the API (1a2b3c4d) of user-name/unapplied/docs conflicts with HEAD in docs.md: nothing was changed")
    await unappliedRow(page, "docs").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Reapply" }).click()
    await expect(toast(page, "Cannot reapply docs")).toContainText("conflicts with HEAD in docs.md")
    await expect(unappliedRow(page, "docs")).toBeVisible()
  })
})
