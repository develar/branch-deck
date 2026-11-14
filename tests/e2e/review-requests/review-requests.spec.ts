// @spec openspec/specs/repository/review-requests/spec.md
import { appendFileSync } from "node:fs"
import path from "node:path"
import type { Locator, Page } from "@playwright/test"
import type { ReviewRunRequest } from "../../../src/lib/review-runs"
import type { PullRequestReviewWindowData, PullRequestStackReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { openActivityWindow } from "../helpers/activity-helpers"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, verifyBranchState } from "../helpers/branch-helpers"
import { availableRobots, codeOwner, evaluatedGroup, robot } from "../helpers/patronus-helpers"
import { availablePullRequests, getOpenedUrls, gitHubRequests, pullRequest, repoPath, reviewer } from "../helpers/pull-request-helpers"
import { THEME_ITEM, commitTwoTasks, expectHeadingCard, turnOnReviewPlan } from "../helpers/review-plan-helpers"
import {
  NOW,
  availableReviewRequests,
  focusPastReviewRequestsAge,
  getReviewRow,
  getYourTurnCard,
  openRequestReview,
  pressDecisionKey,
  reviewFetched,
  reviewRequest,
  routeReviewFetch,
} from "../helpers/review-request-helpers"
import { openedReview } from "../helpers/review-window-helpers"
import { expectSettingsOpened, openSettingsWindow } from "../helpers/settings-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { getEmittedEvents, getOpenedSubWindows, openLinkedSubWindow, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent, moveClockUntil, runAndWaitForAnswer } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { expectClosed, openTooltip } from "../helpers/wait-helpers"

