// @spec spec/branch/next-step.spec.md
import type { Page } from "@playwright/test"
import type { PatronusReviewRequirement, PatronusStartOptions, PullRequest } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { APPROVED_BY_CODE_OWNER, AWAITING_CODE_OWNER, NO_REVIEW_NEEDED, NOT_EVALUATED, availableRobots, codeOwner, evaluatedGroup, robot } from "../helpers/patronus-helpers"
import { availablePullRequests, createPullRequestDialog, getOpenedUrls, pullRequest, reviewer, routeNewPullRequest } from "../helpers/pull-request-helpers"
import { routePush } from "../helpers/push-helpers"
import { availableReviewRequests } from "../helpers/review-request-helpers"
import { rewriteNextRemoteStatus } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { routeCommand } from "../helpers/wait-helpers"

const BRANCH = "JIRA-456"

const START_OPTIONS: PatronusStartOptions = { targetBranch: "master", supported: true, profiles: [], defaultProfileId: null }

/** The pull request of the screenshot of 2026-10-17: the bot approved, the code owner is asked */
/** IJPL Code Platform has to approve, Ilya Korennoy is suggested and not asked */
const UNASKED_CODE_OWNER: PatronusReviewRequirement = {
  ...AWAITING_CODE_OWNER,
  groups: [evaluatedGroup("IJPL Code Platform", false, [codeOwner("Ilya Korennoy", "ikorennoy", { suggested: true, approvalState: "reviewing" })])],
}

const AWAITED = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved"), reviewer("ikorennoy", "requested")] })

/**
 * issue_links mirrored to `acme/monorepo`, JIRA-456 pushed (its remote branch has its commits), with these pull requests;
 * Patronus answers the review pre-check of JIRA-456 with `answer`
 */
