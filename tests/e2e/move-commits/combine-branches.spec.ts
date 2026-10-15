// @spec spec/branch/move-commits.spec.md
import { test, testWithUnassigned, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getUnassignedCommitRow, verifyCommitCount } from "../helpers/branch-helpers"
import { inlineBranchCreator, submitInlineForm } from "../helpers/inline-form-helpers"
import { dropOn, selectCommit, startDrag } from "../helpers/selection-helpers"

test.describe("Combine branches", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_combine")
    await syncAndWaitForBranches()
    await verifyCommitCount(findBranchRow(page, "IJPL-1"), "2 commits")
  })

  test("should drag an issue branch onto a branch: the issue joins it without a rewrite, and Undo moves it back", async ({ page }) => {
    await startDrag(page, findBranchRow(page, "IJPL-2"))
    const docs = findBranchRow(page, "docs")
    await expect(docs).toHaveAttribute("data-drop-target", "ready")
    // the dragged branch is no target of itself
    await expect(findBranchRow(page, "IJPL-2")).not.toHaveAttribute("data-drop-target")
    await dropOn(page, docs)

    await verifyCommitCount(docs, "2 commits")
    await expect(findBranchRow(page, "IJPL-2")).toHaveCount(0)
    const toast = page.locator("[data-slot=toast-description]").filter({ hasText: "IJPL-2 goes to docs by the issue key from now on, without a rewrite" })
    await expect(toast).toBeVisible()
    // the subject stays as it is: no `(docs)` prefix
    await expandBranch(page, docs)
    await expect(getBranchCommitRow(page, "docs", "IJPL-2: Print maps")).not.toContainText("(docs)")

    await page.getByRole("button", { name: "Undo" }).click()
    await verifyCommitCount(findBranchRow(page, "IJPL-2"), "1 commit")
    await verifyCommitCount(docs, "1 commit")
  })

  test("should move the selected branches to a new branch with Move to…", async ({ page }) => {
    for (const name of ["IJPL-1", "IJPL-2", "IJPL-3"]) {
      await findBranchRow(page, name).click({ modifiers: ["ControlOrMeta"] })
      await expect(findBranchRow(page, name)).toHaveAttribute("data-selected", "true")
    }
    const actions = page.getByTestId("branch-selection-actions")
    await expect(actions).toContainText("3 branches selected")
    // a plain click on another row expands it and clears the selection; Escape does too
    await page.keyboard.press("Escape")
    await expect(actions).toHaveCount(0)
    await findBranchRow(page, "IJPL-1").click({ modifiers: ["ControlOrMeta"] })
    await findBranchRow(page, "IJPL-3").click({ modifiers: ["Shift"] })
    await expect(actions).toContainText("3 branches selected")

    await actions.getByRole("button", { name: "Move to…" }).click()
    await inlineBranchCreator.waitForVisible(page)
    const input = inlineBranchCreator.getInput(page)
    await input.fill("maps")
    await expect(inlineBranchCreator.getPortal(page)).toContainText("Creates the branch maps with 4 commits: IJPL-1, IJPL-2 and IJPL-3 go to maps by the issue key from now on, without a rewrite")
    await submitInlineForm(input, "maps")

    await verifyCommitCount(findBranchRow(page, "maps"), "4 commits")
    for (const name of ["IJPL-1", "IJPL-2", "IJPL-3"]) {
      await expect(findBranchRow(page, name)).toHaveCount(0)
    }
    await expect(actions).toHaveCount(0)
  })

  test("should drop a commit on the expanded commits of a branch, and drag an unselected row instead of the selection", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, "IJPL-1"))
    await expandBranch(page, findBranchRow(page, "docs"))
    await selectCommit(page, getBranchCommitRow(page, "IJPL-1", "Parse lists"))

    const maps = getBranchCommitRow(page, "IJPL-1", "Parse maps")
    await startDrag(page, maps)
    // what moves is what is selected
    await expect(maps).toHaveAttribute("data-selected", "true")
    await expect(getBranchCommitRow(page, "IJPL-1", "Parse lists")).toHaveAttribute("data-selected", "false")
    await dropOn(page, getBranchDetailsRow(page, "docs"))

    // a part of an issue gets the prefix
    await verifyCommitCount(findBranchRow(page, "docs"), "2 commits")
    await verifyCommitCount(findBranchRow(page, "IJPL-1"), "1 commit")
  })
})

testWithUnassigned.describe("Move commits without branches", () => {
  testWithUnassigned("should offer the new branch zone when there is no branch yet", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo()
    await expect(page.getByTestId("branch-row")).toHaveCount(0)

    await startDrag(page, getUnassignedCommitRow(page, "Add user authentication"))
    const zone = page.getByTestId("new-branch-drop-zone")
    await expect(zone).toBeVisible()
    await dropOn(page, zone)

    await inlineBranchCreator.waitForVisible(page)
    await submitInlineForm(inlineBranchCreator.getInput(page), "auth")
    await verifyCommitCount(findBranchRow(page, "auth"), "1 commit")
  })
})
