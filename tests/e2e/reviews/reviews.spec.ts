// @spec spec/repository/reviews.spec.md
import type { Locator, Page } from "@playwright/test"
import type { ReviewEntry, ReviewRequest } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { openPalette, runByKeyboard } from "../helpers/command-palette-helpers"
import { availablePullRequests, getOpenedUrls, gitHubRequests, pullRequest } from "../helpers/pull-request-helpers"
import { NOW, availableReviewRequests, openedReviewWindow, reviewRequest } from "../helpers/review-request-helpers"
import type { ReviewRequestsFixture } from "../helpers/review-request-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { getOpenedSubWindows, openLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent, waitForTauriListener } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { TITLE_BARS, useTitleBar } from "../helpers/title-bar-helpers"

// the `issue_links` template: 7 branches; the pull requests of others review them as the branches of others
const ASKED = reviewRequest("ABC-123", 101, 30, { title: "Rework the session store" })
const TEAM = reviewRequest("feature-auth", 201, 50, { title: "Add OAuth login" })
const REVIEWED = reviewRequest("JIRA-456", 202, 10, {
  title: "Backport the pool fix",
  reviewedCommit: "1111111111111111111111111111111111111111",
  reviewedState: "approved",
  updatedAt: NOW - 3600,
})
const OWN = reviewRequest("feature-docs", 203, 5, { title: "Document the CLI", author: "user-name", updatedAt: NOW - 7200 })
const FORGOTTEN = reviewRequest("feature-db", 305, 4, { title: "Migrate the schema" })

function team(request: ReviewRequest): ReviewEntry {
  return { request, involvement: { kind: "team", teams: ["ide-platform"] } }
}

/** What GitHub answers: the request by name, and the rest of Reviews while the view shows; a link may name #305 */
function reviewsFixture(own = OWN): ReviewRequestsFixture {
  return {
    ...availableReviewRequests([ASKED]),
    others: {
      team: { totalCount: 1, entries: [team(TEAM)] },
      reviewed: { totalCount: 1, entries: [{ request: REVIEWED, involvement: { kind: "reviewed", state: "approved", repliedAt: null, newCommits: true } }] },
      authored: { totalCount: 1, entries: [{ request: own, involvement: { kind: "authored", reviewers: [], isDraft: false, branch: null } }] },
    },
    linkable: [FORGOTTEN, { ...reviewRequest("feature-ui", 306, 2, { title: "Merged meanwhile" }), closed: true }],
  }
}

type SetupRepo = (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>

async function setupReviews(page: Page, setupRepo: SetupRepo, syncAndWaitForBranches: () => Promise<void>, options: SetupRepoOptions = {}): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("issue_links", { reviewRequests: reviewsFixture(), ...options })
  await syncAndWaitForBranches()
  await expect(page.getByTestId("your-turn-card")).toBeVisible()
  return repo
}

function reviewsView(page: Page): Locator {
  return page.getByTestId("reviews-view")
}

function section(page: Page, kind: string): Locator {
  return reviewsView(page).getByTestId(`reviews-section-${kind}`)
}

function entryRow(page: Page, kind: string, number: number): Locator {
  return section(page, kind).locator(`[data-testid=review-request-row][data-review-number="${number}"]`)
}

async function showReviews(page: Page): Promise<void> {
  await page.getByTestId("main-view-reviews").click()
  await expect(page.getByTestId("main-view-reviews")).toHaveAttribute("aria-pressed", "true")
  await expect(reviewsView(page)).toBeVisible()
}