async function setup(
  page: Page,
  setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  answer: PatronusReviewRequirement,
  pullRequests: PullRequest[],
  robots = availableRobots(),
  options: object = {},
): Promise<TestRepositoryBuilder> {
  await rewriteNextRemoteStatus(page, status =>
    status.branchName === BRANCH ? { ...status, remoteExists: true, unpushedCommits: [], myUnpushedCount: 0, commitsBehind: 0, fastForward: true } : status,
  )
  const repo = await setupRepo("issue_links", { patronus: { ...robots, githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests(...pullRequests), ...options })
  await repo.setReviewAnswers({ answers: { [BRANCH]: answer } })
  await syncAndWaitForBranches()
  await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
  await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
  return repo
}

function slot(page: Page) {
  return findBranchRow(page, BRANCH).getByTestId("next-step-slot")
}

/** The Safe Push form of JIRA-456 from its slot, which offers it; the review block shows `status` */
async function openSafePushForm(page: Page, status: string) {
  await expect(slot(page)).toHaveAttribute("data-step", "safePush")
  await slot(page).getByTestId("safe-push-button").click()
  const form = page.getByTestId("patronus-start-form")
  await expect(form.getByTestId("patronus-review")).toHaveAttribute("data-status", status)
  return form
}

async function openMenu(page: Page) {
  await findBranchRow(page, BRANCH).click({ button: "right" })
  return page.getByRole("menu")
}

test.describe("Next step", () => {
  test("should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [AWAITED])

    // the quiet wait in the slot, which opens the pull request it waits in
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
    const step = slot(page).getByTestId("next-step")
    await expect(step).toHaveText("Awaiting Ilya Korennoy")
    await expect(step).toHaveAttribute("data-tone", "quiet")
    await step.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText("Awaiting Ilya Korennoy · IJPL Code Platform\nOpen Pull Request #1078O")
    // the chip of the pull request keeps its own state: the row doesn't say it twice
    await expect(findBranchRow(page, BRANCH).getByTestId("pull-request-chip")).not.toContainText("Awaiting")
    await step.click()
    expect(await getOpenedUrls(page)).toEqual([AWAITED.url])

    // "⋯": Safe Push… disabled with the reason, Dry Run… needs no approval, Check Approvals Again
    const menu = await openMenu(page)
    await expect(menu.getByRole("menuitem", { name: "Safe Push…" })).toBeDisabled()
    await expect(menu.getByRole("menuitem", { name: "Dry Run…" })).toBeEnabled()
    await expect(menu.getByRole("menuitem", { name: "Check Approvals Again" })).toBeEnabled()
    await menu.getByRole("menuitem", { name: "Safe Push…" }).hover()
    await expect(page.getByRole("tooltip", { name: "Waits for" })).toHaveText("Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #1078")
    await page.keyboard.press("Escape")

    // S on the row: the same reason, a precondition
    await findBranchRow(page, BRANCH).focus()
    await page.keyboard.press("s")
    const toast = page.locator("[data-slot=toast]", { hasText: `Safe Push of ${BRANCH} waits for Ilya Korennoy` })
    await expect(toast).toHaveAttribute("data-type", "info")
    await expect(toast).toContainText("IJPL Code Platform · requested on #1078")
    await expect(toast.getByRole("button", { name: "Open Pull Request" })).toBeVisible()
    await expect(page.getByTestId("patronus-start-form")).toHaveCount(0)
  })

  test("should say in the form whose approvals don't count, and disable Start while Patronus would fail it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // a Patronus that doesn't evaluate the pull request yet: nobody can tell whether the bot's approval counts
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, NOT_EVALUATED, [AWAITED])
    const form = await openSafePushForm(page, "unknown")

    // Patronus evaluates it now: Recheck asks again, not the kept answer
    await repo.setReviewAnswers({ answers: { [BRANCH]: AWAITING_CODE_OWNER } })
    await form.getByTestId("patronus-review-recheck").click()
    const review = form.getByTestId("patronus-review")
    await expect(review).toHaveAttribute("data-status", "missing")
    await expect(review.getByTestId("patronus-review-groups")).toHaveText("IJPL Code Platform· all files: Ilya Korennoy requested, not approved yet")
    await expect(review.getByTestId("patronus-review-not-counted")).toHaveText("junie-jetbrains approved, but isn't a code owner of these files: Patronus doesn't count it.")
    await expect(review.getByTestId("patronus-review-reason")).toHaveText("Patronus checks the approvals when the robot starts and fails without them: they count in #1078.")
    const start = form.getByRole("button", { name: "Start Safe Push" })
    await expect(start).toHaveAttribute("data-disabled-reason", "Patronus checks the approvals when the robot starts and fails without them")
    await start.hover()
    await expect(page.getByRole("tooltip", { name: "Patronus checks" })).toHaveText("Patronus checks the approvals when the robot starts and fails without them")
    expect((await repo.getReviewReadinessCalls()).some(call => call.recheck && call.branches.includes(BRANCH))).toBe(true)
    // the row says it too
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
  })

  test("should offer Safe Push… once the code owner approved, and name whose approval counts", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const approved = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved"), reviewer("ikorennoy", "approved")] })
    await setup(page, setupRepo, syncAndWaitForBranches, APPROVED_BY_CODE_OWNER, [approved])
    const form = await openSafePushForm(page, "approved")
    await expect(form.getByTestId("patronus-review")).toHaveText("Approved by Ilya Korennoy for IJPL Code Platform.")
    await expect(form.getByRole("button", { name: "Start Safe Push" })).toBeEnabled()
  })

  test("should keep Safe Push… while Patronus doesn't tell whose approvals count, and say so", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const botApproved = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved")] })
    await setup(page, setupRepo, syncAndWaitForBranches, NOT_EVALUATED, [botApproved])
    const form = await openSafePushForm(page, "unknown")
    await expect(form.getByTestId("patronus-review-unknown")).toHaveText("Patronus doesn't tell yet whose approvals count: Branch Deck can't check junie-jetbrains's approval.")
    await expect(form.getByRole("button", { name: "Start Safe Push" })).toBeEnabled()
  })

  test("should offer Create Pull Request… with the code owner when approvals count only in one", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [])
    await routeNewPullRequest(page)
    await expect(slot(page)).toHaveAttribute("data-step", "createPullRequest")
    const step = slot(page).getByTestId("next-step")
    await expect(step).toHaveText("Create Pull Request…")

    const menu = await openMenu(page)
    await menu.getByRole("menuitem", { name: "Safe Push…" }).hover()
    await expect(page.getByRole("tooltip", { name: "Code owners" })).toHaveText("Code owners must approve in a pull request: IJPL Code Platform")
    await page.keyboard.press("Escape")

    await step.click()
    const dialog = createPullRequestDialog(page)
    await expect(dialog.getByTestId("create-pull-request-reviewers").locator("[data-slot=combobox-chip]")).toHaveText(["ikorennoy"])
  })

  test("should ask the code owner nobody asked yet from Request Review…, then wait for him", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the bot approved, nobody of IJPL Code Platform is asked
    const unasked = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved")] })
    await setup(page, setupRepo, syncAndWaitForBranches, UNASKED_CODE_OWNER, [unasked])
    const requests = await routeCommand(page, "request_pull_request_reviewers", () => ({ requested: ["ikorennoy"], refused: [] }))
    await expect(slot(page)).toHaveAttribute("data-step", "requestReview")
    const step = slot(page).getByTestId("next-step")
    await expect(step).toHaveText("Request Review…")
    await step.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText(
      "Waits for the approval of IJPL Code Platform: nobody of it is requested on #1078. Ask Ilya Korennoy to review #1078",
    )

    await step.click()
    const popover = page.getByTestId("request-review-popover")
    await expect(popover).toContainText("Request a review on #1078")
    await expect(popover.getByTestId("request-review-candidate-ikorennoy")).toBeChecked()
    // GitHub lists the request from now on
    await routeCommand(page, "get_pull_requests", () => availablePullRequests({ ...unasked, reviewers: [...unasked.reviewers, reviewer("ikorennoy", "requested")] }))
    await popover.getByTestId("request-review-submit").click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Requested a review from Ilya Korennoy on #1078" })).toBeVisible()
    expect(requests).toEqual([expect.objectContaining({ pullRequestNumber: 1078, reviewers: ["ikorennoy"] })])
    await expect(popover).toHaveCount(0)
    // the readiness reads the new request: the wait
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
  })

  test("should open Request Review… from the branch menu, at the row's next step", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [AWAITED])
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
    const menu = await openMenu(page)
    await menu.getByRole("menuitem", { name: "Request Review…" }).click()
    const popover = page.getByTestId("request-review-popover")
    await expect(popover).toContainText("Request a review on #1078")
    // asked already: the code owner is no candidate, the search finds others
    await expect(popover.getByTestId("request-review-candidates")).toHaveCount(0)
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)
  })

  test("should retarget a pull request into the branch of a parent that landed, whose approvals don't count", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the base is the branch of a parent that is no branch of the table any more
    await setup(page, setupRepo, syncAndWaitForBranches, NO_REVIEW_NEEDED, [pullRequest(BRANCH, 7, { baseRefName: "user-name/virtual/landed-parent" })])
    const retargets = await routeCommand(page, "retarget_pull_request", () => null)
    await expect(slot(page)).toHaveAttribute("data-step", "retarget")
    const menu = await openMenu(page)
    await menu.getByRole("menuitem", { name: "Safe Push…" }).hover()
    await expect(page.getByRole("tooltip", { name: "Approvals" })).toHaveText("Approvals in #7 don't count: its base is user-name/virtual/landed-parent")
    await page.keyboard.press("Escape")

    await slot(page).getByRole("button", { name: "Retarget #7 to master" }).click()
    await expect.poll(() => retargets).toEqual([expect.objectContaining({ pullRequestNumber: 7, base: "master" })])
    await expect(page.locator("[data-slot=toast]", { hasText: "Retargeted #7 to master" })).toBeVisible()
    // into the target now: the start from it counts its approvals
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
  })

  test("should open a pull request into another base that may be meant, and keep its retarget in the menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, NO_REVIEW_NEEDED, [pullRequest(BRANCH, 7, { baseRefName: "release-252" })])
    await expect(slot(page)).toHaveAttribute("data-step", "otherBase")
    const step = slot(page).getByTestId("next-step")
    await expect(step).toHaveText("Open Pull Request")
    await expect(step).toHaveAttribute("data-tone", "quiet")
    await step.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText("Approvals in #7 don't count: its base is release-252. Open Pull Request #7O")
    const menu = await openMenu(page)
    await expect(menu.getByRole("menuitem", { name: "Retarget #7 to master" })).toBeEnabled()
    await expect(menu.getByRole("menuitem", { name: "Safe Push…" })).toBeDisabled()
    await page.keyboard.press("Escape")
    // S says it in a short title
    await findBranchRow(page, BRANCH).focus()
    await page.keyboard.press("s")
    const toast = page.locator("[data-slot=toast]", { hasText: `Approvals in #7 of ${BRANCH} don't count` })
    await expect(toast).toContainText("Its base is release-252, not master")
    await expect(toast.getByRole("button", { name: "Open Pull Request" })).toBeVisible()
  })

  test("should offer nothing after a merge, and say so in the menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const merged = robot(BRANCH, "merged-robot", { status: "success", finishTime: 1704142800 })
    await setup(page, setupRepo, syncAndWaitForBranches, NO_REVIEW_NEEDED, [], availableRobots(merged))
    await expect(slot(page)).toHaveAttribute("data-step", "none")
    const menu = await openMenu(page)
    await expect(menu.getByRole("menuitem", { name: "Safe Push…" })).toBeDisabled()
    await expect(menu.getByRole("menuitem", { name: "Dry Run…" })).toBeDisabled()
    await menu.getByRole("menuitem", { name: "Safe Push…" }).hover()
    await expect(page.getByRole("tooltip", { name: "Merged" })).toHaveText("Merged by Patronus")
  })

  test("should tell the approval that arrives with a poll, with Safe Push… of the branch first", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the review requests are polled: the poll asks for the user's pull requests too while a row waits
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [AWAITED], availableRobots(), { reviewRequests: availableReviewRequests([]) })
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")

    const approved = pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved"), reviewer("ikorennoy", "approved")] })
    await repo.setPullRequests(availablePullRequests(approved))
    await repo.setReviewAnswers({ answers: { [BRANCH]: APPROVED_BY_CODE_OWNER } })
    await page.clock.fastForward("05:00")
    const toast = page.locator("[data-slot=toast]", { hasText: `Ilya Korennoy approved ${BRANCH}` })
    await expect(toast).toBeVisible()
    await expect(toast).toContainText("#1078 · IJPL Code Platform")
    await expect(toast.getByRole("button", { name: "Open Pull Request" })).toBeVisible()
    // the row has the step already
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")

    await toast.getByRole("button", { name: "Safe Push…" }).click()
    const form = page.getByTestId("patronus-start-form")
    await expect(form.getByTestId("patronus-review")).toHaveText("Approved by Ilya Korennoy for IJPL Code Platform.")
    await expect(form.getByRole("button", { name: "Start Safe Push" })).toBeEnabled()
    // Activity keeps it
    await page.getByRole("button", { name: "Activity" }).click()
    await expect(page.getByTestId("activity-popover")).toContainText(`Ilya Korennoy approved ${BRANCH}`)
  })

  test("should offer only the push when it dismisses the approvals Patronus counts", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // JIRA-456 has a commit the remote lacks; the code owner approved #1078, and the rules dismiss stale reviews
    await rewriteNextRemoteStatus(page, status =>
      status.branchName === BRANCH
        ? { ...status, remoteExists: true, unpushedCommits: [status.unpushedCommits[0] ?? "c1"], myUnpushedCount: 1, commitsBehind: 0, fastForward: true }
        : status,
    )
    const approved = pullRequest(BRANCH, 1078, { reviewers: [reviewer("ikorennoy", "approved")] })
    const repo = await setupRepo("issue_links", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests(approved) })
    await repo.setReviewAnswers({ answers: { [BRANCH]: APPROVED_BY_CODE_OWNER } })
    await syncAndWaitForBranches()
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    await routeCommand(page, "get_branch_rules", () => ({ dismissStaleReviewsOnPush: true }))
    const pushes = await routePush(page, {})

    await slot(page).getByTestId("safe-push-button").click()
    const form = page.getByTestId("patronus-start-form")
    await expect(form.getByTestId("patronus-start-dismisses-approvals")).toBeVisible()
    const start = form.getByRole("button", { name: "Push and Start Safe Push" })
    await expect(start).toHaveAttribute("data-disabled-reason", "The push dismisses the approvals of #1078: push first, then ask for the review again")
    await form.getByTestId("patronus-start-push-only").click()
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual([BRANCH])
  })

  test("should ask Patronus again from Check Approvals Again, after the pull request from GitHub", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, AWAITING_CODE_OWNER, [AWAITED])
    await expect(slot(page)).toHaveAttribute("data-step", "awaiting")
    await repo.setReviewAnswers({ answers: { [BRANCH]: APPROVED_BY_CODE_OWNER } })
    const menu = await openMenu(page)
    await menu.getByRole("menuitem", { name: "Check Approvals Again" }).click()
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
    expect((await repo.getReviewReadinessCalls()).at(-1)).toEqual({ branches: [BRANCH], recheck: true })
  })
})
