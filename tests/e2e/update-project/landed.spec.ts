// @spec spec/repository/landed-commits.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getPushButton } from "../helpers/branch-helpers"
import { testCopyButtonOnHover } from "../helpers/clipboard-helpers"

// The `landed_upstream` template: a robot pushed `feature-auth` (one commit) and the first of the two `search` commits to
// origin/master, rebased without the prefix, committer `Patronus`; `ui` stays local. Already fetched. The `search` commit
// names JIRA-42, an issue link by the `.idea/vcs.xml` of the template.

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
    await expect(button).toHaveAttribute("data-status", "clean")
    await expect(button.getByTestId("update-state-icon")).toHaveAttribute("data-landing", "true")
    await button.hover()
    await expect(page.getByTestId("update-project-tooltip")).toContainText(
      "Rebases cleanly onto origin/master · brings in your feature-auth and 1 commit of search",
    )
    await page.mouse.move(0, 0)

    await expect(findBranchRow(page, "feature-auth").getByTestId("landed-chip")).toHaveText("Landed")
    await expect(findBranchRow(page, "search").getByTestId("landed-chip")).toHaveText("1 landed · 1 local")
    await expect(findBranchRow(page, "ui").getByTestId("landed-chip")).toHaveCount(0)

    // the update archives feature-auth: it is landed, its push is quiet and the indicator leaves it out
    const featureAuth = findBranchRow(page, "feature-auth")
    await expect(featureAuth.getByTestId("branch-status")).toHaveText("landed")
    await expect(findBranchRow(page, "search").getByTestId("branch-status")).toHaveText("not pushed")
    const push = getPushButton(featureAuth)
    await expect(push).toHaveAccessibleName("Create Remote (1)")
    await expect(push).toHaveClass(/\btext-muted-foreground\b/)
    await push.hover()
    // the row's button shows the count: its description names the push
    const description = page.getByTestId("push-description")
    // with the key of the focused row
    await expect(description.getByTestId("push-label")).toHaveText("Create Remote (1)P")
    await expect(description.getByTestId("push-label").locator("kbd")).toHaveText("P")
    await expect(description).toHaveText("Create Remote (1)PAlready in origin/master. Update to archive the branch.")
    await page.mouse.move(0, 0)
    await expect(page.getByTestId("outgoing-indicator")).toHaveAccessibleName("3 commits to push in 2 branches")

    // the details have the anatomy of the pull request and Patronus details: the state line, what the update does,
    // then `dl` rows with the commits (their issue links) and what stays
    await findBranchRow(page, "search").getByTestId("landed-chip").hover()
    const details = page.getByTestId("landed-popover").getByTestId("landed-details")
    await expect(details.getByTestId("landed-details-state")).toHaveText("1 landed · 1 local")
    await expect(details).toContainText("Already in origin/master · the update removes the local copies")
    await expect(details.getByTestId("landed-details-commits")).toContainText("Add index")
    await expect(details.getByTestId("landed-details-commits")).toContainText("Patronus")
    await expect(details.getByRole("link", { name: "JIRA-42" })).toHaveAttribute("href", "https://jira.example.com/browse/JIRA-42")
    await expect(details.getByTestId("landed-details-remaining")).toHaveText("1 commit")
    await page.mouse.move(0, 0)

    const search = findBranchRow(page, "search")
    await expandBranch(page, search)
    await expect(getBranchCommitRow(page, "search", "Add index").getByTestId("landed-commit-marker")).toHaveText("in origin/master")
    await expect(getBranchCommitRow(page, "search", "Add query").getByTestId("landed-commit-marker")).toHaveCount(0)
    await expect(landedCard(page)).toHaveCount(0)

    // the footer line of the outgoing overview has the icon of the button
    await page.getByTestId("outgoing-indicator").hover()
    const line = page.getByTestId("outgoing-popover").getByTestId("update-state-line")
    await expect(line.getByTestId("update-state-icon")).toHaveAttribute("data-landing", "true")
  })

  test("should lead the toast with what landed and keep the island until it is dismissed", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("landed_upstream")
    await syncAndWaitForBranches()

    await updateButton(page).click()
    await expect(page.getByText("2 branches landed in origin/master")).toBeVisible()
    await expect(page.getByText("feature-auth and search · rebased 2 commits")).toBeVisible()

    // the report is the backend's: a sync keeps it, as the user saw the island for less than 10 s (a sync after that
    // hides it, see tests/e2e/activity/activity.spec.ts)
    const card = landedCard(page)
    await expect(card).toBeVisible()
    await syncAndWaitForBranches()
    await expect(card).toContainText("Landed in origin/master")
    // the robot pushed both branches: the header names it once, the rows don't repeat it
    await expect(card.getByTestId("landed-header")).toContainText("by Patronus")
    await expect(landedBranch(page, "search")).not.toContainText("by Patronus")
    await expect(landedBranch(page, "feature-auth").getByTestId("landed-count")).toHaveText("1 landed")
    await expect(landedBranch(page, "search").getByTestId("landed-count")).toHaveText("1 landed · 1 stays local")
    await landedBranch(page, "feature-auth").getByRole("button", { expanded: false }).click()
    const commits = landedBranch(page, "feature-auth").getByTestId("landed-commits")
    await expect(commits).toContainText("Add login")
    await expect(commits).toContainText("Patronus")
    // a subject has its issue links, as in the commit rows
    await landedBranch(page, "search").getByRole("button", { expanded: false }).click()
    await expect(landedBranch(page, "search").getByTestId("landed-commits").getByRole("link", { name: "JIRA-42" }))
      .toHaveAttribute("href", "https://jira.example.com/browse/JIRA-42")

    // the whole branch left the active table for the archive, marked new; the part that stays shows what landed
    await expect(findBranchRow(page, "feature-auth")).toHaveCount(0)
    await expect(findBranchRow(page, "search").getByTestId("landed-chip")).toHaveText("1 landed · 1 local")
    await page.getByTestId("archived-branches-card").getByRole("button").first().click()
    await expect(page.getByTestId("archived-branch-new")).toHaveCount(1)

    // Dismiss clears it for good
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
