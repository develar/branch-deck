// @spec spec/branch/parked-branches.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getMoreButton } from "../helpers/branch-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"

// the `branch_filter` template: the stack core → api → web with cli forked off core, and seven branches on their own;
// neither Patronus nor GitHub, so every branch the remote lacks is the user's move, its push
const BRANCH_COUNT = 11

function parkedCard(page: Page): Locator {
  return page.getByTestId("parked-branches-card")
}

function toast(page: Page, title: string): Locator {
  return page.locator("[data-slot=toast]").filter({ hasText: title })
}

/** The branch rows of the Parked card, in its order */
async function parkedRows(page: Page): Promise<string[]> {
  return parkedCard(page)
    .getByTestId("branch-row")
    .evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name") ?? ""))
}

/** Parks `name` with `note` through the form of Park Branch…, opened by `open`; resolves once the sync showed it */
async function parkWithForm(page: Page, open: () => Promise<void>, note: string): Promise<void> {
  await open()
  await page.getByRole("menuitem", { name: "Park Branch…" }).click()
  const input = page.getByTestId("inline-park-input").getByRole("textbox")
  await expect(input).toBeFocused()
  if (note !== "") {
    await page.keyboard.type(note)
  }
  await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
}

test.describe("Parked branches", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("branch_filter")
    await syncAndWaitForBranches()
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT)
  })

  test("should park a branch from its menu with a note, leave it out of every move, and take the park back with Undo", async ({ page }) => {
    const yourMove = page.getByRole("group", { name: "Branch state" }).locator("[data-chip=yourMove]")
    const moves = page.getByTestId("landing-indicator").getByTestId("landing-moves")
    await expect(yourMove).toHaveText(`Your Move${BRANCH_COUNT}`)
    const movesBefore = Number(await moves.textContent())
    await expect(parkedCard(page)).toHaveCount(0)

    await parkWithForm(page, () => findBranchRow(page, "docs").click({ button: "right" }), "waits for the fix of IJPL-1")

    // the row moves into the Parked card, says why, and is no move of the user
    expect(await parkedRows(page)).toEqual(["docs"])
    await expect(parkedCard(page)).toContainText("1 branch")
    const row = findBranchRow(page, "docs")
    await expect(row.getByTestId("parked-status")).toHaveText("Parked")
    await expect(row.getByTestId("branch-summary")).toHaveText("waits for the fix of IJPL-1")
    await expect(row.getByTestId("lane-node")).toHaveAttribute("data-kind", "parked")
    await expect(yourMove).toHaveText(`Your Move${BRANCH_COUNT - 1}`)
    await expect(moves).toHaveText(String(movesBefore - 1))

    // no push of it: the menu says why
    await row.click({ button: "right" })
    const push = page.getByRole("menuitem", { name: /^Push/ }).first()
    await expect(push).toHaveAttribute("aria-disabled", "true")
    await push.hover()
    await expect(page.getByRole("tooltip")).toHaveText("Parked: Unpark it to push")
    await page.keyboard.press("Escape")

    // F2 steps through the moves and never stops at it
    await page.mouse.click(5, 300)
    const focused: string[] = []
    for (let i = 0; i < BRANCH_COUNT - 1; i++) {
      await page.keyboard.press("F2")
      await expect(page.locator("tr[data-testid=branch-row]:focus")).toHaveCount(1)
      focused.push((await page.locator("tr[data-testid=branch-row]:focus").getAttribute("data-branch-name")) ?? "")
    }
    expect(new Set(focused).size).toBe(BRANCH_COUNT - 1)
    expect(focused).not.toContain("docs")

    // Undo of the toast unparks it
    await runAndWaitForSync(page, () => toast(page, "Parked docs").getByRole("button", { name: "Undo" }).click())
    await expect(parkedCard(page)).toHaveCount(0)
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT)
    await expect(yourMove).toHaveText(`Your Move${BRANCH_COUNT}`)
    await expect(findBranchRow(page, "docs").getByTestId("lane-node")).not.toHaveAttribute("data-kind", "parked")
  })

  test("should park a stack root with the branches built on it, edit its note and unpark it from the menus", async ({ page }) => {
    await parkWithForm(page, () => getMoreButton(findBranchRow(page, "core")).click(), "")

    // the branches built on core carry its commits: they are parked with it, as a stack of their own
    expect((await parkedRows(page)).toSorted()).toEqual(["api", "cli", "core", "web"])
    await expect(parkedCard(page)).toContainText("4 branches")
    await expect(findBranchRow(page, "core").getByTestId("parked-status")).toHaveText("Parked")
    await expect(findBranchRow(page, "web").getByTestId("parked-status")).toHaveText("Parked with core")
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT)

    // a branch parked with its parent unparks the parent
    await findBranchRow(page, "web").click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: /^Unpark core/ })).toBeVisible()
    await page.keyboard.press("Escape")

    // the note of the parked root
    await findBranchRow(page, "core").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Edit Park Note…" }).click()
    const input = page.getByTestId("inline-park-input").getByRole("textbox")
    await expect(input).toBeFocused()
    await page.keyboard.type("later")
    await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
    await expect(findBranchRow(page, "core").getByTestId("branch-summary")).toHaveText("later")
    // a branch parked with it keeps its own summary; the hover of its status tells the note
    await expect(findBranchRow(page, "api").getByTestId("parked-status")).toHaveText("Parked with core")
    await findBranchRow(page, "api").getByTestId("parked-status").hover()
    await expect(page.getByRole("tooltip")).toHaveText("Parked with core, kept local: later")

    // Unpark Branch puts the stack back into the table
    await findBranchRow(page, "core").click({ button: "right" })
    await runAndWaitForSync(page, () => page.getByRole("menuitem", { name: /^Unpark Branch/ }).click())
    await expect(parkedCard(page)).toHaveCount(0)
    await expect(toast(page, "Unparked core")).toBeVisible()
    await expect(findBranchRow(page, "web").getByTestId("parked-status")).toHaveCount(0)
  })

  test("should collapse the Parked card and keep its rows out of the table", async ({ page }) => {
    await parkWithForm(page, () => findBranchRow(page, "lint").click({ button: "right" }), "")
    await expect(findBranchRow(page, "lint")).toBeVisible()
    await parkedCard(page)
      .getByRole("button", { name: /Parked/ })
      .first()
      .click()
    await expect(parkedCard(page)).toHaveAttribute("data-state", "closed")
    await expect(findBranchRow(page, "lint")).toHaveCount(0)
    await expect(page.getByTestId("branch-row")).toHaveCount(BRANCH_COUNT - 1)
  })
})
