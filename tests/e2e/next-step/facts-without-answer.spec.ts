// @spec spec/branch/next-step.spec.md
// @spec spec/branch/pull-requests.spec.md
import type { Page } from "@playwright/test"
import type { PatronusReviewRequirement, PullRequest } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getLandingNode } from "../helpers/branch-helpers"
import { clearClipboard, readClipboard } from "../helpers/clipboard-helpers"
import { APPROVED_BY_CODE_OWNER, AWAITING_CODE_OWNER, availableRobots, failedFastTests, robot, unavailableRobots } from "../helpers/patronus-helpers"
import { availablePullRequests, createPullRequestDialog, gitHubRequests, pullRequest, reviewer, unavailablePullRequests } from "../helpers/pull-request-helpers"
import { hoverStatusChip, landingRow, openLandingOverview } from "../helpers/review-status-helpers"
import { openSafePushWithKey, popoverStart, safePushPopover, slotSafePush } from "../helpers/safe-push-helpers"
import { oneCommitUnpushedOnNextSync, pushedOnNextSync } from "../helpers/sync-helpers"
import type { HeldAnswer, TestRepositoryBuilder } from "../helpers/test-repository"

const BRANCH = "JIRA-456"

/** #1078 of JIRA-456, approved by the code owner Patronus counts */
const APPROVED = pullRequest(BRANCH, 1078, { reviewers: [reviewer("ikorennoy", "approved")] })

/** "Safe Push" (its key S in the name), not "Safe Push With Profile…" nor "Safe Push Without Review…" */
const SAFE_PUSH_ITEM = { name: /^Safe Push( S)?$/ }

/**
 * issue_links mirrored to `acme/monorepo`, JIRA-456 pushed, with these pull requests, Patronus answering the review
 * pre-check of JIRA-456 with `answer`; the answers `held` are held at the fakes from before the first sync, so the
 * rows come before them
 */
async function setupHeld(
  page: Page,
  setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
  answer: PatronusReviewRequirement,
  pullRequests: PullRequest[],
  held: HeldAnswer[],
): Promise<TestRepositoryBuilder> {
  await pushedOnNextSync(page, BRANCH)
  const repo = await setupRepo("issue_links", {
    patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
    pullRequests: { ...availablePullRequests(...pullRequests), users: [{ login: "ikorennoy", name: "Ilya Korennoy" }] },
  })
  await repo.setReviewAnswers({ answers: { [BRANCH]: answer } })
  await repo.holdRemoteAnswers({ hold: held })
  return repo
}

/** Patronus fails from now on, asked again from the landing overview, which says it */
async function failPatronusRefresh(page: Page, repo: TestRepositoryBuilder): Promise<void> {
  await repo.setPatronus(unavailableRobots({ status: "error", message: "Patronus is down" }))
  const overview = await openLandingOverview(page)
  await overview.getByTestId("patronus-refresh").click()
  await expect(overview.getByTestId("patronus-status-line")).toContainText("Patronus: ")
  await page.keyboard.press("Escape")
  await expect(overview).toBeHidden()
}

function slot(page: Page) {
  return findBranchRow(page, BRANCH).getByTestId("next-step-slot")
}

/** The page asked for `answer`, which the fake holds: the page shows what it shows before it */
async function expectHeld(repo: TestRepositoryBuilder, answer: HeldAnswer): Promise<void> {
  await expect.poll(() => repo.getArrivedAnswers()).toContain(answer)
}

/** The step waits with "Checking…", disabled, `reason` on hover, the node of the row pending too */
async function expectChecking(page: Page, reason: string): Promise<void> {
  await expect(slot(page)).toHaveAttribute("data-step", "pending")
  const step = slot(page).getByTestId("next-step")
  await expect(step).toHaveText("Checking…")
  await expect(step).toBeDisabled()
  await expect(step).toHaveAttribute("aria-busy", "true")
  await step.hover()
  await expect(page.getByTestId("next-step-hover")).toHaveText(reason)
  await expect(getLandingNode(findBranchRow(page, BRANCH))).toHaveAttribute("data-kind", "waiting")
}

/** "⋯" of the row: Safe Push disabled with `reason`, which its hover says */
async function expectMenuSafePushDisabled(page: Page, reason: string | RegExp): Promise<void> {
  await expectMenuItemsDisabled(page, BRANCH, [SAFE_PUSH_ITEM], reason)
}

