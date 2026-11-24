// @spec openspec/specs/branch/shown-reviews/spec.md
import type { Locator, Page } from "@playwright/test"
import type { PatronusRobot, PullRequest } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { NO_REVIEW_NEEDED, availableRobots, expectPatronusStarts, mergeRunningRobots, robot, startRequest } from "../helpers/patronus-helpers"
import type { GitHubFixture } from "../helpers/pull-request-helpers"
import { GITHUB_REPOSITORY, availablePullRequests, gitHubMutations, gitHubRequests, pullRequest, reviewer } from "../helpers/pull-request-helpers"
import { availableReviewRequests, getReviewRow, getYourTurnCard, openRequestReview, pressDecisionKey, reviewRequest } from "../helpers/review-request-helpers"
import { openSafePushWithKey, popoverStart, safePushPopover } from "../helpers/safe-push-helpers"
import { openSubmenu } from "../helpers/selection-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { emitTauriEvent, moveClockUntil, nextAnswer, runAndWaitForAnswer } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { toastWith } from "../helpers/title-bar-helpers"
import { expectClosed, openTooltip, waitForStepFacts } from "../helpers/wait-helpers"

// the `stacked_ownership` template: auth, one commit of auth/token.js, whose owner is Auth Team by the OWNERSHIP files of
// master; login is stacked on it
const BRANCH = "auth"
const HEAD = "user-name/review/auth/head"
const BASE = "user-name/review/auth/base"
/** The users of the repository a review is asked of: GitHub refuses a request of anyone else */
const USERS = [
  { login: "AlexPl292", name: "Alex Plate" },
  { login: "alice", name: "Alice Smith" },
  { login: "bob", name: "Bob Jones" },
]

type SetupRepo = (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>

/** The record of the body of a review after landing that asks `reviewers` (`domain_policy::shown::reviewers_record`) */
function reviewersRecord(...reviewers: string[]): string {
  return `<!-- branch-deck show reviewers: ${reviewers.join(" ")} -->`
}

/** The open review after landing #12 of auth, ready, its body recording `asked`, its latest reviews `reviewers` */
function shownReview(asked: string[], reviewers: PullRequest["reviewers"]): NonNullable<GitHubFixture["shown"]>[number] {
  return {
    pullRequest: pullRequest(BRANCH, 12, { title: "Add the token", headRefName: HEAD, baseRefName: BASE, reviewers, reviewDecision: "reviewRequired" }),
    body: `The token of auth.\n\n${reviewersRecord(...asked)}`,
  }
}

/** The GitHub of the repository: its users, no pull request of a branch, the review after landing `shown` */
function gitHub(...shown: NonNullable<GitHubFixture["shown"]>): GitHubFixture {
  return { ...availablePullRequests(), users: USERS, shown }
}

/**
 * The repository on Patronus and GitHub, auth needing no code-owner review (the REST start counts none), Auth Team of
 * the CodeOwners service answered as the real one answers (`real_lookup`: one member away on vacation), synced
 */
async function setupShow(page: Page, setupRepo: SetupRepo, syncAndWaitForBranches: () => Promise<void>, options: SetupRepoOptions = {}): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("stacked_ownership", {
    patronus: { ...availableRobots(), githubRepository: `${GITHUB_REPOSITORY.owner}/${GITHUB_REPOSITORY.name}` },
    pullRequests: gitHub(),
    gitHubSignIn: { signedIn: true },
    ...options,
  })
  await repo.setReviewAnswers({ answers: { [BRANCH]: NO_REVIEW_NEEDED } })
  await repo.setCodeOwners({ real: ["Auth Team"] })
  await syncAndWaitForBranches()
  await waitForStepFacts(findBranchRow(page, BRANCH).getByTestId("next-step-slot"))
  return repo
}

/**
 * The branches of the review after landing of auth on the remote as a Safe Push with it left them: its head at the commit
 * the robot takes, its base at the fork point; returned
 */
async function pushReviewBranches(repo: TestRepositoryBuilder): Promise<{ head: string; base: string }> {
  const [head, base] = await Promise.all([repo.revision(`user-name/virtual/${BRANCH}`), repo.revision("origin/master")])
  await repo.pushInTerminal(HEAD, { revision: head })
  await repo.pushInTerminal(BASE, { revision: base })
  return { head, base }
}

