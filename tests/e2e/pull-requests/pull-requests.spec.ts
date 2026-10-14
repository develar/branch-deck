// @spec spec/branch/pull-requests.spec.md
import type { Locator, Page } from "@playwright/test"
import { test as baseTest, expect } from "../fixtures/test-fixtures"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { findBranchRow, getPushButton, verifyBranchState } from "../helpers/branch-helpers"
import { availablePullRequests, getOpenedUrls, pullRequest, unavailablePullRequests } from "../helpers/pull-request-helpers"
import { routePush } from "../helpers/push-helpers"
import { hoverStatusChip, openBranchPreview, openOutgoingOverview, switchBranchPreview } from "../helpers/review-status-helpers"

// the `issue_links` template: 7 branches, none pushed
const APPROVED = pullRequest("JIRA-456", 760, {
  title: "Fix the database connection pool",
  reviewDecision: "approved",
  checks: { state: "passing", passed: 4, failed: 0, pending: 0, failedNames: [] },
  reviewers: [{ login: "alice", state: "approved" }, { login: "ij-platform", state: "requested" }],
})
const CHANGES_REQUESTED = pullRequest("ABC-123", 761, {
  reviewDecision: "changesRequested",
  checks: { state: "failing", passed: 9, failed: 1, pending: 2, failedNames: ["tests"] },
  reviewers: [{ login: "bob", state: "changesRequested" }],
})
const DRAFT = pullRequest("feature-docs", 762, { isDraft: true })
const MERGED = pullRequest("feature-db", 763, { state: "merged", reviewDecision: "approved" })
const CLOSED = pullRequest("feature-ui", 764, { state: "closed" })
// shows a commit the local branch no longer has
const OUTDATED = pullRequest("feature-api", 765, { headRefOid: "0123456789abcdef0123456789abcdef01234567" })
const ALL = availablePullRequests(APPROVED, CHANGES_REQUESTED, DRAFT, MERGED, CLOSED, OUTDATED)

const test = baseTest.extend<{ repo: TestRepositoryBuilder }>({
  repo: [async ({ page, setupRepo, syncAndWaitForBranches }, use) => {
    const repo = await setupRepo("issue_links", { pullRequests: ALL })
    await syncAndWaitForBranches()
    // the unassigned commits card can render after the branches and move the rows away from the pointer
    await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
    await expect(page.getByTestId("pull-request-chip")).toHaveCount(6)
    await use(repo)
  }, { auto: true }],
})

function getChip(page: Page, branchName: string): Locator {
  return findBranchRow(page, branchName).getByTestId("pull-request-chip")
}

function hoverChip(page: Page, branchName: string): Promise<Locator> {
  return hoverStatusChip(page, getChip(page, branchName), "pull-request-popover")
}

test.describe("Pull request chip", () => {
  test("should show each pull request state in the status cell", async ({ page }) => {
    const approved = getChip(page, "JIRA-456")
    await expect(approved).toHaveText("#760")
    await expect(approved).toHaveAccessibleName("Pull request #760: Approved")
    await expect(approved).toHaveAttribute("data-color", "success")

    const changesRequested = getChip(page, "ABC-123")
    await expect(changesRequested).toHaveAttribute("data-color", "error")
    await expect(changesRequested.getByTestId("pull-request-checks-dot")).toBeVisible()

    await expect(getChip(page, "feature-docs")).toHaveAttribute("data-kind", "draft")
    await expect(getChip(page, "feature-db")).toHaveAttribute("data-kind", "merged")
    await expect(getChip(page, "feature-db")).toHaveAttribute("data-color", "info")
    await expect(getChip(page, "feature-ui")).toHaveAttribute("data-kind", "closed")

    const outdated = getChip(page, "feature-api")
    await expect(outdated).toHaveAccessibleName("Pull request #765: Review required")
    await expect(outdated.getByTestId("pull-request-outdated-dot")).toBeVisible()
    // up to date: no warning dot
    await expect(approved.getByTestId("pull-request-outdated-dot")).toHaveCount(0)

    await expect(getChip(page, "feature-auth")).toHaveCount(0)
  })

  test("should show the details on hover and open the pull request on click", async ({ page }) => {
    const popover = await hoverChip(page, "JIRA-456")
    await expect(popover.getByTestId("pull-request-title")).toHaveText("Fix the database connection pool")
    await expect(popover.getByTestId("pull-request-state")).toHaveText("Approved")
    await expect(popover.getByTestId("pull-request-checks")).toHaveText("All 4 passed")
    await expect(popover.getByTestId("pull-request-reviewers")).toHaveText("alice approvedij-platform review requested")

    await popover.getByRole("button", { name: "Open on GitHub" }).click()
    expect(await getOpenedUrls(page)).toEqual([APPROVED.url])
    // the popover would cover the next rows
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()

    // the row does not toggle
    await getChip(page, "ABC-123").click()
    expect(await getOpenedUrls(page)).toEqual([APPROVED.url, CHANGES_REQUESTED.url])
    await verifyBranchState(findBranchRow(page, "ABC-123"), "closed")
  })

  test("should say when the pull request does not show the local branch", async ({ page }) => {
    const popover = await hoverChip(page, "feature-api")
    await expect(popover.getByTestId("pull-request-outdated")).toHaveText("Not updated: the pull request shows 01234567. Push to update it.")
  })

  test("should refresh after a push", async ({ page, repo }) => {
    await routePush(page, {})
    // GitHub now shows the pushed commit
    await repo.setPullRequests(availablePullRequests(APPROVED, CHANGES_REQUESTED, DRAFT, MERGED, CLOSED, { ...OUTDATED, headRefOid: "" }))
    const calls = await repo.getPullRequestCalls()

    await getPushButton(findBranchRow(page, "feature-api")).click()

    await expect(getChip(page, "feature-api").getByTestId("pull-request-outdated-dot")).toHaveCount(0)
    expect(await repo.getPullRequestCalls()).toBe(calls + 1)
  })

  test("should open the pull request from the branch context menu", async ({ page }) => {
    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Open Pull Request" }).click()
    expect(await getOpenedUrls(page)).toEqual([APPROVED.url])

    await findBranchRow(page, "feature-auth").click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Copy Branch Name" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Open Pull Request" })).toHaveCount(0)
  })

  test("should show the pull request in the branch preview and open it with O", async ({ page }) => {
    const preview = await openBranchPreview(page, "ABC-123")

    const section = preview.getByTestId("branch-preview-pull-request")
    await expect(section.getByTestId("pull-request-state")).toHaveText("Changes requested")
    await expect(section.getByTestId("pull-request-checks")).toHaveText("1 failing (tests), 2 running, 9 passed")
    await expect(section.getByTestId("pull-request-reviewers")).toHaveText("bob requested changes")
    await expect(preview.getByTestId("pull-request-chip")).toHaveText("#761")

    await page.keyboard.press("o")
    expect(await getOpenedUrls(page)).toEqual([CHANGES_REQUESTED.url])

    // a branch without a pull request has no section
    await switchBranchPreview(page, preview, "feature-auth")
    await expect(preview.getByTestId("branch-preview-pull-request")).toHaveCount(0)
    await expect(preview.getByRole("button", { name: "Open Pull Request" })).toHaveCount(0)
  })
})

