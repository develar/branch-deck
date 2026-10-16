// @spec spec/repository/review-requests.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, verifyBranchState } from "../helpers/branch-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import { availablePullRequests, getOpenedUrls, pullRequest } from "../helpers/pull-request-helpers"
import { availableReviewRequests, getReviewRow, getYourTurnCard, openReviewDialog, reviewRequest, routeReviewFetch } from "../helpers/review-request-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { routeCommand, routeCommandError } from "../helpers/wait-helpers"

// the `issue_links` template: 7 branches; the requests review them as the branches of others
const OLD = reviewRequest("ABC-123", 101, 50, { title: "Rework the session store", checksState: "failing" })
const RECENT = reviewRequest("feature-auth", 102, 3, { title: "Add OAuth login", reRequested: true, reviewedCommit: null })
const RELEASE = reviewRequest("JIRA-456", 103, 2, { title: "Backport the pool fix", baseRefName: "release/253", baseIsDefault: false, checksState: "pending" })
const FOURTH = reviewRequest("feature-docs", 104, 1, { title: "Document the CLI" })
const FIFTH = reviewRequest("feature-db", 105, 0.5, { title: "Migrate the schema" })

type SetupRepo = (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>

async function setupRequests(page: Page, setupRepo: SetupRepo, syncAndWaitForBranches: () => Promise<void>, ...requests: ReturnType<typeof reviewRequest>[]): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("issue_links", { reviewRequests: availableReviewRequests(requests) })
  await syncAndWaitForBranches()
  await expect(getYourTurnCard(page)).toBeVisible()
  return repo
}

/**
 * The requests in a repository Patronus guards, its target master; the reviews and the comment are answered in the
 * page, GitHub showing `head` (by default the commit the user approved). Returns the recorded calls.
 */
async function setupSafePush(page: Page, setupRepo: SetupRepo, syncAndWaitForBranches: () => Promise<void>, requests: ReturnType<typeof reviewRequest>[], head?: string) {
  await setupRepo("issue_links", { reviewRequests: availableReviewRequests(requests), patronus: availableRobots() })
  await syncAndWaitForBranches()
  await expect(getYourTurnCard(page)).toBeVisible()
  await routeCommand(page, "get_patronus_target_branch", () => "master")
  const reviews = await routeCommand(page, "submit_pull_request_review", () => null)
  const heads = await routeCommand(page, "get_pull_request_head", () => head ?? reviews[0]?.commitId)
  const posts = await routeCommand(page, "post_patronus_command", () => ({ id: "4402154133", url: "https://github.com/acme/monorepo/pull/101#issuecomment-4402154133" }))
  const replies = await routeCommand(page, "read_patronus_command_reply", () => ({ status: "accepted" }))
  return { reviews, heads, posts, replies }
}

/** Runs the fake clock over the app's 3 s wait before it reads the bot's reply, until it read it */
async function skipReplyWait(page: Page, replies: unknown[]): Promise<void> {
  await expect.poll(async () => {
    await page.clock.fastForward(3000)
    return replies.length
  }).toBeGreaterThan(0)
}

/** The toast of a new request, by its title */
function reviewToast(page: Page, title: string) {
  return page.locator("[data-slot=toast]").filter({ has: page.locator("[data-slot=toast-title]", { hasText: title }) })
}