/** The mark of the facts beside the first column of the row of auth, its words (the hover of the dot) */
function statusMark(page: Page): Locator {
  return findBranchRow(page, BRANCH).getByTestId("row-status-mark")
}

/** The session reads the pull requests and the robots again, at a focus after their answers aged */
async function listAgain(page: Page, repo: TestRepositoryBuilder): Promise<void> {
  const calls = await repo.getPullRequestCalls()
  await emitTauriEvent(page, "tauri://focus")
  await repo.advanceBackendClock(121_000)
  await emitTauriEvent(page, "tauri://focus")
  // however long the listing takes under load
  await expect.poll(() => repo.getPullRequestCalls(), { timeout: 0 }).toBeGreaterThan(calls)
}

/** The session lists the pull requests again: its clock moves past the age of their answer, a focus at each step */
async function listPullRequestsAgain(page: Page, repo: TestRepositoryBuilder): Promise<void> {
  const calls = await repo.getPullRequestCalls()
  await moveClockUntil(repo, 60_000, async () => (await repo.getPullRequestCalls()) > calls, { focus: page, asked: () => repo.getPullRequestCalls() })
}

/** The bodies of the writes of the fake GitHub to the pull request `number` (`PATCH pulls/<n>`), in order */
async function bodyWrites(repo: TestRepositoryBuilder, number: number): Promise<string[]> {
  return (await gitHubRequests(repo, "PATCH", "pulls", number)).map(body => (body as { body?: string }).body ?? "").filter(body => body !== "")
}

/** The merged robot of auth that landed the commit `head` an hour ago, before the review opened */
function landedRobot(head: string): PatronusRobot {
  return robot(BRANCH, "landed-1", { status: "success", sourceRevision: head, startTime: 1704139200, finishTime: 1704142800 })
}

/**
 * The repository of `setupShow` with the review after landing #12 of auth open: its branches on the remote (its head at
 * `head`, the head before when none), the merged robot that landed its head, the review asking alice, whose latest review
 * is `state`; listed. Returns the repository and the head of the review
 */
async function setupOpenReview(
  page: Page,
  setupRepo: SetupRepo,
  syncAndWaitForBranches: () => Promise<void>,
  state: "requested" | "changesRequested",
  moveHead?: (repo: TestRepositoryBuilder) => Promise<string>,
): Promise<{ repo: TestRepositoryBuilder; head: string }> {
  const repo = await setupShow(page, setupRepo, syncAndWaitForBranches, { pullRequests: availablePullRequests() })
  const pushed = (await pushReviewBranches(repo)).head
  const head = moveHead === undefined ? pushed : await moveHead(repo)
  await repo.setPatronus(availableRobots(landedRobot(head)))
  await repo.setPullRequests(gitHub(shownReview(["alice"], [reviewer("alice", state)])))
  await listAgain(page, repo)
  return { repo, head }
}

/** A fix of auth committed and synced, and its Safe Push running beside the robot that landed `head`: the robot of the fix */
async function startFix(page: Page, repo: TestRepositoryBuilder, syncAndWaitForBranches: () => Promise<void>, head: string): Promise<PatronusRobot> {
  await repo.commit("(auth) Keep the session", { "auth/session.js": "export const session = 1\n" }, 1704189000)
  await syncAndWaitForBranches()
  const fix = robot(BRANCH, "fix-1", { sourceRevision: await repo.revision(`user-name/virtual/${BRANCH}`), startTime: 1704189300 })
  await repo.setPatronus(availableRobots(landedRobot(head), fix))
  await listAgain(page, repo)
  return fix
}

/** The robot of the fix merged (`mergeRunningRobots`), and the answer of the extend its news runs */
async function mergeFix(page: Page, repo: TestRepositoryBuilder, fix: PatronusRobot, head: string): Promise<void> {
  const extended = nextAnswer(page, "execute")
  await mergeRunningRobots(page, repo, [fix], 1704192000, [landedRobot(head)])
  await extended
}

