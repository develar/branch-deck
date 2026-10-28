// @spec spec/branch/landing-routes.spec.md
import type { Locator, Page } from "@playwright/test"
import type { BranchReviewRequest } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getLandingNode } from "../helpers/branch-helpers"
import { availablePullRequests, gitHubWrites, pullRequest, repoPath } from "../helpers/pull-request-helpers"
import { virtualBranch } from "../helpers/push-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { openSafePushWithKey, popoverStart, safePushPopover, waitForStepFacts } from "../helpers/safe-push-helpers"
import { openSubmenu } from "../helpers/selection-helpers"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { emitTauriEvent, runAndWaitForSync, runAndWaitForUpdate } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { closeToast } from "../helpers/toast-helpers"

// the `issue_links` template: 7 branches off Space, none pushed; JIRA-456 has 3 commits. The repository lands by a push
// of the copy into master (`branchdeck.landing=push`), its GitHub the fake one of acme/monorepo with no pull request

const BRANCH = "JIRA-456"

/** The repository of `issue_links` that lands by a push into master, synced, the facts of the steps in */
async function setup(
  page: Page,
  setupRepo: (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  options: SetupRepoOptions = {},
  { template = "issue_links", branch = BRANCH }: { template?: string; branch?: string } = {},
): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo(template, { pullRequests: availablePullRequests(), landing: { setting: "push" }, ...options })
  await syncAndWaitForBranches()
  await waitForStepFacts(findBranchRow(page, branch).getByTestId("next-step-slot"))
  return repo
}

/** The step of the row: "Push to master" of a repository that lands by a push */
function pushStep(page: Page, branchName: string): Locator {
  return findBranchRow(page, branchName).getByTestId("safe-push-button")
}

/** The toast of a push into master, by its title */
function pushToast(page: Page, title: string): Locator {
  return page.locator("[data-slot=toast]", { hasText: title })
}

/**
 * The popover asks what CI did not pass on the commit it lands (no pull request ran it), Enter pushes the copy into
 * master: the remote's master is the copy, the toast says what landed, the update that follows drops the originals
 */
async function confirmAndExpectLanded(page: Page, repo: TestRepositoryBuilder, popover: Locator, copy: string): Promise<void> {
  await expect(popover).toHaveAttribute("data-decision", "confirm")
  await expect(popover.getByTestId("safe-push-lands")).toHaveText(`Pushes ${BRANCH} into master: 3 commits.`)
  await expect(popover.getByTestId("safe-push-checks")).toHaveText(`${BRANCH} has no open pull request: no CI ran on the commit it lands.`)
  await expect(popover.getByTestId("safe-push-dry-run")).toHaveCount(0)
  await expect(popoverStart(page)).toHaveText("Push to master")
  await expect(popoverStart(page)).toBeFocused()
  await page.keyboard.press("Enter")
  await expect.poll(async () => (await repo.getRemoteBranches())["master"]).toBe(copy)
  const toast = pushToast(page, `Pushed ${BRANCH} into master · 3 commits`)
  await expect(toast).toBeVisible()
  // the update that follows it, in place: the originals of the copy left the branch, its row with them
  await expect(toast).toContainText(`${BRANCH} landed in origin/master · Updated master`)
  await expect(toast.getByRole("button", { name: "Update Project" })).toHaveCount(0)
  await expect(findBranchRow(page, BRANCH)).toHaveCount(0)
}