test.describe("Your Turn card", () => {
  test("should stay hidden off GitHub, and not ask again on focus", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    await expect(getYourTurnCard(page)).toHaveCount(0)
    await page.clock.fastForward("02:01")
    await emitTauriEvent(page, "tauri://focus")
    await expect(getYourTurnCard(page)).toHaveCount(0)
    expect(await repo.getReviewRequestCalls()).toBe(1)
  })

  test("should stay hidden while gh is logged out or nothing waits, and show once something does", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links", { reviewRequests: { availability: { status: "notAuthenticated" }, totalCount: 0, requests: [] } })
    await syncAndWaitForBranches()
    await expect(getYourTurnCard(page)).toHaveCount(0)

    // logged out: no poll, the next focus asks again
    await repo.setReviewRequests(availableReviewRequests([]))
    await page.clock.fastForward("02:01")
    await emitTauriEvent(page, "tauri://focus")
    await expect.poll(() => repo.getReviewRequestCalls()).toBe(2)
    await expect(getYourTurnCard(page)).toHaveCount(0)

    await repo.setReviewRequests(availableReviewRequests([OLD]))
    await page.clock.fastForward("02:01")
    await emitTauriEvent(page, "tauri://focus")
    await expect(getReviewRow(page, 101)).toBeVisible()
  })

  test("should show the oldest three first, the rest behind Show More, and say how long each waits", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, FIFTH, RECENT, OLD, FOURTH, RELEASE)
    const card = getYourTurnCard(page)
    await expect(card.getByTestId("your-turn-count")).toHaveText("5")
    const rows = card.getByTestId("review-request-row")
    await expect(rows.getByTestId("review-request-title")).toHaveText(["Rework the session store", "Add OAuth login", "Backport the pool fix"])

    const old = getReviewRow(page, 101)
    await expect(old.getByTestId("review-request-meta")).toHaveText("#101 · carol · 2 files · +12 −3")
    await expect(old.getByTestId("review-request-age")).toHaveText("waiting 2 days")
    // a working day passed: the warning hue
    await expect(old.getByTestId("review-request-age")).toHaveAttribute("data-old", "true")
    await expect(old.getByTestId("review-request-checks-failing")).toBeVisible()
    const recent = getReviewRow(page, 102)
    await expect(recent.getByTestId("review-request-age")).toHaveText("re-requested 3 hours ago")
    await expect(recent.getByTestId("review-request-age")).not.toHaveAttribute("data-old")
    await expect(getReviewRow(page, 103).getByTestId("review-request-meta")).toHaveText("#103 · carol · 2 files · +12 −3 · → release/253")
    await expect(getReviewRow(page, 103).getByTestId("review-request-checks-pending")).toBeVisible()

    await card.getByTestId("review-requests-show-more").click()
    await expect(rows).toHaveCount(5)
    await expect(card.getByTestId("review-requests-show-more")).toHaveText("Show Less")

    // collapsed: the count and the oldest age in the header, per repository
    await card.getByTestId("your-turn-header").click()
    await expect(card.getByTestId("your-turn-summary")).toHaveText("5 reviews · oldest waiting 2 days")
    await expect(rows).toHaveCount(0)
  })

  test("should say 50 of 63 when GitHub has more than it returns", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", { reviewRequests: availableReviewRequests([OLD, RECENT, RELEASE, FOURTH], 63) })
    await syncAndWaitForBranches()
    const card = getYourTurnCard(page)
    await expect(card.getByTestId("your-turn-count")).toHaveText("63")
    await card.getByTestId("review-requests-show-more").click()
    await expect(card.getByTestId("review-requests-more-on-github")).toHaveText("4 of 63Open All on GitHub")
    await card.getByRole("button", { name: "Open All on GitHub" }).click()
    expect(await getOpenedUrls(page)).toEqual(["https://github.com/acme/monorepo/pulls?q=is%3Apr%20is%3Aopen%20archived%3Afalse%20draft%3Afalse%20user-review-requested%3A%40me"])
  })

  test("should move with the arrows, review with Enter and open the menu from the keyboard", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT, RELEASE)
    await getReviewRow(page, 101).focus()
    await page.keyboard.press("ArrowDown")
    await expect(getReviewRow(page, 102)).toBeFocused()
    await page.keyboard.press("Shift+F10")
    const menu = page.getByRole("menu")
    // the actions of the footer of its review dialog: a write opens the dialog for it
    // no Patronus in the repository: no Approve and Safe Push…
    await expect(menu.getByRole("menuitem")).toHaveText(["Review↵", "Approve…", "Request Changes…", "Open Pull RequestO", "Copy Pull Request Link", "Copy Branch Name"])
    await page.keyboard.press("Escape")
    await expect(getReviewRow(page, 102)).toBeFocused()
    await page.keyboard.press("Enter")
    const dialog = page.getByTestId("review-dialog")
    await expect(dialog.getByRole("heading", { name: "Add OAuth login" })).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
    // the focus returns to the row
    await expect(getReviewRow(page, 102)).toBeFocused()
  })

  test("should name the own pull requests a reviewer handed back, and reveal the branch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", {
      reviewRequests: availableReviewRequests([OLD]),
      pullRequests: availablePullRequests(
        pullRequest("JIRA-456", 760, { reviewDecision: "changesRequested" }),
        // failing checks are told by the chip and the outgoing dot
        pullRequest("feature-api", 761, { checks: { state: "failing", passed: 1, failed: 1, pending: 0, failedNames: ["tests"] } }),
      ),
    })
    await syncAndWaitForBranches()
    const returned = getYourTurnCard(page).getByTestId("returned-to-you")
    await expect(returned).toHaveText("Returned to you:JIRA-456 (changes requested)")
    await returned.getByRole("button", { name: "JIRA-456 (changes requested)" }).click()
    await verifyBranchState(findBranchRow(page, "JIRA-456"), "open")
  })

  test("should keep the rows when a refresh fails, with the reason in the footer", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    await expect(getYourTurnCard(page).getByTestId("review-requests-status-line")).toHaveText("Checked just nowRefresh")
    await routeCommandError(page, "get_review_requests", "offline")
    await getYourTurnCard(page).getByTestId("review-requests-refresh").click()
    await expect(getYourTurnCard(page).getByTestId("review-requests-status-line")).toHaveText("GitHub: offline · checked just nowRefresh")
    await expect(getReviewRow(page, 101)).toBeVisible()
  })
})

