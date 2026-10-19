// @spec spec/repository/review-requests.spec.md
import type { Page } from "@playwright/test"
import type { ReviewWindowRequest } from "../../../src/lib/pull-request-review"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, verifyBranchState } from "../helpers/branch-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import { availablePullRequests, getOpenedUrls, pullRequest, reviewer } from "../helpers/pull-request-helpers"
import { availableReviewRequests, getReviewRow, getYourTurnCard, openedReviewWindow, openReviewWindow, reviewRequest, routeReviewFetch } from "../helpers/review-request-helpers"
import { expectSettingsOpened, openSettingsWindow } from "../helpers/settings-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { getEmittedEvents, getOpenedSubWindows, openLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { routeCommand, routeCommandError } from "../helpers/wait-helpers"

// the `issue_links` template: 7 branches; the requests review them as the branches of others
const OLD = reviewRequest("ABC-123", 101, 50, { title: "Rework the session store", checksState: "failing" })
const RECENT = reviewRequest("feature-auth", 102, 3, { title: "Add OAuth login", reRequested: true, reviewedCommit: null })
const RELEASE = reviewRequest("JIRA-456", 103, 2, { title: "Backport the pool fix", baseRefName: "release/253", baseIsDefault: false, checksState: "pending" })
const FOURTH = reviewRequest("feature-docs", 104, 1, { title: "Document the CLI" })
const FIFTH = reviewRequest("feature-db", 105, 0.5, { title: "Migrate the schema" })