test.describe("Push to master", () => {
  test("should push the copy into master from the slot of the row, the popover asking what CI did not pass, and update after it", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const step = pushStep(page, BRANCH)
    await expect(step).toHaveText("Push to master")
    await expect(getLandingNode(findBranchRow(page, BRANCH))).toHaveAttribute("data-kind", "ready")
    // each branch's Push to master is the user's move: the header counts the seven
    await expect(page.getByTestId("landing-indicator").getByTestId("landing-moves")).toHaveText("7")
    const copy = await repo.revision(virtualBranch(BRANCH))
    await step.click()
    await confirmAndExpectLanded(page, repo, safePushPopover(page), copy)
  })

  test("should open the popover of Push to master with S on the row", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const copy = await repo.revision(virtualBranch(BRANCH))
    const popover = await openSafePushWithKey(page, BRANCH)
    await confirmAndExpectLanded(page, repo, popover, copy)
  })

  test("should push into master from the menu of the row", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const copy = await repo.revision(virtualBranch(BRANCH))
    await findBranchRow(page, BRANCH).click({ button: "right" })
    await page.getByRole("menuitem", { name: "Push to master" }).click()
    await confirmAndExpectLanded(page, repo, safePushPopover(page), copy)
  })

  test("should push into master from the footer of the branch preview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const copy = await repo.revision(virtualBranch(BRANCH))
    const preview = await openBranchPreview(page, BRANCH)
    // the start is the row's: the preview closes, the popover opens at the slot of the row
    await preview.getByRole("button", { name: "S Push to master" }).click()
    await expect(preview).toHaveCount(0)
    await confirmAndExpectLanded(page, repo, safePushPopover(page), copy)
  })

  test("should push into master with S in the branch preview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const copy = await repo.revision(virtualBranch(BRANCH))
    const preview = await openBranchPreview(page, BRANCH)
    await page.keyboard.press("s")
    await expect(preview).toHaveCount(0)
    await confirmAndExpectLanded(page, repo, safePushPopover(page), copy)
  })

  test("should push into master when the review window asks the main window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    const copy = await repo.revision(virtualBranch(BRANCH))
    // the button of the title bar of the review window, asked of the main window over the link of the window
    await emitTauriEvent(page, "branch-review/request", { id: "push", repositoryPath: repo.path, branchName: BRANCH, itemId: "push-to-baseline" } satisfies BranchReviewRequest)
    await confirmAndExpectLanded(page, repo, safePushPopover(page), copy)
  })

  test("should push the stack branch of a whole stack into master from the header of its island", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, {}, { template: "stack_island", branch: "air" })
    const start = page.getByTestId("stack-island-actions").getByTestId("safe-push-button")
    await expect(start).toHaveText("Push to master · 7")
    const copy = await repo.revision("user-name/stack/air")
    await start.click()
    const popover = safePushPopover(page)
    await expect(popover).toHaveAttribute("data-decision", "confirm")
    await expect(popover.getByTestId("safe-push-lands")).toHaveText(/^Pushes 7 branches · 14 commits into master in one push: /)
    await expect(popover.getByTestId("safe-push-stack-branch")).toHaveText("Pushes its stack branch user-name/stack/air.")
    await expect(popoverStart(page)).toBeFocused()
    await page.keyboard.press("Enter")
    await expect.poll(async () => (await repo.getRemoteBranches())["master"]).toBe(copy)
    await expect(page.locator("[data-slot=toast]", { hasText: "Pushed the whole air stack into master · 14 commits" })).toBeVisible()
    // the update dropped the originals: the island and its branches are gone
    await expect(findBranchRow(page, "air")).toHaveCount(0)
    await expect(findBranchRow(page, "docs")).toBeVisible()
  })

  test("should push a whole stack into master from Whole Stack of the menu of its root", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, {}, { template: "stacked_branches", branch: "auth" })
    const copy = await repo.revision("user-name/stack/auth")
    await findBranchRow(page, "auth").click({ button: "right" })
    await openSubmenu(page, "Whole Stack")
    await page.getByRole("menuitem", { name: "Push to master · 4" }).click()
    const popover = safePushPopover(page)
    await expect(popover).toHaveAttribute("data-decision", "confirm")
    await expect(popover.getByTestId("safe-push-lands")).toHaveText(/^Pushes 4 branches · 4 commits into master in one push: /)
    await expect(popoverStart(page)).toBeFocused()
    await page.keyboard.press("Enter")
    await expect.poll(async () => (await repo.getRemoteBranches())["master"]).toBe(copy)
    await expect(findBranchRow(page, "auth")).toHaveCount(0)
    await expect(findBranchRow(page, "docs")).toBeVisible()
  })

  test("should close the pull requests of what a push of a stacked piece landed that show an older version of a branch, its own too", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // auth and login are on the remote as their pull requests show them, an older version of each: a push of the copy
    // of login puts neither head into master, so GitHub merges neither pull request by itself
    const repo = await setupRepo("stacked_branches", { pullRequests: availablePullRequests(), landing: { setting: "push" } })
    await syncAndWaitForBranches()
    // the remote branches are an earlier build of the same commits, which the sync rebuilt since
    const auth = await repo.pushInTerminal(virtualBranch("auth"), { earlierBuild: true })
    const login = await repo.pushInTerminal(virtualBranch("login"), { earlierBuild: true })
    await repo.setPullRequests(availablePullRequests(pullRequest("auth", 4, { headRefOid: auth }), pullRequest("login", 5, { headRefOid: login })))
    await repo.closeSessions()
    await page.reload()
    await syncAndWaitForBranches()
    await waitForStepFacts(findBranchRow(page, "login").getByTestId("next-step-slot"))
    const copy = await repo.revision(virtualBranch("login"))
    await pushStep(page, "login").click()
    const popover = safePushPopover(page)
    await expect(popover).toHaveAttribute("data-decision", "confirm")
    await expect(popoverStart(page)).toBeFocused()
    await page.keyboard.press("Enter")
    await expect.poll(async () => (await repo.getRemoteBranches())["master"]).toBe(copy)
    // the session told what the push landed before its update dropped the branches: each close with its comment
    const landed = `Landed in master as ${copy.slice(0, 7)} with the push of user-name/virtual/login`
    await expect
      .poll(async () => gitHubWrites(await repo.getGitHubLog()))
      .toEqual([
        ["POST", repoPath("issues", 4, "comments"), { body: `${landed}, which is built on this branch.` }],
        ["PATCH", repoPath("pulls", 4), { state: "closed" }],
        ["POST", repoPath("issues", 5, "comments"), { body: `${landed}.` }],
        ["PATCH", repoPath("pulls", 5), { state: "closed" }],
      ])
    // the update dropped the originals that landed: their rows are gone, oauth is built on master now
    await expect(findBranchRow(page, "login")).toHaveCount(0)
    await expect(findBranchRow(page, "auth")).toHaveCount(0)
    await expect(findBranchRow(page, "oauth")).toBeVisible()
  })

  test("should say that master moved, push nothing, and offer the push of the copy rebuilt on it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    // someone else pushed into master since the copy was built: the remote-tracking ref stays where it was
    const theirs = await repo.commitOnRemote("master", { subject: "Fix the build", author: "Ann <ann@example.com>", timestamp: 1704200000, files: { "build.txt": "fixed\n" } })
    const copy = await repo.revision(virtualBranch(BRANCH))
    await pushStep(page, BRANCH).click()
    await expect(safePushPopover(page)).toHaveAttribute("data-decision", "confirm")
    await expect(popoverStart(page)).toBeFocused()
    await page.keyboard.press("Enter")
    const notice = page.locator("[data-slot=toast]", { hasText: `master moved: ${BRANCH} is rebuilt on the new commits` })
    // the update that follows builds the copy anew on their commit: the step offers its push again
    await expect(notice).toContainText("Updated master")
    await expect(notice).toContainText("Push to master again")
    expect((await repo.getRemoteBranches())["master"]).toBe(theirs)
    await expect.poll(() => repo.revision(virtualBranch(BRANCH))).not.toBe(copy)
    await expect(pushStep(page, BRANCH)).toBeEnabled()
    await expect(pushStep(page, BRANCH)).toHaveText("Push to master")
    await expect(findBranchRow(page, BRANCH)).toBeVisible()
  })

  test("should offer Update Project while the fetched master has commits the branch is not built on, and push after it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    // someone else pushed into master and a fetch in the terminal brought it: the copy is built on the commit before it
    await repo.commitOnRemote("master", { subject: "Fix the build", author: "Ann <ann@example.com>", timestamp: 1704200000, files: { "build.txt": "fixed\n" } })
    await repo.fetchInTerminal()
    await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
    const slot = findBranchRow(page, BRANCH).getByTestId("next-step-slot")
    await expect(slot).toHaveAttribute("data-step", "updateProject")
    const update = slot.getByTestId("next-step")
    await expect(update).toHaveText("Update Project")
    await expect(getLandingNode(findBranchRow(page, BRANCH))).toHaveAttribute("data-kind", "yourStep")
    // the push would be no fast-forward: every surface holds it back with why
    await findBranchRow(page, BRANCH).click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Push to master" })).toBeDisabled()
    await page.keyboard.press("Escape")
    await runAndWaitForUpdate(page, () => update.click())
    // the update built the copy anew on master: the step offers its push again
    await expect(pushStep(page, BRANCH)).toHaveText("Push to master")
    await expect(pushStep(page, BRANCH)).toBeEnabled()
  })

  test("should say in the words of the server why it refused the push, and ask about it before the next push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches)
    await repo.refuseRemotePushes("master is protected")
    const before = (await repo.getRemoteBranches())["master"]
    await pushStep(page, BRANCH).click()
    await expect(popoverStart(page)).toBeFocused()
    await page.keyboard.press("Enter")
    const refusal = page.locator("[data-slot=toast]", { hasText: `Cannot push ${BRANCH} into master` })
    await expect(refusal).toContainText("The server refused the push into master: [remote rejected] (pre-receive hook declined). remote: master is protected")
    expect((await repo.getRemoteBranches())["master"]).toBe(before)
    await closeToast(refusal)
    // the next push asks about the refusal first
    await pushStep(page, BRANCH).click()
    await expect(safePushPopover(page).getByTestId("safe-push-route")).toContainText("The server refused a push into master before: ")
    await expect(safePushPopover(page).getByTestId("safe-push-route")).toContainText("remote: master is protected")
  })

  test("should disable Push to master with the reason when the token has no write access to the repository", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setup(page, setupRepo, syncAndWaitForBranches, {
      pullRequests: { ...availablePullRequests(), viewerPermission: "READ" },
      landing: { setting: "push", gitHub: "acme/monorepo" },
    })
    const step = pushStep(page, BRANCH)
    await expect(step).toHaveText("Push to master")
    await expect(step).toBeDisabled()
    await step.hover()
    await expect(page.getByRole("tooltip")).toHaveText("You have no write access to acme/monorepo: someone who has lands it into master")
    // the menu says the same
    await findBranchRow(page, BRANCH).click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Push to master" })).toBeDisabled()
  })

  test("should ask about the required status checks of master and push by the GitHub URL of origin", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setup(page, setupRepo, syncAndWaitForBranches, {
      pullRequests: { ...availablePullRequests(), requiredStatusChecks: true },
      landing: { setting: "push", gitHub: "acme/monorepo" },
    })
    const copy = await repo.revision(virtualBranch(BRANCH))
    await pushStep(page, BRANCH).click()
    const popover = safePushPopover(page)
    await expect(popover.getByTestId("safe-push-route")).toHaveText("master requires status checks: a push lands without them.")
    await confirmAndExpectLanded(page, repo, popover, copy)
  })
})
