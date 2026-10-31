// @spec spec/repository/review-requests.spec.md
import { appendFileSync } from "node:fs"
import path from "node:path"
import type { Page } from "@playwright/test"
import type { ReviewWindowRequest } from "../../../src/lib/pull-request-review"
import { test, expect } from "../fixtures/test-fixtures"
import { openActivityWindow } from "../helpers/activity-helpers"
import { findBranchRow, verifyBranchState } from "../helpers/branch-helpers"
import { availableRobots, codeOwner, evaluatedGroup, robot } from "../helpers/patronus-helpers"
import { availablePullRequests, getOpenedUrls, gitHubRequests, pullRequest, repoPath, reviewer } from "../helpers/pull-request-helpers"
import {
  NOW,
  availableReviewRequests,
  getReviewRow,
  getYourTurnCard,
  openedReviewWindow,
  openReviewWindow,
  pressDecisionKey,
  reviewRequest,
  routeReviewFetch,
} from "../helpers/review-request-helpers"
import { expectSettingsOpened, openSettingsWindow } from "../helpers/settings-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { getEmittedEvents, getOpenedSubWindows, openLinkedSubWindow, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

// the `issue_links` template: 7 branches; the requests review them as the branches of others
const OLD = reviewRequest("ABC-123", 101, 50, { title: "Rework the session store", checksState: "failing" })
const RECENT = reviewRequest("feature-auth", 102, 3, { title: "Add OAuth login", reRequested: true, reviewedCommit: null })
const RELEASE = reviewRequest("JIRA-456", 103, 2, { title: "Backport the pool fix", baseRefName: "release/253", baseIsDefault: false, checksState: "pending" })
const FOURTH = reviewRequest("feature-docs", 104, 1, { title: "Document the CLI" })
// asked 5 hours ago, the user commented, carol replied half an hour ago: the turn is the user's again
const FIFTH = reviewRequest("feature-db", 105, 0.5, { title: "Migrate the schema", requestedAt: NOW - 5 * 3600, backToYou: { at: NOW - 1800, repliedBy: "carol" } })

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
 * The requests in a repository Patronus guards, its target master; the fake GitHub holds their pull requests, its bot
 * accepts a `/patronus` comment, and with `head` GitHub shows that commit as the head of #101 (someone pushed since)
 */