test.describe("New review requests", () => {
  test("should tell a request that comes with a poll in a toast whose Review opens it, unread until opened", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links", { reviewRequests: availableReviewRequests([OLD, RECENT, RELEASE]) })
    await syncAndWaitForBranches()
    // the first answer is silent
    await expect(getReviewRow(page, 101)).toBeVisible()
    await expect(page.locator("[data-slot=toast]")).toHaveCount(0)

    await repo.setReviewRequests(availableReviewRequests([OLD, RECENT, RELEASE, FOURTH]))
    await page.clock.fastForward("05:00")
    const toast = reviewToast(page, "dave requested your review")
    await expect(toast).toBeVisible()
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("#104 Document the CLI")
    // the unread row is shown above the cap, in the place of a read one
    const row = getReviewRow(page, 104)
    await expect(row.getByTestId("review-request-unread")).toBeVisible()
    await expect(getYourTurnCard(page).getByTestId("review-request-row")).toHaveCount(3)

    await toast.getByRole("button", { name: "Review" }).click()
    await expect(page.getByTestId("review-dialog").getByRole("heading", { name: "Document the CLI" })).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(row.getByTestId("review-request-unread")).toHaveCount(0)

    // the Activity keeps it
    await page.getByRole("button", { name: "Activity" }).click()
    await expect(page.getByTestId("activity-title").first()).toHaveText("dave requested your review on #104")
  })
})