test.describe("Reviews after landing", () => {
  test(
    "should start a Safe Push and Request Review… from the popover of S with the code owners of the changes, the draft opened into the fork point before the robot",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupShow(page, setupRepo, syncAndWaitForBranches)

      // the popover of S offers the review after landing beside its start
      const popover = await openSafePushWithKey(page, BRANCH)
      await popover.getByTestId("safe-push-show-option-start").click()
      await expect(popover.getByTestId("safe-push-show")).toBeVisible()
      // one available member of Auth Team, as the CodeOwners service answers it: the one away on vacation is skipped
      await expect(popover.getByTestId("safe-push-show-reviewers").locator("[data-slot=combobox-chip]")).toHaveText(["AlexPl292"])
      await expect(popover.getByTestId("safe-push-show-candidates")).toHaveText("One available code owner of Auth Team.")
      expect(await repo.getCodeOwnersLookups()).toEqual(["Auth Team"])
      await expect(popoverStart(page)).toHaveText("Safe Push and Request Review")
      await expect(popoverStart(page)).toBeEnabled()

      await runAndWaitForAnswer(page, "execute", () => popoverStart(page).click())
      await expectClosed(popover)

      // the branches of the review on the remote, the head at the commit the robot takes, the base its fork point
      const [tip, forkPoint, remote] = await Promise.all([repo.revision(`user-name/virtual/${BRANCH}`), repo.revision("origin/master"), repo.getRemoteBranches()])
      expect({ head: remote[HEAD], base: remote[BASE] }).toEqual({ head: tip, base: forkPoint })
      // a draft from the head into the base, asking nobody until the robot merged, its body recording whom it asks then
      const created = (await gitHubRequests(repo, "POST", "pulls")) as { head: string; base: string; draft: boolean; body: string }[]
      expect(created.map(({ head, base, draft }) => ({ head, base, draft }))).toEqual([{ head: HEAD, base: BASE, draft: true }])
      expect(created[0]!.body).toContain(reviewersRecord("AlexPl292"))
      expect(await gitHubRequests(repo, "POST", "pulls", "*", "requested_reviewers")).toEqual([])
      // the robot through the API, after the draft
      await expectPatronusStarts(repo, [startRequest(BRANCH)])
      await expect(toastWith(page, "Safe Push started")).toBeVisible()
    },
  )

  test(
    "should start it from the Patronus menu with the reviewers the user picks, and open the review with them once the robot merged",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupShow(page, setupRepo, syncAndWaitForBranches)

      await findBranchRow(page, BRANCH).click({ button: "right" })
      await openSubmenu(page, "Patronus")
      await page.getByRole("menuitem", { name: "Safe Push and Request Review…" }).click()
      const popover = safePushPopover(page)
      const chips = popover.getByTestId("safe-push-show-reviewers").locator("[data-slot=combobox-chip]")
      await expect(chips).toHaveText(["AlexPl292"])
      // without a reviewer the start asks nobody: refused
      await chips.getByRole("button").click()
      await expect(chips).toHaveCount(0)
      await expect(popover.getByTestId("safe-push-status")).toHaveText("Add a reviewer: a review after landing asks at least one")
      await expect(popoverStart(page)).toBeDisabled()
      // a GitHub login of the repository, by its search
      await popover.getByTestId("safe-push-show-reviewer-search").fill("ali")
      await page.getByRole("option", { name: /^alice/ }).click()
      await expect(chips).toHaveText(["alice"])
      await expect(popoverStart(page)).toBeEnabled()
      await runAndWaitForAnswer(page, "execute", () => popoverStart(page).click())
      const created = (await gitHubRequests(repo, "POST", "pulls")) as { draft: boolean; body: string }[]
      expect(created.map(({ draft }) => draft)).toEqual([true])
      expect(created[0]!.body).toContain(reviewersRecord("alice"))

      // the draft listed, the robot merged: the review opens, ready, alice asked
      await listAgain(page, repo)
      // the open runs from the news of the session: its answer, then what it wrote
      const opened = nextAnswer(page, "execute")
      await repo.setPatronus(availableRobots(robot(BRANCH, "started-1", { status: "success", finishTime: 1704192000 })))
      await listAgain(page, repo)
      await opened
      expect(await gitHubRequests(repo, "POST", "pulls", "*", "requested_reviewers")).toEqual([{ reviewers: ["alice"] }])
      expect(gitHubMutations(await repo.getGitHubLog()).map(([name]) => name)).toContain("markPullRequestReadyForReview")
      // the row marks it beside the landing: whom it waits for
      await expect(statusMark(page)).toHaveAccessibleName(/^In review after landing: waits for alice in #\d+$/)
    },
  )

  test("should return the review to the author on changes requested, wait while a fix lands, then put the fix onto its head and ask again", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the review opened on what the robot landed; alice requested changes
    const { repo, head } = await setupOpenReview(page, setupRepo, syncAndWaitForBranches, "changesRequested")
    await expect(statusMark(page)).toHaveAccessibleName("alice requested changes in #12: commit a fix as (auth) …, and its Safe Push adds it to the review")
    const returned = getYourTurnCard(page).getByTestId("returned-pull-request-row")
    await expect(returned).toHaveAttribute("data-returned-branch", BRANCH)

    // a fix of auth: its Safe Push runs, the review waits for it, nobody's move
    const fix = await startFix(page, repo, syncAndWaitForBranches, head)
    const fixTip = fix.sourceRevision!
    await expect(statusMark(page)).toHaveAccessibleName("A fix of auth lands: #12 takes it once its Safe Push merged and asks its reviewers again")
    await expect(returned).toHaveCount(0)

    // merged: the fix on the head of the review, alice asked again, then the record names it
    await mergeFix(page, repo, fix, head)
    expect(await gitHubRequests(repo, "POST", "pulls", 12, "requested_reviewers")).toEqual([{ reviewers: ["alice"] }])
    const moved = (await repo.getRemoteBranches())[HEAD]!
    expect(moved).not.toBe(head)
    expect(await repo.revision(`${moved}~1`)).toBe(head)
    expect((await bodyWrites(repo, 12)).at(-1)).toContain(`<!-- branch-deck show landed: ${fixTip} -->`)
  })

  test("should comment once on a fix that doesn't apply on the commits of the review", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the head of the review has the file of the fix already: the Safe Push that landed it took a commit of alice too
    const { repo, head: other } = await setupOpenReview(page, setupRepo, syncAndWaitForBranches, "changesRequested", repo =>
      repo.commitOnRemote(HEAD, { subject: "Add the session", author: "Alice Smith <alice@example.com>", timestamp: 1704188000, files: { "auth/session.js": "other\n" } }),
    )
    const fix = await startFix(page, repo, syncAndWaitForBranches, other)
    const fixTip = fix.sourceRevision!
    await mergeFix(page, repo, fix, other)
    const comments = () => gitHubRequests(repo, "POST", "issues", 12, "comments")
    expect(await comments()).toEqual([
      { body: expect.stringMatching(new RegExp(`^Landed a follow-up as ${fixTip.slice(0, 10)}; it does not apply on this review's commits: https://`)) },
    ])
    // the head keeps what it had; the next listing and robots ask nothing again
    expect((await repo.getRemoteBranches())[HEAD]).toBe(other)
    expect((await bodyWrites(repo, 12)).at(-1)).toContain(`<!-- branch-deck show landed: ${fixTip} -->`)
    await listPullRequestsAgain(page, repo)
    expect(await comments()).toHaveLength(1)
  })

  test("should close a review after landing every reviewer it asked approved, with its comment, and delete its branches", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const { repo } = await setupOpenReview(page, setupRepo, syncAndWaitForBranches, "requested")
    await expect(statusMark(page)).toHaveAccessibleName("In review after landing: waits for alice in #12")

    // the close runs from the news of the session: its answer, then what it wrote
    const closed = nextAnswer(page, "execute")
    await repo.reviewOnGitHub(12, "alice", "approved")
    await listAgain(page, repo)
    await closed
    expect(await gitHubRequests(repo, "POST", "issues", 12, "comments")).toEqual([{ body: "Reviewed after landing: approved by alice. The change is in master." }])
    expect(Object.keys(await repo.getRemoteBranches()).filter(name => name.includes("/review/"))).toEqual([])
    expect(await gitHubRequests(repo, "PATCH", "pulls", 12)).toContainEqual({ state: "closed" })
  })

  test(
    "should ask more reviewers with Request Review… of an open review after landing, and close it unapproved with Close Review… after its confirmation",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const { repo } = await setupOpenReview(page, setupRepo, syncAndWaitForBranches, "requested")
      await expect(statusMark(page)).toHaveAccessibleName("In review after landing: waits for alice in #12")

      // Request Review… of the row: bob, by the search of the repository
      await findBranchRow(page, BRANCH).click({ button: "right" })
      await page.getByRole("menuitem", { name: "Request Review…" }).click()
      const form = page.getByTestId("shown-reviewers")
      await expect(form).toContainText("It asks alice: the reviewers you add are asked too and recorded in its body.")
      await form.getByTestId("shown-reviewers-reviewer-search").fill("bob")
      await page.getByRole("option", { name: /^bob/ }).click()
      await runAndWaitForAnswer(page, "execute", () => form.getByRole("button", { name: "Request Review" }).click())
      await expect(toastWith(page, "Asked bob to review #12 after landing")).toBeVisible()
      expect(await gitHubRequests(repo, "POST", "pulls", 12, "requested_reviewers")).toEqual([{ reviewers: ["bob"] }])
      expect((await bodyWrites(repo, 12)).at(-1)).toContain(reviewersRecord("alice", "bob"))

      // Close Review… of the row: what it closes without, then the close with its comment, its branches deleted
      await findBranchRow(page, BRANCH).click({ button: "right" })
      await page.getByRole("menuitem", { name: "Close Review…" }).click()
      const confirm = page.getByTestId("close-shown-review")
      await expect(confirm).toContainText("has not approved #12: it closes without that approval.")
      await runAndWaitForAnswer(page, "execute", () => confirm.getByRole("button", { name: "Close Review" }).click())
      await expect(toastWith(page, "Closed review after landing #12 of auth")).toBeVisible()
      expect(await gitHubRequests(repo, "POST", "issues", 12, "comments")).toEqual([
        { body: expect.stringMatching(/^Closed without the approval of .+\. The change is in master\.$/) },
      ])
      expect(Object.keys(await repo.getRemoteBranches()).filter(name => name.includes("/review/"))).toEqual([])
    },
  )

  test(
    "should tell a reviewer a review after landing by its badge, offer no Safe Push of it, and take the approval",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // carol's review after landing of login: its base is the fork point under her prefix
      const shown = reviewRequest("login", 301, 2, { title: "Add the login form", author: "carol", baseRefName: "carol/review/login/base", baseIsDefault: false })
      const repo = await setupRepo("stacked_ownership", {
        reviewRequests: availableReviewRequests([shown]),
        patronus: { ...availableRobots(), githubRepository: `${GITHUB_REPOSITORY.owner}/${GITHUB_REPOSITORY.name}` },
        pullRequests: availablePullRequests(),
        gitHubSignIn: { signedIn: true },
      })
      await syncAndWaitForBranches()
      await expect(getReviewRow(page, 301)).toBeVisible()
      const badge = getReviewRow(page, 301).getByTestId("shown-badge")
      await expect(badge).toHaveText("Shown")
      await badge.hover()
      await expect(openTooltip(page)).toHaveText("In master already: your review gates no Safe Push, and the approval of every reviewer it asked closes it")
      await page.mouse.move(0, 0)

      // the row's menu offers no Safe Push of it: its change is in master already
      await getReviewRow(page, 301).click({ button: "right" })
      const rowSafePush = page.getByRole("menuitem", { name: /^Approve and Safe Push…/ })
      await expect(rowSafePush).toHaveAttribute("aria-disabled", "true")
      await rowSafePush.hover()
      await expect(openTooltip(page)).toHaveText("#301 is a review after landing: its change is in master already, so there is nothing to Safe Push")
      await page.keyboard.press("Escape")

      const { window: reviewPage, review, fetched } = await openRequestReview(page, 301)
      await fetched
      await expect(review.getByTestId("shown-badge")).toHaveText("Shown")
      await expect(review.getByTestId("review-safe-push")).toBeDisabled()
      // the approval is a write of the session, its answer the window's
      await expect(review.getByTestId("review-commit").first()).toBeVisible()
      await expect(review.getByTestId("review-approve")).toBeEnabled()
      await runAndWaitForAnswer(reviewPage, "execute", () => pressDecisionKey(reviewPage, review, "a"))
      await expect(review.getByTestId("review-approve")).toHaveText("Approved")
      expect(await gitHubRequests(repo, "POST", "pulls", 301, "reviews")).toEqual([expect.objectContaining({ event: "APPROVE" })])
    },
  )
})
