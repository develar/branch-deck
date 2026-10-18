// @spec spec/repository/activity.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test, testWithIssueLinks } from "../fixtures/test-fixtures"
import {
  collapseBranch,
  expandBranch,
  expectNothingToPush,
  findBranchRow,
  getBranchCommitRow,
  getBranchDetailsRow,
  getPushButton,
  verifyBranchState,
  verifyCommitCount,
} from "../helpers/branch-helpers"
import { HEADER_HEIGHT, MIN_WINDOW } from "../helpers/header-helpers"
import { confirmForcePush, routePush } from "../helpers/push-helpers"
import { multiSelectCommit, selectCommit } from "../helpers/selection-helpers"

// `issue_links`: 7 branches, none pushed, ABC-123 has 4 commits. `behind_upstream` / `update_conflict`: master is behind
// origin/master, the update rebases cleanly / conflicts in src/app.txt. `landed_upstream`: a robot pushed feature-auth
// and the first of the two search commits to origin/master.

function activityButton(page: Page): Locator {
  return page.getByTestId("activity-button")
}

async function openActivity(page: Page): Promise<Locator> {
  await activityButton(page).click()
  const popover = page.getByTestId("activity-popover")
  await expect(popover).toBeVisible()
  return popover
}

/** The entry with this title; a branch name in it is a button, so the title is matched as the whole text */
function activityEntry(popover: Locator, title: string): Locator {
  const escaped = title.replace(/[.*+?^$()[\]{}|\\]/g, "\\$&")
  const exactTitle = new RegExp(`^${escaped}$`)
  return popover.getByTestId("activity-entry").filter({ has: popover.page().getByTestId("activity-title").filter({ hasText: exactTitle }) })
}

testWithIssueLinks.describe("Activity", () => {
  testWithIssueLinks("should record the pushes with the remote commits a force push dropped, and show the branch", async ({ page }) => {
    // nothing happened yet: no button
    await expect(activityButton(page)).toHaveCount(0)
    await routePush(page, {
      "ABC-123": [
        // someone else pushed a commit the branch lacks
        {
          unpushedCommits: ["a1"],
          commitsBehind: 1,
          remoteTip: "remote-head-1",
          droppedCommits: [{ id: "c0ffee0123456789", subject: "Fix login", author: "Ann", authorTime: Date.parse("2024-01-02T07:00:00Z") / 1000 }],
        },
        {},
      ],
    })
    const row = findBranchRow(page, "ABC-123")
    await getPushButton(row).click()
    // it drops Ann's commit: the click asks first, the Force Push in the description pushes
    await row.getByRole("button", { name: "Force Push (1)" }).click()
    await confirmForcePush(page, "Force Push (1)")
    await expectNothingToPush(row)

    const popover = await openActivity(page)
    await expect(popover.getByTestId("activity-day")).toHaveCount(1)
    await expect(popover.getByTestId("activity-day").getByRole("heading")).toHaveText("Today")
    // newest first
    await expect(popover.getByTestId("activity-title")).toHaveText(["Force pushed ABC-123", "Pushed ABC-123"])
    await expect(activityEntry(popover, "Pushed ABC-123").getByTestId("activity-description")).toHaveText("4 commits · created the remote branch")
    const forced = activityEntry(popover, "Force pushed ABC-123")
    await expect(forced).toHaveAttribute("data-kind", "push")
    await expect(forced.getByTestId("activity-description")).toHaveText("1 commit · dropped 1 remote commit")
    const dropped = forced.getByTestId("dropped-commits")
    await expect(dropped).toContainText("Fix login")
    await expect(dropped).toContainText("Ann · c0ffee01 · 3 hours ago")

    // the branch name shows its row
    await page.keyboard.press("Escape")
    await collapseBranch(page, row)
    await openActivity(page)
    await forced.getByTestId("activity-branch").filter({ hasText: "ABC-123" }).click()
    await expect(popover).toBeHidden()
    await verifyBranchState(row, "open")
  })

  testWithIssueLinks("should offer the Undo of a squash while the toast's lasts", async ({ page }) => {
    const row = findBranchRow(page, "ABC-123")
    await expandBranch(page, row)
    await selectCommit(getBranchCommitRow(page, "ABC-123", "Add session management"))
    await multiSelectCommit(getBranchCommitRow(page, "ABC-123", "Add authentication tests"))
    await getBranchDetailsRow(page, "ABC-123").getByTestId("selection-actions").getByRole("button", { name: "Squash" }).click()
    const dialog = page.getByTestId("squash-dialog")
    await expect(dialog).toBeVisible()
    // the shortcut is the editor's: pressed before the dialog moved the focus there, it does nothing
    await expect(dialog.getByTestId("squash-message-editor")).toBeFocused()
    await page.keyboard.press("ControlOrMeta+Enter")
    await expect(dialog).toBeHidden()
    await verifyCommitCount(row, "3 commits")

    const popover = await openActivity(page)
    const entry = activityEntry(popover, "Squashed 2 commits")
    await expect(entry).toHaveAttribute("data-kind", "rewrite")
    // the Undo of Activity is the toast's: it undoes once and closes the toast
    await entry.getByTestId("activity-undo").click()
    await expect(entry.getByTestId("activity-undone")).toHaveText("undone")
    await expect(entry.getByTestId("activity-undo")).toHaveCount(0)
    await verifyCommitCount(row, "4 commits")
    await expect(page.getByRole("button", { name: "Undo" })).toHaveCount(0)
  })

  testWithIssueLinks("should clear the log of the repository", async ({ page }) => {
    await routePush(page, {})
    await getPushButton(findBranchRow(page, "feature-auth")).click()
    const popover = await openActivity(page)
    await expect(activityEntry(popover, "Pushed feature-auth")).toBeVisible()
    await popover.getByTestId("activity-clear").click()
    await expect(popover).toBeHidden()
    await expect(activityButton(page)).toHaveCount(0)
  })
})