async function setupSafePush(page: Page, setupRepo: SetupRepo, syncAndWaitForBranches: () => Promise<void>, requests: ReturnType<typeof reviewRequest>[], head?: string) {
  // the card reads it (master, the baseline) once it shows: the menus of its rows say which pull requests Patronus merges into it
  const repo = await setupRepo("issue_links", { reviewRequests: availableReviewRequests(requests), patronus: availableRobots() })
  await syncAndWaitForBranches()
  await expect(getYourTurnCard(page)).toBeVisible()
  if (head !== undefined) {
    await repo.answerGitHub("GET", repoPath("pulls", 101), 200, { number: 101, head: { ref: "user-name/virtual/ABC-123", sha: head } })
  }
  return repo
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
    // the session asks a focus two minutes after the last answer, but never off GitHub
    await repo.advanceBackendClock(121_000)
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
    await repo.advanceBackendClock(121_000)
    await emitTauriEvent(page, "tauri://focus")
    await expect.poll(() => repo.getReviewRequestCalls()).toBe(2)
    await expect(getYourTurnCard(page)).toHaveCount(0)

    await repo.setReviewRequests(availableReviewRequests([OLD]))
    await repo.advanceBackendClock(121_000)
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
    // answered by the user with a comment: what the other side did since, sorted by it
    await expect(getReviewRow(page, 105).getByTestId("review-request-age")).toHaveText("carol replied 30 minutes ago")

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
    // the poll five minutes after the last answer
    await repo.advanceBackendClock(300_000)
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
    expect(await getOpenedUrls(page)).toEqual(["https://github.com/acme/monorepo/pull/760"])
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
    // the node of its branch, as the branch row has it: changes requested need a fix
    const node = card.getByTestId("returned-pull-request-row").getByTestId("review-entry-node")
    await expect(node).toHaveAttribute("data-kind", "needsFix")
    await node.hover()
    await expect(page.getByTestId("lane-node-details")).toHaveAttribute("data-kind", "needsFix")
    await page.mouse.move(0, 0)
    await card.getByTestId("your-turn-header").click()
    await expect(card.getByTestId("your-turn-summary")).toHaveText("1 returned")
    await expect(card.getByTestId("your-turn-count")).toHaveText("1")
  })

  test("should refresh from the menu, and keep the rows when a refresh fails, with the reason in the footer", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    // nothing to say: no footer, the island is its rows
    await expect(getYourTurnCard(page).getByTestId("review-requests-status-line")).toHaveCount(0)
    await repo.setReviewRequests({ availability: { status: "error", message: "offline" }, totalCount: 0, requests: [] })
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
    // the session polls five minutes after the last answer, focused or not
    await repo.advanceBackendClock(300_000)
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
    const activity = await openActivityWindow(page)
    await expect(activity.getByTestId("activity-title").first()).toHaveText("dave requested your review on #104")
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

  test("should show the review plan of the pull request above its changes once the repository turns it on, its commits in the tree", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    // the flag of the repository, which the global config of the test machine turns off (`TEST_IDENTITY` of the server)
    appendFileSync(path.join(repo.path, ".git", "config"), "[branchdeck]\n\treviewExperimental = true\n")
    await getReviewRow(page, 101).focus()
    await page.keyboard.press("d")
    const review = (await openLinkedSubWindow(page, await openedReviewWindow(page))).getByTestId("review-window")
    await expect(review.getByTestId("commit-review")).toBeVisible()
    // the plan of what the window shows: its card, Rust's words
    await expect(review.getByTestId("review-plan-card").getByTestId("review-plan-summary")).toHaveText(/possible break/)

    // the commits of the pull request in the tree of its story, the column keeps the scope; Enter shows one alone
    const tree = review.getByRole("tree", { name: "Changed files" })
    const column = review.getByTestId("range-review-commits").getByTestId("range-review-commit")
    await expect(column).toHaveText([/^All Changes\d+ commits$/])
    const subject = (await tree.getByTestId("review-commit-subject").first().textContent())!
    await tree.getByRole("treeitem", { name: subject, exact: true }).click()
    await tree.getByRole("treeitem", { name: subject, exact: true }).press("Enter")
    await expect(review.getByTestId("range-review-commit-header")).toContainText(subject)
    await expect(column.filter({ hasText: subject })).toHaveAttribute("aria-selected", "true")
  })

  test("should list in the tree of Since Your Review only the commits after the user's review, and mark the reviewed one in All Changes", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("issue_links", { reviewRequests: availableReviewRequests([]) })
    await syncAndWaitForBranches()
    appendFileSync(path.join(repo.path, ".git", "config"), "[branchdeck]\n\treviewExperimental = true\n")
    // the user reviewed the second of the four commits of ABC-123, and the request came again
    const reviewed = await repo.revision("user-name/virtual/ABC-123~2")
    await repo.setReviewRequests(availableReviewRequests([{ ...OLD, reRequested: true, reviewedCommit: reviewed }]))
    await repo.advanceBackendClock(121_000)
    await emitTauriEvent(page, "tauri://focus")
    await expect(getReviewRow(page, 101)).toBeVisible()
    await getReviewRow(page, 101).focus()
    await page.keyboard.press("d")
    const review = (await openLinkedSubWindow(page, await openedReviewWindow(page))).getByTestId("review-window")
    const tree = review.getByRole("tree", { name: "Changed files" })
    const commitSubjects = tree.getByTestId("review-commit-subject")
    const column = review.getByTestId("range-review-commits").getByTestId("range-review-commit")

    // All Changes: every commit in the tree, the one of the review says so, as the boundary of the column did
    await expect(column).toHaveText(["All Changes4 commits", "Since Your Review2 commits"])
    await expect(commitSubjects).toHaveCount(4)
    const reviewedRow = tree.getByRole("treeitem", { name: "ABC-123 Add password hashing utility", exact: true })
    await expect(reviewedRow.getByTestId("review-commit-review")).toHaveText("Your review")
    await expect(reviewedRow).toHaveAttribute("aria-description", /Your review/)
    await expect(tree.getByTestId("review-commit-review")).toHaveCount(1)

    // Since Your Review: its own plan, only the two commits after the review in its tree, none the user saw
    await column.filter({ hasText: "Since Your Review" }).click()
    await expect(column.nth(1)).toHaveAttribute("aria-selected", "true")
    await expect.poll(async () => (await commitSubjects.allTextContents()).toSorted()).toEqual(["ABC-123 Add authentication tests", "ABC-123 Add session management"])
    await expect(column).toHaveText(["All Changes4 commits", "Since Your Review2 commits"])
    await expect(tree.getByTestId("review-commit-review")).toHaveCount(0)
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
    await pressDecisionKey(reviewPage, review, "a")
    // the toast is the window's; the main window records it in Activity
    await expect(reviewToast(reviewPage, "Approved #101")).toBeVisible()
    await expect(reviewToast(page, "Approved #101")).toHaveCount(0)
    expect(await gitHubRequests(repo, "POST", "pulls", 101, "reviews")).toEqual([{ event: "APPROVE", commit_id: expect.stringMatching(/^[0-9a-f]{40}$/) }])
    // done: "Approved", and A approves nothing again
    await expect(approve).toHaveText("Approved")
    await expect(approve).toHaveAttribute("data-approved")
    await reviewPage.keyboard.press("a")
    expect(await gitHubRequests(repo, "POST", "pulls", 101, "reviews")).toHaveLength(1)
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
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    // GitHub holds the review: the write runs
    await repo.holdRemoteAnswers({ hold: ["submitReview"] })
    const { window: reviewPage, review } = await openReviewWindow(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await pressDecisionKey(reviewPage, review, "a")
    await expect(review.getByTestId("review-approve")).toBeDisabled()
    // Escape keeps the window while GitHub has not answered
    await reviewPage.keyboard.press("Escape")
    expect(await windowCalls(reviewPage)).not.toContain("close")

    // the window went meanwhile: the main window says how it went
    await emitTauriEvent(page, "tauri://destroyed", null, "pull-request-review")
    await expect.poll(() => repo.getArrivedAnswers()).toContain("submitReview")
    await repo.holdRemoteAnswers({ release: ["submitReview"] })
    await expect(reviewToast(page, "Approved #101")).toBeVisible()
    expect(await gitHubRequests(repo, "POST", "pulls", 101, "reviews")).toEqual([expect.objectContaining({ event: "APPROVE" })])
  })

  test("should request changes with a comment whose letters are no keys of the review, and keep the draft for the pull request", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
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
    await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
    expect(await gitHubRequests(repo, "POST", "pulls", "*", "reviews")).toEqual([])
    expect(await getOpenedUrls(page)).toEqual([])

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
    await pressDecisionKey(reviewPage, review, "r")
    await expect(comment).toHaveValue("")
    await expect(comment).toBeFocused()
    await reviewPage.keyboard.press("Escape")
    await review.getByTestId("review-dialog-body").focus()
    await reviewPage.keyboard.press("ArrowUp")
    await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
    await pressDecisionKey(reviewPage, review, "r")
    await expect(comment).toHaveValue("a d o r: sort by the snapshot")
    // reopened while the last form still closes, the comment has the focus all the same
    await expect(comment).toBeFocused()
    await reviewPage.keyboard.press("Control+Enter")
    await expect(form).toBeHidden()
    await expect(reviewToast(reviewPage, "Requested changes on #101")).toBeVisible()
    expect(await gitHubRequests(repo, "POST", "pulls", 101, "reviews")).toEqual([
      { event: "REQUEST_CHANGES", commit_id: expect.stringMatching(/^[0-9a-f]{40}$/), body: "a d o r: sort by the snapshot" },
    ])
    await expect(review.getByTestId("review-dialog-submitted")).toHaveText("changes requested by you")
    // sent: the form gave the focus back to its button, disabled while the write ends; the draft is gone
    await expect(review.getByTestId("review-request-changes")).toBeFocused()
    await pressDecisionKey(reviewPage, review, "r")
    await expect(comment).toHaveValue("")
  })

  test("should say why GitHub refused a review", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    await repo.answerGitHub("POST", repoPath("pulls", 101, "reviews"), 422, {
      message: "Unprocessable Entity",
      errors: ["User can only have one pending review per pull request"],
    })
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
    // the robot is not among the user's: the thumbs up is the start
    const repo = await setupSafePush(page, setupRepo, syncAndWaitForBranches, [OLD, RELEASE])

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
    await pressDecisionKey(reviewPage, review, "m")
    const form = reviewPage.getByTestId("review-safe-push-form")
    await expect(form.getByTestId("review-safe-push-description")).toHaveText(
      /^Approves [0-9a-f]{8}, then comments \/patronus merge on #101: Patronus rebases user-name\/virtual\/ABC-123 onto master, runs the checks, merges it and deletes the branch\. It reports in the pull request\.$/,
    )
    // the keys of the review wait while the form is open
    await reviewPage.keyboard.press("a")
    await expect(form.getByTestId("review-safe-push-start")).toBeFocused()
    expect(await gitHubRequests(repo, "POST", "pulls", "*", "reviews")).toEqual([])
    await reviewPage.keyboard.press("Enter")
    // the bot's 👍 is the start: the engine looks for its robot among the user's for 10 s of the session's clock
    await expect
      .poll(async () => {
        await repo.advanceBackendClock(3000)
        return reviewPage.locator("[data-slot=toast]", { hasText: "Safe Push started" }).count()
      })
      .toBe(1)
    // the approval, the head GitHub shows, then the plain command: Patronus deletes the branch of a reviewed pull request
    expect(await gitHubRequests(repo, "POST", "pulls", 101, "reviews")).toEqual([expect.objectContaining({ event: "APPROVE" })])
    expect(await gitHubRequests(repo, "GET", "pulls", 101)).toHaveLength(1)
    expect(await gitHubRequests(repo, "POST", "issues", 101, "comments")).toEqual([{ body: "/patronus merge" }])
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
    const repo = await setupSafePush(page, setupRepo, syncAndWaitForBranches, [OLD])
    // the page's clock is at 2024-01-02 10:00Z; the robot runs on no branch of the user's
    const started = robot("ABC-123", "review-robot", { branchName: null, startTime: 1704189600, pullRequest: { number: 101, url: "https://github.com/acme/monorepo/pull/101" } })
    // Patronus lists it once the bot started it: the list the app asks after the thumbs up has it
    await repo.setPatronus(availableRobots(started))

    const { window: reviewPage, review } = await openReviewWindow(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await pressDecisionKey(reviewPage, review, "m")
    await reviewPage.getByTestId("review-safe-push-form").getByTestId("review-safe-push-start").click()
    await expect(reviewPage.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()

    // the header of the main window lists it by the pull request, without a row to show
    await page.getByTestId("landing-indicator").hover()
    const row = page.getByTestId("landing-popover").getByTestId("landing-running").getByTestId("robots-row")
    await expect(row).toHaveAttribute("data-branch-name", "#101 Rework the session store")
    await expect(row.getByRole("button", { name: "Open Pull Request #101 of #101 Rework the session store" })).toBeVisible()

    // the session polls it while it runs, every minute of its clock: its merge is told by what it tested
    await repo.setPatronus(availableRobots({ ...started, status: "success", finishTime: 1704189700 }))
    const merged = page.locator("[data-slot=toast]", { hasText: "#101 Rework the session store merged into master" })
    await expect
      .poll(async () => {
        await repo.advanceBackendClock(60_000)
        return merged.count()
      })
      .toBe(1)
    expect((await repo.getPatronusLog()).pollCalls).toBeGreaterThan(0)
  })

  test("should not Safe Push a pull request that got commits since the user reviewed it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupSafePush(page, setupRepo, syncAndWaitForBranches, [OLD], "f".repeat(40))
    // from the row's menu: the window opens with the confirmation once the pull request is fetched
    await getReviewRow(page, 101).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Approve and Safe Push…" }).click()
    const opened = await openedReviewWindow(page)
    expect(opened.data).toMatchObject({ number: 101, intent: "safePush" })
    const reviewPage = await openLinkedSubWindow(page, opened)
    const form = reviewPage.getByTestId("review-safe-push-form")
    await form.getByTestId("review-safe-push-start").click()
    await expect(reviewToast(reviewPage, "Cannot start the Safe Push")).toContainText(/#101 has new commits since you reviewed [0-9a-f]{8}: review ffffffff first/)
    expect(await gitHubRequests(repo, "GET", "pulls", 101)).toHaveLength(1)
    expect(await gitHubRequests(repo, "POST", "issues", "*", "comments")).toEqual([])
  })

  test("should keep the review when the request is withdrawn meanwhile, and its refs", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links", { reviewRequests: availableReviewRequests([OLD, RECENT]) })
    await syncAndWaitForBranches()
    const { review } = await openReviewWindow(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    // the window shows #101: the session prunes the review refs of every other pull request, an earlier start's too
    await expect.poll(async () => (await repo.getReviewRequestsFixture()).prunes).toEqual([[101]])
    await repo.setReviewRequests(availableReviewRequests([RECENT]))
    await repo.advanceBackendClock(300_000)
    await expect(review.getByTestId("review-withdrawn")).toHaveText("No longer waiting for your review")
    await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
    await expect(review.getByTestId("review-dialog-position")).toHaveCount(0)
    // the window fetched it and still shows it: no listing prunes its refs
    const fixture = await repo.getReviewRequestsFixture()
    expect(fixture.fetchCalls).toBe(1)
    expect(fixture.prunes).toEqual([[101]])
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

// the `stacked_ownership` template: `login` changes auth/token.js (Auth Team), ui/login.css (Design), ui/login.js (UI
// Team) and README.md (Monorepo) by its OWNERSHIP files
const LOGIN = reviewRequest("login", 7, 5, { title: "Add the login form" })
const LOGIN_STYLE = reviewRequest("login", 8, 4, { title: "Style the login form" })

/** What Patronus answers of a pull request of `login`: Auth Team approved, Design (whose member is the user) missing */
const LOGIN_GROUPS = {
  required: true,
  mergeRequest: null,
  skipped: false,
  groups: [
    evaluatedGroup("Auth Team", true, [codeOwner("Ann Lee", "ann", { approvalState: "approved" })], { paths: ["auth/token.js"], branchWide: false }),
    evaluatedGroup("Design", false, [codeOwner("Test User", "user-name", { suggested: true, approvalState: "reviewing" })], { paths: ["ui/login.css"], branchWide: false }),
  ],
}

/** The requests of `login` in a repository on Patronus, the user signed in to GitHub as `user-name` */
async function setupOwnedRequests(setupRepo: SetupRepo, syncAndWaitForBranches: () => Promise<void>, options: SetupRepoOptions = {}): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("stacked_ownership", {
    reviewRequests: availableReviewRequests([LOGIN, LOGIN_STYLE]),
    patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
    pullRequests: availablePullRequests(),
    gitHubSignIn: { signedIn: true },
    ...options,
  })
  await repo.setReviewAnswers({ answers: { "request:7": LOGIN_GROUPS, "request:8": LOGIN_GROUPS } })
  await syncAndWaitForBranches()
  return repo
}

/** The owner chips by the OWNERSHIP files alone, while Patronus has not answered */
const OWNERSHIP_CHIPS = ["All4", "Auth Team1", "Design1", "Monorepo1", "UI Team1"]

/** The owner chips once Patronus answered: the groups, the user's first, then the other owners */
const GROUP_CHIPS = ["All4", "Design· You1", "Auth Team1", "Monorepo1", "UI Team1"]

/** `name·You count` of the owner chips of the review window */
function ownerChips(window: Page): Promise<string[]> {
  return window.getByTestId("branch-review-owner").evaluateAll(chips => chips.map(chip => chip.textContent!))
}

function ownerChip(window: Page, name: string) {
  return window.locator(`[data-testid=branch-review-owner][data-owner='${name}']`)
}

/** The header of the diff of `path` */
function diffHeader(window: Page, path: string) {
  return window.locator(`[data-testid=review-file-header][data-path='${path}']`)
}

/** The changes of the request the window shows (D), at its first diff */
async function showChanges(window: Page, review: ReturnType<Page["getByTestId"]>): Promise<void> {
  await expect(review.getByTestId("review-dialog-review")).toBeEnabled()
  await window.keyboard.press("d")
  await expect(window.getByTestId("review-file-header").first()).toBeVisible()
}

/** The review window of #7 from Your Turn, its changes shown */
async function openOwnedChanges(page: Page): Promise<{ window: Page; review: ReturnType<Page["getByTestId"]> }> {
  await expect(getYourTurnCard(page)).toBeVisible()
  const { window, review } = await openReviewWindow(page, 7)
  await showChanges(window, review)
  return { window, review }
}

/** The review window of #7 while Patronus holds its answer: the owners by the OWNERSHIP files, All chosen */
async function openOwnedChangesBeforeGroups(page: Page, repo: TestRepositoryBuilder): Promise<{ window: Page; review: ReturnType<Page["getByTestId"]> }> {
  await repo.holdRemoteAnswers({ hold: ["readiness"] })
  const opened = await openOwnedChanges(page)
  await expect.poll(() => ownerChips(opened.window)).toEqual(OWNERSHIP_CHIPS)
  await expect(ownerChip(opened.window, "All")).toHaveAttribute("aria-pressed", "true")
  return opened
}

/** The request ↑ or ↓ shows from the overview, its changes shown */
async function showOtherRequest(window: Page, review: ReturnType<Page["getByTestId"]>, key: "ArrowUp" | "ArrowDown", title: string): Promise<void> {
  await window.keyboard.press("Escape")
  await review.getByTestId("review-dialog-body").focus()
  await window.keyboard.press(key)
  await expect(review.getByRole("heading", { name: title })).toBeVisible()
  await showChanges(window, review)
}

/** A new head of `login` (#7 and #8) with ui/button.css, which the next focus polls and the open window fetches */
async function commitNewHead(page: Page, repo: TestRepositoryBuilder, syncAndWaitForBranches: () => Promise<void>): Promise<void> {
  await repo.commit("(login) Style the button", { "ui/button.css": "button {}\n" }, 1704124800)
  await syncAndWaitForBranches()
  await repo.advanceBackendClock(121_000)
  await emitTauriEvent(page, "tauri://focus")
}

test.describe("Owner chips", () => {
  test("should open on the user's group that needs an approval, then on the chip the user chose, which All forgets", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
    testStore,
  }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    const { window, review } = await openOwnedChanges(page)

    // the groups of the request, the user's first, then the other owners by the OWNERSHIP files
    await expect.poll(() => ownerChips(window)).toEqual(GROUP_CHIPS)
    // the session asked Patronus about the head of #7 the window fetched
    expect((await repo.getReviewChecks()).checks.map(check => check.pullRequest?.number)).toContain(7)
    await expect(ownerChip(window, "Design")).toHaveAttribute("data-yours", "")
    await expect(ownerChip(window, "Auth Team")).toHaveAttribute("data-approved", "")
    await ownerChip(window, "Auth Team").hover()
    await expect(window.getByRole("tooltip", { name: "Approved" })).toBeVisible()
    // nothing remembered: the window opens on Design, its file marked, the others muted
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "ui/login.css")).not.toHaveAttribute("data-muted")
    await expect(diffHeader(window, "README.md")).toHaveAttribute("data-muted", "")

    // a chip the user chooses is remembered for the repository: the next request opens on it
    await ownerChip(window, "UI Team").click()
    await expect(diffHeader(window, "ui/login.css")).toHaveAttribute("data-muted", "")
    await expect.poll(() => testStore.get("recentProjects")).toMatchObject([{ reviewOwnerChip: { kind: "owner", name: "UI Team" } }])
    await showOtherRequest(window, review, "ArrowDown", "Style the login form")
    await expect(ownerChip(window, "UI Team")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "ui/login.js")).not.toHaveAttribute("data-muted")
    await expect(ownerChip(window, "Design")).toHaveAttribute("data-yours", "")

    // All forgets it: the first request opens on the user's group again
    await ownerChip(window, "All").click()
    await expect(diffHeader(window, "README.md")).not.toHaveAttribute("data-muted")
    await expect.poll(async () => ((await testStore.get("recentProjects")) as { reviewOwnerChip?: unknown }[])[0]?.reviewOwnerChip).toBeUndefined()
    await showOtherRequest(window, review, "ArrowUp", "Add the login form")
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
  })

  test("should keep the chip the user chose when the groups of the request come later", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    // Patronus hasn't answered: the owners by the OWNERSHIP files, All chosen
    const { window } = await openOwnedChangesBeforeGroups(page, repo)
    await ownerChip(window, "Monorepo").click()

    // it answers: the groups show, the chip the user chose stays
    await repo.holdRemoteAnswers({ release: ["readiness"] })
    await expect.poll(() => ownerChips(window)).toEqual(GROUP_CHIPS)
    await expect(ownerChip(window, "Monorepo")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "ui/login.css")).toHaveAttribute("data-muted", "")
  })

  test("should make the owner chip chosen or remembered before the groups came the group of its name once they come, one chip the user can choose", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
    testStore,
  }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    const { window, review } = await openOwnedChangesBeforeGroups(page, repo)
    const design = ownerChip(window, "Design")
    const expectOneDesignGroup = async () => {
      // one chip of the name, the group's, with its count: no owner chip "Design0" beside it
      await expect.poll(() => ownerChips(window)).toEqual(GROUP_CHIPS)
      await expect(design).toHaveCount(1)
      await expect(design).toHaveAttribute("data-yours", "")
      await expect(design).toHaveAttribute("aria-pressed", "true")
      await expect(window.locator("[data-testid=branch-review-owner][aria-pressed=true]")).toHaveCount(1)
      await expect(diffHeader(window, "ui/login.css")).not.toHaveAttribute("data-muted")
      await expect(diffHeader(window, "README.md")).toHaveAttribute("data-muted", "")
    }

    // the user chooses the owner Design of the OWNERSHIP files before Patronus answers
    await design.click()
    await expect(design).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => testStore.get("recentProjects")).toMatchObject([{ reviewOwnerChip: { kind: "owner", name: "Design" } }])
    await repo.holdRemoteAnswers({ release: ["readiness"] })
    await expectOneDesignGroup()
    // a press of it keeps it, now the group's
    await design.click()
    await expect.poll(() => testStore.get("recentProjects")).toMatchObject([{ reviewOwnerChip: { kind: "group", name: "Design" } }])
    await expectOneDesignGroup()

    // the next request, its answer held: the remembered group is the owner of its name, then the group again
    await repo.holdRemoteAnswers({ hold: ["readiness"] })
    await showOtherRequest(window, review, "ArrowDown", "Style the login form")
    await expect.poll(() => ownerChips(window)).toEqual(OWNERSHIP_CHIPS)
    await expect(design).toHaveAttribute("aria-pressed", "true")
    await expect(design).not.toHaveAttribute("data-yours")
    await repo.holdRemoteAnswers({ release: ["readiness"] })
    await expectOneDesignGroup()
    // another chip, then the group again: the user reaches it
    await ownerChip(window, "UI Team").click()
    await expect(diffHeader(window, "ui/login.css")).toHaveAttribute("data-muted", "")
    await design.click()
    await expectOneDesignGroup()
  })

  test("should mark by the OWNERSHIP files alone where Patronus doesn't answer, opening on All", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches, { patronus: undefined })
    const { window } = await openOwnedChanges(page)
    await expect.poll(() => ownerChips(window)).toEqual(OWNERSHIP_CHIPS)
    await expect(ownerChip(window, "All")).toHaveAttribute("aria-pressed", "true")
    expect((await repo.getReviewChecks()).checks).toEqual([])
    // an owner chip marks as in the review of a branch
    await ownerChip(window, "Design").click()
    await expect(diffHeader(window, "README.md")).toHaveAttribute("data-muted", "")
    await expect(diffHeader(window, "ui/login.css")).not.toHaveAttribute("data-muted")
  })

  test("should move from All to the user's group when the groups of the request come later, unless the user chose All", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    const { window, review } = await openOwnedChangesBeforeGroups(page, repo)

    // Patronus answers: nothing chosen yet, the window moves to the user's group
    await repo.holdRemoteAnswers({ release: ["readiness"] })
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "README.md")).toHaveAttribute("data-muted", "")

    // the next request, its answer held: All the user chooses stays when the groups come
    await repo.holdRemoteAnswers({ hold: ["readiness"] })
    await showOtherRequest(window, review, "ArrowDown", "Style the login form")
    await expect.poll(() => ownerChips(window)).toEqual(OWNERSHIP_CHIPS)
    await ownerChip(window, "All").click()
    await repo.holdRemoteAnswers({ release: ["readiness"] })
    await expect.poll(() => ownerChips(window)).toEqual(GROUP_CHIPS)
    await expect(ownerChip(window, "All")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "README.md")).not.toHaveAttribute("data-muted")
  })

  test("should open again on the remembered chip when the main window opens the same request again, a chip chosen in another window not moving the one shown", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
    testStore,
  }) => {
    await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    const { window } = await openOwnedChanges(page)
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")

    // the review window of the branch login chooses UI Team: remembered, heard by the window of #7, which keeps Design
    await findBranchRow(page, "login").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(2)
    const branchWindow = await openLinkedSubWindow(page, (await getOpenedSubWindows(page))[1]!)
    await ownerChip(branchWindow, "UI Team").click()
    await expect.poll(() => testStore.get("recentProjects")).toMatchObject([{ reviewOwnerChip: { name: "UI Team" } }])
    await expect.poll(async () => JSON.stringify(await getEmittedEvents(window, "settings_changed"))).toContain("UI Team")
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "ui/login.js")).toHaveAttribute("data-muted", "")

    // the main window opens #7 again: the window opens on the remembered chip
    await getReviewRow(page, 7).getByTestId("review-request-title").click()
    await reopenLinkedSubWindow(window, await openedReviewWindow(page, 3))
    await expect(ownerChip(window, "UI Team")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "ui/login.js")).not.toHaveAttribute("data-muted")
    await expect(diffHeader(window, "ui/login.css")).toHaveAttribute("data-muted", "")
  })

  test("should count the chips by the files of the scope shown, Since Your Review too, and keep the chip on a new head of the request", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    // the user reviewed the first commit of login, and the request came again
    const reviewed = await repo.revision("user-name/virtual/login~1")
    await repo.setReviewRequests(availableReviewRequests([{ ...LOGIN, reRequested: true, reviewedCommit: reviewed }]))
    await repo.advanceBackendClock(121_000)
    await emitTauriEvent(page, "tauri://focus")
    await expect(getReviewRow(page, 8)).toHaveCount(0)
    const { window, review } = await openReviewWindow(page, 7)
    await showChanges(window, review)
    const column = review.getByTestId("range-review-commits").getByTestId("range-review-commit")
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => ownerChips(window)).toEqual(GROUP_CHIPS)

    // Since Your Review: the chips count its files, the chip stays
    await column.filter({ hasText: "Since Your Review" }).click()
    await expect(column.nth(1)).toHaveAttribute("aria-selected", "true")
    // its one commit has the files of Design and Monorepo; Auth Team still needs nothing of it, UI Team has none
    await expect.poll(() => ownerChips(window)).toEqual(["All2", "Design· You1", "Auth Team0", "Monorepo1"])
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")

    // a new head of the request: its files are counted, the chip stays
    await repo.setReviewAnswers({
      answers: {
        "request:7": {
          ...LOGIN_GROUPS,
          groups: [
            LOGIN_GROUPS.groups[0]!,
            evaluatedGroup("Design", false, [codeOwner("Test User", "user-name", { suggested: true, approvalState: "reviewing" })], {
              paths: ["ui/button.css", "ui/login.css"],
              branchWide: false,
            }),
          ],
        },
      },
    })
    await commitNewHead(page, repo, syncAndWaitForBranches)
    await expect.poll(() => ownerChips(window)).toEqual(["All3", "Design· You2", "Auth Team0", "Monorepo1"])
    await expect(column.nth(1)).toHaveAttribute("aria-selected", "true")
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
  })
})