type SetupRepo = (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>

async function setupRequests(
  page: Page,
  setupRepo: SetupRepo,
  syncAndWaitForBranches: () => Promise<void>,
  ...requests: ReturnType<typeof reviewRequest>[]
): Promise<TestRepositoryBuilder> {
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
  // the card reads it once it shows: the menus of its rows say which pull requests Patronus merges into it
  await routeCommand(page, "get_patronus_target_branch", () => "master")
  await setupRepo("issue_links", { reviewRequests: availableReviewRequests(requests), patronus: availableRobots() })
  await syncAndWaitForBranches()
  await expect(getYourTurnCard(page)).toBeVisible()
  const reviews = await routeCommand(page, "submit_pull_request_review", () => null)
  const heads = await routeCommand(page, "get_pull_request_head", () => head ?? reviews[0]?.commitId)
  const posts = await routeCommand(page, "post_patronus_command", () => ({ id: "4402154133", url: "https://github.com/acme/monorepo/pull/101#issuecomment-4402154133" }))
  const replies = await routeCommand(page, "read_patronus_command_reply", () => ({ status: "accepted" }))
  return { reviews, heads, posts, replies }
}

/** Runs the fake clock over the app's 3 s wait before it reads the bot's reply, until it read it */
async function skipReplyWait(page: Page, replies: unknown[]): Promise<void> {
  await expect
    .poll(async () => {
      await page.clock.fastForward(3000)
      return replies.length
    })
    .toBeGreaterThan(0)
}

/** The calls of the window API of a page: `close` is Escape closing the review window */
function windowCalls(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __e2eWindowCalls?: string[] }).__e2eWindowCalls ?? [])
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

  test("should stay hidden while GitHub rejects the token or nothing waits, and show once something does", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links", { reviewRequests: { availability: { status: "notAuthenticated", source: "gh" }, totalCount: 0, requests: [] } })
    await syncAndWaitForBranches()
    await expect(getYourTurnCard(page)).toHaveCount(0)

    // a rejected token: no poll, the next focus asks again
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
    // dave asked carol's pull request of the user: the one who waits is named
    await expect(old.getByTestId("review-request-meta")).toHaveText("#101 · carol · requested by dave · 2 files · +12 −3")
    await expect(old.getByTestId("review-request-age")).toHaveText("waiting 2 days")
    // a working day passed: the warning hue
    await expect(old.getByTestId("review-request-age")).toHaveAttribute("data-old", "true")
    await expect(old.getByTestId("review-request-checks-failing")).toBeVisible()
    const recent = getReviewRow(page, 102)
    await expect(recent.getByTestId("review-request-age")).toHaveText("re-requested 3 hours ago")
    await expect(recent.getByTestId("review-request-age")).not.toHaveAttribute("data-old")
    await expect(getReviewRow(page, 103).getByTestId("review-request-meta")).toHaveText("#103 · carol · requested by dave · 2 files · +12 −3 · → release/253")
    await expect(getReviewRow(page, 103).getByTestId("review-request-checks-pending")).toBeVisible()

    await card.getByTestId("review-requests-show-more").click()
    await expect(rows).toHaveCount(5)
    await expect(card.getByTestId("review-requests-show-more")).toHaveText("Show Less")

    // collapsed: the count stays, as on every island, the oldest age after it, per repository
    await card.getByTestId("your-turn-header").click()
    await expect(card.getByTestId("your-turn-summary")).toHaveText("oldest waiting 2 days")
    await expect(card.getByTestId("your-turn-count")).toHaveText("5")
    await expect(rows).toHaveCount(0)
  })

  test("should say 50 of 63 when GitHub has more than it returns, expanded or not, and keep the count in the collapsed header", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("issue_links", { reviewRequests: availableReviewRequests([OLD, RECENT, RELEASE, FOURTH], 63) })
    await syncAndWaitForBranches()
    const card = getYourTurnCard(page)
    const more = card.getByTestId("review-requests-more-on-github")
    await expect(card.getByTestId("your-turn-count")).toHaveText("63")
    // three rows, the fourth behind Show More: the list says how many of the 63 GitHub lists
    await expect(card.getByTestId("review-request-row")).toHaveCount(3)
    await expect(more).toHaveText("4 of 63Open All on GitHub")
    await card.getByTestId("review-requests-show-more").click()
    await expect(card.getByTestId("review-request-row")).toHaveCount(4)
    await expect(more).toHaveText("4 of 63Open All on GitHub")
    await card.getByRole("button", { name: "Open All on GitHub" }).click()
    expect(await getOpenedUrls(page)).toEqual(["https://github.com/acme/monorepo/pulls?q=is%3Apr%20is%3Aopen%20archived%3Afalse%20draft%3Afalse%20user-review-requested%3A%40me"])

    // collapsed, the header keeps the count of all 63
    await card.getByTestId("your-turn-header").click()
    await expect(card.getByTestId("review-request-row")).toHaveCount(0)
    await expect(card.getByTestId("your-turn-count")).toHaveText("63")
    await expect(card.getByTestId("your-turn-summary")).toHaveText("oldest waiting 2 days")
    await card.getByTestId("your-turn-header").click()

    // two listed of 63: no Show More, and the list still says it
    await repo.setReviewRequests(availableReviewRequests([OLD, RECENT], 63))
    await page.clock.fastForward("05:01")
    await emitTauriEvent(page, "tauri://focus")
    await expect(card.getByTestId("review-request-row")).toHaveCount(2)
    await expect(card.getByTestId("review-requests-show-more")).toHaveCount(0)
    await expect(more).toHaveText("2 of 63Open All on GitHub")
  })

  test("should move with the arrows, review with Enter and open the menu from the keyboard", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT, RELEASE)
    await getReviewRow(page, 101).focus()
    await page.keyboard.press("ArrowDown")
    await expect(getReviewRow(page, 102)).toBeFocused()
    await page.keyboard.press("Shift+F10")
    const menu = page.getByRole("menu")
    // the actions of the header of its review: a write opens the review for it
    // no Patronus in the repository: no Approve and Safe Push…
    await expect(menu.getByRole("menuitem")).toHaveText(["Review↵", "Approve…", "Request Changes…", "Open Pull RequestO", "Copy Pull Request Link", "Copy Branch Name"])
    await page.keyboard.press("Escape")
    await expect(getReviewRow(page, 102)).toBeFocused()
    // Enter: the review window of the row; the main window keeps the focus on the row
    await page.keyboard.press("Enter")
    const opened = await openedReviewWindow(page)
    expect(opened).toMatchObject({ title: "Review #102 Add OAuth login", data: { number: 102, index: 1, intent: null } })
    await expect(getReviewRow(page, 102)).toBeFocused()
  })

  test("should list the own pull requests a reviewer handed back after the requests, show the branch and review it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", {
      reviewRequests: availableReviewRequests([OLD]),
      pullRequests: availablePullRequests(
        pullRequest("JIRA-456", 760, {
          title: "Fix the pool timeout",
          reviewDecision: "changesRequested",
          reviewers: [reviewer("alice", "changesRequested"), reviewer("bob", "approved")],
        }),
        // failing checks are told by the chip and the outgoing dot
        pullRequest("feature-api", 761, { checks: { state: "failing", passed: 1, failed: 1, pending: 0, failedNames: ["tests"] } }),
      ),
    })
    await syncAndWaitForBranches()
    const card = getYourTurnCard(page)
    // both kinds count
    await expect(card.getByTestId("your-turn-count")).toHaveText("2")
    const row = card.getByTestId("returned-pull-request-row")
    await expect(row).toHaveCount(1)
    await expect(row.getByTestId("returned-pull-request-title")).toHaveText("Fix the pool timeout")
    await expect(row.getByTestId("returned-pull-request-meta")).toHaveText("#760 · changes requested by alice")
    await expect(row.getByTestId("returned-pull-request-branch")).toHaveText("JIRA-456")
    // the changes are made in the branch: a click shows its row; O opens the pull request
    await row.click()
    await verifyBranchState(findBranchRow(page, "JIRA-456"), "open")
    await row.focus()
    await page.keyboard.press("o")
    expect(await getOpenedUrls(page)).toEqual(["https://github.com/JetBrains/ultimate/pull/760"])
    // D, and Review Changes of its menu: the review of its branch, where the changes requested are read
    await row.focus()
    await page.keyboard.press("d")
    await expect.poll(async () => (await getOpenedSubWindows(page)).map(window => [window.windowId, window.title])).toEqual([["branch-review", "Review JIRA-456"]])
    await row.click({ button: "right" })
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(2)
  })

  test("should show the island for a returned pull request alone", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", {
      reviewRequests: availableReviewRequests([]),
      pullRequests: availablePullRequests(pullRequest("JIRA-456", 760, { reviewDecision: "changesRequested" })),
    })
    await syncAndWaitForBranches()
    const card = getYourTurnCard(page)
    await expect(card.getByTestId("returned-pull-request-row")).toHaveCount(1)
    await expect(card.getByTestId("review-request-row")).toHaveCount(0)
    await card.getByTestId("your-turn-header").click()
    await expect(card.getByTestId("your-turn-summary")).toHaveText("1 returned")
    await expect(card.getByTestId("your-turn-count")).toHaveText("1")
  })

  test("should refresh from the menu, and keep the rows when a refresh fails, with the reason in the footer", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    // nothing to say: no footer, the island is its rows
    await expect(getYourTurnCard(page).getByTestId("review-requests-status-line")).toHaveCount(0)
    await routeCommandError(page, "get_review_requests", "offline")
    await getYourTurnCard(page).getByRole("button", { name: "More actions for Your Turn" }).click()
    await page.getByRole("menuitem", { name: "Refresh" }).click()
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

    // Review: the review window of the request, read now
    await toast.getByRole("button", { name: "Review" }).click()
    expect((await openedReviewWindow(page)).data).toMatchObject({ number: 104, intent: null })
    await expect(row.getByTestId("review-request-unread")).toHaveCount(0)

    // the Activity keeps it
    await page.getByRole("button", { name: "Activity" }).click()
    await expect(page.getByTestId("activity-title").first()).toHaveText("dave requested your review on #104")
  })
})