// the `issue_links` template: 7 branches; the requests review them as the branches of others
const OLD = reviewRequest("ABC-123", 101, 50, { title: "Rework the session store", checksState: "failing" })
const RECENT = reviewRequest("feature-auth", 102, 3, { title: "Add OAuth login", reRequested: true, reviewedCommit: null })
const RELEASE = reviewRequest("JIRA-456", 103, 2, { title: "Backport the pool fix", baseRefName: "release/253", baseIsDefault: false, checksState: "pending" })
const FOURTH = reviewRequest("feature-docs", 104, 1, { title: "Document the CLI" })
// asked 5 hours ago, the user commented, carol replied half an hour ago: the turn is the user's again
const FIFTH = reviewRequest("feature-db", 105, 0.5, { title: "Migrate the schema", requestedAt: NOW - 5 * 3600, backToYou: { at: NOW - 1800, repliedBy: "carol" } })
// GitHub's body as its author wrote it: the hint of its template, its headings, a table with a column aligned right
const DESCRIPTION = [
  "<!-- What does this change do? -->",
  "## Goal",
  "Basically: ability to play video in chat.",
  "",
  "## Sizes",
  "",
  "| Module | Installed size |",
  "| --- | ---: |",
  "| `view.gui.media` | 6.4 MB |",
  "| **core** | 12 KB |",
].join("\n")

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

    // a rejected token: no poll, a focus two minutes after the answer asks again; GitHub lists nothing, so no card
    await repo.setReviewRequests(availableReviewRequests([]))
    await focusPastReviewRequestsAge(page, repo, async () => (await repo.getReviewRequestCalls()) >= 2)
    await expect(getYourTurnCard(page)).toHaveCount(0)

    // a request: the card shows with it, once a focus asked it
    await repo.setReviewRequests(availableReviewRequests([OLD]))
    await focusPastReviewRequestsAge(page, repo, () => getReviewRow(page, 101).isVisible())
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
    await moveClockUntil(repo, 300_000, async () => (await card.getByTestId("review-request-row").count()) === 2)
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
    // the menu gives the focus back as it unmounts, after its exit
    await expectClosed(menu)
    await expect(getReviewRow(page, 102)).toBeFocused()
    // Enter: the review window of the row; the main window keeps the focus on the row
    await page.keyboard.press("Enter")
    const opened = await openedReview<PullRequestReviewWindowData>(page, 1)
    expect(opened).toMatchObject({ title: "Review #102 Add OAuth login", data: { number: 102, index: 1, intent: null } })
    await expect(getReviewRow(page, 102)).toBeFocused()
  })

  test("should show a chain of requests as one stack, its layers under it by a click and the keys, each with its labels", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // three layers, each into the branch of the one below, the bottom into master
    const author = { author: "niyaz", baseIsDefault: false }
    const bottom = reviewRequest("feature-docs", 201, 6, { title: "Record the session id", author: "niyaz", labels: ["refactoring"] })
    const middle = reviewRequest("feature-auth", 202, 5, {
      ...author,
      title: "Drop workTreeRoot",
      baseRefName: bottom.headRefName,
      labels: ["refactoring", "proxy", "cleanup", "tests"],
    })
    const top = reviewRequest("JIRA-456", 203, 4, { ...author, title: "Count the recovered parts", baseRefName: middle.headRefName })
    await setupRequests(page, setupRepo, syncAndWaitForBranches, top, OLD, middle, bottom)
    const card = getYourTurnCard(page)
    const rows = card.getByTestId("review-request-row")
    const stack = card.getByTestId("review-stack-row")
    await expect(card.getByTestId("your-turn-count")).toHaveText("4")
    // one row of the card, at the place of its layer that waits the longest; collapsed, its layers are not rows
    await expect(stack.getByTestId("review-stack-title")).toHaveText("Stack of 3 pull requests")
    await expect(stack.getByTestId("review-stack-meta")).toHaveText("#201–#203 · niyaz · +36 −9")
    await expect(stack.getByTestId("review-request-age")).toHaveText("waiting 6 hours")
    await expect(rows.getByTestId("review-request-title")).toHaveText(["Rework the session store"])
    await expect(card.getByTestId("review-requests-show-more")).toHaveCount(0)

    // a click shows the layers, the bottom first, under the header; a layer's base is the layer below, which it doesn't repeat
    await stack.click()
    await expect(stack).toHaveAttribute("aria-expanded", "true")
    await expect(rows.getByTestId("review-request-title")).toHaveText(["Rework the session store", "Record the session id", "Drop workTreeRoot", "Count the recovered parts"])
    await expect(getReviewRow(page, 202).getByTestId("review-request-meta")).toHaveText("#202 · niyaz · requested by dave · 2 files · +12 −3")
    // the labels of GitHub, the first two and the count of the rest
    await expect(getReviewRow(page, 201).getByTestId("review-request-label")).toHaveText(["refactoring"])
    await expect(getReviewRow(page, 202).getByTestId("review-request-labels")).toHaveText("refactoringproxy+2")
    await expect(getReviewRow(page, 203).getByTestId("review-request-labels")).toHaveCount(0)

    // ← hides the layers, → shows them, Enter toggles; ↓ walks from the header into its layers
    await stack.focus()
    await page.keyboard.press("ArrowLeft")
    await expect(stack).toHaveAttribute("aria-expanded", "false")
    await expect(rows).toHaveCount(1)
    await page.keyboard.press("ArrowRight")
    await expect(rows).toHaveCount(4)
    await page.keyboard.press("Enter")
    await expect(stack).toHaveAttribute("aria-expanded", "false")
    await page.keyboard.press("Enter")
    await expect(stack).toHaveAttribute("aria-expanded", "true")
    await page.keyboard.press("ArrowUp")
    await expect(getReviewRow(page, 101)).toBeFocused()
    await page.keyboard.press("ArrowDown")
    await page.keyboard.press("ArrowDown")
    await expect(getReviewRow(page, 201)).toBeFocused()
    // a layer is a request: Enter reviews it
    await page.keyboard.press("Enter")
    expect((await openedReview<PullRequestReviewWindowData>(page, 1)).data.number).toBe(201)
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
    await expect.poll(async () => (await getOpenedSubWindows(page)).map(window => [window.windowId, window.title])).toEqual([["review", "Review JIRA-456"]])
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
    const toast = reviewToast(page, "dave requested your review")
    await moveClockUntil(repo, 300_000, () => toast.isVisible())
    await expect(toast).toBeVisible()
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("#104 Document the CLI")
    // the unread row is shown above the cap, in the place of a read one
    const row = getReviewRow(page, 104)
    await expect(row.getByTestId("review-request-unread")).toBeVisible()
    await expect(getYourTurnCard(page).getByTestId("review-request-row")).toHaveCount(3)

    // Review: the review window of the request, read now
    await toast.getByRole("button", { name: "Review" }).click()
    expect((await openedReview<PullRequestReviewWindowData>(page, 1)).data).toMatchObject({ number: 104, intent: null })
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
    } = await openRequestReview(page, 101, {
      beforeLoad: async window => {
        fetches = await routeReviewFetch(window, { progress: [42], release: new Promise<void>(resolve => (release = resolve)) })
      },
    })
    expect(opened).toMatchObject({
      title: "Review #101 Rework the session store",
      windowId: "review",
      data: { kind: "pullRequest", number: 101, index: 0, intent: null, source: "yourTurn" },
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
    // O: the window opens it on GitHub, and the main window marks it read
    await reviewPage.keyboard.press("o")
    await expect.poll(() => getOpenedUrls(reviewPage)).toEqual(["https://github.com/acme/monorepo/pull/101"])
    // Escape in the overview closes the window
    await reviewPage.keyboard.press("Escape")
    await expect.poll(() => windowCalls(reviewPage)).toContain("close")
  })

  test("should show the description of the pull request as its Markdown: its headings and its tables", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const described = reviewRequest("ABC-123", 101, 50, { title: "Play video in the chat", body: DESCRIPTION })
    await setupRequests(page, setupRepo, syncAndWaitForBranches, described)
    const { review } = await openRequestReview(page, 101)
    const body = review.getByTestId("review-dialog-body").getByTestId("commit-message-body")
    await expect(body.locator("[data-slot=commit-body-heading]")).toHaveText(["Goal", "Sizes"])
    await expect(body).toContainText("ability to play video in chat.")
    await expect(body).not.toContainText("What does this change do?")
    const table = body.getByRole("table")
    await expect(table.getByRole("columnheader")).toHaveText(["Module", "Installed size"])
    // the cells by their words: the code of a cell is code, its strong emphasis no marks
    await expect(table.getByRole("cell")).toHaveText(["view.gui.media", "6.4 MB", "core", "12 KB"])
    await expect(table.getByRole("cell", { name: "6.4 MB" })).toHaveCSS("text-align", "right")
    await captureHtmlSnapshot(body, "review-request-markdown")
  })

  test("should review a commit of the pull request in the changes, step through the commits, and show the next request with ↓", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    const { window: reviewPage, review } = await openRequestReview(page, 101)
    // the rail lists the commits newest first, as the column of the changes
    const commits = review.getByTestId("review-commit")
    await expect(commits.first()).toBeVisible()
    const subjects = await commits.getByTestId("review-commit-subject").allTextContents()
    await commits.first().click()
    const changes = review.getByTestId("commit-review")
    await expect(changes).toBeVisible()
    await expectClosed(reviewPage.getByTestId("commit-preview"))
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
    expect((await getEmittedEvents<ReviewRunRequest>(reviewPage, "review/run")).at(-1)).toMatchObject({ subject: "pull:102", itemId: "read", request: { number: 102 } })
    // the ↑ of the title bar shows the one before
    await review.getByTestId("review-previous").click()
    await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
  })

  test(
    "should go back to the pull request with ⌘[ after the review of a branch in the same window, and fetch it again",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
      let fetches: Record<string, unknown>[] = []
      const { window: reviewPage, review } = await openRequestReview(page, 101, {
        beforeLoad: async window => {
          fetches = await routeReviewFetch(window, { progress: [], release: Promise.resolve() })
        },
      })
      await expect(review.getByTestId("review-commit").first()).toBeVisible()
      expect(fetches).toHaveLength(1)

      // the review of a branch of the main window: the same window, the pull request in its history
      await findBranchRow(page, "JIRA-456").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Review Changes" }).click()
      await reopenLinkedSubWindow(reviewPage, await openedReview(page, 2))
      await expect(reviewPage.getByTestId("branch-review-segment")).toHaveCount(1)

      // Back: the pull request again, fetched again
      const mac = await reviewPage.evaluate(() => navigator.userAgent.toLowerCase().includes("mac"))
      await reviewPage.keyboard.press(mac ? "Meta+BracketLeft" : "Control+BracketLeft")
      await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
      await expect(review.getByTestId("review-commit").first()).toBeVisible()
      expect(fetches).toHaveLength(2)
      await expect(reviewPage.getByTestId("review-back")).toHaveCount(0)
    },
  )

  test("should review the changes of a row with D", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    await getReviewRow(page, 101).focus()
    await page.keyboard.press("d")
    const opened = await openedReview<PullRequestReviewWindowData>(page, 1)
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
    // the plan held in every window: while it is read the overview says so, never a document that looks whole and then
    // moves as the plan comes
    let release!: () => void
    const held = new Promise<void>(resolve => {
      release = resolve
    })
    await page.context().route("**/invoke/get_review_plan", async route => {
      await held
      await route.fallback()
    })
    const { review } = await openPlanChanges(page, setupRepo, syncAndWaitForBranches)
    await expect(review.getByTestId("commit-review")).toBeVisible()
    await expect(review.getByTestId("review-overview").getByTestId("review-overview-reading")).toHaveText("Reading the review plan…")
    await expect(review.getByTestId("review-step-bar")).toHaveCount(0)
    release()
    // the plan of what the window shows: its overview at the top of the document, Rust's words
    await expect(review.getByTestId("review-overview")).toBeVisible()
    await expect(review.getByTestId("review-overview-reading")).toHaveCount(0)

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

  test(
    "should give the heading of the commit of a pull request of one commit the mark of its card, which the hover and I open, Escape giving the focus back",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("review_tour", { reviewRequests: availableReviewRequests([reviewRequest("story", 103, 30)]) })
      turnOnReviewPlan(repo)
      await commitTwoTasks(repo)
      await syncAndWaitForBranches()
      // D on the row reviews the changes: one commit, no column, its heading above the tasks of its tree
      await getReviewRow(page, 103).focus()
      await page.keyboard.press("d")
      const reviewPage = await openLinkedSubWindow(page, await openedReview<PullRequestReviewWindowData>(page, 1))
      const review = reviewPage.getByTestId("review-window")
      await expect(review.getByTestId("range-review-commits")).toHaveCount(0)
      const header = review.getByTestId("range-review-commit-header")
      await expect(header).toContainText("the cold start paints a splash in the accent of the theme")
      await expectHeadingCard(reviewPage, header, THEME_ITEM.replaceAll("`", ""))
    },
  )

  test("should lead the changes of the pull request with the summary of its description, whose Description shows it whole in the overview", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const described = reviewRequest("ABC-123", 101, 50, { title: "Play video in the chat", body: DESCRIPTION })
    // the next request: the same commits, the description of the template
    const again = reviewRequest("ABC-123", 106, 1, { title: "Keep the session store" })
    const { reviewPage, review } = await openPlanChanges(page, setupRepo, syncAndWaitForBranches, described, again)
    // the changes: the overview of the plan opens on the summary of the description, its title is the island's alone
    const lead = review.getByTestId("review-overview").getByTestId("review-overview-lead")
    await expect(lead.getByTestId("commit-message-body")).toHaveText("ability to play video in chat.")
    await expect(lead.locator("[data-slot=commit-body-heading]")).toHaveCount(0)
    await expect(lead.getByRole("table")).toHaveCount(0)
    await expect(review.getByTestId("review-overview-headline")).toHaveCount(0)

    // Description: the overview, the description whole in view, no document of the plan beside it
    await lead.getByRole("button", { name: "Description" }).click()
    await expect(review.getByTestId("commit-review")).toBeHidden()
    await expect(review.getByTestId("review-tab-overview")).toHaveAttribute("data-active")
    const description = review.getByTestId("review-description")
    await expect(description).toBeInViewport()
    await expect(description.locator("[data-slot=commit-body-heading]")).toHaveText(["Goal", "Sizes"])
    await expect(description.getByRole("table").getByRole("cell")).toHaveText(["view.gui.media", "6.4 MB", "core", "12 KB"])
    await expect(review.getByTestId("review-overview")).toHaveCount(0)
    // the commits of the rail are commit rows, by their subjects
    await expect(review.getByTestId("review-commits").getByTestId("review-commit-subject")).toHaveCount(4)

    // a description of its summary alone leads the changes whole, with no link to the same words
    await reviewPage.keyboard.press("ArrowDown")
    await expect(review.getByRole("heading", { name: "Keep the session store" })).toBeVisible()
    await expect(review.getByTestId("review-dialog-review")).toBeEnabled()
    await reviewPage.keyboard.press("d")
    const short = review.getByTestId("review-overview").getByTestId("review-overview-lead")
    await expect(short.getByTestId("commit-message-body")).toHaveText("The change of ABC-123.")
    await expect(short.getByRole("button", { name: "Description" })).toHaveCount(0)
  })

  test("should list the shape of the plan in the rail of the overview, a module showing its first file in the changes", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the branch `media` of the `review_modules` template as a pull request: a new module, two changed ones
    const repo = await setupRepo("review_modules", { reviewRequests: availableReviewRequests([reviewRequest("media", 301, 1, { title: "Play the media embeds inline" })]) })
    turnOnReviewPlan(repo)
    await syncAndWaitForBranches()
    const { window, review } = await openRequestReview(page, 301)
    // the rows of the map in place of the files: the new module first, its badge leading the line under it in the rail
    const shape = review.getByTestId("review-shape")
    await expect(shape.getByTestId("review-map-label")).toHaveText(["view.gui.media", "view.gui", "shared/code"])
    await expect(review.getByTestId("review-files")).toHaveCount(0)
    const media = shape.getByTestId("review-map-module").filter({ has: window.getByTestId("review-map-label").getByText("view.gui.media", { exact: true }) })
    await expect(media.getByTestId("review-map-badge")).toBeHidden()
    await expect(media.getByTestId("review-map-detail-badge")).toHaveText("New module")
    await expect(media.getByTestId("review-map-detail")).toContainText("uses view.gui and shared/code")
    await captureHtmlSnapshot(shape, "review-request-shape")

    // a row: all changes at the first file of the module in the story
    await media.getByRole("button").click()
    await expect(review.getByTestId("commit-review")).toBeVisible()
    const tree = window.getByRole("tree", { name: "Changed files" })
    await expect(tree.getByRole("treeitem", { name: MEDIA_FIRST_FILE, exact: true })).toHaveAttribute("aria-selected", "true")
    await expect(review.getByTestId("range-review-commits").getByTestId("range-review-commit").first()).toHaveAttribute("aria-selected", "true")
    // the map heads the changes with the same rows, each with its bar
    await expect(review.getByTestId("review-overview").getByTestId("review-map-label")).toHaveText(["view.gui.media", "view.gui", "shared/code"])
    await expect(review.getByTestId("review-overview").getByTestId("review-map-badge")).toHaveText("New module")
  })

  test("should list the files of the pull request in the rail without a plan, a name whole before its directory, which gives way", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("issue_links", { reviewRequests: availableReviewRequests([OLD]) })
    // a directory that gives way to its last folder, and one whose last folder does not fit after its name
    await repo.commit(
      "(ABC-123) the player of the session",
      {
        [`${LONG_DIRECTORY}/media/Bridge.kt`]: "package media\n",
        [`${LONG_DIRECTORY}/avfoundation/AVFoundationPlayerBridge.kt`]: "package avfoundation\n",
      },
      1704124800,
    )
    await syncAndWaitForBranches()
    await expect(getYourTurnCard(page)).toBeVisible()
    const { review } = await openRequestReview(page, 101)
    const rows = review.getByTestId("review-files").getByTestId("review-file")
    await expect(rows.first()).toBeVisible()

    // the name whole, the directory right after it on its line, cut at its start
    const bridge = rows.filter({ hasText: "Bridge.kt" }).filter({ hasNotText: "AVFoundation" })
    const name = await box(bridge.getByTestId("file-path-label-name"))
    const directory = await box(bridge.getByTestId("file-label-directory"))
    expect(name.x + name.width).toBeLessThanOrEqual(directory.x)
    expect(Math.abs(directory.y - name.y)).toBeLessThan(name.height / 2)
    await expect(bridge.getByTestId("file-label-directory")).toContainText("media")
    expect(await isCut(bridge.getByTestId("file-label-directory"))).toBe(true)
    expect(await isCut(bridge.getByTestId("file-path-label-name-start"))).toBe(false)

    // a name that leaves no room for the last folder: the directory wraps out of sight, under the name, never over it
    const player = rows.filter({ hasText: "AVFoundationPlayerBridge.kt" })
    const playerName = await box(player.getByTestId("file-path-label-name"))
    const playerDirectory = await box(player.getByTestId("file-label-directory"))
    expect(playerDirectory.y).toBeGreaterThanOrEqual(playerName.y + playerName.height - 1)
    expect(await isCut(player.getByTestId("file-path-label-name-start"))).toBe(false)
    await expect(player).toHaveAttribute("title", `${LONG_DIRECTORY}/avfoundation/AVFoundationPlayerBridge.kt`)
  })

  test(
    "should name a commit alike in the rail, the commit column and the tree, each opening its card by the hover of its subject and by I",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // the branch `layout` of the `review_tour` template as a pull request, its plan on
      const repo = await setupRepo("review_tour", { reviewRequests: availableReviewRequests([reviewRequest("layout", 201, 1, { title: "Keep the stored layout" })]) })
      turnOnReviewPlan(repo)
      await syncAndWaitForBranches()
      const { window, review } = await openRequestReview(page, 201)
      const tree = window.getByRole("tree", { name: "Changed files" })
      await expect(review.getByTestId("review-dialog-review")).toBeEnabled()
      await window.keyboard.press("d")
      await expect(review.getByTestId("commit-review")).toBeVisible()

      // one commit is named alike in the rail, the commit column and the tree: the title Rust gives it
      const title = "the rule names its flag again"
      await expect(tree.getByRole("treeitem", { name: title, exact: true })).toBeVisible()
      await window.keyboard.press("Escape")
      const rail = review.getByTestId("review-commits").getByTestId("review-commit")
      await expect(rail.getByTestId("review-commit-subject")).toHaveText([title, ...(await rail.getByTestId("review-commit-subject").allTextContents()).slice(1)])
      // the commit of the rail opens its card on the hover of its subject and by I, the focus in it; Escape gives it back
      const card = window.getByTestId("commit-card")
      const railRow = rail.filter({ hasText: title })
      await window.mouse.move(0, 0)
      await railRow.getByTestId("review-commit-subject").hover()
      await expect(card).toContainText("The review asked for the name of the flag to say what it holds.")
      await window.mouse.move(0, 0)
      await expectClosed(card)
      await railRow.focus()
      await window.keyboard.press("i")
      await expect(card).toBeFocused()
      await window.keyboard.press("Escape")
      await expectClosed(card)
      await expect(railRow).toBeFocused()

      await railRow.click()
      const column = window.getByTestId("range-review-commits").getByTestId("range-review-commit")
      await expect(column.filter({ hasText: title })).toHaveAttribute("aria-selected", "true")
      await expect(column.filter({ hasText: title }).getByText(title, { exact: true })).toBeVisible()
      // so does the commit of the column: the hover of its subject, I on the column, Escape back to the column
      await window.mouse.move(0, 0)
      await column.filter({ hasText: title }).getByText(title, { exact: true }).hover()
      await expect(card).toContainText("The review asked for the name of the flag to say what it holds.")
      await window.mouse.move(0, 0)
      await expectClosed(card)
      const listbox = window.getByTestId("range-review-commits")
      await listbox.focus()
      await window.keyboard.press("i")
      await expect(card).toBeFocused()
      await window.keyboard.press("Escape")
      await expectClosed(card)
      await expect(listbox).toBeFocused()
    },
  )

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
    await focusPastReviewRequestsAge(page, repo, () => getReviewRow(page, 101).isVisible())
    await expect(getReviewRow(page, 101)).toBeVisible()
    await getReviewRow(page, 101).focus()
    await page.keyboard.press("d")
    const review = (await openLinkedSubWindow(page, await openedReview<PullRequestReviewWindowData>(page, 1))).getByTestId("review-window")
    const tree = review.getByRole("tree", { name: "Changed files" })
    const commitSubjects = tree.getByTestId("review-commit-subject")
    const column = review.getByTestId("range-review-commits").getByTestId("range-review-commit")

    // All Changes: every commit in the tree, the one of the review says so, as the boundary of the column did
    await expect(column).toHaveText(["All Changes4 commits", "Since Your Review2 commits"])
    await expect(commitSubjects).toHaveCount(4)
    const reviewedRow = tree.getByRole("treeitem", { name: "Add password hashing utility", exact: true })
    await expect(reviewedRow.getByTestId("review-commit-review")).toHaveText("Your review")
    await expect(reviewedRow).toHaveAttribute("aria-description", /Your review/)
    await expect(tree.getByTestId("review-commit-review")).toHaveCount(1)

    // Since Your Review: its own plan, only the two commits after the review in its tree, none the user saw
    await column.filter({ hasText: "Since Your Review" }).click()
    await expect(column.nth(1)).toHaveAttribute("aria-selected", "true")
    await expect.poll(async () => (await commitSubjects.allTextContents()).toSorted()).toEqual(["Add authentication tests", "Add session management"])
    await expect(column).toHaveText(["All Changes4 commits", "Since Your Review2 commits"])
    await expect(tree.getByTestId("review-commit-review")).toHaveCount(0)
  })

  test("should say why a fetch failed and fetch again on Retry", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD)
    await repo.setReviewRequests(availableReviewRequests([OLD]), "GitHub asked for credentials: store HTTPS credentials of github.com in git (gh auth setup-git does)")
    const { review } = await openRequestReview(page, 101)
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
    const { window: reviewPage, review } = await openRequestReview(page, 101)
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
    // Open Pull Request of the toast: the window opens it
    await reviewToast(reviewPage, "Approved #101").getByRole("button", { name: "Open Pull Request" }).click()
    await expect.poll(() => getOpenedUrls(reviewPage)).toEqual(["https://github.com/acme/monorepo/pull/101"])

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

  test("should keep the window while a write runs, and tell its result in the window, which runs it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, OLD, RECENT)
    // GitHub holds the review: the write runs
    await repo.holdRemoteAnswers({ hold: ["submitReview"] })
    const { window: reviewPage, review } = await openRequestReview(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await pressDecisionKey(reviewPage, review, "a")
    await expect(review.getByTestId("review-approve")).toBeDisabled()
    // Escape keeps the window while GitHub has not answered
    await reviewPage.keyboard.press("Escape")
    expect(await windowCalls(reviewPage)).not.toContain("close")

    // the window runs the approval in its own session: it says how it went, nothing asked of the main window
    await repo.arrival("submitReview")
    await repo.holdRemoteAnswers({ release: ["submitReview"] })
    await expect(reviewToast(reviewPage, "Approved #101")).toBeVisible()
    expect(await gitHubRequests(repo, "POST", "pulls", 101, "reviews")).toEqual([expect.objectContaining({ event: "APPROVE" })])
    expect((await getEmittedEvents<ReviewRunRequest>(reviewPage, "review/run")).filter(it => it.itemId !== "read")).toEqual([])
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
    const opened = await openedReview<PullRequestReviewWindowData>(page, 1)
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
    await expectClosed(form)
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
    await expectClosed(form)
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
    const { window: reviewPage, review } = await openRequestReview(page, 101)
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
    await expect(openTooltip(page)).toHaveText("#103 merges into release/253: Patronus would merge it there, not into master")
    await page.keyboard.press("Escape")
    await getReviewRow(page, 101).click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: /^Approve and Safe Push…/ })).not.toHaveAttribute("aria-disabled", "true")
    await page.keyboard.press("Escape")

    const { window: reviewPage, review } = await openRequestReview(page, 101)
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
    await moveClockUntil(repo, 3000, async () => (await reviewPage.locator("[data-slot=toast]", { hasText: "Safe Push started" }).count()) === 1)
    // the approval, the head GitHub shows, then the plain command: Patronus deletes the branch of a reviewed pull request
    expect(await gitHubRequests(repo, "POST", "pulls", 101, "reviews")).toEqual([expect.objectContaining({ event: "APPROVE" })])
    expect(await gitHubRequests(repo, "GET", "pulls", 101)).toHaveLength(1)
    expect(await gitHubRequests(repo, "POST", "issues", 101, "comments")).toEqual([{ body: "/patronus merge" }])
    await expectClosed(form)
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

    const { window: reviewPage, review } = await openRequestReview(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await pressDecisionKey(reviewPage, review, "m")
    // the main window runs the start the review window asks for (askReviewRun)
    await runAndWaitForAnswer(page, "execute", () => reviewPage.getByTestId("review-safe-push-form").getByTestId("review-safe-push-start").click())
    await expect(reviewPage.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()

    // the header of the main window lists it by the pull request, without a row to show
    await page.getByTestId("landing-indicator").hover()
    const row = page.getByTestId("landing-popover").getByTestId("landing-running").getByTestId("robots-row")
    await expect(row).toHaveAttribute("data-branch-name", "#101 Rework the session store")
    await expect(row.getByRole("button", { name: "Open Pull Request #101 of #101 Rework the session store" })).toBeVisible()

    // the session polls it while it runs, every minute of its clock: its merge is told by what it tested
    await repo.setPatronus(availableRobots({ ...started, status: "success", finishTime: 1704189700 }))
    const merged = page.locator("[data-slot=toast]", { hasText: "#101 Rework the session store merged into master" })
    await moveClockUntil(repo, 60_000, async () => (await merged.count()) === 1)
    expect((await repo.getPatronusLog()).pollCalls).toBeGreaterThan(0)
  })

  test("should not Safe Push a pull request that got commits since the user reviewed it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupSafePush(page, setupRepo, syncAndWaitForBranches, [OLD], "f".repeat(40))
    // from the row's menu: the window opens with the confirmation once the pull request is fetched
    await getReviewRow(page, 101).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Approve and Safe Push…" }).click()
    const opened = await openedReview<PullRequestReviewWindowData>(page, 1)
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
    const { review } = await openRequestReview(page, 101)
    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    // the session prunes the review refs of every pull request but those that wait, an earlier start's too; the window of
    // #101 keeps the same
    await expect.poll(async () => (await repo.getReviewRequestsFixture()).prunes).toEqual([[101, 102]])
    await repo.setReviewRequests(availableReviewRequests([RECENT]))
    await moveClockUntil(repo, 300_000, () => review.getByTestId("review-withdrawn").isVisible())
    await expect(review.getByTestId("review-withdrawn")).toHaveText("No longer waiting for your review")
    await expect(review.getByRole("heading", { name: "Rework the session store" })).toBeVisible()
    await expect(review.getByTestId("review-dialog-position")).toHaveCount(0)
    // the window fetched it and still shows it: no listing prunes its refs
    const fixture = await repo.getReviewRequestsFixture()
    expect(fixture.fetchCalls).toBe(1)
    expect(fixture.prunes).toEqual([[101, 102]])
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
  const { window, review } = await openRequestReview(page, 7)
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

/**
 * A new head of `login` (#7 and #8) with ui/button.css, which the next focus polls and the open window fetches: the
 * focus two minutes past the last answer, until the session listed the requests again (`focusPastReviewRequestsAge`)
 */
async function commitNewHead(page: Page, repo: TestRepositoryBuilder, syncAndWaitForBranches: () => Promise<void>): Promise<void> {
  await repo.commit("(login) Style the button", { "ui/button.css": "button {}\n" }, 1704124800)
  await syncAndWaitForBranches()
  const listed = await repo.getReviewRequestCalls()
  await focusPastReviewRequestsAge(page, repo, async () => (await repo.getReviewRequestCalls()) > listed)
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
    await expect(openTooltip(window, { name: "Approved" })).toBeVisible()
    // nothing remembered: the window opens on Design and shows only its file
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "ui/login.css")).toBeVisible()
    await expect(diffHeader(window, "README.md")).toHaveCount(0)

    // a chip the user chooses is remembered for the repository: the next request opens on it
    await ownerChip(window, "UI Team").click()
    await expect(diffHeader(window, "ui/login.css")).toHaveCount(0)
    await expect.poll(() => testStore.get("recentProjects")).toMatchObject([{ reviewOwnerChip: { kind: "owner", name: "UI Team" } }])
    await showOtherRequest(window, review, "ArrowDown", "Style the login form")
    await expect(ownerChip(window, "UI Team")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "ui/login.js")).toBeVisible()
    await expect(diffHeader(window, "ui/login.css")).toHaveCount(0)
    await expect(ownerChip(window, "Design")).toHaveAttribute("data-yours", "")

    // All forgets it: the first request opens on the user's group again
    await ownerChip(window, "All").click()
    await expect(diffHeader(window, "README.md")).toBeVisible()
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
    await expect(diffHeader(window, "ui/login.css")).toHaveCount(0)
    await expect(diffHeader(window, "README.md")).toBeVisible()
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
      await expect(diffHeader(window, "ui/login.css")).toBeVisible()
      await expect(diffHeader(window, "README.md")).toHaveCount(0)
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
    await expect(diffHeader(window, "ui/login.css")).toHaveCount(0)
    await design.click()
    await expectOneDesignGroup()
  })

  test("should mark by the OWNERSHIP files alone where Patronus doesn't answer, opening on All", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches, { patronus: undefined })
    const { window } = await openOwnedChanges(page)
    await expect.poll(() => ownerChips(window)).toEqual(OWNERSHIP_CHIPS)
    await expect(ownerChip(window, "All")).toHaveAttribute("aria-pressed", "true")
    expect((await repo.getReviewChecks()).checks).toEqual([])
    // an owner chip filters as in the review of a branch
    await ownerChip(window, "Design").click()
    await expect(diffHeader(window, "README.md")).toHaveCount(0)
    await expect(diffHeader(window, "ui/login.css")).toBeVisible()
  })

  test("should move from All to the user's group when the groups of the request come later, unless the user chose All", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    const { window, review } = await openOwnedChangesBeforeGroups(page, repo)

    // Patronus answers: nothing chosen yet, the window moves to the user's group
    await repo.holdRemoteAnswers({ release: ["readiness"] })
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "README.md")).toHaveCount(0)
    await expect(diffHeader(window, "ui/login.css")).toBeVisible()

    // the next request, its answer held: All the user chooses stays when the groups come
    await repo.holdRemoteAnswers({ hold: ["readiness"] })
    await showOtherRequest(window, review, "ArrowDown", "Style the login form")
    await expect.poll(() => ownerChips(window)).toEqual(OWNERSHIP_CHIPS)
    await ownerChip(window, "All").click()
    await repo.holdRemoteAnswers({ release: ["readiness"] })
    await expect.poll(() => ownerChips(window)).toEqual(GROUP_CHIPS)
    await expect(ownerChip(window, "All")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "README.md")).toBeVisible()
  })

  test("should open again on the remembered chip when the main window opens the same request again, a chip chosen in the review of a branch in between", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
    testStore,
  }) => {
    await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    const { window, review } = await openOwnedChanges(page)
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")

    // the review of the branch login, in the same window, chooses UI Team: remembered
    await findBranchRow(page, "login").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    await reopenLinkedSubWindow(window, await openedReview(page, 2))
    await ownerChip(window, "UI Team").click()
    await expect.poll(() => testStore.get("recentProjects")).toMatchObject([{ reviewOwnerChip: { name: "UI Team" } }])
    // the writer's answer reached the window: its copy of the settings has the chip
    await expect.poll(async () => JSON.stringify(await getEmittedEvents(window, "settings_changed"))).toContain("UI Team")

    // the main window opens #7 again: the window opens on the remembered chip
    await getReviewRow(page, 7).getByTestId("review-request-title").click()
    await reopenLinkedSubWindow(window, await openedReview<PullRequestReviewWindowData>(page, 3))
    await showChanges(window, review)
    await expect(ownerChip(window, "UI Team")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "ui/login.js")).toBeVisible()
    await expect(diffHeader(window, "ui/login.css")).toHaveCount(0)
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
    const { window, review } = await openRequestReview(page, 7)
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
    // the window fetches the new head and reads its groups, however long that takes
    await expect.poll(() => ownerChips(window), { timeout: 0 }).toEqual(["All3", "Design· You2", "Auth Team0", "Monorepo1"])
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
  const tooltip = openTooltip(window)
  await expect(tooltip).toBeVisible()
  const text = (await tooltip.textContent()) ?? ""
  await window.mouse.move(0, 0)
  await expectClosed(tooltip)
  return text
}