test.describe("Review dialog", () => {
  test("should preview the pull request, fetch it with progress, then review its range with the file tree", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    let release = () => {}
    const fetches = await routeReviewFetch(page, { progress: [42], release: new Promise<void>(resolve => (release = resolve)) })
    const dialog = await openReviewDialog(page, 101)
    await expect(dialog.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
    await expect(dialog.getByTestId("review-dialog-meta")).toHaveText("#101·carol·master ← user-name/virtual/ABC-123·2 files·+12 −3·checks failing")
    await expect(dialog.getByTestId("review-dialog-body")).toContainText("The change of ABC-123.")
    await expect(dialog.getByTestId("review-fetch-progress")).toHaveText("Fetching #101 from origin… 42%")
    await expect(dialog.getByTestId("review-dialog-review")).toBeDisabled()
    await expect(dialog.getByTestId("review-dialog-position")).toHaveText("1 / 2")
    expect(fetches).toHaveLength(1)

    release()
    await expect(dialog.getByTestId("review-commit").first()).toBeVisible()
    await expect(dialog.getByTestId("review-dialog-review")).toBeEnabled()
    await page.keyboard.press("d")
    const review = dialog.getByTestId("commit-review")
    await expect(review).toBeVisible()
    await expect(dialog.getByTestId("review-dialog-position")).toHaveText(/^File 1 \/ \d+$/)
    // the diffs have the focus, as in the review of a commit
    await expect(review.locator("[aria-label='Changes of the commit']")).toBeFocused()
    await page.keyboard.press("j")
    await expect(dialog.getByTestId("review-dialog-position")).toHaveText(/^File 2 \/ \d+$/)

    // Escape goes back to the preview, then closes
    await page.keyboard.press("Escape")
    await expect(review).toBeHidden()
    await expect(dialog.getByTestId("review-commits")).toBeVisible()
    await page.keyboard.press("o")
    expect(await getOpenedUrls(page)).toEqual(["https://github.com/acme/monorepo/pull/101"])
    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
  })

  test("should open a commit of the pull request in the commit preview, and show the next request with ↓", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    const dialog = await openReviewDialog(page, 101)
    const commit = dialog.getByTestId("review-commit").first()
    await expect(commit).toBeVisible()
    await commit.click()
    const preview = page.getByTestId("commit-preview")
    await expect(preview).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(preview).toBeHidden()
    await expect(dialog).toBeVisible()

    await dialog.getByTestId("review-dialog-body").focus()
    await page.keyboard.press("ArrowDown")
    await expect(dialog.getByRole("heading", { name: "Add OAuth login" })).toBeVisible()
    await expect(dialog.getByTestId("review-dialog-position")).toHaveText("2 / 2")
  })

  test("should say why a fetch failed and fetch again on Retry", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    await repo.setReviewRequests(availableReviewRequests([OLD]), "GitHub asked for credentials: run gh auth setup-git")
    const dialog = await openReviewDialog(page, 101)
    const failure = dialog.getByTestId("review-fetch-failed")
    await expect(failure).toContainText("Cannot fetch pull request #101")
    await expect(failure).toContainText("GitHub asked for credentials: run gh auth setup-git")

    await repo.setReviewRequests(availableReviewRequests([OLD]))
    await failure.getByTestId("review-fetch-retry").click()
    await expect(failure).toBeHidden()
    await expect(dialog.getByTestId("review-commit").first()).toBeVisible()
  })

  test("should approve the commit the review shows, in the review too, and keep its place once it left the list", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    const reviews = await routeCommand(page, "submit_pull_request_review", () => null)
    const dialog = await openReviewDialog(page, 101)
    const approve = dialog.getByTestId("review-approve")
    // nothing to approve before the commit the user sees is there
    await expect(dialog.getByTestId("review-commit").first()).toBeVisible()
    await expect(approve).toBeEnabled()
    await expect(dialog.getByTestId("review-request-changes")).toBeEnabled()

    // the actions of the pull request stay in the review of its changes
    await page.keyboard.press("d")
    await expect(dialog.getByTestId("commit-review")).toBeVisible()
    await expect(approve).toBeVisible()
    await repo.setReviewRequests(availableReviewRequests([RECENT]))
    await page.keyboard.press("a")
    await expect(reviewToast(page, "Approved #101")).toBeVisible()
    expect(reviews).toEqual([expect.objectContaining({ pullRequestNumber: 101, event: "approve", commitId: expect.stringMatching(/^[0-9a-f]{40}$/), body: null })])
    await expect(dialog.getByTestId("review-dialog-submitted")).toHaveText("approved by you")
    await expect(approve).toBeDisabled()

    // GitHub is asked at once: the approved request leaves the card, the dialog keeps it and its place
    await expect(getReviewRow(page, 101)).toHaveCount(0)
    await page.keyboard.press("Escape")
    await expect(dialog.getByTestId("review-commits")).toBeVisible()
    await expect(dialog.getByTestId("review-withdrawn")).toHaveCount(0)
    await page.keyboard.press("ArrowDown")
    await expect(dialog.getByRole("heading", { name: "Add OAuth login" })).toBeVisible()
  })

  test("should request changes with a comment whose letters are no keys of the dialog", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    const reviews = await routeCommand(page, "submit_pull_request_review", () => null)
    // the row's menu opens the dialog with the form, once the pull request is fetched
    await getReviewRow(page, 101).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Request Changes…" }).click()
    const dialog = page.getByTestId("review-dialog")
    const form = page.getByTestId("review-request-changes-form")
    await expect(form).toBeVisible()
    const send = form.getByTestId("review-request-changes-send")
    await expect(send).toBeDisabled()

    const comment = form.getByTestId("review-request-changes-comment")
    await expect(comment).toBeFocused()
    await comment.pressSequentially("a d o r: sort by the snapshot")
    await page.keyboard.press("ArrowDown")
    expect(reviews).toEqual([])
    expect(await getOpenedUrls(page)).toEqual([])
    await expect(dialog.getByRole("heading", { name: "Rework the session store" })).toBeVisible()

    // Escape closes the form and keeps the draft; the focus stays in the dialog, whose keys work again
    await page.keyboard.press("Escape")
    await expect(form).toBeHidden()
    await expect(dialog.getByTestId("review-request-changes")).toBeFocused()
    await page.keyboard.press("r")
    await expect(comment).toHaveValue("a d o r: sort by the snapshot")
    await page.keyboard.press("Control+Enter")
    await expect(form).toBeHidden()
    await expect(reviewToast(page, "Requested changes on #101")).toBeVisible()
    expect(reviews).toEqual([expect.objectContaining({ pullRequestNumber: 101, event: "requestChanges", body: "a d o r: sort by the snapshot" })])
    await expect(dialog.getByTestId("review-dialog-submitted")).toHaveText("changes requested by you")
  })

  test("should say why GitHub refused a review", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    await routeCommandError(page, "submit_pull_request_review", "User can only have one pending review per pull request")
    const dialog = await openReviewDialog(page, 101)
    await expect(dialog.getByTestId("review-commit").first()).toBeVisible()
    await dialog.getByTestId("review-approve").click()
    await expect(reviewToast(page, "Cannot approve pull request #101")).toContainText("User can only have one pending review per pull request")
    await expect(dialog.getByTestId("review-approve")).toBeEnabled()
  })

  test("should approve and Safe Push a pull request into the Patronus target with a plain /patronus merge, after a confirmation", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // GitHub shows the commit the user approved
    const { reviews, heads, posts, replies } = await setupSafePush(page, setupRepo, syncAndWaitForBranches, [OLD, RELEASE])
    // the robot is not among the user's: the thumbs up is the start
    await routeCommand(page, "get_patronus_robots", () => availableRobots())

    const dialog = await openReviewDialog(page, 101)
    await expect(dialog.getByTestId("review-commit").first()).toBeVisible()
    const safePush = dialog.getByTestId("review-safe-push")
    await expect(safePush).toHaveText("MApprove and Safe Push…")
    await page.keyboard.press("m")
    const form = page.getByTestId("review-safe-push-form")
    await expect(form.getByTestId("review-safe-push-description")).toHaveText(/^Approves [0-9a-f]{8}, then comments \/patronus merge on #101: Patronus rebases user-name\/virtual\/ABC-123 onto master, runs the checks, merges it and deletes the branch\. It reports in the pull request\.$/)
    // the keys of the dialog wait while the form is open
    await page.keyboard.press("a")
    expect(reviews).toEqual([])
    await expect(form.getByTestId("review-safe-push-start")).toBeFocused()
    await page.keyboard.press("Enter")
    await skipReplyWait(page, replies)
    await expect.poll(async () => {
      await page.clock.fastForward(3000)
      return page.locator("[data-slot=toast]", { hasText: "Safe Push started" }).count()
    }).toBe(1)
    expect(reviews).toEqual([expect.objectContaining({ pullRequestNumber: 101, event: "approve" })])
    expect(heads).toEqual([expect.objectContaining({ pullRequestNumber: 101, githubRepository: "acme/monorepo" })])
    expect(posts).toEqual([expect.objectContaining({ pullRequestNumber: 101, mode: "rebaseAutosquash", subject: "reviewedPullRequest", githubRepository: "acme/monorepo" })])
    await expect(form).toBeHidden()
    // approved: the Safe Push alone is left
    await expect(safePush).toHaveText("MSafe Push…")

    // a pull request into another base than the target: Patronus would merge it there
    await page.keyboard.press("ArrowDown")
    await expect(dialog.getByRole("heading", { name: "Backport the pool fix" })).toBeVisible()
    await expect(dialog.getByTestId("review-commit").first()).toBeVisible()
    await expect(dialog.getByTestId("review-safe-push")).toBeDisabled()
  })

  test("should watch the robot of a reviewed pull request: listed by what it tests, its merge told", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { replies } = await setupSafePush(page, setupRepo, syncAndWaitForBranches, [OLD])
    // the page's clock is at 2024-01-02 10:00Z; the robot runs on no branch of the user's
    const started = robot("ABC-123", "review-robot", { branchName: null, startTime: 1704189600, pullRequest: { number: 101, url: "https://github.com/acme/monorepo/pull/101" } })
    await routeCommand(page, "get_patronus_robots", () => availableRobots(started))
    const polls = await routeCommand(page, "poll_patronus_robots", () => availableRobots({ ...started, status: "success", finishTime: 1704189700 }))

    const dialog = await openReviewDialog(page, 101)
    await expect(dialog.getByTestId("review-commit").first()).toBeVisible()
    await page.keyboard.press("m")
    await page.getByTestId("review-safe-push-form").getByTestId("review-safe-push-start").click()
    await skipReplyWait(page, replies)
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()

    // the header lists it by the pull request, without a row to show
    await page.getByTestId("robots-indicator").hover()
    const row = page.getByTestId("robots-popover").getByTestId("robots-running").getByTestId("robots-row")
    await expect(row).toHaveAttribute("data-branch-name", "#101 Rework the session store")
    await expect(row.getByRole("button", { name: "Open Pull Request #101 of #101 Rework the session store" })).toBeVisible()

    // polled while it runs: its merge is told by what it tested
    await expect.poll(async () => {
      await page.clock.fastForward(60_000)
      return polls.length
    }).toBeGreaterThan(0)
    await expect(page.locator("[data-slot=toast]", { hasText: "#101 Rework the session store merged into master" })).toBeVisible()
  })

  test("should not Safe Push a pull request that got commits since the user reviewed it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { posts } = await setupSafePush(page, setupRepo, syncAndWaitForBranches, [OLD], "f".repeat(40))
    // from the row's menu: the dialog opens with the confirmation once the pull request is fetched
    await getReviewRow(page, 101).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Approve and Safe Push…" }).click()
    const form = page.getByTestId("review-safe-push-form")
    await form.getByTestId("review-safe-push-start").click()
    await expect(reviewToast(page, "Cannot start the Safe Push")).toContainText(/#101 has new commits since you reviewed [0-9a-f]{8}: review ffffffff first/)
    expect(posts).toEqual([])
  })

  test("should keep the dialog when the request is withdrawn meanwhile, and its refs", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links", { reviewRequests: availableReviewRequests([OLD, RECENT]) })
    await syncAndWaitForBranches()
    // the refs of an earlier start go once, after the first answer
    await expect.poll(async () => (await repo.getReviewRequestsFixture()).prunes).toEqual([[]])
    const dialog = await openReviewDialog(page, 101)
    await expect(dialog.getByTestId("review-commit").first()).toBeVisible()
    await repo.setReviewRequests(availableReviewRequests([RECENT]))
    await page.clock.fastForward("05:00")
    await expect(dialog.getByTestId("review-withdrawn")).toHaveText("No longer waiting for your review")
    await expect(dialog.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
    await expect(dialog.getByTestId("review-dialog-position")).toHaveCount(0)
    // the commits were local: no ref was written, nothing to prune
    const fixture = await repo.getReviewRequestsFixture()
    expect(fixture.fetchCalls).toBe(1)
    expect(fixture.prunes).toEqual([[]])
  })
})