/** Patronus's answer of a request of `login` whose group Design owes `paths` and has `member` */
function loginGroupsWithDesign(paths: string[], member = "user-name") {
  const design = evaluatedGroup("Design", false, [codeOwner("Test User", member, { suggested: true, approvalState: "reviewing" })], { paths, branchWide: false })
  return { ...LOGIN_GROUPS, groups: [LOGIN_GROUPS.groups[0]!, design] }
}

/** What the footer of the review window says */
function reviewPosition(window: Page) {
  return window.getByTestId("review-file-position")
}

/** J or K from the diffs, which have the focus once the changes show: the footer says where it went */
async function pressFileKey(window: Page, key: "j" | "k", position: string): Promise<void> {
  await expect(window.locator("[aria-label='Changes of the commit']")).toBeFocused()
  await window.keyboard.press(key)
  await expect(reviewPosition(window)).toHaveText(position)
}

/** The description of Approve, by its hover */
async function approveDescription(window: Page, review: ReturnType<Page["getByTestId"]>): Promise<string> {
  await review.getByTestId("review-approve").hover()
  const tooltip = window.getByRole("tooltip")
  await expect(tooltip).toBeVisible()
  const text = (await tooltip.textContent()) ?? ""
  await window.mouse.move(0, 0)
  await expect(tooltip).toBeHidden()
  return text
}

