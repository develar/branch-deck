// @spec openspec/specs/branch/conflict-fix/spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { branchTree, collapseBranch, expandBranch, findBranchRow, getBranchDetailsRow, verifyCommitCount } from "../helpers/branch-helpers"
import { diffRows, diffSides } from "../helpers/diff-helpers"
import { availableRobots, mergeRunningRobot, robot, robotUrl, willFail } from "../helpers/patronus-helpers"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import { runAndWaitForAnswer, runAndWaitForSync, runAndWaitForUpdate } from "../helpers/sync-helpers"
import { waitForConflictFixSearch } from "../helpers/wait-helpers"

/** The fix of the conflict of ui, its row open, once the search found it */
async function foundFix(page: Page): Promise<Locator> {
  const uiRow = findBranchRow(page, "ui")
  if ((await uiRow.getAttribute("data-state")) !== "open") {
    await expandBranch(page, uiRow)
  }
  const fix = getBranchDetailsRow(page, "ui").getByTestId("conflict-fix")
  await waitForConflictFixSearch(fix)
  await expect(fix).toHaveAttribute("data-state", "found")
  return fix
}

test.describe("Conflict fix", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("conflict_fix")
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "ui").getByTestId("branch-status")).toHaveText("merge conflict")
  })

  test("should offer the stack a trial build verified, apply it and undo it", async ({ page }) => {
    // the search runs when the conflict is shown
    const fix = await foundFix(page)
    // the commit of ui needs api, the other commit of ui doesn't: ui lands after api, both commits in it
    await expect(fix).toContainText("Stack ui on api")
    await expect(fix).not.toContainText("Move")
    const before = await branchTree(page)
    expect(before).toContain("ui@0")
    await expect(fix.getByTestId("fix-result")).toHaveText("Afterwards ui applies cleanly.")
    // the conflict itself: the merge of settings.js in three columns, named by the sides. Both sides changed the same
    // word of the timeout, so the proposed result keeps the base there
    const file = getBranchDetailsRow(page, "ui").getByTestId("conflicting-files").getByTestId("file-diff")
    await expect(file.getByTestId("file-conflict-count")).toHaveText("1 conflict")
    await expect(file.getByTestId("merge-columns")).toContainText(/^(origin\/)?master/)
    await expect(file.getByTestId("merge-columns")).toContainText("“Show the timeout in seconds”")
    const conflict = diffRows(file, "merge")
    await expect(diffSides(conflict, 0)).toHaveText(/timeout = 10$/)
    await expect(diffSides(conflict, 1)).toHaveText(/timeout = 30$/)
    await expect(diffSides(conflict, 2)).toHaveText(/timeout = 30 \/\/ seconds$/)
    await expect(file.getByTestId("merge-notice")).toHaveCount(0)

    await runAndWaitForSync(page, () => fix.getByTestId("apply-conflict-fix").click())
    await expect(page.getByText("Fixed the conflict of ui")).toBeVisible()
    await expect(findBranchRow(page, "ui").getByTestId("branch-status")).not.toHaveText("merge conflict")
    await expect.poll(() => branchTree(page)).toEqual(["core@0", "api@1", "ui@2"])
    await verifyCommitCount(findBranchRow(page, "api"), 1)
    await verifyCommitCount(findBranchRow(page, "ui"), 2)

    await runAndWaitForSync(page, () => page.getByRole("button", { name: "Undo" }).click())
    await expect.poll(() => branchTree(page)).toEqual(before)
    await expect(findBranchRow(page, "ui").getByTestId("branch-status")).toHaveText("merge conflict")
  })

  test("should open the fix from the menu of the branch", async ({ page }) => {
    const uiRow = findBranchRow(page, "ui")
    if ((await uiRow.getAttribute("data-state")) === "open") {
      await collapseBranch(page, uiRow)
    }
    await openContextMenu(page, uiRow)
    await clickContextMenuItem(page, "Fix Conflict…")
    await expect(uiRow).toHaveAttribute("data-state", "open")
    const fix = getBranchDetailsRow(page, "ui").getByTestId("conflict-fix")
    await waitForConflictFixSearch(fix)
    await expect(fix).toHaveAttribute("data-state", "found")
  })
})