test.describe("Review window", () => {
  test("should preview the pull request, fetch it with progress, then review its range with the file tree", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    let release = () => {}
    let fetches: Record<string, unknown>[] = []
    // the window fetches the pull request itself
    const {
      window: reviewPage,
      review,
      opened,
    } = await openReviewWindow(page, 101, {
      beforeLoad: async window => {
        fetches = await routeReviewFetch(window, { progress: [42], release: new Promise<void>(resolve => (release = resolve)) })
      },
    })
    expect(opened).toMatchObject({
      title: "Review #101 Rework the session store",
      data: { number: 101, index: 0, intent: null, state: { selected: true, requests: [{ number: 101 }, { number: 102 }] } },
    })
    await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
    // who asks, for how long, then the chips of what GitHub said
    await expect(review.getByTestId("review-asked-by")).toHaveText("Ddave asks for your review of carol's pull request·waiting 2 days")
    await expect(review.getByTestId("review-dialog-age")).toHaveAttribute("data-old", "true")
    await expect(review.getByTestId("review-dialog-meta")).toHaveText("#101master ← user-name/virtual/ABC-1232 files+12 −3checks failing")
    await expect(review.getByTestId("review-dialog-body")).toContainText("The change of ABC-123.")
    await expect(review.getByTestId("review-fetch-progress")).toHaveText("Fetching #101 from origin… 42%")
    await expect(review.getByTestId("review-dialog-review")).toBeDisabled()
    // the rail is the only progress of the fetch: no spinner in Changes beside it
    await expect(review.getByTestId("review-dialog-review").getByRole("status")).toHaveCount(0)
    await expect(review.getByTestId("review-dialog-position")).toHaveText("1 of 2")
    // Approve is the decision the review is for: the primary, last
    await expect(review.getByTestId("review-approve")).toHaveAttribute("data-variant", "default")
    await expect(review.getByTestId("review-decisions").getByRole("button").last()).toHaveAttribute("data-testid", "review-approve")
    // the window is the review: no Open in Window, no close button
    await expect(review.getByRole("button", { name: "Open in Window" })).toHaveCount(0)
    await expect(review.getByRole("button", { name: "Close" })).toHaveCount(0)
    expect(fetches).toHaveLength(1)

    release()
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await expect(review.getByTestId("review-dialog-review")).toBeEnabled()
    // the files of the range beside the description
    const files = review.getByTestId("review-file")
    await expect(files.first()).toBeVisible()
    await reviewPage.keyboard.press("d")
    const changes = review.getByTestId("commit-review")
    await expect(changes).toBeVisible()
    await expect(review.getByTestId("review-tab-overview")).not.toHaveAttribute("data-active")
    await expect(review.getByTestId("review-file-position")).toHaveText(/File 1 \/ \d+$/)
    // the diffs have the focus, as in the review of a commit
    await expect(changes.locator("[aria-label='Changes of the commit']")).toBeFocused()
    await reviewPage.keyboard.press("j")
    await expect(review.getByTestId("review-file-position")).toHaveText(/File 2 \/ \d+$/)

    // Escape goes back to the overview, the window stays
    await reviewPage.keyboard.press("Escape")
    await expect(changes).toBeHidden()
    await expect(review.getByTestId("review-commits")).toBeVisible()
    expect(await windowCalls(reviewPage)).not.toContain("close")

    // a file of the rail reviews the changes at it
    await files.nth(1).click()
    await expect(changes).toBeVisible()
    await expect(review.getByTestId("review-file-position")).toHaveText(/File 2 \/ \d+$/)
    await reviewPage.keyboard.press("Escape")
    await expect(changes).toBeHidden()
    // O: the main window opens it on GitHub
    await reviewPage.keyboard.press("o")
    await expect.poll(() => getOpenedUrls(page)).toEqual(["https://github.com/acme/monorepo/pull/101"])
    // Escape in the overview closes the window
    await reviewPage.keyboard.press("Escape")
    await expect.poll(() => windowCalls(reviewPage)).toContain("close")
  })

  test("should review a commit of the pull request in the changes, step through the commits, and show the next request with ↓", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    const { window: reviewPage, review } = await openReviewWindow(page, 101)
    // the rail lists the commits newest first, as the column of the changes
    const commits = review.getByTestId("review-commit")
    await expect(commits.first()).toBeVisible()
    const subjects = await commits.getByTestId("review-commit-subject").allTextContents()
    await commits.first().click()
    const changes = review.getByTestId("commit-review")
    await expect(changes).toBeVisible()
    await expect(reviewPage.getByTestId("commit-preview")).toHaveCount(0)
    const header = review.getByTestId("range-review-commit-header")
    if (subjects.length > 1) {
      // the column beside the diffs, the commit chosen; ⌥↓ the next one
      const column = review.getByTestId("range-review-commits")
      await expect(column.getByTestId("range-review-commit").nth(1)).toHaveAttribute("aria-selected", "true")
      await expect(header).toContainText(subjects[0]!)
      await expect(review.getByTestId("review-file-position")).toHaveText(/^Commit 1 \/ \d+ · File 1 \/ \d+$/)
      await reviewPage.keyboard.press("Alt+ArrowDown")
      await expect(header).toContainText(subjects[1]!)
      await reviewPage.keyboard.press("Alt+ArrowUp")
      await reviewPage.keyboard.press("Alt+ArrowUp")
      await expect(column.getByTestId("range-review-commit").first()).toHaveAttribute("aria-selected", "true")
      await expect(header).toHaveCount(0)
    } else {
      // one commit: no column, the commit above its diffs
      await expect(header).toContainText(subjects[0]!)
    }
    await reviewPage.keyboard.press("Escape")
    await expect(changes).toBeHidden()

    // ↓ shows the next request in the window and tells the main window, which marks it read
    await review.getByTestId("review-dialog-body").focus()
    await reviewPage.keyboard.press("ArrowDown")
    await expect(review.getByRole("heading", { name: "Add OAuth login" })).toBeVisible()
    await expect(review.getByTestId("review-dialog-position")).toHaveText("2 of 2")
    expect((await getEmittedEvents<ReviewWindowRequest>(reviewPage, "pull-request-review/request")).at(-1)).toMatchObject({ action: "shown", request: { number: 102 } })
    // the ↑ of the title bar shows the one before
    await review.getByTestId("review-previous").click()
    await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
  })

  test("should review the changes of a row with D", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    await getReviewRow(page, 101).focus()
    await page.keyboard.press("d")
    const opened = await openedReviewWindow(page)
    expect(opened.data).toMatchObject({ number: 101, intent: "review" })
    // once the pull request is fetched: its changes
    const reviewPage = await openLinkedSubWindow(page, opened)
    const review = reviewPage.getByTestId("review-window")
    await expect(review.getByTestId("commit-review")).toBeVisible()
    await expect(review.getByTestId("review-dialog-review")).toHaveAttribute("data-active")
  })

  test("should say why a fetch failed and fetch again on Retry", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    await repo.setReviewRequests(availableReviewRequests([OLD]), "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)")
    const { review } = await openReviewWindow(page, 101)
    const failure = review.getByTestId("review-fetch-failed")
    await expect(failure).toContainText("Cannot fetch pull request #101")
    await expect(failure).toContainText("GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)")

    await repo.setReviewRequests(availableReviewRequests([OLD]))
    await failure.getByTestId("review-fetch-retry").click()
    await expect(failure).toBeHidden()
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
  })

  test("should approve the commit the review shows, in the review too, and keep its place once it left the list", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    // the main window runs the write
    const reviews = await routeCommand(page, "submit_pull_request_review", () => null)
    const { window: reviewPage, review } = await openReviewWindow(page, 101)
    const approve = review.getByTestId("review-approve")
    // nothing to approve before the commit the user sees is there
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await expect(approve).toBeEnabled()
    await expect(review.getByTestId("review-request-changes")).toBeEnabled()

    // the actions of the pull request stay in the review of its changes
    await reviewPage.keyboard.press("d")
    await expect(review.getByTestId("commit-review")).toBeVisible()
    await expect(approve).toBeVisible()
    await repo.setReviewRequests(availableReviewRequests([RECENT]))
    await reviewPage.keyboard.press("a")
    // the toast is the window's; the main window records it in Activity
    await expect(reviewToast(reviewPage, "Approved #101")).toBeVisible()
    await expect(reviewToast(page, "Approved #101")).toHaveCount(0)
    expect(reviews).toEqual([expect.objectContaining({ pullRequestNumber: 101, event: "approve", commitId: expect.stringMatching(/^[0-9a-f]{40}$/), body: null })])
    // done: "Approved", and A approves nothing again
    await expect(approve).toHaveText("Approved")
    await expect(approve).toHaveAttribute("data-approved")
    await reviewPage.keyboard.press("a")
    expect(reviews).toHaveLength(1)
    // Open Pull Request of the toast: the main window opens it
    await reviewToast(reviewPage, "Approved #101").getByRole("button", { name: "Open Pull Request" }).click()
    await expect.poll(() => getOpenedUrls(page)).toEqual(["https://github.com/acme/monorepo/pull/101"])

    // GitHub is asked at once: the approved request leaves the card, the window keeps it and its place
    await expect(getReviewRow(page, 101)).toHaveCount(0)
    await review.getByTestId("review-tab-overview").click()
    await expect(review.getByTestId("review-commits")).toBeVisible()
    await expect(review.getByTestId("review-dialog-submitted")).toHaveText("approved by you")
    await expect(review.getByTestId("review-withdrawn")).toHaveCount(0)
    await review.getByTestId("review-dialog-body").focus()
    await reviewPage.keyboard.press("ArrowDown")
    await expect(review.getByRole("heading", { name: "Add OAuth login" })).toBeVisible()
  })

  test("should keep the window while a write runs, and tell its result in the main window once the window closed", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    let answer = () => {}
    const answered = new Promise<void>(resolve => (answer = resolve))
    await page.route("**/invoke/submit_pull_request_review", async route => {
      await answered
      await route.fulfill({ json: { status: "ok", data: null } })
    })
    const { window: reviewPage, review } = await openReviewWindow(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await reviewPage.keyboard.press("a")
    await expect(review.getByTestId("review-approve")).toBeDisabled()
    // Escape keeps the window while GitHub has not answered
    await reviewPage.keyboard.press("Escape")
    expect(await windowCalls(reviewPage)).not.toContain("close")

    // the window went meanwhile: the main window says how it went
    await emitTauriEvent(page, "tauri://destroyed", null, "pull-request-review")
    answer()
    await expect(reviewToast(page, "Approved #101")).toBeVisible()
  })

  test("should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    const reviews = await routeCommand(page, "submit_pull_request_review", () => null)
    // the row's menu opens the window with the form, once the pull request is fetched
    await getReviewRow(page, 101).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Request Changes…" }).click()
    const opened = await openedReviewWindow(page)
    expect(opened.data).toMatchObject({ number: 101, intent: "requestChanges" })
    const reviewPage = await openLinkedSubWindow(page, opened)
    const review = reviewPage.getByTestId("review-window")
    const form = reviewPage.getByTestId("review-request-changes-form")
    await expect(form).toBeVisible()
    const send = form.getByTestId("review-request-changes-send")
    await expect(send).toBeDisabled()

    const comment = form.getByTestId("review-request-changes-comment")
    await expect(comment).toBeFocused()
    await comment.pressSequentially("a d o r: sort by the snapshot")
    await reviewPage.keyboard.press("ArrowDown")
    expect(reviews).toEqual([])
    expect(await getOpenedUrls(page)).toEqual([])
    await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()

    // Escape closes the form (Base UI prevents it), not the window, and keeps the draft; the focus stays in the review,
    // whose keys work again
    await reviewPage.keyboard.press("Escape")
    await expect(form).toBeHidden()
    expect(await windowCalls(reviewPage)).not.toContain("close")
    await expect(review.getByTestId("review-request-changes")).toBeFocused()
    // another request and back: the draft is the pull request's
    await review.getByTestId("review-dialog-body").focus()
    await reviewPage.keyboard.press("ArrowDown")
    await expect(review.getByRole("heading", { name: "Add OAuth login" })).toBeVisible()
    // a write waits for the commit the user sees
    await expect(review.getByTestId("review-request-changes")).toBeEnabled()
    await reviewPage.keyboard.press("r")
    await expect(comment).toHaveValue("")
    await expect(comment).toBeFocused()
    await reviewPage.keyboard.press("Escape")
    await review.getByTestId("review-dialog-body").focus()
    await reviewPage.keyboard.press("ArrowUp")
    await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
    await expect(review.getByTestId("review-request-changes")).toBeEnabled()
    await reviewPage.keyboard.press("r")
    await expect(comment).toHaveValue("a d o r: sort by the snapshot")
    // reopened while the last form still closes, the comment has the focus all the same
    await expect(comment).toBeFocused()
    await reviewPage.keyboard.press("Control+Enter")
    await expect(form).toBeHidden()
    await expect(reviewToast(reviewPage, "Requested changes on #101")).toBeVisible()
    expect(reviews).toEqual([expect.objectContaining({ pullRequestNumber: 101, event: "requestChanges", body: "a d o r: sort by the snapshot" })])
    await expect(review.getByTestId("review-dialog-submitted")).toHaveText("changes requested by you")
    // sent: the draft is gone
    await reviewPage.keyboard.press("r")
    await expect(comment).toHaveValue("")
  })

  test("should say why GitHub refused a review", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    await routeCommandError(page, "submit_pull_request_review", "User can only have one pending review per pull request")
    const { window: reviewPage, review } = await openReviewWindow(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await review.getByTestId("review-approve").click()
    await expect(reviewToast(reviewPage, "Cannot approve pull request #101")).toContainText("User can only have one pending review per pull request")
    await expect(review.getByTestId("review-approve")).toBeEnabled()
  })

  test("should approve and Safe Push a pull request into the Patronus target with a plain /patronus merge, after a confirmation", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // GitHub shows the commit the user approved
    const { reviews, heads, posts, replies } = await setupSafePush(page, setupRepo, syncAndWaitForBranches, [OLD, RELEASE])
    // the robot is not among the user's: the thumbs up is the start
    await routeCommand(page, "get_patronus_robots", () => availableRobots())

    // the row's menu blocks it as the review does: Patronus would merge #103 into its own base
    await getReviewRow(page, 103).click({ button: "right" })
    const rowSafePush = page.getByRole("menuitem", { name: /^Approve and Safe Push…/ })
    await expect(rowSafePush).toHaveAttribute("aria-disabled", "true")
    await rowSafePush.hover()
    await expect(page.getByRole("tooltip")).toHaveText("#103 merges into release/253: Patronus would merge it there, not into master")
    await page.keyboard.press("Escape")
    await getReviewRow(page, 101).click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: /^Approve and Safe Push…/ })).not.toHaveAttribute("aria-disabled", "true")
    await page.keyboard.press("Escape")

    const { window: reviewPage, review } = await openReviewWindow(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    const safePush = review.getByTestId("review-safe-push")
    await expect(safePush).toHaveText("Approve and Safe Push…M")
    // Approve stays the primary; the Safe Push is the next step after it
    await expect(safePush).toHaveAttribute("data-variant", "outline")
    await expect(review.getByTestId("review-approve")).toHaveAttribute("data-variant", "default")
    await reviewPage.keyboard.press("m")
    const form = reviewPage.getByTestId("review-safe-push-form")
    await expect(form.getByTestId("review-safe-push-description")).toHaveText(
      /^Approves [0-9a-f]{8}, then comments \/patronus merge on #101: Patronus rebases user-name\/virtual\/ABC-123 onto master, runs the checks, merges it and deletes the branch\. It reports in the pull request\.$/,
    )
    // the keys of the review wait while the form is open
    await reviewPage.keyboard.press("a")
    expect(reviews).toEqual([])
    await expect(form.getByTestId("review-safe-push-start")).toBeFocused()
    await reviewPage.keyboard.press("Enter")
    await skipReplyWait(page, replies)
    await expect
      .poll(async () => {
        await page.clock.fastForward(3000)
        return reviewPage.locator("[data-slot=toast]", { hasText: "Safe Push started" }).count()
      })
      .toBe(1)
    expect(reviews).toEqual([expect.objectContaining({ pullRequestNumber: 101, event: "approve" })])
    expect(heads).toEqual([expect.objectContaining({ pullRequestNumber: 101, githubRepository: "acme/monorepo" })])
    expect(posts).toEqual([expect.objectContaining({ pullRequestNumber: 101, mode: "rebaseAutosquash", subject: "reviewedPullRequest", githubRepository: "acme/monorepo" })])
    await expect(form).toBeHidden()
    // approved: the Safe Push alone is left, the primary now
    await expect(safePush).toHaveText("Safe Push…M")
    await expect(safePush).toHaveAttribute("data-variant", "default")
    await expect(review.getByTestId("review-approve")).toHaveText("Approved")

    // a pull request into another base than the target: Patronus would merge it there
    await review.getByTestId("review-dialog-body").focus()
    await reviewPage.keyboard.press("ArrowDown")
    await expect(review.getByRole("heading", { name: "Backport the pool fix" })).toBeVisible()
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await expect(review.getByTestId("review-safe-push")).toBeDisabled()
  })

  test("should watch the robot of a reviewed pull request: listed by what it tests, its merge told", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { replies } = await setupSafePush(page, setupRepo, syncAndWaitForBranches, [OLD])
    // the page's clock is at 2024-01-02 10:00Z; the robot runs on no branch of the user's
    const started = robot("ABC-123", "review-robot", { branchName: null, startTime: 1704189600, pullRequest: { number: 101, url: "https://github.com/acme/monorepo/pull/101" } })
    await routeCommand(page, "get_patronus_robots", () => availableRobots(started))
    const polls = await routeCommand(page, "poll_patronus_robots", () => availableRobots({ ...started, status: "success", finishTime: 1704189700 }))

    const { window: reviewPage, review } = await openReviewWindow(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await reviewPage.keyboard.press("m")
    await reviewPage.getByTestId("review-safe-push-form").getByTestId("review-safe-push-start").click()
    await skipReplyWait(page, replies)
    await expect(reviewPage.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()

    // the header of the main window lists it by the pull request, without a row to show
    await page.getByTestId("landing-indicator").hover()
    const row = page.getByTestId("landing-popover").getByTestId("landing-running").getByTestId("robots-row")
    await expect(row).toHaveAttribute("data-branch-name", "#101 Rework the session store")
    await expect(row.getByRole("button", { name: "Open Pull Request #101 of #101 Rework the session store" })).toBeVisible()

    // polled while it runs: its merge is told by what it tested
    await expect
      .poll(async () => {
        await page.clock.fastForward(60_000)
        return polls.length
      })
      .toBeGreaterThan(0)
    await expect(page.locator("[data-slot=toast]", { hasText: "#101 Rework the session store merged into master" })).toBeVisible()
  })

  test("should not Safe Push a pull request that got commits since the user reviewed it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { posts } = await setupSafePush(page, setupRepo, syncAndWaitForBranches, [OLD], "f".repeat(40))
    // from the row's menu: the window opens with the confirmation once the pull request is fetched
    await getReviewRow(page, 101).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Approve and Safe Push…" }).click()
    const opened = await openedReviewWindow(page)
    expect(opened.data).toMatchObject({ number: 101, intent: "safePush" })
    const reviewPage = await openLinkedSubWindow(page, opened)
    const form = reviewPage.getByTestId("review-safe-push-form")
    await form.getByTestId("review-safe-push-start").click()
    await expect(reviewToast(reviewPage, "Cannot start the Safe Push")).toContainText(/#101 has new commits since you reviewed [0-9a-f]{8}: review ffffffff first/)
    expect(posts).toEqual([])
  })

  test("should keep the review when the request is withdrawn meanwhile, and its refs", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links", { reviewRequests: availableReviewRequests([OLD, RECENT]) })
    await syncAndWaitForBranches()
    // the refs of an earlier start go once, after the first answer
    await expect.poll(async () => (await repo.getReviewRequestsFixture()).prunes).toEqual([[]])
    const { review } = await openReviewWindow(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await repo.setReviewRequests(availableReviewRequests([RECENT]))
    await page.clock.fastForward("05:00")
    await expect(review.getByTestId("review-withdrawn")).toHaveText("No longer waiting for your review")
    await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
    await expect(review.getByTestId("review-dialog-position")).toHaveCount(0)
    // the window fetched it: the next prune keeps its refs
    const fixture = await repo.getReviewRequestsFixture()
    expect(fixture.fetchCalls).toBe(1)
    expect(fixture.prunes).toEqual([[], [101]])
  })
})