test.describe("Owner part and Approve", () => {
  test("should count the chosen owner's part in the footer as J moves, and say in Approve what it counts for and what the user opened", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    await repo.setReviewAnswers({ answers: { "request:7": loginGroupsWithDesign(["ui/login.css", "ui/login.js"]) } })
    const { window, review } = await openOwnedChanges(page)
    await expect.poll(() => ownerChips(window)).toEqual(["All4", "Design· You2", "Auth Team1", "Monorepo1", "UI Team1"])
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")

    // the first file of the tree is Auth Team's: the footer counts Design's files, Approve names the group, none opened
    await expect(reviewPosition(window)).toHaveText("All Changes · File 1 / 4 · Design 2")
    expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 0 of its 2 files")

    // J moves through Design's files: the footer counts the place among them, Approve what the window showed
    await pressFileKey(window, "j", "All Changes · File 2 / 4 · Design 1 / 2")
    await pressFileKey(window, "j", "All Changes · File 3 / 4 · Design 2 / 2")
    expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 2 of its 2 files")
    // All: no owner part
    await ownerChip(window, "All").click()
    await expect(reviewPosition(window)).toHaveText("All Changes · File 3 / 4")

    // the confirmation of Approve and Safe Push says it too; Escape closes it
    await pressDecisionKey(window, review, "m")
    const form = window.getByTestId("review-safe-push-form")
    await expect(form.getByTestId("review-safe-push-scope")).toHaveText("Counts for Design · You opened 2 of its 2 files")
    await window.keyboard.press("Escape")
    await expect(form).toBeHidden()

    // information only: Approve submits
    await pressDecisionKey(window, review, "a")
    await expect(review.getByTestId("review-approve")).toHaveText("Approved")
    expect(await gitHubRequests(repo, "POST", "pulls", 7, "reviews")).toEqual([expect.objectContaining({ event: "APPROVE" })])
  })

  test("should count the opened files again from zero on a new head, and say when the approval counts for no group that needs one", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    // Design of #8 is someone else's
    await repo.setReviewAnswers({
      answers: { "request:7": loginGroupsWithDesign(["ui/login.css", "ui/login.js"]), "request:8": loginGroupsWithDesign(["ui/login.css"], "bob") },
    })
    const { window, review } = await openOwnedChanges(page)
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
    await expect(reviewPosition(window)).toHaveText("All Changes · File 1 / 4 · Design 2")
    await pressFileKey(window, "j", "All Changes · File 2 / 4 · Design 1 / 2")
    await pressFileKey(window, "j", "All Changes · File 3 / 4 · Design 2 / 2")
    expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 2 of its 2 files")

    // a new head with a third file of Design: of what the window showed, only the file on screen counts for it
    await repo.setReviewAnswers({
      answers: { "request:7": loginGroupsWithDesign(["ui/button.css", "ui/login.css", "ui/login.js"]), "request:8": loginGroupsWithDesign(["ui/login.css"], "bob") },
    })
    await commitNewHead(page, repo, syncAndWaitForBranches)
    await expect.poll(() => ownerChips(window)).toEqual(["All5", "Design· You3", "Auth Team1", "Monorepo1", "UI Team1"])
    await expect(reviewPosition(window)).toHaveText("All Changes · File 4 / 5 · Design 3 / 3")
    expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 1 of its 3 files")
    await pressFileKey(window, "k", "All Changes · File 3 / 5 · Design 2 / 3")
    expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 2 of its 3 files")

    // #8: the user is in no group that still needs an approval
    await showOtherRequest(window, review, "ArrowDown", "Style the login form")
    // the groups of #8 came (Auth Team approved), in the order of the check: none is the user's
    await expect(ownerChip(window, "Auth Team")).toHaveAttribute("data-approved", "")
    await expect.poll(() => ownerChips(window)).toEqual(["All5", "Auth Team1", "Design1", "Monorepo1", "UI Team1"])
    expect(await approveDescription(window, review)).toBe("Counts for no group that still needs an approval")

    // back to #7, whose head did not change: what the window showed of it still counts
    await showOtherRequest(window, review, "ArrowUp", "Add the login form")
    await expect(ownerChip(window, "Design")).toHaveAttribute("data-yours", "")
    expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 2 of its 3 files")
  })

  test("should count every file as the part of a branch-wide group, in the footer and in Approve", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    const wide = evaluatedGroup("Design", false, [codeOwner("Test User", "user-name", { suggested: true, approvalState: "reviewing" })], { paths: [], branchWide: true })
    await repo.setReviewAnswers({ answers: { "request:7": { ...LOGIN_GROUPS, groups: [LOGIN_GROUPS.groups[0]!, wide] } } })
    const { window, review } = await openOwnedChanges(page)
    // the window opens on the user's branch-wide group: every file is its, none is muted
    await expect.poll(() => ownerChips(window)).toEqual(["All4", "Design· You4", "Auth Team1", "Monorepo1", "UI Team1"])
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "README.md")).not.toHaveAttribute("data-muted")
    await expect(reviewPosition(window)).toHaveText("All Changes · File 1 / 4 · Design 1 / 4")
    await pressFileKey(window, "j", "All Changes · File 2 / 4 · Design 2 / 4")
    expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 2 of its 4 files")
  })

  test("should say nothing of what Approve counts for while the GitHub sign-in names no login", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // GitHub through the token of gh, which names no login: nobody knows whose the groups are
    await setupOwnedRequests(setupRepo, syncAndWaitForBranches, { gitHubSignIn: { signedIn: false, ghToken: "gh-token" } })
    const { window, review } = await openOwnedChanges(page)
    // the groups came, none the user's: All
    await expect(ownerChip(window, "Auth Team")).toHaveAttribute("data-approved", "")
    await expect.poll(() => ownerChips(window)).toEqual(["All4", "Auth Team1", "Design1", "Monorepo1", "UI Team1"])
    await expect(ownerChip(window, "All")).toHaveAttribute("aria-pressed", "true")
    // the confirmation of Approve and Safe Push has no line of what it counts for, rather than "no group"
    await pressDecisionKey(window, review, "m")
    const form = window.getByTestId("review-safe-push-form")
    await expect(form).toBeVisible()
    await expect(form.getByTestId("review-safe-push-scope")).toHaveCount(0)
    await window.keyboard.press("Escape")
    await expect(form).toBeHidden()
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
