// @spec spec/branch/pull-requests.spec.md
import type { Locator, Page } from "@playwright/test"
import { test as baseTest, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getPushButton, verifyBranchState } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import {
  availablePullRequests,
  createdPullRequest,
  createPullRequestDialog,
  getOpenedUrls,
  pullRequest,
  reviewer,
  routeNewPullRequest,
  unavailablePullRequests,
} from "../helpers/pull-request-helpers"
import { expectPushedToast, pushedToast, routePush } from "../helpers/push-helpers"
import {
  EARLIER_COMMIT,
  hoverStatusChip,
  openBranchPreview,
  openOutgoingOverview,
  openOverviewWithAttention,
  pushBranchFromRow,
  switchBranchPreview,
} from "../helpers/review-status-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { routeCommand, routeCommandError } from "../helpers/wait-helpers"

// the `issue_links` template: 7 branches, none pushed
const APPROVED = pullRequest("JIRA-456", 760, {
  title: "Fix the database connection pool",
  reviewDecision: "approved",
  checks: { state: "passing", passed: 4, failed: 0, pending: 0, failedNames: [] },
  reviewers: [reviewer("alice", "approved"), reviewer("ij-platform", "requested")],
})
const CHANGES_REQUESTED = pullRequest("ABC-123", 761, {
  reviewDecision: "changesRequested",
  checks: { state: "failing", passed: 9, failed: 1, pending: 2, failedNames: ["tests"] },
  reviewers: [reviewer("bob", "changesRequested")],
})
const DRAFT = pullRequest("feature-docs", 762, { isDraft: true })
const MERGED = pullRequest("feature-db", 763, { state: "merged", reviewDecision: "approved" })
const CLOSED = pullRequest("feature-ui", 764, { state: "closed" })
// shows a commit the local branch no longer is at, and the remote doesn't have (the branch is not pushed)
const OUTDATED = pullRequest("feature-api", 765, { headRefOid: EARLIER_COMMIT })
const ALL = availablePullRequests(APPROVED, CHANGES_REQUESTED, DRAFT, MERGED, CLOSED, OUTDATED)

const test = baseTest.extend<{ repo: TestRepositoryBuilder }>({
  repo: [
    async ({ page, setupRepo, syncAndWaitForBranches }, use) => {
      const repo = await setupRepo("issue_links", { pullRequests: ALL })
      await syncAndWaitForBranches()
      // the unassigned commits card can render after the branches and move the rows away from the pointer
      await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
      await expect(page.getByTestId("pull-request-chip")).toHaveCount(6)
      await use(repo)
    },
    { auto: true },
  ],
})