test.describe("Pull requests in the outgoing overview", () => {
  test("should show the chips in a column and a dot for a pull request that waits for the author", async ({ page }) => {
    await expect(page.getByTestId("outgoing-attention-dot")).toBeVisible()
    const popover = await openOutgoingOverview(page)
    const row = popover.locator("[data-testid=outgoing-branch][data-branch-name=ABC-123]")
    await expect(row.getByTestId("pull-request-chip")).toHaveText("#761")
    await expect(popover.locator("[data-testid=outgoing-branch][data-branch-name=feature-auth]").getByTestId("pull-request-chip")).toHaveCount(0)
    await expect(popover.getByTestId("pull-request-status-line")).toContainText("Pull requests checked just now")
  })

  test("should refresh from the overview", async ({ page, repo }) => {
    const calls = await repo.getPullRequestCalls()
    const popover = await openOutgoingOverview(page)
    await popover.getByTestId("pull-request-refresh").click()
    await expect.poll(() => repo.getPullRequestCalls()).toBe(calls + 1)
    await expect(popover.getByTestId("pull-request-refresh")).toBeEnabled()
  })
})

baseTest.describe("Pull requests without GitHub", () => {
  baseTest("should hint quietly when gh is not logged in", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", { pullRequests: unavailablePullRequests({ status: "notAuthenticated" }) })
    await syncAndWaitForBranches()
    const popover = await openOutgoingOverview(page)
    await expect(popover.getByTestId("pull-request-status-line")).toContainText("Run gh auth login to see pull requests")
    await expect(page.getByTestId("pull-request-chip")).toHaveCount(0)
    // never a toast
    await expect(page.locator("[data-slot=toast]")).toHaveCount(0)
  })

  baseTest("should say nothing for a repository that is not on GitHub", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const popover = await openOutgoingOverview(page)
    await expect(popover.getByTestId("pull-request-status-line")).toHaveCount(0)
  })

  baseTest("should keep the indicator for an open pull request when nothing is outgoing", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple", { pullRequests: availablePullRequests(pullRequest("test-branch", 800, { reviewDecision: "changesRequested" })) })
    await syncAndWaitForBranches()
    await routePush(page, {})
    await getPushButton(findBranchRow(page, "test-branch")).click()
    await expect(page.getByText("Branch test-branch pushed successfully")).toBeVisible()

    const indicator = page.getByTestId("outgoing-indicator")
    await expect(indicator).toHaveAccessibleName("Nothing to push, 1 pull request open")
    await expect(indicator.getByTestId("outgoing-review-count")).toHaveText("1")
    await expect(indicator.getByTestId("outgoing-attention-dot")).toBeVisible()
    const popover = await openOutgoingOverview(page)
    await expect(popover.getByRole("heading")).toHaveText("Nothing to push, 1 pull request open")
    await expect(popover.locator("[data-testid=outgoing-review-branch][data-branch-name=test-branch]").getByTestId("pull-request-chip")).toHaveText("#800")
  })
})
