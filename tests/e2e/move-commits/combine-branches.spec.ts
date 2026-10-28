// @spec spec/branch/move-commits.spec.md
import { test, testWithUnassigned, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getUnassignedCommitRow, verifyCommitCount } from "../helpers/branch-helpers"
import { inlineBranchCreator, submitInlineForm } from "../helpers/inline-form-helpers"
import { dropOn, selectCommit, startDrag } from "../helpers/selection-helpers"
import { finishedSyncCount, waitForSyncComplete } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

test.describe("Combine branches", () => {
  let repo: TestRepositoryBuilder

  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    repo = await setupRepo("issue_combine")
    await syncAndWaitForBranches()
    await verifyCommitCount(findBranchRow(page, "IJPL-1"), 2)
  })

  test("should drag an issue branch onto a branch: the issue joins it without a rewrite, and Undo moves it back", async ({ page }) => {
    await startDrag(page, findBranchRow(page, "IJPL-2"))
    const docs = findBranchRow(page, "docs")
    await expect(docs).toHaveAttribute("data-drop-target", "ready")
    // the dragged branch is no target of itself
    await expect(findBranchRow(page, "IJPL-2")).not.toHaveAttribute("data-drop-target")
    await dropOn(page, docs)

    await verifyCommitCount(docs, 2)
    await expect(findBranchRow(page, "IJPL-2")).toHaveCount(0)
    const toast = page.locator("[data-slot=toast-description]").filter({ hasText: "IJPL-2 goes to docs by the issue key from now on, without a rewrite" })
    await expect(toast).toBeVisible()
    // the subject stays as it is: no `(docs)` prefix
    await expandBranch(page, docs)
    await expect(getBranchCommitRow(page, "docs", "IJPL-2: Print maps")).not.toContainText("(docs)")

    await page.getByRole("button", { name: "Undo" }).click()
    await verifyCommitCount(findBranchRow(page, "IJPL-2"), 1)
    await verifyCommitCount(docs, 1)
  })

  test("should take the drop of a branch while a sync runs: its move goes to the session at once", async ({ page }) => {
    // the sync of the session runs until the test releases it
    await repo.holdRemoteAnswers({ hold: ["sync"] })
    const previousCount = await finishedSyncCount(page)
    const syncButton = page.getByTestId("sync-button")
    await syncButton.click()
    await expect(syncButton).toHaveAttribute("aria-busy", "true")

    // the row drags during the sync; a target takes it, no reason to wait
    await startDrag(page, findBranchRow(page, "IJPL-2"))
    const docs = findBranchRow(page, "docs")
    await expect(docs).toHaveAttribute("data-drop-target", "ready")
    await dropOn(page, docs)
    await expect(page.getByTestId("drag-target-status")).toHaveCount(0)

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await waitForSyncComplete(page, previousCount)
    await verifyCommitCount(docs, 2)
    await expect(findBranchRow(page, "IJPL-2")).toHaveCount(0)
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
    await expect(inlineBranchCreator.getPortal(page)).toContainText(
      "Creates the branch maps with 4 commits: IJPL-1, IJPL-2 and IJPL-3 go to maps by the issue key from now on, without a rewrite",
    )
    await submitInlineForm(input, "maps")

    await verifyCommitCount(findBranchRow(page, "maps"), 4)
    for (const name of ["IJPL-1", "IJPL-2", "IJPL-3"]) {
      await expect(findBranchRow(page, name)).toHaveCount(0)
    }
    await expect(actions).toHaveCount(0)
  })

  test("should open Move to… for the focused row with M, without a selection", async ({ page }) => {
    const row = findBranchRow(page, "IJPL-2")
    await row.click({ button: "right" })
    // the key the menu item shows works on the focused row
    await expect(page.getByRole("menuitem", { name: "Move to…" }).locator("[data-slot=kbd]")).toHaveText("M")
    await page.keyboard.press("Escape")
    await expect(page.getByRole("menu")).toHaveCount(0)

    await row.focus()
    await page.keyboard.press("m")
    await inlineBranchCreator.waitForVisible(page)
    // as the menu item does: the row is selected, the form moves its commits
    await expect(row).toHaveAttribute("data-selected", "true")
    const input = inlineBranchCreator.getInput(page)
    await input.fill("maps")
    await expect(inlineBranchCreator.getPortal(page)).toContainText("IJPL-2 goes to maps by the issue key from now on, without a rewrite")
    await submitInlineForm(input, "maps")

    await verifyCommitCount(findBranchRow(page, "maps"), 1)
    await expect(findBranchRow(page, "IJPL-2")).toHaveCount(0)
  })

  test("should drop a commit on the expanded commits of a branch, and drag an unselected row alone, the selection as it was", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, "IJPL-1"))
    await expandBranch(page, findBranchRow(page, "docs"))
    const lists = getBranchCommitRow(page, "IJPL-1", "Parse lists")
    await selectCommit(lists)

    const maps = getBranchCommitRow(page, "IJPL-1", "Parse maps")
    const before = (await maps.boundingBox())!
    await startDrag(page, maps)
    // the row goes alone: the selection and its actions stay, so the row stays under the pointer
    await expect(page.getByTestId("drag-target-bar")).toBeVisible()
    await expect(maps).toHaveAttribute("data-selected", "false")
    await expect(lists).toHaveAttribute("data-selected", "true")
    expect((await maps.boundingBox())!.y).toBeCloseTo(before.y, 0)
    await dropOn(page, getBranchDetailsRow(page, "docs"))

    // a part of an issue gets the prefix
    await verifyCommitCount(findBranchRow(page, "docs"), 2)
    await verifyCommitCount(findBranchRow(page, "IJPL-1"), 1)
    await expect(getBranchCommitRow(page, "IJPL-1", "Parse lists")).toBeVisible()
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
    await verifyCommitCount(findBranchRow(page, "auth"), 1)
  })
})