test.describe("Settings", () => {
  test("should open Settings at Notifications from the card menu, whose switches the main window keeps", async ({ page, setupRepo, syncAndWaitForBranches, testStore }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    const count = (await getOpenedSubWindows(page)).length
    await getYourTurnCard(page).getByRole("button", { name: "More actions for Your Turn" }).click()
    await page.getByRole("menuitem", { name: "Notification Settings…" }).click()
    const settings = await openLinkedSubWindow(page, await expectSettingsOpened(page, { page: "notifications" }, count + 1))
    const notifications = settings.getByTestId("settings-page-notifications")
    await expect(notifications.getByRole("heading", { name: "Notifications" })).toBeVisible()
    const reviews = notifications.getByTestId("setting-notifyReviewRequests").getByRole("switch")
    await expect(reviews).toBeChecked()
    await reviews.click()
    await expect(reviews).not.toBeChecked()
    // the robots: merged, and failing (a required check failed while it runs)
    await expect(notifications.getByTestId("setting-notifyPatronusMerges").getByRole("switch")).toBeChecked()
    await expect(notifications.getByTestId("setting-notifyPatronusFailures").getByRole("switch")).toBeChecked()
    // Windows has no Dock badge: no group
    await expect(notifications.getByTestId("setting-reviewBadge")).toHaveCount(0)
    // the window asked the main window, which saved it
    await expect.poll(() => testStore.get("appSettings")).toMatchObject({ notifyReviewRequests: false })
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
    await getReviewRow(page, 102)
      .getByTestId("review-request-title")
      .click({ modifiers: ["Meta"] })
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
    // none of them opened the review
    expect(await getOpenedSubWindows(page)).toEqual([])
  })

  test("should count the review requests, behind its switch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    const badges = () =>
      page.evaluate(() =>
        ((window as unknown as { __e2eDockCalls?: { badgeCount?: number | null }[] }).__e2eDockCalls ?? []).filter(it => "badgeCount" in it).map(it => it.badgeCount),
      )
    await expect.poll(badges).toEqual([2])

    // the switch of the Settings window: the main window sets the badge
    const settings = await openSettingsWindow(page, { page: "notifications" })
    await settings.getByTestId("setting-reviewBadge").getByRole("switch").click()
    await expect.poll(badges).toEqual([2, null])
  })
})
