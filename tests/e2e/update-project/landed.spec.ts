// @spec spec/repository/landed-commits.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import { testCopyButtonOnHover } from "../helpers/clipboard-helpers"

// The `landed_upstream` template: a robot pushed `feature-auth` (one commit) and the first of the two `search` commits to
// origin/master, rebased without the prefix, committer `Patronus`; `ui` stays local. Already fetched.

function updateButton(page: Page): Locator {
  return page.getByTestId("update-project-button")
}

function landedCard(page: Page): Locator {
  return page.getByTestId("landed-card")
}

function landedBranch(page: Page, name: string): Locator {
  return landedCard(page).locator(`[data-testid="landed-branch"][data-branch-name="${name}"]`)
}

test.describe("Landed commits", () => {
  test("should name what the fetched baseline already has before the update", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("landed_upstream")
    await syncAndWaitForBranches()

    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-state", "clean")
    await expect(button.getByTestId("update-state-icon")).toHaveAttribute("data-landing", "true")
    await button.hover()
    await expect(page.getByTestId("update-project-tooltip")).toContainText(
      "Rebases cleanly onto origin/master · brings in your feature-auth and 1 of 2 commits of search",
    )
    await page.mouse.move(0, 0)

    await expect(findBranchRow(page, "feature-auth").getByTestId("landed-chip")).toHaveText("Landed")
    await expect(findBranchRow(page, "search").getByTestId("landed-chip")).toHaveText("1 of 2 landed")
    await expect(findBranchRow(page, "ui").getByTestId("landed-chip")).toHaveCount(0)

    const search = findBranchRow(page, "search")
    await expandBranch(page, search)
    await expect(getBranchCommitRow(page, "search", "Add index").getByTestId("landed-commit-marker")).toHaveText("in origin/master")
    await expect(getBranchCommitRow(page, "search", "Add query").getByTestId("landed-commit-marker")).toHaveCount(0)
    await expect(landedCard(page)).toHaveCount(0)
  })

  test("should lead the toast with what landed and keep the island until it is dismissed", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("landed_upstream")
    await syncAndWaitForBranches()

    await updateButton(page).click()
    await expect(page.getByText("2 branches landed in origin/master")).toBeVisible()
    await expect(page.getByText("feature-auth and search · rebased 2 commits")).toBeVisible()

    const card = landedCard(page)
    await expect(card).toContainText("Landed in origin/master")
    await expect(landedBranch(page, "feature-auth").getByTestId("landed-count")).toHaveText("1 commit")
    await expect(landedBranch(page, "search").getByTestId("landed-count")).toHaveText("1 of 2 · 1 stay local")
    await landedBranch(page, "feature-auth").getByRole("button", { expanded: false }).click()
    const commits = landedBranch(page, "feature-auth").getByTestId("landed-commits")
    await expect(commits).toContainText("Add login")
    await expect(commits).toContainText("Patronus")

    // the whole branch left the active table for the archive, marked new; the part that stays shows what landed
    await expect(findBranchRow(page, "feature-auth")).toHaveCount(0)
    await expect(findBranchRow(page, "search").getByTestId("landed-chip")).toHaveText("1 landed")
    await page.getByTestId("archived-branches-card").getByRole("button").first().click()
    await expect(page.getByTestId("archived-branch-new")).toHaveCount(1)

    // the report is the backend's: a sync keeps it, Dismiss clears it for good
    await syncAndWaitForBranches()
    await expect(landedBranch(page, "search")).toBeVisible()
    await card.getByTestId("landed-dismiss").click()
    await expect(card).toHaveCount(0)
    await expect(page.getByTestId("archived-branch-new")).toHaveCount(0)
    await syncAndWaitForBranches()
    await expect(landedCard(page)).toHaveCount(0)
  })

  test("should show the copy button of a baseline commit on hover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("landed_upstream")
    await syncAndWaitForBranches()
    await updateButton(page).click()

    const branch = landedBranch(page, "feature-auth")
    await branch.getByRole("button", { expanded: false }).click()
    const commit = branch.getByTestId("landed-commits").locator("li", { hasText: "Add login" })
    const hash = commit.getByTestId("landed-upstream-hash")
    await expect(hash).toHaveText(/^[0-9a-f]{8}$/)
    await testCopyButtonOnHover(page, commit, new RegExp(`^${await hash.textContent()}[0-9a-f]{32}$`), "Copy the baseline commit")
  })
})