test.describe("Owner part and Approve", () => {
  test(
    "should show owner-filtered file positions and say in Approve what it counts for and what the user opened",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
      await repo.setReviewAnswers({ answers: { "request:7": loginGroupsWithDesign(["ui/login.css", "ui/login.js"]) } })
      const { window, review } = await openOwnedChanges(page)
      await expect.poll(() => ownerChips(window)).toEqual(["All4", "Design· You2", "Auth Team1", "Monorepo1", "UI Team1"])
      await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")

      // the chip filters the files to Design: the first visible file has already been opened
      await expect(reviewPosition(window)).toHaveText("All Changes · File 1 / 2 · Design")
      expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 1 of its 2 files")

      // J moves through Design's filtered files; Approve counts what the window showed
      await pressFileKey(window, "j", "All Changes · File 2 / 2 · Design")
      await pressFileKey(window, "j", "All Changes · File 2 / 2 · Design")
      expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 2 of its 2 files")
      // All: no owner part
      await ownerChip(window, "All").click()
      await expect(reviewPosition(window)).toHaveText("All Changes · File 3 / 4")

      // the confirmation of Approve and Safe Push says it too; Escape closes it
      await pressDecisionKey(window, review, "m")
      const form = window.getByTestId("review-safe-push-form")
      await expect(form.getByTestId("review-safe-push-scope")).toHaveText("Counts for Design · You opened 2 of its 2 files")
      await window.keyboard.press("Escape")
      await expectClosed(form)

      // information only: Approve submits
      await pressDecisionKey(window, review, "a")
      await expect(review.getByTestId("review-approve")).toHaveText("Approved")
      expect(await gitHubRequests(repo, "POST", "pulls", 7, "reviews")).toEqual([expect.objectContaining({ event: "APPROVE" })])
    },
  )

  test(
    "should count no file for Approve while the overview of the plan is at the top of the changes, and the first file J shows",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
      turnOnReviewPlan(repo)
      await repo.setReviewAnswers({ answers: { "request:7": loginGroupsWithDesign(["README.md", "auth/token.js", "ui/login.css", "ui/login.js"]) } })
      const { window, review } = await openOwnedChanges(page)
      await expect(window.getByTestId("review-overview")).toBeVisible()
      // the overview at the top: no file is shown, none counts, though the tree keeps the first one for J
      await expect(reviewPosition(window)).toHaveText("All Changes · Overview")
      expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 0 of its 4 files")
      await window.locator("[aria-label='Changes of the commit']").focus()
      await window.keyboard.press("j")
      await expect(reviewPosition(window)).toHaveText(/^All Changes · (Step \d+ · )?File \d \/ 4/)
      expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 1 of its 4 files")
    },
  )

  test(
    "should open the description of Approve that came while the pointer rested on it, after the delay of its hover",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
      await repo.setReviewAnswers({ answers: { "request:7": loginGroupsWithDesign(["README.md", "auth/token.js", "ui/login.css", "ui/login.js"]) } })
      // the code-owner review of #7 held: Approve has no description, its tooltip nothing to say when the delay of the
      // hover runs out
      await repo.holdRemoteAnswers({ hold: ["readiness"] })
      const { window, review } = await openOwnedChanges(page)
      await review.getByTestId("review-approve").hover()
      await window.clock.fastForward(1000)
      expect(await openTooltip(window).count()).toBe(0)
      // the review comes, the pointer still: the description opens after the delay, as for a pointer that came now
      await repo.holdRemoteAnswers({ release: ["readiness"] })
      await expect(openTooltip(window)).toHaveText("Counts for Design · You opened 1 of its 4 files")
    },
  )

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
    await expect(reviewPosition(window)).toHaveText("All Changes · File 1 / 2 · Design")
    await pressFileKey(window, "j", "All Changes · File 2 / 2 · Design")
    await pressFileKey(window, "j", "All Changes · File 2 / 2 · Design")
    expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 2 of its 2 files")

    // a new head with a third file of Design: of what the window showed, only the file on screen counts for it
    await repo.setReviewAnswers({
      answers: { "request:7": loginGroupsWithDesign(["ui/button.css", "ui/login.css", "ui/login.js"]), "request:8": loginGroupsWithDesign(["ui/login.css"], "bob") },
    })
    await commitNewHead(page, repo, syncAndWaitForBranches)
    // the window fetches the new head and reads its groups, however long that takes
    await expect.poll(() => ownerChips(window), { timeout: 0 }).toEqual(["All5", "Design· You3", "Auth Team1", "Monorepo1", "UI Team1"])
    // the review stays at the file on screen by its path, also while the chip of the new head showed fewer files, the
    // groups of the new head not told yet: login.js, after the new button.css and login.css
    const tree = window.getByRole("tree", { name: "Changed files" })
    await expect(reviewPosition(window)).toHaveText("All Changes · File 3 / 3 · Design")
    await expect(tree.getByRole("treeitem", { name: "login.js" })).toHaveAttribute("aria-selected", "true")
    expect(await approveDescription(window, review)).toBe("Counts for Design · You opened 1 of its 3 files")
    await pressFileKey(window, "k", "All Changes · File 2 / 3 · Design")
    await expect(tree.getByRole("treeitem", { name: "login.css" })).toHaveAttribute("aria-selected", "true")
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

  test("should keep every file visible for a branch-wide group and count it in Approve", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupOwnedRequests(setupRepo, syncAndWaitForBranches)
    const wide = evaluatedGroup("Design", false, [codeOwner("Test User", "user-name", { suggested: true, approvalState: "reviewing" })], { paths: [], branchWide: true })
    await repo.setReviewAnswers({ answers: { "request:7": { ...LOGIN_GROUPS, groups: [LOGIN_GROUPS.groups[0]!, wide] } } })
    const { window, review } = await openOwnedChanges(page)
    // the window opens on the user's branch-wide group: every file is in its filtered set
    await expect.poll(() => ownerChips(window)).toEqual(["All4", "Design· You4", "Auth Team1", "Monorepo1", "UI Team1"])
    await expect(ownerChip(window, "Design")).toHaveAttribute("aria-pressed", "true")
    await expect(diffHeader(window, "README.md")).toBeVisible()
    await expect(reviewPosition(window)).toHaveText("All Changes · File 1 / 4 · Design")
    await pressFileKey(window, "j", "All Changes · File 2 / 4 · Design")
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
    await expectClosed(form)
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

/** The changes of the request #101 by `D`, the review plan turned on in its repository */
async function openPlanChanges(page: Page, setupRepo: SetupRepo, syncAndWaitForBranches: () => Promise<void>, ...requests: ReturnType<typeof reviewRequest>[]) {
  const repo = await setupRequests(page, setupRepo, syncAndWaitForBranches, ...(requests.length === 0 ? [OLD, RECENT] : requests))
  // the flag of the repository, which the global config of the test machine turns off (`TEST_IDENTITY` of the server)
  appendFileSync(path.join(repo.path, ".git", "config"), "[branchdeck]\n\treviewExperimental = true\n")
  await getReviewRow(page, 101).focus()
  await page.keyboard.press("d")
  const reviewPage = await openLinkedSubWindow(page, await openedReview<PullRequestReviewWindowData>(page, 1))
  return { reviewPage, review: reviewPage.getByTestId("review-window") }
}

/**
 * The first file of the new module of the `review_modules` template in its story, where its row of the shape goes: the
 * flag of its second step, not its descriptor, which the dependency order of the plan puts first
 */
const MEDIA_FIRST_FILE = "plugins/player/view.gui.media/src/com/example/player/media/MediaPlaybackFlag.kt"

/** The directory of the files whose directory gives way in the rail */
const LONG_DIRECTORY = "plugins/air/frontend/session/view.gui.media/src/com/intellij/air"

/** The box of an element that has one */
async function box(locator: Locator): Promise<{ x: number; y: number; width: number; height: number }> {
  const found = await locator.boundingBox()
  expect(found).not.toBeNull()
  return found!
}

/** Whether the text of an element is cut at its edge */
function isCut(locator: Locator): Promise<boolean> {
  return locator.evaluate(element => element.scrollWidth > element.clientWidth)
}

// the `stacked_chain` template: oauth on login on auth, a commit each, as the branches of others; the requests go each
// into the branch of the one below
const LAYER_AUTH = reviewRequest("auth", 301, 6, { title: "Add the auth module", author: "niyaz" })
const LAYER_LOGIN = reviewRequest("login", 302, 5, { title: "Add the login form", author: "niyaz", baseRefName: LAYER_AUTH.headRefName, baseIsDefault: false })
const LAYER_OAUTH = reviewRequest("oauth", 303, 4, { title: "Add the OAuth provider", author: "niyaz", baseRefName: LAYER_LOGIN.headRefName, baseIsDefault: false })

/** Review Stack by the button of the header of the stack of Your Turn: its window, once its fetch answered */
async function openStackReview(page: Page): Promise<{ reviewPage: Page; review: Locator }> {
  const stack = getYourTurnCard(page).getByTestId("review-stack-row")
  await stack.hover()
  await stack.getByTestId("review-stack-review").click()
  const opened = await openedReview<PullRequestStackReviewWindowData>(page, 1)
  let fetched!: Promise<unknown>
  const reviewPage = await openLinkedSubWindow(page, opened, {
    beforeLoad: async window => {
      fetched = reviewFetched(window)
    },
  })
  await fetched
  return { reviewPage, review: reviewPage.getByTestId("review-window") }
}

test.describe("Review Stack", () => {
  test("should open one window of a stack by the button, the menu and D of its header", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_chain", { reviewRequests: availableReviewRequests([LAYER_AUTH, LAYER_LOGIN, LAYER_OAUTH]) })
    await syncAndWaitForBranches()
    const stack = getYourTurnCard(page).getByTestId("review-stack-row")
    await expect(stack.getByTestId("review-stack-title")).toHaveText("Stack of 3 pull requests")

    // the button on hover: the stack, not its layers
    await stack.hover()
    await stack.getByTestId("review-stack-review").click()
    const byButton = await openedReview<PullRequestStackReviewWindowData>(page, 1)
    expect(byButton).toMatchObject({ title: "Review Stack #301–#303", data: { kind: "pullRequestStack", top: 303, bottom: 301, intent: null } })
    expect(byButton.data.layers.map(layer => layer.number)).toEqual([301, 302, 303])
    await expect(stack).toHaveAttribute("aria-expanded", "false")

    // the menu by Shift+F10 on the focused header, then by a right click
    await stack.focus()
    await page.keyboard.press("Shift+F10")
    await page.getByRole("menuitem", { name: "Review Stack" }).click()
    expect((await openedReview<PullRequestStackReviewWindowData>(page, 2)).data).toMatchObject({ kind: "pullRequestStack", top: 303, intent: null })
    await stack.click({ button: "right" })
    await page.getByRole("menuitem", { name: "Show Pull Requests" }).click()
    await expect(stack).toHaveAttribute("aria-expanded", "true")

    // D: its changes; Enter still shows or hides the layers
    await stack.focus()
    await page.keyboard.press("d")
    expect((await openedReview<PullRequestStackReviewWindowData>(page, 3)).data).toMatchObject({ kind: "pullRequestStack", top: 303, intent: "review" })
    await expect(stack).toBeFocused()
    await page.keyboard.press("Enter")
    await expect(stack).toHaveAttribute("aria-expanded", "false")
  })

  test("should review all changes of a stack and each layer, and approve the layer shown on its head", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_chain", { reviewRequests: availableReviewRequests([LAYER_AUTH, LAYER_LOGIN, LAYER_OAUTH]) })
    await syncAndWaitForBranches()
    const { reviewPage, review } = await openStackReview(page)

    // All Changes: the stack, its layers to choose; no decision is of the whole stack
    await expect(review.getByTestId("review-navigation")).toContainText("Stack")
    await expect(review.getByTestId("review-dialog-position")).toHaveCount(0)
    await expect(review.getByTestId("review-stack-header").getByRole("heading")).toHaveText("Stack of 3 pull requests")
    await expect(review.getByTestId("review-stack-meta")).toHaveText("#301–#303 · niyaz · into master")
    await expect(review.getByTestId("review-stack-layer")).toHaveText(["#301Add the auth module1 commit", "#302Add the login form1 commit", "#303Add the OAuth provider1 commit"])
    await expect(review.getByTestId("review-commits").getByTestId("review-commit")).toHaveCount(3)
    await expect(review.getByTestId("review-approve")).toBeDisabled()

    // the changes: All Changes of the stack, a scope per layer, the commits under their layers
    await reviewPage.keyboard.press("d")
    const column = review.getByTestId("range-review-commits").getByTestId("range-review-commit")
    await expect(column).toHaveText([
      /^All Changes3 commits$/,
      /^#301 Add the auth module1 commit$/,
      /^#302 Add the login form1 commit$/,
      /^#303 Add the OAuth provider1 commit$/,
      /Add OAuth provider/,
      /Add login form/,
      /Add auth module/,
    ])
    await expect(column.first()).toHaveAttribute("aria-selected", "true")
    // a layer's scope: its own changes, its decisions in the title bar
    await column.nth(2).click()
    await expect(column.nth(2)).toHaveAttribute("aria-selected", "true")
    await expect(review.getByTestId("review-dialog-position")).toHaveText("2 of 3")
    await expect(review.getByRole("heading", { name: "Add the login form" })).toBeVisible()
    await reviewPage.keyboard.press("Escape")
    await expect(review.getByTestId("review-commits").getByTestId("review-commit")).toHaveCount(1)
    await expect(review.getByTestId("review-description")).toContainText("The change of login.")

    // Approve: the layer shown, on its head
    await pressDecisionKey(reviewPage, review, "a")
    await expect(reviewPage.locator("[data-slot=toast-title]", { hasText: "Approved #302" })).toBeVisible()
    expect(await gitHubRequests(repo, "POST", "pulls", 302, "reviews")).toEqual([{ event: "APPROVE", commit_id: await repo.revision("user-name/virtual/login") }])
    expect(await gitHubRequests(repo, "POST", "pulls", 303, "reviews")).toEqual([])

    // ↓ walks the layers: the top, its own description
    await review.getByTestId("review-dialog-body").focus()
    await reviewPage.keyboard.press("ArrowDown")
    await expect(review.getByRole("heading", { name: "Add the OAuth provider" })).toBeVisible()
    await expect(review.getByTestId("review-dialog-position")).toHaveText("3 of 3")
  })
})

