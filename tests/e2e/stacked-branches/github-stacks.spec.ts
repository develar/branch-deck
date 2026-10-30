// @spec spec/branch/stacked-branches.spec.md
import type { Page } from "@playwright/test"
import type { PullRequest } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { availablePullRequests, createPullRequestDialog, gitHubWrites, pullRequest, repoPath } from "../helpers/pull-request-helpers"
import { popoverStart, safePushPopover } from "../helpers/safe-push-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import { waitForStepFacts } from "../helpers/wait-helpers"

// the `stacked_chain` template: oauth on login on auth and docs alone, none pushed. The repository lands by a merge of the pull
// request on GitHub (`branchdeck.landing=pull-request`), off Space: the straight chain is a stack of GitHub
// (docs/design/github-stacks.md), each layer's pull request into the branch of the one below it
const LANDING = { setting: "pull-request", gitHub: "acme/monorepo" } as const

/** The pull request `number` of `branch`, a layer of the stack of GitHub #4 into master at `position` of `size` */
function layer(branch: string, number: number, base: string, position: number, size: number, patch: Partial<PullRequest> = {}): PullRequest {
  return pullRequest(branch, number, {
    baseRefName: base,
    stack: { number: 4, baseRefName: "master", size, position },
    reviewDecision: "approved",
    mergeState: "clean",
    checks: { state: "passing", passed: 3, failed: 0, pending: 0, failedNames: [] },
    ...patch,
  })
}

async function createFromMenu(page: Page, branch: string): Promise<void> {
  await openContextMenu(page, findBranchRow(page, branch))
  await clickContextMenuItem(page, "Create Pull Request…")
}

test.describe("Stacks of GitHub", () => {
  test("should create the pull request of a layer into its parent's branch with those of the layers below it, and link the stack", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // on GitHub, no pull request yet; the next one created is #900
    const repo = await setupRepo("stacked_chain", { pullRequests: { ...availablePullRequests(), nextNumber: 900 }, landing: LANDING })
    await syncAndWaitForBranches()
    await createFromMenu(page, "oauth")
    const dialog = createPullRequestDialog(page)
    // into login's branch, with only its own commits; the dialog says what else it opens
    await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("oauth → user-name/virtual/login")
    await expect(dialog.getByTestId("create-pull-request-includes")).toHaveText(
      "A layer of the GitHub stack auth → login → oauth: it shows only the commits of oauth, into login. Also creates the pull requests of auth and login.",
    )
    await expect(dialog.getByTestId("create-pull-request-title")).toHaveValue("Add OAuth provider")
    await dialog.getByTestId("create-pull-request-submit").click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Created pull request #902" })).toBeVisible()
    // the push of the chain, then the pull requests bottom first, each into the branch below it, then the stack of them
    const writes = gitHubWrites(await repo.getGitHubLog()).filter(([, path]) => !path.endsWith("/assignees"))
    expect(writes).toEqual([
      ["POST", repoPath("pulls"), expect.objectContaining({ head: "user-name/virtual/auth", base: "master" })],
      ["POST", repoPath("pulls"), expect.objectContaining({ head: "user-name/virtual/login", base: "user-name/virtual/auth" })],
      ["POST", repoPath("pulls"), expect.objectContaining({ head: "user-name/virtual/oauth", base: "user-name/virtual/login", title: "Add OAuth provider" })],
      ["POST", repoPath("stacks"), { pull_requests: [900, 901, 902] }],
    ])
  })

  test("should merge a piece of a stack of GitHub with the layers below it in one asynchronous merge from the slot of the row", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const pulls = [layer("auth", 6, "master", 1, 3), layer("login", 7, "user-name/virtual/auth", 2, 3), layer("oauth", 8, "user-name/virtual/login", 3, 3)]
    const repo = await setupRepo("stacked_chain", {
      pullRequests: { ...availablePullRequests(...pulls), mergeMethods: { rebase: true, merge: true, squash: true, autoMerge: true } },
      landing: LANDING,
    })
    await syncAndWaitForBranches()
    const slot = findBranchRow(page, "login").getByTestId("safe-push-button")
    await waitForStepFacts(findBranchRow(page, "login").getByTestId("next-step-slot"))
    await expect(slot).toHaveText("Merge #7")
    // it lands more than its row: the popover at the slot says so, Enter merges
    await slot.click()
    await expect(safePushPopover(page).getByTestId("safe-push-lands")).toContainText("GitHub merges #7 of login")
    await expect(popoverStart(page)).toHaveText("Merge #7")
    await expect(popoverStart(page)).toBeFocused()
    await page.keyboard.press("Enter")
    // one merge of login's #7, which GitHub runs with auth's #6 below it; oauth stays open
    await expect
      .poll(async () => gitHubWrites(await repo.getGitHubLog()).filter(([method]) => method === "PUT"))
      .toEqual([["PUT", repoPath("pulls", 7, "merge-async"), expect.objectContaining({ merge_method: "rebase" })]])
    // the toast says what landed, and the update that follows drops the originals of both
    const toast = page.locator("[data-slot=toast]", { hasText: "Merged #7 of login into master · 2 commits" })
    await expect(toast).toContainText("2 branches landed in origin/master · auth and login")
    await expect(findBranchRow(page, "login")).toHaveCount(0)
    await expect(findBranchRow(page, "auth")).toHaveCount(0)
    await expect(findBranchRow(page, "oauth")).toBeVisible()
  })

  test("should take an old pull request into master into the stack with Link GitHub Stack from the menu of the row", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // auth and login each opened their pull request into master before the stacks of GitHub
    const pulls = [pullRequest("auth", 6, { baseRefName: "master" }), pullRequest("login", 7, { baseRefName: "master" })]
    const repo = await setupRepo("stacked_chain", { pullRequests: availablePullRequests(...pulls), landing: LANDING })
    await syncAndWaitForBranches()
    await openContextMenu(page, findBranchRow(page, "login"))
    await clickContextMenuItem(page, "Link GitHub Stack")
    await expect(page.locator("[data-slot=toast]", { hasText: "Linked the GitHub stack of login" })).toBeVisible()
    // login's #7 goes into auth's branch first, then the stack of both: GitHub refuses the retarget of a member
    expect(gitHubWrites(await repo.getGitHubLog())).toEqual([
      ["PATCH", repoPath("pulls", 7), { base: "user-name/virtual/auth" }],
      ["POST", repoPath("stacks"), { pull_requests: [6, 7] }],
    ])
  })

  test("should link the stack of a layer outside it from the slot of the row, as its merge would land nothing", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // login's #7 goes into auth's branch, and GitHub lists it in no stack
    const pulls = [pullRequest("auth", 6, { baseRefName: "master" }), pullRequest("login", 7, { baseRefName: "user-name/virtual/auth" })]
    const repo = await setupRepo("stacked_chain", { pullRequests: availablePullRequests(...pulls), landing: LANDING })
    await syncAndWaitForBranches()
    const row = findBranchRow(page, "login")
    await waitForStepFacts(row.getByTestId("next-step-slot"))
    await row.getByTestId("next-step-slot").getByRole("button", { name: "Link Stack" }).click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Linked the GitHub stack of login" })).toBeVisible()
    expect(gitHubWrites(await repo.getGitHubLog())).toEqual([["POST", repoPath("stacks"), { pull_requests: [6, 7] }]])
  })
})