/** Pushes feature-auth and opens the "Create pull request" dialog from the push toast */
async function openDialogAfterPush(page: Page): Promise<Locator> {
  await getPushButton(findBranchRow(page, "feature-auth")).click()
  await pushedToast(page, "feature-auth").getByRole("button", { name: "Create Pull Request…" }).click()
  return createPullRequestDialog(page)
}

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

    await popover.getByRole("button", { name: "Open Pull Request" }).click()
    expect(await getOpenedUrls(page)).toEqual([APPROVED.url])
    // the popover would cover the next rows
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()

    // the row does not toggle
    await getChip(page, "ABC-123").click()
    expect(await getOpenedUrls(page)).toEqual([APPROVED.url, CHANGES_REQUESTED.url])
    await verifyBranchState(findBranchRow(page, "ABC-123"), "closed")
  })

  test("should say when the branch changed since the pull request's head", async ({ page }) => {
    const popover = await hoverChip(page, "feature-api")
    await expect(popover.getByTestId("pull-request-outdated")).toHaveText("Shows 01234567; the branch has changed since: push to update the pull request.")
  })

  test("should say only quietly that the pull request shows an earlier build of the same commits", async ({ page }) => {
    // the remote branch is at the head and has the commits of the branch, which the sync rebuilt on a newer baseline
    await pushBranchFromRow(page, "feature-api", { remoteTip: EARLIER_COMMIT })
    await expect(getChip(page, "feature-api").getByTestId("pull-request-outdated-dot")).toHaveCount(0)
    const popover = await hoverChip(page, "feature-api")
    await expect(popover.getByTestId("pull-request-rebased")).toHaveText("Shows an earlier build of the same commits.")
    await expect(popover.getByTestId("pull-request-outdated")).toHaveCount(0)
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
    await expect(page.getByRole("menuitem", { name: "Copy Pull Request Link" })).toHaveCount(0)
  })

  test("should open the pull request with O on the focused row, and copy its link from the branch menu and the popover", async ({ page }) => {
    // the key the menu item shows works on the focused row
    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Open Pull Request" }).locator("[data-slot=kbd]")).toHaveText("O")
    await page.getByRole("menuitem", { name: "Copy Pull Request Link" }).click()
    expect(await readClipboard(page)).toBe(APPROVED.url)

    await findBranchRow(page, "JIRA-456").focus()
    await page.keyboard.press("o")
    expect(await getOpenedUrls(page)).toEqual([APPROVED.url])
    // a branch without one opens nothing
    await findBranchRow(page, "feature-auth").focus()
    await page.keyboard.press("o")
    expect(await getOpenedUrls(page)).toEqual([APPROVED.url])

    const popover = await hoverChip(page, "ABC-123")
    await popover.getByRole("button", { name: "Copy Pull Request Link" }).click()
    await expect(popover.getByRole("button", { name: "Copied" })).toBeVisible()
    expect(await readClipboard(page)).toBe(CHANGES_REQUESTED.url)
  })

  test("should offer to create the pull request of a pushed branch in the push toast, the branch menu and the preview", async ({ page }) => {
    await routePush(page, {})
    const texts = await routeNewPullRequest(page)
    const dialog = createPullRequestDialog(page)
    const row = findBranchRow(page, "feature-auth")

    // not on the remote yet: GitHub has nothing to open a pull request of
    await row.click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Copy Branch Name" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Create Pull Request…" })).toHaveCount(0)
    await page.keyboard.press("Escape")

    await getPushButton(row).click()
    const toast = pushedToast(page, "feature-auth")
    await toast.getByRole("button", { name: "Create Pull Request…" }).click()
    await expect(dialog).toBeVisible()
    // the baseline `origin/master` is `master` on the remote
    await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("feature-auth → master")
    await expect(dialog.getByTestId("create-pull-request-title")).toHaveValue("Add auth")
    await expect(dialog.getByTestId("create-pull-request-body")).toHaveValue("- **Add login**")
    expect(texts).toEqual([{ repositoryPath: expect.any(String), head: "user-name/virtual/feature-auth", base: "master" }])
    await page.keyboard.press("Escape")
    await expect(dialog).toHaveCount(0)

    // the preview offers it where "Open Pull Request" would be, and closes for the dialog; Space would press the
    // focused push button
    await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur())
    const preview = await openBranchPreview(page, "feature-auth")
    await preview.getByRole("button", { name: "Create Pull Request…" }).click()
    await expect(preview).toHaveCount(0)
    await expect(dialog).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(dialog).toHaveCount(0)

    await row.click({ button: "right" })
    await page.getByRole("menuitem", { name: "Create Pull Request…" }).click()
    await expect(dialog).toBeVisible()
    // nothing went to GitHub's form
    expect(await getOpenedUrls(page)).toEqual([])
  })

  test("should create the pull request with the reviewers it searched and the user as its assignee", async ({ page }) => {
    await routePush(page, {})
    await routeNewPullRequest(page)
    const searches = await routeCommand(page, "search_github_users", params =>
      (params.query as string).startsWith("bo")
        ? [
            { login: "user-name", name: "Me" },
            { login: "bob", name: "Bob Builder" },
          ]
        : [],
    )
    const created = pullRequest("feature-auth", 900, { title: "Add auth & login", reviewers: [reviewer("bob", "requested")] })
    const creates = await routeCommand(page, "create_pull_request", () => createdPullRequest(created))

    const dialog = await openDialogAfterPush(page)
    const title = dialog.getByTestId("create-pull-request-title")
    await expect(title).toHaveValue("Add auth")
    await title.fill("Add auth & login")

    const search = dialog.getByTestId("create-pull-request-reviewer-search")
    await search.fill("bo")
    // the user is never asked to review the own pull request
    const option = page.getByRole("option", { name: /bob/ })
    await expect(option).toHaveText("bobBob Builder")
    await expect(page.getByRole("option")).toHaveCount(1)
    await option.click()
    await expect(dialog.getByTestId("create-pull-request-reviewers").locator("[data-slot=combobox-chip]")).toHaveText(["bob"])
    expect(searches.at(-1)).toEqual({ repositoryPath: expect.any(String), query: "bo" })
    await expect(dialog.getByTestId("create-pull-request-assign")).toBeChecked()

    await title.press("ControlOrMeta+Enter")
    await expect(dialog).toHaveCount(0)
    expect(creates).toEqual([
      {
        repositoryPath: expect.any(String),
        branchPrefix: "user-name",
        branchName: "feature-auth",
        head: "user-name/virtual/feature-auth",
        base: "master",
        title: "Add auth & login",
        body: "- **Add login**",
        reviewers: ["bob"],
        assignSelf: true,
      },
    ])
    const toast = page.locator("[data-slot=toast]", { hasText: "Created pull request #900" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("feature-auth → master · review requested from bob · assigned to you")
    // the row has it at once, before GitHub's search lists it
    await expect(getChip(page, "feature-auth")).toBeVisible()
    await toast.getByRole("button", { name: "Open Pull Request" }).click()
    expect(await getOpenedUrls(page)).toEqual([created.url])
  })

  test("should say whom GitHub refused, and keep the dialog when GitHub creates nothing", async ({ page }) => {
    await routePush(page, {})
    await routeNewPullRequest(page)
    const dialog = await openDialogAfterPush(page)
    const title = dialog.getByTestId("create-pull-request-title")
    await title.fill("Add auth & login")

    // a branch GitHub doesn't have yet: nothing was created, the text stays for another try
    await routeCommandError(page, "create_pull_request", "GitHub doesn't have user-name/virtual/feature-auth yet: a mirror copies a pushed branch a moment later; try again")
    await dialog.getByTestId("create-pull-request-submit").click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Cannot create pull request" })).toBeVisible()
    await expect(dialog).toBeVisible()
    await expect(title).toHaveValue("Add auth & login")

    await page.unroute("**/invoke/create_pull_request")
    const created = pullRequest("feature-auth", 901)
    await routeCommand(page, "create_pull_request", () =>
      createdPullRequest(created, { assignees: [], warnings: ["cannot request review from stranger: Reviews may only be requested from collaborators"] }),
    )
    await dialog.getByTestId("create-pull-request-submit").click()
    await expect(dialog).toHaveCount(0)
    const toast = page.locator("[data-slot=toast]", { hasText: "Created pull request #901" })
    await expect(toast.locator("[data-slot=toast-icon] .lucide-triangle-alert")).toBeVisible()
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText(
      "feature-auth → master · cannot request review from stranger: Reviews may only be requested from collaborators",
    )
  })

  test("should say why it can't create, and offer only Open Pull Request once the branch has one", async ({ page, repo }) => {
    await routePush(page, {})
    await routeNewPullRequest(page)
    const dialog = await openDialogAfterPush(page)
    const title = dialog.getByTestId("create-pull-request-title")
    await expect(title).toHaveValue("Add auth")

    // a disabled Create says why on hover
    await title.fill("")
    const submit = dialog.getByTestId("create-pull-request-submit")
    await expect(submit).toBeDisabled()
    await submit.hover()
    await expect(page.getByRole("tooltip")).toHaveText("Enter a title")
    await title.fill("Add auth")
    await expect(submit).toBeEnabled()

    // a refresh on focus finds the pull request someone made meanwhile
    await repo.setPullRequests(availablePullRequests(APPROVED, CHANGES_REQUESTED, DRAFT, MERGED, CLOSED, OUTDATED, pullRequest("feature-auth", 905)))
    await page.clock.fastForward("02:01")
    await emitTauriEvent(page, "tauri://focus")
    await expect(dialog.getByTestId("create-pull-request-exists")).toContainText("feature-auth already has pull request #905.")
    await expect(submit).toBeDisabled()
    // GitHub's form would open nothing: disabled with the reason, the alert opens the pull request
    const onGitHub = dialog.getByTestId("create-pull-request-on-github")
    await expect(onGitHub).toBeDisabled()
    await onGitHub.hover()
    await expect(page.getByRole("tooltip")).toHaveText("feature-auth already has pull request #905")
    await dialog.getByTestId("create-pull-request-exists").getByRole("button", { name: "Open Pull Request" }).click()
    expect(await getOpenedUrls(page)).toEqual([pullRequest("feature-auth", 905).url])
  })

  test("should open the pull request of a pushed branch from the push toast", async ({ page }) => {
    await routePush(page, {})
    await getPushButton(findBranchRow(page, "JIRA-456")).click()
    const toast = pushedToast(page, "JIRA-456")
    await toast.getByRole("button", { name: "Open Pull Request" }).click()
    expect(await getOpenedUrls(page)).toEqual([APPROVED.url])

    // it has one: nothing to create
    await findBranchRow(page, "JIRA-456").click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Open Pull Request" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Create Pull Request…" })).toHaveCount(0)
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
    // the overview says what the dot means
    const popover = await openOverviewWithAttention(page, "12 commits to push in 7 branches", "ABC-123 needs attention: changes requested, failing checks")
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
  baseTest("should hint quietly when GitHub rejects the token", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", { pullRequests: unavailablePullRequests({ status: "notAuthenticated", source: "gh" }) })
    await syncAndWaitForBranches()
    const popover = await openOutgoingOverview(page)
    await expect(popover.getByTestId("pull-request-status-line")).toContainText("GitHub rejected the token from gh")
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
    await expectPushedToast(page, "test-branch")

    const indicator = page.getByTestId("outgoing-indicator")
    await expect(indicator).toHaveAccessibleName("Nothing to push, 1 pull request open, 1 needs attention")
    await expect(indicator.getByTestId("outgoing-review-count")).toHaveText("1")
    await expect(indicator.getByTestId("outgoing-attention-dot")).toBeVisible()
    const popover = await openOutgoingOverview(page)
    await expect(popover.getByRole("heading")).toHaveText("Nothing to push, 1 pull request open")
    await expect(popover.locator("[data-testid=outgoing-review-branch][data-branch-name=test-branch]").getByTestId("pull-request-chip")).toHaveText("#800")
  })
})