test.describe("Activity of updates", () => {
  test("should record an applied update, the button on the header line at the minimum window width", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize(MIN_WINDOW)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await page.getByTestId("update-project-button").click()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()

    const header = await page.getByTestId("configuration-header").boundingBox()
    const button = await activityButton(page).boundingBox()
    const landing = await page.getByTestId("landing-indicator").boundingBox()
    expect(header!.height).toBeCloseTo(HEADER_HEIGHT, 0)
    expect(button!.y).toBeGreaterThanOrEqual(header!.y)
    expect(button!.y + button!.height).toBeLessThanOrEqual(header!.y + header!.height)
    // after the landing indicator, apart from it
    expect(button!.x).toBeGreaterThanOrEqual(landing!.x + landing!.width)

    const popover = await openActivity(page)
    await expect(activityEntry(popover, "Updated master: rebased 1 commit onto origin/master")).toHaveAttribute("data-kind", "update")
  })

  test("should record a refused update", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("update_conflict")
    await syncAndWaitForBranches()
    await page.getByTestId("update-project-button").click()
    const dialog = page.getByTestId("update-blocked-dialog")
    await expect(dialog).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()

    const popover = await openActivity(page)
    const entry = activityEntry(popover, "Didn't update master")
    await expect(entry.getByTestId("activity-description")).toHaveText("Conflicts in 1 file: src/app.txt. Nothing was changed.")
  })

  test("should keep the landed report after a sync hid the island", async ({ page, setupRepo, syncAndWaitForBranches, testStore }) => {
    await setupRepo("landed_upstream")
    await syncAndWaitForBranches()
    await page.getByTestId("update-project-button").click()
    const card = page.getByTestId("landed-card")
    await expect(card).toBeVisible()

    // the user saw the island for less than 10 s: a sync keeps it
    await syncAndWaitForBranches()
    await expect(card).toBeVisible()
    // seen long enough: the next sync hides it, for good (remembered for the repository); the chips stay until Dismiss
    await page.clock.fastForward("00:11")
    await syncAndWaitForBranches()
    await expect(card).toHaveCount(0)
    await expect.poll(async () => ((await testStore.get("recentProjects")) as { landedHiddenUntil?: number }[])[0]?.landedHiddenUntil ?? 0).toBeGreaterThan(0)
    const chip = findBranchRow(page, "search").getByTestId("landed-chip")
    await expect(chip).toHaveText("1 landed · 1 local")

    // the entry of the update keeps the report, with the rows of the island
    const popover = await openActivity(page)
    const entry = activityEntry(popover, "2 branches landed in origin/master")
    await expect(entry.getByTestId("activity-description")).toHaveText("feature-auth and search · rebased 2 commits")
    await entry.getByTestId("activity-landed-toggle").click()
    const search = entry.getByTestId("activity-landed-report").locator('[data-testid="landed-branch"][data-branch-name="search"]')
    await expect(search.getByTestId("landed-count")).toHaveText("1 landed · 1 stays local")
    await search.getByRole("button", { expanded: false }).click()
    await expect(search.getByTestId("landed-commits")).toContainText("Add index")

    // Dismiss from the entry clears the chips; the entry stays
    await entry.getByTestId("activity-landed-dismiss").click()
    await expect(chip).toHaveCount(0)
    await expect(entry.getByTestId("activity-landed-dismiss")).toHaveCount(0)
    await expect(entry).toBeVisible()
  })
})
