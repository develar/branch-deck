// @spec spec/branch/next-step.spec.md
import type { Page } from "@playwright/test"
import type { PatronusReviewRequirement, PullRequest } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getLandingNode } from "../helpers/branch-helpers"
import { APPROVED_BY_CODE_OWNER, AWAITING_CODE_OWNER, availableRobots } from "../helpers/patronus-helpers"
import { availablePullRequests, gitHubRequests, pullRequest, reviewer, unavailablePullRequests } from "../helpers/pull-request-helpers"
import { openSafePushWithKey, safePushPopover, slotSafePush } from "../helpers/safe-push-helpers"
import { rewriteNextRemoteStatus } from "../helpers/sync-helpers"
import type { HeldAnswer, TestRepositoryBuilder } from "../helpers/test-repository"

const BRANCH = "JIRA-456"

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
  await rewriteNextRemoteStatus(page, status =>
    status.branchName === BRANCH ? { ...status, remoteExists: true, unpushedCommits: [], myUnpushedCount: 0, commitsBehind: 0, fastForward: true } : status,
  )
  const repo = await setupRepo("issue_links", {
    patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
    pullRequests: { ...availablePullRequests(...pullRequests), users: [{ login: "ikorennoy", name: "Ilya Korennoy" }] },
  })
  await repo.setReviewAnswers({ answers: { [BRANCH]: answer } })
  await repo.holdRemoteAnswers({ hold: held })
  return repo
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
async function expectMenuSafePushDisabled(page: Page, reason: string): Promise<void> {
  await findBranchRow(page, BRANCH).click({ button: "right" })
  const item = page.getByRole("menu").getByRole("menuitem", SAFE_PUSH_ITEM)
  await expect(item).toBeDisabled()
  await item.hover()
  await expect(page.getByRole("tooltip", { name: reason })).toBeVisible()
  await page.keyboard.press("Escape")
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
    const approved = pullRequest(BRANCH, 1078, { reviewers: [reviewer("ikorennoy", "approved")] })
    const repo = await setupHeld(page, setupRepo, APPROVED_BY_CODE_OWNER, [approved], ["pullRequests", "readiness"])
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

  test("should never offer Create Pull Request… before GitHub lists the pull requests, and show a failed listing, not a missing pull request", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupHeld(page, setupRepo, AWAITING_CODE_OWNER, [], ["pullRequests"])
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

    // GitHub lists none, Patronus needs the code owner: the pull request is the step
    await repo.holdRemoteAnswers({ release: ["pullRequests"] })
    await expect(slot(page)).toHaveAttribute("data-step", "createPullRequest")
    await expect(slot(page).getByTestId("next-step")).toHaveText("Create Pull Request…")

    // GitHub fails the next listing: the pull requests are unknown, the start counts no approval of one and says why
    await repo.setPullRequests(unavailablePullRequests({ status: "error", message: "offline" }))
    await findBranchRow(page, BRANCH).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Patronus" }).hover()
    await page.getByRole("menuitem", { name: "Check Approvals Again" }).click()
    await expect(slot(page)).toHaveAttribute("data-step", "safePush")
    await expect(slot(page).getByTestId("next-step")).toHaveCount(0)
    await slotSafePush(page, BRANCH).hover()
    await expect(page.getByRole("tooltip", { name: "Lands" })).toHaveText(
      `Lands ${BRANCH} into master once the checks pass · GitHub: offline: Patronus checks the approvals when the robot startsS`,
    )
    await expect(getLandingNode(findBranchRow(page, BRANCH))).toHaveAttribute("data-kind", "yourStep")
  })
})
