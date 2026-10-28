// @spec spec/branch/landing-routes.spec.md
import type { Locator, Page } from "@playwright/test"
import type { ChecksSummary, PullRequest } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getLandingNode } from "../helpers/branch-helpers"
import { availablePullRequests, gitHubMutations, pullRequest } from "../helpers/pull-request-helpers"
import { virtualBranch } from "../helpers/push-helpers"
import { openSafePushWithKey, popoverStart, waitForStepFacts } from "../helpers/safe-push-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

// the `issue_links` template: 7 branches off Space, none pushed; JIRA-456 has 3 commits. The repository lands by a merge
// of the pull request on GitHub (`branchdeck.landing=pull-request`, `origin` pushing to acme/monorepo), which allows
// every merge method and auto-merge; JIRA-456 has the open pull request #7 into master on its copy

const BRANCH = "JIRA-456"
const PASSING: ChecksSummary = { state: "passing", passed: 3, failed: 0, pending: 0, failedNames: [] }
const RUNNING: ChecksSummary = { state: "pending", passed: 2, failed: 0, pending: 1, failedNames: [] }

/** The pull request #7 of JIRA-456 as GitHub lists it, approved and green unless `patch` says otherwise */
function seven(patch: Partial<PullRequest> = {}): PullRequest {
  return pullRequest(BRANCH, 7, { baseRefName: "master", reviewDecision: "approved", checks: PASSING, mergeState: "clean", ...patch })
}

/** The GitHub of the pull request `pull` of a repository that allows every merge method and auto-merge */
function merging(pull: PullRequest) {
  return { ...availablePullRequests(pull), mergeMethods: { rebase: true, merge: true, squash: true, autoMerge: true } }
}

/** The repository of `issue_links` that lands by a merge of the pull request `pull`, synced, the facts of the steps in */
async function setup(
  page: Page,
  setupRepo: (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  pull: PullRequest = seven(),
): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("issue_links", { pullRequests: merging(pull), landing: { setting: "pull-request", gitHub: "acme/monorepo" } })
  await syncAndWaitForBranches()
  await waitForStepFacts(findBranchRow(page, BRANCH).getByTestId("next-step-slot"))
  return repo
}

/** The step of the row: "Merge #7" while GitHub can merge it */
function mergeStep(page: Page): Locator {
  return findBranchRow(page, BRANCH).getByTestId("safe-push-button")
}

/** The toast of the merge or the auto-merge of #7, by its title */
function mergeToast(page: Page, title: string): Locator {
  return page.locator("[data-slot=toast]", { hasText: title })
}

/**
 * GitHub merged #7 on the head the user saw by rebase (`expectedHeadOid`): the remote's master is the copy, the toast
 * says what landed, and the update that follows drops the originals
 */
async function expectMerged(page: Page, repo: TestRepositoryBuilder, copy: string): Promise<void> {
  await expect.poll(async () => gitHubMutations(await repo.getGitHubLog())).toEqual([["mergePullRequest", { id: "PR_7", method: "REBASE", head: copy }]])
  await expect.poll(async () => (await repo.getRemoteBranches())["master"]).toBe(copy)
  const toast = mergeToast(page, `Merged #7 of ${BRANCH} into master · 3 commits`)
  await expect(toast).toBeVisible()
  await expect(toast).toContainText(`${BRANCH} landed in origin/master · Updated master`)
  await expect(findBranchRow(page, BRANCH)).toHaveCount(0)
}

test.describe("Merge #7", () => {
  test("should merge the approved green pull request from the slot of the row by rebase, and update after it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    await expect(mergeStep(page)).toHaveText("Merge #7")
    await expect(getLandingNode(findBranchRow(page, BRANCH))).toHaveAttribute("data-kind", "ready")
    const copy = await repo.revision(virtualBranch(BRANCH))
    // it asks nothing: the click merges at once
    await mergeStep(page).click()
    await expectMerged(page, repo, copy)
  })

  test("should open the popover of Merge #7 with S on the row, naming the method", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const copy = await repo.revision(virtualBranch(BRANCH))
    const popover = await openSafePushWithKey(page, BRANCH)
    await expect(popover.getByTestId("safe-push-lands")).toHaveText(`GitHub merges #7 of ${BRANCH} into master by rebase: 3 commits.`)
    await expect(popover.getByTestId("safe-push-dry-run")).toHaveCount(0)
    await expect(popoverStart(page)).toHaveText("Merge #7")
    await expect(popoverStart(page)).toBeFocused()
    await page.keyboard.press("Enter")
    await expectMerged(page, repo, copy)
  })

  test("should merge from the menu of the row", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const copy = await repo.revision(virtualBranch(BRANCH))
    await findBranchRow(page, BRANCH).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Merge #7" }).click()
    await expectMerged(page, repo, copy)
  })

  test("should enable the auto-merge while the required checks run, and offer Update Project once GitHub merged it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, seven({ mergeState: "blocked", checks: RUNNING }))
    await expect(mergeStep(page)).toHaveText("Auto-Merge #7")
    const copy = await repo.revision(virtualBranch(BRANCH))
    await mergeStep(page).click()
    await expect.poll(async () => gitHubMutations(await repo.getGitHubLog())).toEqual([["enablePullRequestAutoMerge", { id: "PR_7", method: "REBASE", head: copy }]])
    const enabled = mergeToast(page, "Auto-Merge of #7 is on")
    await expect(enabled).toContainText("GitHub merges it into master by rebase once its checks pass")
    const slot = findBranchRow(page, BRANCH).getByTestId("next-step-slot")
    await expect(slot).toContainText("Merges when checks pass")
    await expect(getLandingNode(findBranchRow(page, BRANCH))).toHaveAttribute("data-kind", "landing")
    // the checks passed, GitHub merged it: the listing of the next sync tells it (a sync lists at most once a minute,
    // on the clock of the session), no update ran by itself
    await repo.setPullRequests(merging(seven({ state: "merged", autoMerge: "rebase" })))
    await repo.advanceBackendClock(61_000)
    await syncAndWaitForBranches()
    const merged = mergeToast(page, "Merged #7 into master")
    await expect(merged).toBeVisible()
    await expect(merged.getByRole("button", { name: "Update Project" })).toBeVisible()
  })

  test("should cancel the auto-merge from the menu of the row", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, seven({ mergeState: "blocked", checks: RUNNING, autoMerge: "rebase" }))
    await expect(findBranchRow(page, BRANCH).getByTestId("next-step-slot")).toContainText("Merges when checks pass")
    await findBranchRow(page, BRANCH).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Cancel Auto-Merge" }).click()
    await expect.poll(async () => gitHubMutations(await repo.getGitHubLog())).toEqual([["disablePullRequestAutoMerge", { id: "PR_7" }]])
    await expect(mergeToast(page, "Canceled the auto-merge of #7")).toBeVisible()
  })

  test("should disable Merge with the reason while the pull request waits for the approvals its base requires", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, seven({ mergeState: "blocked", reviewDecision: "reviewRequired" }))
    await expect(findBranchRow(page, BRANCH).getByTestId("next-step-slot")).toContainText("Awaiting Approval")
    await expect(getLandingNode(findBranchRow(page, BRANCH))).toHaveAttribute("data-kind", "waiting")
    await findBranchRow(page, BRANCH).click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Merge" })).toBeDisabled()
    await page.keyboard.press("Escape")
    // S says why, and merges nothing
    await page.keyboard.press("s")
    expect(gitHubMutations(await repo.getGitHubLog())).toEqual([])
  })
})