test.describe("Settings", () => {
  test("should open from the card menu and keep the switches", async ({ page, setupRepo, syncAndWaitForBranches, testStore }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    await getYourTurnCard(page).getByRole("button", { name: "More actions for Your Turn" }).click()
    await page.getByRole("menuitem", { name: "Notification Settings…" }).click()
    const dialog = page.getByTestId("settings-dialog")
    await expect(dialog.getByRole("heading", { name: "Settings" })).toBeVisible()
    const reviews = dialog.getByTestId("setting-notifyReviewRequests").getByRole("switch")
    await expect(reviews).toBeChecked()
    await reviews.click()
    await expect(reviews).not.toBeChecked()
    // Windows has no Dock badge: no group
    await expect(dialog.getByTestId("setting-reviewBadge")).toHaveCount(0)
    await expect.poll(() => testStore.get("appSettings")).toMatchObject({ notifyReviewRequests: false })
    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
  })

  test("should open from the app menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    await emitTauriEvent(page, "open_settings")
    await expect(page.getByTestId("settings-dialog")).toBeVisible()
  })
})

// the E2E user agent is the one of Windows: no badge, Ctrl instead of ⌘
test.describe("On macOS", () => {
  test.use({ userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36" })

  test("should open a pull request on GitHub from its button, a ⌘-click and its menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    const row = getReviewRow(page, 101)
    await row.hover()
    await row.getByRole("button", { name: "Open pull request #101" }).click()
    await getReviewRow(page, 102).getByTestId("review-request-title").click({ modifiers: ["Meta"] })
    await row.click({ button: "right" })
    await page.getByRole("menuitem", { name: "Open Pull Request" }).click()
    // O on the focused row, the key of the menu item
    await getReviewRow(page, 102).focus()
    await page.keyboard.press("o")
    expect(await getOpenedUrls(page)).toEqual([
      "https://github.com/acme/monorepo/pull/101",
      "https://github.com/acme/monorepo/pull/102",
      "https://github.com/acme/monorepo/pull/101",
      "https://github.com/acme/monorepo/pull/102",
    ])
    await expect(page.getByTestId("review-dialog")).toHaveCount(0)
  })

  test("should count the review requests, behind its switch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    const badges = () => page.evaluate(() => ((window as unknown as { __e2eDockCalls?: { badgeCount?: number | null }[] }).__e2eDockCalls ?? []).filter(it => "badgeCount" in it).map(it => it.badgeCount))
    await expect.poll(badges).toEqual([2])

    await emitTauriEvent(page, "open_settings")
    const dialog = page.getByTestId("settings-dialog")
    await dialog.getByTestId("setting-reviewBadge").getByRole("switch").click()
    await expect.poll(badges).toEqual([2, null])
  })
})