// the `stacked_reading` template: tidy on check on format, login on tidy, a commit each over auth.js: format and tidy
// change the spaces between tokens alone, check and login the body of `auth`
const READ_FORMAT = reviewRequest("format", 401, 8, { title: "Format auth", author: "niyaz" })
const READ_CHECK = reviewRequest("check", 402, 7, { title: "Check the user", author: "niyaz", baseRefName: READ_FORMAT.headRefName, baseIsDefault: false })
const READ_TIDY = reviewRequest("tidy", 403, 6, { title: "Tidy auth", author: "niyaz", baseRefName: READ_CHECK.headRefName, baseIsDefault: false })
const READ_LOGIN = reviewRequest("login", 404, 5, { title: "Log the user in", author: "niyaz", baseRefName: READ_TIDY.headRefName, baseIsDefault: false })

test.describe("What a range leaves to read", () => {
  test("should say a pull request of formatting alone leaves nothing to read, its label of a refactoring then holding", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_reading", { reviewRequests: availableReviewRequests([{ ...READ_FORMAT, labels: ["refactoring"] }]) })
    turnOnReviewPlan(repo)
    await syncAndWaitForBranches()
    const { review, fetched } = await openRequestReview(page, 401)
    await fetched
    await expect(review.getByTestId("review-reading")).toHaveText("Nothing to read · formatting")
    await expect(review.getByTestId("review-dialog-label")).toHaveText(["refactoring"])
    await expect(review.getByTestId("review-author-claim")).toHaveCount(0)
  })

  test("should flag a pull request labelled a refactoring whose diff changes the body of a function", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_reading", { reviewRequests: availableReviewRequests([{ ...READ_CHECK, labels: ["refactoring"] }]) })
    turnOnReviewPlan(repo)
    await syncAndWaitForBranches()
    const { window, review, fetched } = await openRequestReview(page, 402)
    await fetched
    // the label itself says the diff proves it wrong, no second chip repeats it
    const claim = review.getByTestId("review-dialog-label")
    await expect(claim).toHaveText(["refactoring"])
    await expect(claim).toHaveAttribute("data-claim", "true")
    await expect(review.getByTestId("review-author-claim")).toHaveCount(0)
    await claim.focus()
    await expect(window.getByRole("tooltip")).toHaveText(
      "The author calls it refactoring, but 1 change of code in 1 file is left to read: no formatting, rename, import or move proves it",
    )
    // the changes keep what is left to read in their header
    await window.keyboard.press("d")
    await expect(review.getByTestId("review-reading")).toHaveText("1 change to read in 1 file")
  })

  test("should open a stack on its first layer to read, fold the layers with nothing to read and pass them by ↑/↓", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_reading", { reviewRequests: availableReviewRequests([READ_FORMAT, READ_CHECK, READ_TIDY, READ_LOGIN]) })
    turnOnReviewPlan(repo)
    await syncAndWaitForBranches()
    const { reviewPage, review } = await openStackReview(page)

    // the bottom has nothing to read: the window opens on #402, the first layer to read
    await expect(review.getByRole("heading", { name: "Check the user" })).toBeVisible()
    await expect(review.getByTestId("review-dialog-position")).toHaveText("2 of 4")
    await expect(review.getByTestId("review-reading")).toHaveText("1 change to read in 1 file")

    // ↓ passes #403, which has nothing to read; ↑ comes back to #402 and passes #401 no further
    await review.getByTestId("review-dialog-body").focus()
    await reviewPage.keyboard.press("ArrowDown")
    await expect(review.getByRole("heading", { name: "Log the user in" })).toBeVisible()
    await expect(review.getByTestId("review-dialog-position")).toHaveText("4 of 4")
    await review.getByTestId("review-previous").click()
    await expect(review.getByRole("heading", { name: "Check the user" })).toBeVisible()
    await expect(review.getByTestId("review-previous")).toBeDisabled()

    // All Changes: the sum, the layers with nothing to read folded with their proofs and their Approve
    await review.getByTestId("review-dialog-body").focus()
    await reviewPage.keyboard.press("d")
    const column = review.getByTestId("range-review-commits").getByTestId("range-review-commit")
    // the column says what each layer leaves to read, the layers with nothing to read muted in success
    const scopeReadings = review.getByTestId("range-review-commits").getByTestId("range-review-scope-reading")
    await expect(scopeReadings).toHaveText(["Nothing to read", "1 to read", "Nothing to read", "1 to read"])
    await expect(review.locator("[data-testid=range-review-scope-reading][data-nothing]")).toHaveCount(2)
    await column.first().click()
    await reviewPage.keyboard.press("Escape")
    await expect(review.getByTestId("review-stack-reading")).toHaveText("2 of 4 with nothing to read · 2 changes to read")
    const rows = review.getByTestId("review-stack-layer-row")
    await expect(rows.getByTestId("review-stack-layer-reading")).toHaveText([
      "Nothing to read · formatting",
      "1 change to read in 1 file",
      "Nothing to read · formatting",
      "1 change to read in 1 file",
    ])
    await expect(review.locator("[data-testid=review-stack-layer-row][data-folded]")).toHaveCount(2)
    await expect(rows.getByTestId("review-stack-layer-approve")).toHaveCount(2)

    // Approve of a folded layer: its own head
    await rows.nth(2).getByTestId("review-stack-layer-approve").click()
    await expect(reviewPage.locator("[data-slot=toast-title]", { hasText: "Approved #403" })).toBeVisible()
    expect(await gitHubRequests(repo, "POST", "pulls", 403, "reviews")).toEqual([{ event: "APPROVE", commit_id: await repo.revision("user-name/virtual/tidy") }])
    expect(await gitHubRequests(repo, "POST", "pulls", 401, "reviews")).toEqual([])
  })
})