/** "⋯" of the row of `branch`: the `items` disabled with `reason`, which the hover of each says, each in a menu of its own */
async function expectMenuItemsDisabled(page: Page, branch: string, items: Array<{ name: string | RegExp }>, reason: string | RegExp): Promise<void> {
  for (const name of items) {
    await findBranchRow(page, branch).click({ button: "right" })
    const item = page.getByRole("menu").getByRole("menuitem", name)
    await expect(item).toBeDisabled()
    await item.hover()
    // the open one: the pointer crossed the item above on its way, whose tooltip closes
    await expect(page.getByRole("tooltip", { name: reason }).and(page.locator("[data-open]"))).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(page.getByRole("menu")).toHaveCount(0)
  }
}

/**
 * The robot of `branch` in the landing overview and in the details of its chip: no restart and no prompt to take, the
 * details saying why (`reason`). The overview shows the branch waiting with its step while the problems are to come
 * (`checking`), its robot to fix with the restart and the prompt disabled once Patronus didn't give them
 */
async function expectRobotSurfacesWait(page: Page, branch: string, reason: string | RegExp, overviewShows: "checking" | "robot"): Promise<void> {
  const overview = await openLandingOverview(page)
  if (overviewShows === "checking") {
    await expect(landingRow(overview, branch).getByTestId("next-step")).toHaveText("Checking…")
    await expect(overview.getByRole("button", { name: `Restart Safe Push of ${branch}` })).toHaveCount(0)
  } else {
    const robotRow = overview.locator(`[data-testid="robots-row"][data-branch-name="${branch}"]`)
    await expect(robotRow.getByRole("button", { name: `Restart Safe Push of ${branch}` })).toBeDisabled()
    await expect(robotRow.getByRole("button", { name: `Copy Prompt for Agent of ${branch}` })).toBeDisabled()
  }
  await page.mouse.move(0, 0)
  await expect(overview).toBeHidden()
  const details = await hoverStatusChip(page, findBranchRow(page, branch).getByTestId("patronus-chip"), "patronus-popover")
  await expect(details.getByTestId("patronus-problems-unknown")).toHaveText(reason)
  await expect(details.getByRole("button", { name: "Restart Safe Push" })).toBeDisabled()
  await page.mouse.move(0, 0)
  await expect(details).toBeHidden()
}

/** Check Approvals Again of the row's "Patronus ›": GitHub is asked for the pull requests, then Patronus again */
async function checkApprovalsAgain(page: Page, branch = BRANCH): Promise<void> {
  await findBranchRow(page, branch).click({ button: "right" })
  await page.getByRole("menuitem", { name: "Patronus", exact: true }).hover()
  await page.getByRole("menuitem", { name: "Check Approvals Again" }).click()
  await expect(page.getByRole("menu")).toHaveCount(0)
}

/** S on the row says what the step waits for, as an `info` notice: no popover, no start */
async function expectNoticeOnS(page: Page, title: string, description: string): Promise<void> {
  await findBranchRow(page, BRANCH).focus()
  await page.keyboard.press("s")
  const toast = page.locator("[data-slot=toast]", { hasText: title })
  await expect(toast).toHaveAttribute("data-type", "info")
  await expect(toast).toContainText(description)
  await expect(safePushPopover(page)).toHaveCount(0)
}