test.describe("Conflict fix while Patronus answers", () => {
  test("should apply the fix only once Patronus told what its robots hold", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("conflict_fix", { patronus: availableRobots() })
    await repo.holdRemoteAnswers({ hold: ["robots"] })
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "ui").getByTestId("branch-status")).toHaveText("merge conflict")
    const fix = await foundFix(page)
    // a robot may land what the fix brings, which waiting fixes: no offer on "none", the reason beside the button
    const apply = fix.getByTestId("apply-conflict-fix")
    await expect(apply).toBeDisabled()
    await expect(fix).toContainText("Waits for the answer of Patronus")

    await repo.holdRemoteAnswers({ release: ["robots"] })
    await expect(apply).toBeEnabled()
    await expect(fix).toContainText("Undo is in the toast and in Activity")
    await runAndWaitForSync(page, () => apply.click())
    await expect(page.getByText("Fixed the conflict of ui")).toBeVisible()
  })
})

// the fix of ui stacks it on api, which has what it needs: a Safe Push of api lands that without the fix
const API_ROBOT = robot("api", "api-robot")

test.describe("Conflict fix while a Safe Push holds what it brings", () => {
  test("should say to wait for the Safe Push that lands what the fix brings, and still apply the fix", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("conflict_fix", { patronus: availableRobots(API_ROBOT) })
    await syncAndWaitForBranches()
    const fix = await foundFix(page)
    await expect(fix).toContainText("Wait for the Safe Push of api")
    await expect(fix.getByTestId("fix-hold")).toHaveText("api has what ui needs: once its Safe Push merges, Update Project applies ui cleanly, without a fix.")
    await expect(fix).toContainText("Or fix it now: Stack ui on api")

    // the robot and its row, where its cancel is
    await fix.getByRole("button", { name: "Open in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual([robotUrl(API_ROBOT.id)])
    await expect(fix.getByRole("button", { name: "Cancel Safe Push" })).toHaveCount(0)
    const apiRow = findBranchRow(page, "api")
    await expect(apiRow).toHaveAttribute("data-state", "closed")
    await fix.getByRole("button", { name: "Show api" }).click()
    await expect(apiRow).toHaveAttribute("data-state", "open")

    // the fix is local: the robot lands the copy it took, so the hold disables nothing
    await runAndWaitForSync(page, () => fix.getByTestId("apply-conflict-fix").click())
    await expect(page.getByText("Fixed the conflict of ui")).toBeVisible()
    await expect.poll(() => branchTree(page)).toEqual(["core@0", "api@1", "ui@2"])
    await verifyCommitCount(apiRow, 1)
  })

  test("should make the fix the way while the Safe Push that holds what it brings is failing, its restart and cancel beside it", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("conflict_fix", { patronus: availableRobots(API_ROBOT), patronusDetails: willFail(API_ROBOT.id) })
    await syncAndWaitForBranches()
    const fix = await foundFix(page)
    await expect(fix.getByTestId("fix-hold")).toHaveText("The Safe Push of api is failing, so waiting for it lands nothing: fix the conflict now, or restart the Safe Push.")
    await expect(fix).toContainText("Stack ui on api")
    await expect(fix).not.toContainText("Wait for the Safe Push")

    await runAndWaitForAnswer(page, "execute", () => fix.getByRole("button", { name: "Cancel Safe Push" }).click())
    expect((await repo.getPatronusLog()).cancels).toEqual([API_ROBOT.id])
  })

  test("should restart the failing Safe Push that holds what the fix brings", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("conflict_fix", { patronus: availableRobots(API_ROBOT), patronusDetails: willFail(API_ROBOT.id) })
    await syncAndWaitForBranches()
    const fix = await foundFix(page)
    await runAndWaitForAnswer(page, "execute", () => fix.getByRole("button", { name: /Restart/ }).click())
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })).toBeVisible()
    expect((await repo.getPatronusLog()).restarts).toEqual([API_ROBOT.id])
  })

  test("should offer Update Project in place of the fix once a Safe Push landed what it brings", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("conflict_fix", { patronus: availableRobots(API_ROBOT) })
    await syncAndWaitForBranches()
    await expect(await foundFix(page)).toContainText("Wait for the Safe Push of api")
    await mergeRunningRobot(page, repo, API_ROBOT)
    const fix = getBranchDetailsRow(page, "ui").getByTestId("conflict-fix")
    await expect(fix).toContainText("api is merged by Patronus")
    await expect(fix.getByTestId("fix-hold")).toHaveText("Update Project applies ui cleanly, without a fix.")
    await expect(fix.getByTestId("apply-conflict-fix")).toHaveCount(0)
    await runAndWaitForUpdate(page, () => fix.getByTestId("fix-update-project").click())
  })
})