test.describe("Read ahead", () => {
  /** #401 reformats `auth.js`, #402 changes a body over it: each into master, both labelled a refactoring */
  const FORMAT_ALONE = reviewRequest("format", 401, 8, { title: "Format auth", author: "niyaz", labels: ["refactoring"] })
  const CHECK_ALONE = reviewRequest("check", 402, 7, { title: "Check the user", author: "niyaz", labels: ["refactoring"] })

  /** `stacked_reading` with `requests` waiting and the review plan on, the read ahead held: each row says "Checking…" */
  async function setupHeld(setupRepo: SetupRepo, syncAndWaitForBranches: () => Promise<void>, requests: ReturnType<typeof reviewRequest>[]): Promise<TestRepositoryBuilder> {
    const repo = await setupRepo("stacked_reading", { reviewRequests: availableReviewRequests(requests), remoteAnswers: { hold: ["readAhead"] } })
    turnOnReviewPlan(repo)
    // the listing after the sync reads the requests ahead with the review plan on
    await syncAndWaitForBranches()
    return repo
  }

  test("should say what each request leaves to read before any window opens, checking first, its rows in place", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupHeld(setupRepo, syncAndWaitForBranches, [FORMAT_ALONE, CHECK_ALONE])
    const card = getYourTurnCard(page)
    const titles = card.getByTestId("review-request-title")
    await expect(card.getByTestId("review-request-reading")).toHaveText(["Checking…", "Checking…"])
    await expect(titles).toHaveText(["Format auth", "Check the user"])

    await repo.holdRemoteAnswers({ release: ["readAhead"] })
    const formatted = getReviewRow(page, 401).getByTestId("review-request-reading")
    await expect(formatted).toHaveText("Nothing to read · formatting")
    await expect(formatted).toHaveAttribute("data-nothing", "true")
    await expect(getReviewRow(page, 402).getByTestId("review-request-reading")).toHaveText("1 change to read in 1 file")
    // no row moved by its verdict, and no window opened
    await expect(titles).toHaveText(["Format auth", "Check the user"])
    expect(await getOpenedSubWindows(page)).toEqual([])
    // the review refs of both stay: they wait
    expect((await repo.getReviewRequestsFixture()).prunes).toEqual([[401, 402]])
  })

  test("should flag in its row a request labelled a refactoring whose diff changes a body", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupHeld(setupRepo, syncAndWaitForBranches, [FORMAT_ALONE, CHECK_ALONE])
    await repo.holdRemoteAnswers({ release: ["readAhead"] })
    // the label chip itself is in the warning tone, its hover why and how much is left; no second chip repeats the count
    const claim = getReviewRow(page, 402).getByTestId("review-request-label")
    await expect(claim).toHaveText("refactoring")
    await expect(claim).toHaveAttribute("data-claim", "true")
    await expect(getReviewRow(page, 402).getByTestId("review-request-claim")).toHaveCount(0)
    await claim.hover()
    await expect(openTooltip(page)).toHaveText("The author calls it refactoring, but 1 change of code in 1 file is left to read: no formatting, rename, import or move proves it")
    // the label of a refactoring of formatting alone holds
    await expect(getReviewRow(page, 401).getByTestId("review-request-reading")).toHaveText("Nothing to read · formatting")
    await expect(getReviewRow(page, 401).getByTestId("review-request-label")).toHaveText("refactoring")
    await expect(getReviewRow(page, 401).getByTestId("review-request-label")).not.toHaveAttribute("data-claim")
  })

  test("should give way in a narrow row with the meta first, hidden whole without room for its number, then the title, its labels, its reading and its age whole", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const long = reviewRequest("check", 402, 7, {
      title: "Ledger: rename the sum helper, check the user and keep every other part of the ledger unchanged",
      author: "github-actions",
      labels: ["refactoring", "documentation"],
    })
    const repo = await setupHeld(setupRepo, syncAndWaitForBranches, [long])
    await repo.holdRemoteAnswers({ release: ["readAhead"] })
    await page.setViewportSize({ width: 1000, height: 720 })
    const row = getReviewRow(page, 402)
    await expect(row.getByTestId("review-request-reading")).toHaveText("1 change to read in 1 file")
    const cut = (locator: Locator) => locator.evaluate(element => element.scrollWidth > element.clientWidth)
    // the meta gives way first, all of it, then the title, cut with an ellipsis
    expect(await row.getByTestId("review-request-meta").evaluate(element => element.clientWidth)).toBe(0)
    expect(await cut(row.getByTestId("review-request-title"))).toBe(true)
    // the labels, the one the diff proves wrong among them, what it leaves to read and its age show whole
    const labels = row.getByTestId("review-request-label")
    await expect(labels).toHaveText(["refactoring", "documentation"])
    for (const label of await labels.all()) {
      expect(await cut(label.locator("span"))).toBe(false)
    }
    expect(await cut(row.getByTestId("review-request-reading").locator("span"))).toBe(false)
    expect(await cut(row.getByTestId("review-request-age"))).toBe(false)

    // as the window widens, the meta is hidden whole or shows its number whole ("#402…"), never a bare part of it: the
    // narrowest window that shows it, found by halving (each width a round trip of the browser, seconds under load for a
    // scan of every 10 px), shows the number whole, as do the next pixels, and the pixel below hides it
    const meta = row.getByTestId("review-request-meta")
    const stateAt = async (width: number) => {
      await page.setViewportSize({ width, height: 720 })
      return meta.evaluate(element => {
        if (element.clientWidth === 0 || getComputedStyle(element).visibility === "hidden") {
          return "hidden"
        }
        const text = element.firstChild!
        const number = document.createRange()
        number.setStart(text, 0)
        number.setEnd(text, text.textContent!.indexOf(" · "))
        return number.getBoundingClientRect().width <= element.clientWidth ? "number" : "cut"
      })
    }
    let [hidden, shown] = [1000, 1600]
    expect(await stateAt(shown)).toBe("number")
    while (shown - hidden > 1) {
      const width = Math.floor((hidden + shown) / 2)
      const state = await stateAt(width)
      expect(state, `at ${width} px`).not.toBe("cut")
      if (state === "hidden") {
        hidden = width
      } else {
        shown = width
      }
    }
    for (const width of [shown, shown + 1, shown + 2]) {
      expect(await stateAt(width), `at ${width} px`).toBe("number")
    }
    expect(await stateAt(hidden)).toBe("hidden")
  })

  test("should sum a stack in its header and say what each layer under it leaves to read", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupHeld(setupRepo, syncAndWaitForBranches, [READ_FORMAT, READ_CHECK, READ_TIDY, READ_LOGIN])
    const stack = getYourTurnCard(page).getByTestId("review-stack-row")
    await expect(stack.getByTestId("review-stack-reading")).toHaveText("Checking…")
    await repo.holdRemoteAnswers({ release: ["readAhead"] })
    await expect(stack.getByTestId("review-stack-reading")).toHaveText("2 of 4 with nothing to read · 2 changes to read")
    await stack.click()
    await expect(getYourTurnCard(page).getByTestId("review-request-row").getByTestId("review-request-reading")).toHaveText([
      "Nothing to read · formatting",
      "1 change to read in 1 file",
      "Nothing to read · formatting",
      "1 change to read in 1 file",
    ])
  })

  test("should say a private member renamed where the repository has no index is not proven, and not read it again on the next listing", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const renamed = reviewRequest("rename", 405, 4, { title: "Rename the sum helper", author: "niyaz", labels: ["refactoring"] })
    const repo = await setupHeld(setupRepo, syncAndWaitForBranches, [renamed])
    await repo.holdRemoteAnswers({ release: ["readAhead"] })
    const row = getReviewRow(page, 405)
    const reading = row.getByTestId("review-request-reading")
    // no index of the repository will answer the rename: not proven, never "checking renames"
    await expect(reading).toHaveText("4 changes to read in 1 file · 1 file not proven")
    // nothing proven contradicts the label: it stays muted
    await expect(row.getByTestId("review-request-label")).toHaveText("refactoring")
    await expect(row.getByTestId("review-request-label")).not.toHaveAttribute("data-claim")

    // the next listing, the read ahead held: the request it reads says "Checking…", the final one keeps its reading
    await repo.holdRemoteAnswers({ hold: ["readAhead"] })
    await repo.setReviewRequests(availableReviewRequests([renamed, FORMAT_ALONE]))
    const rows = getYourTurnCard(page).getByTestId("review-request-row")
    await moveClockUntil(repo, 300_000, async () => (await rows.count()) === 2)
    await expect(getReviewRow(page, 401).getByTestId("review-request-reading")).toHaveText("Checking…")
    await expect(reading).toHaveText("4 changes to read in 1 file · 1 file not proven")
    await repo.holdRemoteAnswers({ release: ["readAhead"] })
    await expect(getReviewRow(page, 401).getByTestId("review-request-reading")).toHaveText("Nothing to read · formatting")
  })

  test("should show a final reading after a restart without reading it again", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupHeld(setupRepo, syncAndWaitForBranches, [FORMAT_ALONE])
    await repo.holdRemoteAnswers({ release: ["readAhead"] })
    const reading = getReviewRow(page, 401).getByTestId("review-request-reading")
    await expect(reading).toHaveText("Nothing to read · formatting")

    // a new session, the read ahead held: a reading it read again would say "Checking…" until the release
    await repo.holdRemoteAnswers({ hold: ["readAhead"] })
    await repo.closeSessions()
    await page.reload()
    await expect(reading).toHaveText("Nothing to read · formatting")
    await expect(getYourTurnCard(page).locator("[data-testid=review-request-reading][data-checking]")).toHaveCount(0)
    await repo.holdRemoteAnswers({ release: ["readAhead"] })
  })
})