test.describe("Reviews view", () => {
  test("should show every pull request the user takes part in once, by section, and read them only while the view shows", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupReviews(page, setupRepo, syncAndWaitForBranches)
    // the branches show: the requests by name only
    expect(await repo.getReviewsCalls()).toBe(0)

    await showReviews(page)
    await expect(section(page, "team")).toBeVisible()
    expect(await repo.getReviewsCalls()).toBe(1)
    // Your Turn's rows lead the view, then the sections in their order
    const sections = reviewsView(page).locator("[data-testid^=reviews-section-]")
    await expect(sections).toHaveCount(4)
    expect(await sections.evaluateAll(nodes => nodes.map(node => node.getAttribute("data-testid")))).toEqual([
      "reviews-section-waiting",
      "reviews-section-team",
      "reviews-section-reviewed",
      "reviews-section-authored",
    ])
    await expect(section(page, "waiting").getByTestId("review-request-title")).toHaveText(["Rework the session store"])
    await expect(entryRow(page, "team", 201).getByTestId("review-request-meta")).toContainText("#201 · carol · requested from ide-platform")
    await expect(entryRow(page, "team", 201).getByTestId("review-request-age")).toHaveText("waiting 2 days")
    await expect(entryRow(page, "reviewed", 202).getByTestId("review-request-meta")).toContainText("you approved 11111111")
    await expect(entryRow(page, "reviewed", 202).getByTestId("review-request-age")).toHaveText("new commits since your review")
    await expect(entryRow(page, "authored", 203).getByTestId("review-request-meta")).toContainText("#203")
    // GitHub's own pull request takes no review of its author: its menu says why
    await entryRow(page, "authored", 203).click({ button: "right" })
    const approve = page.getByRole("menuitem", { name: /Approve/ }).first()
    await expect(approve).toBeDisabled()
    await page.keyboard.press("Escape")

    // hidden, the poll asks the requests alone, once the session heard the window no longer shows the view
    await waitForTauriListener(page, "show_branches")
    const hidden = page.waitForResponse(response => {
      const body = response.request().postDataJSON() as { demand?: { reviews?: boolean } } | null
      return response.url().endsWith("/invoke/set_demand") && body?.demand?.reviews === false
    })
    await emitTauriEvent(page, "show_branches")
    await expect(reviewsView(page)).toBeHidden()
    await hidden
    await repo.advanceBackendClock(300_000)
    await expect.poll(() => repo.getReviewRequestCalls()).toBeGreaterThan(1)
    expect(await repo.getReviewsCalls()).toBe(1)

    // ⌘2 of the native View menu shows it again: read once more, its rest older than two minutes
    await emitTauriEvent(page, "show_reviews")
    await expect(reviewsView(page)).toBeVisible()
    await expect.poll(() => repo.getReviewsCalls()).toBe(2)

    // Reviews of the palette
    await emitTauriEvent(page, "show_branches")
    await openPalette(page)
    await runByKeyboard(page, "reviews", "Reviews")
    await expect(reviewsView(page)).toBeVisible()
  })

  test("should review a pull request of a team in the window of Reviews and approve it there", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupReviews(page, setupRepo, syncAndWaitForBranches)
    await showReviews(page)
    await entryRow(page, "team", 201).getByTestId("review-request-title").click()
    const opened = await openedReviewWindow(page)
    expect([opened.data.number, opened.data.source]).toEqual([201, "reviews"])
    const window = await openLinkedSubWindow(page, opened)
    const review = window.getByTestId("review-window")
    await expect(review.getByTestId("review-dialog-body")).toBeFocused()
    // the list of the window is the page's: the request by name, then the sections
    await expect(window.getByTestId("review-navigation")).toContainText("Reviews")
    await expect(window.getByTestId("review-dialog-position")).toHaveText("2 of 4")
    await expect(review.getByTestId("review-asked-by")).toContainText("ide-platform asked for a review of carol's pull request")

    await expect(review.getByTestId("review-commit").first()).toBeVisible()
    await window.keyboard.press("a")
    await expect.poll(() => gitHubRequests(repo, "POST", "pulls", 201, "reviews")).toEqual([{ event: "APPROVE", commit_id: expect.stringMatching(/^[0-9a-f]{40}$/) }])
    await expect(review.getByTestId("review-approve")).toHaveText("Approved")
  })

  test("should add a pull request by its link from the field the palette focuses, keep it, and remove it from its menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupReviews(page, setupRepo, syncAndWaitForBranches)
    // a filter typed before
    await showReviews(page)
    await expect(section(page, "team")).toBeVisible()
    await reviewsView(page).getByTestId("reviews-filter-input").fill("oauth")
    await expect(section(page, "reviewed")).toHaveCount(0)
    // "Add Pull Request by Link…" of the palette: the view, the focus in the field, its text selected for the paste
    await openPalette(page)
    await runByKeyboard(page, "add pull request by link", "Add Pull Request by Link…")
    const field = reviewsView(page).getByTestId("reviews-filter-input")
    await expect(field).toBeFocused()
    await expect.poll(() => field.evaluate((input: HTMLInputElement) => [input.selectionStart, input.selectionEnd])).toEqual([0, "oauth".length])

    // another repository and a number the repository lacks are refused under the field
    await field.fill("https://github.com/other/repo/pull/305")
    await field.press("Enter")
    await expect(reviewsView(page).getByTestId("reviews-link-error")).toHaveText("other/repo#305 is in another repository: Reviews shows acme/monorepo")
    await expect(field).toHaveAttribute("aria-invalid", "true")
    await field.fill("#999")
    await field.press("Enter")
    await expect(reviewsView(page).getByTestId("reviews-link-error")).toHaveText("acme/monorepo has no pull request #999")
    expect(await getOpenedSubWindows(page)).toEqual([])

    // the link the author forgot to ask the user for: the review opens, the section keeps it
    await field.fill("https://github.com/acme/monorepo/pull/305/files")
    await field.press("Enter")
    const opened = await openedReviewWindow(page)
    expect([opened.data.number, opened.data.source]).toEqual([305, "reviews"])
    await expect(field).toHaveValue("")
    await expect(entryRow(page, "linked", 305).getByTestId("review-request-meta")).toContainText("#305 · carol · not reviewed")
    // a merged one opens, but is not kept
    await field.fill("#306")
    await field.press("Enter")
    await openedReviewWindow(page, 2)
    await expect(section(page, "linked").getByTestId("review-request-row")).toHaveCount(1)

    // the link outlives the window
    await page.reload()
    await showReviews(page)
    await expect(entryRow(page, "linked", 305)).toBeVisible()
    await entryRow(page, "linked", 305).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Remove from Reviews" }).click()
    await expect(section(page, "linked")).toHaveCount(0)
  })

  test("should filter every section by the words typed, open the one match on Enter, and add a pull request by its link when nothing matches", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupReviews(page, setupRepo, syncAndWaitForBranches)
    const linkCalls: unknown[] = []
    page.on("request", request => {
      if (request.url().endsWith("/invoke/link_pull_request")) {
        linkCalls.push(request.postDataJSON())
      }
    })
    await showReviews(page)
    await expect(section(page, "waiting")).toBeVisible()
    await expect(section(page, "reviewed")).toBeVisible()
    // a section the user collapses stays collapsed through a filter
    await section(page, "authored")
      .getByRole("button", { name: /Your Pull Requests/ })
      .click()
    await expect(section(page, "authored")).toHaveAttribute("data-state", "closed")

    // `/` focuses the field from the view
    await reviewsView(page).getByRole("heading", { name: "Reviews" }).focus()
    await page.keyboard.press("/")
    const field = reviewsView(page).getByTestId("reviews-filter-input")
    await expect(field).toBeFocused()
    await page.keyboard.type("oauth")
    await expect(entryRow(page, "team", 201)).toBeVisible()
    await expect(section(page, "team").getByTestId("review-request-row")).toHaveCount(1)
    await expect(section(page, "team")).toContainText("1 of 1")
    await expect(section(page, "waiting")).toHaveCount(0)
    await expect(section(page, "reviewed")).toHaveCount(0)
    await expect(section(page, "authored")).toHaveCount(0)
    // Enter opens the one match
    await field.press("Enter")
    const opened = await openedReviewWindow(page)
    expect([opened.data.number, opened.data.source]).toEqual([201, "reviews"])

    // Escape clears the filter: every section again, the collapsed one collapsed
    await field.press("Escape")
    await expect(field).toHaveValue("")
    await expect(section(page, "waiting")).toBeVisible()
    await expect(section(page, "reviewed")).toBeVisible()
    await expect(section(page, "authored")).toHaveAttribute("data-state", "closed")
    // Escape in the empty field leaves it for the heading
    await field.press("Escape")
    await expect(reviewsView(page).getByRole("heading", { name: "Reviews" })).toBeFocused()

    // ↓ moves to the first match
    await field.fill("pool")
    await expect(section(page, "team")).toHaveCount(0)
    await field.press("ArrowDown")
    await expect(entryRow(page, "reviewed", 202)).toBeFocused()

    // words that match nothing: said, and Enter neither opens nor adds anything
    await field.fill("migrate")
    const empty = reviewsView(page).getByTestId("reviews-filter-empty")
    await expect(empty).toContainText("No pull requests match “migrate”")
    await expect(empty.getByRole("button", { name: /Add Pull Request/ })).toHaveCount(0)
    await field.press("Enter")
    await expect(reviewsView(page).getByTestId("reviews-link-error")).toHaveCount(0)
    expect(linkCalls).toEqual([])
    expect(await getOpenedSubWindows(page)).toHaveLength(1)
    await empty.getByRole("button", { name: "Clear Filter" }).click()
    await expect(field).toHaveValue("")
    await expect(section(page, "team")).toBeVisible()

    // a number Reviews lacks: Add Pull Request, and Enter adds it by its link
    await field.fill("#305")
    await expect(empty.getByRole("button", { name: /Add Pull Request/ })).toBeVisible()
    await field.press("Enter")
    expect((await openedReviewWindow(page, 2)).data.number).toBe(305)
    await expect(field).toHaveValue("")
    await expect(entryRow(page, "linked", 305)).toBeVisible()
    expect(linkCalls).toHaveLength(1)
    // a shown one opens without a link
    await field.fill("#202")
    await field.press("Enter")
    expect((await openedReviewWindow(page, 3)).data.number).toBe(202)
    expect(linkCalls).toHaveLength(1)
  })

  test("should search GitHub with the words typed where it returned only a part of a section", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const fixture = reviewsFixture()
    await setupReviews(page, setupRepo, syncAndWaitForBranches, { reviewRequests: { ...fixture, others: { ...fixture.others!, team: { totalCount: 63, entries: [team(TEAM)] } } } })
    await showReviews(page)
    await expect(section(page, "team").getByTestId("reviews-more-on-github")).toHaveText("1 of 63Open All on GitHub")
    await reviewsView(page).getByTestId("reviews-filter-input").fill("oauth")
    await expect(section(page, "team")).toContainText("1 of 63")
    const filtered = section(page, "team").getByTestId("reviews-filtered-on-github")
    await expect(filtered).toHaveText("First 1 of 63 filteredSearch on GitHub")
    await filtered.getByRole("button", { name: "Search on GitHub" }).click()
    expect(await getOpenedUrls(page)).toEqual([
      `https://github.com/acme/monorepo/pulls?q=${encodeURIComponent("is:pr is:open archived:false draft:false review-requested:@me oauth")}`,
    ])
  })

  test("should show the branch of an own pull request of the table", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const own = reviewRequest("feature-docs", 203, 5, { title: "Document the CLI", author: "user-name", headRefName: "user-name/virtual/feature-docs" })
    await setupReviews(page, setupRepo, syncAndWaitForBranches, {
      reviewRequests: reviewsFixture(own),
      pullRequests: availablePullRequests(pullRequest("feature-docs", 203)),
    })
    await showReviews(page)
    await entryRow(page, "authored", 203).getByTestId("review-request-title").click()
    await expect(page.getByTestId("main-view-branches")).toHaveAttribute("aria-pressed", "true")
    await expect(findBranchRow(page, "feature-docs")).toBeVisible()
    expect(await getOpenedSubWindows(page)).toEqual([])
  })

  test("should say off GitHub why there are no reviews", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    await syncAndWaitForBranches()
    await showReviews(page)
    await expect(reviewsView(page).getByTestId("reviews-state")).toContainText("The pull requests of this repository are not on GitHub")
  })
})

test.describe("Your Turn and Reviews", () => {
  test("should show all reviews from the menu of Your Turn", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupReviews(page, setupRepo, syncAndWaitForBranches)
    await page.getByTestId("your-turn-card").getByRole("button", { name: "More actions for Your Turn" }).click()
    await page.getByRole("menuitem", { name: /Show All Reviews/ }).click()
    await expect(reviewsView(page)).toBeVisible()
  })

  test("should open Reviews by its keys of a custom title bar: Ctrl+2", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, TITLE_BARS.gnome)
    await setupReviews(page, setupRepo, syncAndWaitForBranches)
    await page.keyboard.press("Control+Digit2")
    await expect(page.getByTestId("main-view-reviews")).toHaveAttribute("aria-pressed", "true")
    await expect(reviewsView(page)).toBeVisible()
  })
})