test.describe("Next step before the answers", () => {
  test("should wait with Checking… for the pull requests and the approvals, S and the menu saying why, then start the Safe Push they allow", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupHeld(page, setupRepo, APPROVED_BY_CODE_OWNER, [APPROVED], ["pullRequests", "readiness"])
    await syncAndWaitForBranches()

    // GitHub hasn't listed the pull requests: no pull request is known, which is no "no pull request"
    await expectHeld(repo, "pullRequests")
    await expectChecking(page, "Waits for GitHub to list the pull requests")
    await expect(findBranchRow(page, BRANCH).getByTestId("pull-request-chip")).toHaveCount(0)
    await expectMenuSafePushDisabled(page, "Waits for GitHub to list the pull requests")
    await expectNoticeOnS(page, `Safe Push of ${BRANCH} waits for GitHub`, "Waits for GitHub to list the pull requests")

    // GitHub answered: the pull request is there, Patronus is asked whether its approvals count
    await repo.holdRemoteAnswers({ release: ["pullRequests"] })
    await expect(findBranchRow(page, BRANCH).getByTestId("pull-request-chip")).toBeVisible()
    await expectHeld(repo, "readiness")
    await expectChecking(page, "Waits for Patronus to check the approvals")
    await expectMenuSafePushDisabled(page, "Waits for Patronus to check the approvals")

    // Patronus answered: the Safe Push it allows, S opens its popover, a click starts it from the pull request
    await repo.holdRemoteAnswers({ release: ["readiness"] })
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
    await expect(getLandingNode(findBranchRow(page, BRANCH))).toHaveAttribute("data-kind", "ready")
    const popover = await openSafePushWithKey(page, BRANCH)
    await expect(popover.getByTestId("safe-push-review")).toHaveText("Approved by Ilya Korennoy for IJPL Code Platform.")
    await page.keyboard.press("Escape")
    await expect(popover).toHaveCount(0)
    // no start before: the comment of the click is the first one
    expect(await gitHubRequests(repo, "POST", "issues", 1078, "comments")).toEqual([])
    await slotSafePush(page, BRANCH).click()
    await expect.poll(() => gitHubRequests(repo, "POST", "issues", 1078, "comments")).toEqual([{ body: "/patronus merge --keep-source-branch" }])
  })

  test("should never offer Create Pull Request… before GitHub lists the pull requests, say a failed listing, and keep the last listing through a failed refresh", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupHeld(page, setupRepo, AWAITING_CODE_OWNER, [], ["pullRequests"])
    // GitHub fails its first answer
    await repo.setPullRequests(unavailablePullRequests({ status: "error", message: "offline" }))
    await syncAndWaitForBranches()

    // the branch may have a pull request: neither the step nor the menu creates one
    await expectHeld(repo, "pullRequests")
    await expectChecking(page, "Waits for GitHub to list the pull requests")
    await findBranchRow(page, BRANCH).click({ button: "right" })
    const create = page.getByRole("menu").getByRole("menuitem", { name: "Create Pull Request…" })
    await expect(create).toBeDisabled()
    await create.hover()
    await expect(page.getByRole("tooltip", { name: "Waits for GitHub to list the pull requests" })).toBeVisible()
    await page.keyboard.press("Escape")

    // GitHub failed: the pull requests are unknown, the start counts no approval of one and says why
    await repo.holdRemoteAnswers({ release: ["pullRequests"] })
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
    await expect(slot(page).getByTestId("next-step")).toHaveCount(0)
    await slotSafePush(page, BRANCH).hover()
    await expect(page.getByRole("tooltip", { name: "Lands" })).toHaveText(
      `Lands ${BRANCH} into master once the checks pass · GitHub: offline: Patronus checks the approvals when the robot startsS`,
    )
    await expect(getLandingNode(findBranchRow(page, BRANCH))).toHaveAttribute("data-kind", "yourStep")

    // GitHub lists none now, Patronus needs the code owner: the pull request is the step
    await repo.setPullRequests(availablePullRequests())
    await checkApprovalsAgain(page)
    await expect(slot(page)).toHaveAttribute("data-step", "createPullRequest")
    await expect(slot(page).getByTestId("next-step")).toHaveText("Create Pull Request…")

    // a refresh GitHub fails keeps the last listing: the step stays, never a Safe Push the approvals refuse
    const rechecks = async () => (await repo.getReviewReadinessCalls()).filter(call => call.recheck).length
    const before = await rechecks()
    await repo.setPullRequests(unavailablePullRequests({ status: "error", message: "offline" }))
    await checkApprovalsAgain(page)
    // Patronus is asked again once the failed answer of GitHub is in
    await expect.poll(rechecks).toBeGreaterThan(before)
    await expect(slot(page)).toHaveAttribute("data-step", "createPullRequest")
    await expect(slot(page).getByTestId("next-step")).toBeEnabled()
  })

  test("should preselect the code owners in Create Pull Request… from the answer that comes after the dialog opened", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupHeld(page, setupRepo, AWAITING_CODE_OWNER, [], ["readiness"])
    await syncAndWaitForBranches()
    await expectHeld(repo, "readiness")
    await expectChecking(page, "Waits for Patronus to check the approvals")

    // GitHub listed no pull request: the dialog opens, its code owners still to come
    await findBranchRow(page, BRANCH).click({ button: "right" })
    await page.getByRole("menu").getByRole("menuitem", { name: "Create Pull Request…" }).click()
    const dialog = createPullRequestDialog(page)
    await expect(dialog).toContainText("Asking Patronus for the code owners…")
    await expect(dialog.getByTestId("create-pull-request-reviewers").locator("[data-slot=combobox-chip]")).toHaveCount(0)

    // Patronus names them: the code owner is chosen, as the dialog would have opened with him
    await repo.holdRemoteAnswers({ release: ["readiness"] })
    await expect(dialog.getByTestId("create-pull-request-reviewers").locator("[data-slot=combobox-chip]")).toHaveText(["ikorennoy"])
  })

  test("should say in Request Review… while Patronus names no code owners, when it fails to, and when the rules of the branch can't be read", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // a commit the remote lacks and a pull request someone approved: the request pushes into it, so the rules matter
    await oneCommitUnpushedOnNextSync(page, BRANCH)
    const repo = await setupRepo("issue_links", {
      patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
      pullRequests: availablePullRequests(pullRequest(BRANCH, 1078, { reviewers: [reviewer("junie-jetbrains", "approved")], baseRefName: "master" })),
    })
    await repo.holdRemoteAnswers({ hold: ["readiness"], fail: ["branchRules"] })
    await syncAndWaitForBranches()
    await expectHeld(repo, "readiness")

    await findBranchRow(page, BRANCH).click({ button: "right" })
    await page.getByRole("menu").getByRole("menuitem", { name: "Request Review…" }).click()
    const popover = page.getByTestId("request-review-popover")
    await expect(popover).toContainText("Request a review on #1078")
    // asked, not answered: no code owner is known, which is no "none needed"
    const codeOwners = popover.getByTestId("request-review-code-owners-unknown")
    await expect(codeOwners).toHaveText("Asking Patronus which code owners must approve…")
    // the push may take the approval away: rules GitHub didn't give are said, never read as keeping it
    await expect(popover.getByTestId("request-review-rules-unknown")).toHaveText(
      "Cannot check whether the push of the new commits keeps the approvals of #1078: the fixture fails this read.",
    )

    // Patronus fails the check: the popover says so, the reviewers are the user's to choose
    await repo.holdRemoteAnswers({ fail: ["readiness"], release: ["readiness"] })
    await expect(codeOwners).toHaveText(/^Patronus didn't answer which code owners must approve \(.+\): ask the reviewers yourself\.$/)
    await expect(popover.getByTestId("request-review-candidates")).toHaveCount(0)
  })

  test("should offer the login instead of a Safe Push while the Safe Push CLI is not logged in, and a refresh while Patronus fails", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const login = "Run ./safePush.cmd once to log in to Patronus"
    const repo = await setupRepo("issue_links", { patronus: unavailableRobots({ status: "notLoggedIn" }) })
    await syncAndWaitForBranches()

    // no robot is known: the step is the login, its command copied from the slot (the tests run as Windows: PowerShell)
    const step = slot(page).getByTestId("next-step")
    await expect(slot(page)).toHaveAttribute("data-step", "pending")
    await expect(step).toHaveText("Copy Login Command")
    await step.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText(`${login}: the command goes to the clipboard, for a terminal`)
    await step.click()
    await expect.poll(() => readClipboard(page)).toMatch(/^cd .+; \.\\safePush\.cmd$/)
    await expectMenuSafePushDisabled(page, login)
    // S says why no Safe Push starts, the login from its toast
    await clearClipboard(page)
    await expectNoticeOnS(page, `Safe Push of ${BRANCH} can't start`, login)
    await page
      .locator("[data-slot=toast]", { hasText: `Safe Push of ${BRANCH} can't start` })
      .getByRole("button", { name: "Copy Login Command" })
      .click()
    await expect.poll(() => readClipboard(page)).toMatch(/safePush\.cmd$/)

    // logged in, Patronus fails (asked again from the landing overview): the step asks it again, and the answer decides
    // the Safe Push
    await failPatronusRefresh(page, repo)
    await expect(step).toHaveText("Refresh Patronus")
    // the failure of the fake (its 500) is the reason
    await expectMenuSafePushDisabled(page, /^Patronus: \S/)
    await repo.setPatronus({ ...availableRobots(), githubRepository: "acme/monorepo" })
    await step.click()
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
  })

  test("should leave the Safe Push without review to the user while GitHub is signed out, from the pull request dialog", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await pushedOnNextSync(page, BRANCH)
    await setupRepo("issue_links", {
      patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
      pullRequests: unavailablePullRequests({ status: "signedOut" }),
    })
    await syncAndWaitForBranches()
    // no pull request is known and the approvals may be missing (row 7): the start without review stays possible
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
    await findBranchRow(page, BRANCH).click({ button: "right" })
    await page.getByRole("menu").getByRole("menuitem", { name: "Create Pull Request…" }).click()
    const dialog = createPullRequestDialog(page)
    await dialog.getByRole("button", { name: "Safe Push Without Review…" }).click()
    await expect(dialog).toHaveCount(0)
    await expect(safePushPopover(page)).toBeVisible()
    await expect(popoverStart(page)).toHaveText("Safe Push Without Review")
  })

  test("should offer no start while Patronus gave no view of its robots, and stand on what it listed through a failed refresh", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupHeld(page, setupRepo, APPROVED_BY_CODE_OWNER, [APPROVED], [])
    await repo.holdRemoteAnswers({ fail: ["repositoryView"] })
    await syncAndWaitForBranches()

    // what the robots hold, land and need is unknown, which is no "none": the step asks Patronus again
    const step = slot(page).getByTestId("next-step")
    const noView = "Patronus didn't tell what its robots hold and land (the fixture fails this read)"
    await expect(step).toHaveText("Refresh Patronus")
    await step.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText(noView)
    await expectMenuSafePushDisabled(page, noView)
    await repo.holdRemoteAnswers({ release: ["repositoryView"] })
    await step.click()
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")

    // a refresh that fails after the list leaves the steps on the robots it listed, as GitHub's last listing stays
    await failPatronusRefresh(page, repo)
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
    await expect(slotSafePush(page, BRANCH)).toBeEnabled()
  })

  test("should restart a failed Safe Push and copy its prompt only once Patronus tells why it failed, and say a failed answer", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const failed = "ABC-123"
    const repo = await setupRepo("issue_links", { patronus: availableRobots(robot(failed, "failed-robot", { status: "failure", finishTime: 1704142800 })) })
    // the CI failed it, not the approvals
    await repo.setPatronusDetails({ ...failedFastTests("failed-robot"), problems: { "failed-robot": [] } })
    await repo.holdRemoteAnswers({ hold: ["robotProblems"] })
    await syncAndWaitForBranches()
    const failedSlot = findBranchRow(page, failed).getByTestId("next-step-slot")
    const step = failedSlot.getByTestId("next-step")
    const unknownFailure = [{ name: "Restart Safe Push" }, { name: "Copy Prompt for Agent" }]

    // why it failed is still to come: no restart, no prompt as if it hadn't failed for the approvals
    await expectHeld(repo, "robotProblems")
    await expect(failedSlot).toHaveAttribute("data-step", "pending")
    await expect(step).toHaveText("Checking…")
    await expectMenuItemsDisabled(page, failed, unknownFailure, "Waits for Patronus to tell why the Safe Push failed")
    await expectRobotSurfacesWait(page, failed, "Waits for Patronus to tell why the Safe Push failed", "checking")

    // Patronus fails to tell it: the restart stays disabled, with the failure and the way to ask again
    await repo.holdRemoteAnswers({ fail: ["robotProblems"], release: ["robotProblems"] })
    const failure = /^Patronus didn't tell why the Safe Push failed \(.+\): Check Approvals Again asks again$/
    await expect(step).toHaveText("Restart Safe Push")
    await expect(step).toBeDisabled()
    await step.hover()
    await expect(page.getByTestId("next-step-hover")).toHaveText(failure)
    await expectMenuItemsDisabled(page, failed, unknownFailure, failure)
    await expectRobotSurfacesWait(page, failed, failure, "robot")

    // asked again, Patronus tells it: the restart and the prompt act
    await repo.holdRemoteAnswers({ release: ["robotProblems"] })
    await checkApprovalsAgain(page, failed)
    await expect(failedSlot).toHaveAttribute("data-step", "restart")
    await findBranchRow(page, failed).click({ button: "right" })
    await page.getByRole("menu").getByRole("menuitem", { name: "Copy Prompt for Agent" }).click()
    await expect.poll(() => readClipboard(page)).toContain(`Safe Push of \`${failed}\` into \`master\` failed`)
    await step.click()
    await expect.poll(async () => (await repo.getPatronusLog()).restarts).toEqual(["failed-robot"])
  })
})
