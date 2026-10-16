// @spec spec/branch/branch-review.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { availableRobots } from "../helpers/patronus-helpers"
import { availablePullRequests, getOpenedUrls, pullRequest } from "../helpers/pull-request-helpers"
import { routePush } from "../helpers/push-helpers"
import { getEmittedEvents, getOpenedSubWindows, openSubWindowPage } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { routeCommand } from "../helpers/wait-helpers"
import type { BranchActionRequest, BranchReviewActionsChanged } from "../../../src/lib/branch-review"
import type { PatronusStartOptions } from "../../../src/lib/bindings"
import type { BranchReviewActions, BranchReviewWindowData } from "../../../src/types/sub-window"

/** The paths of the diffs the review shows, in the order of the tree: directories first */
async function reviewedPaths(page: Page): Promise<string[]> {
  return page.getByTestId("review-file-header").evaluateAll(headers => headers.map(header => header.getAttribute("data-path")!))
}

/** `name:count` of the owner chips */
async function ownerChips(page: Page): Promise<string[]> {
  return page.getByTestId("branch-review-owner").evaluateAll(chips => chips.map(chip => chip.textContent!))
}

/** "Review Changes" of the branch menu: the window the main window opens, still in the main window */
async function openReviewWindow(page: Page, branch: string) {
  await findBranchRow(page, branch).click({ button: "right" })
  await page.getByRole("menuitem", { name: "Review Changes" }).click()
  const [opened] = await getOpenedSubWindows<BranchReviewWindowData>(page)
  expect(opened).toMatchObject({ windowId: "branch-review", url: "/branch-review", title: `Review ${branch}` })
  return opened!
}

/** Shows the window in the page, at its first diff */
async function showReviewWindow(page: Page, opened: Awaited<ReturnType<typeof openReviewWindow>>): Promise<BranchReviewWindowData> {
  await openSubWindowPage(page, opened)
  await expect(page.getByTestId("review-file-header").first()).toBeVisible()
  return opened.data
}

async function openReviewOf(page: Page, branch: string): Promise<BranchReviewWindowData> {
  return showReviewWindow(page, await openReviewWindow(page, branch))
}

const START_OPTIONS: PatronusStartOptions = {
  targetBranch: "master",
  supported: true,
  profiles: [{ id: "lsc", name: "Large Scale Change", description: "All the tests", allowedPushModes: [] }],
  defaultProfileId: null,
}

const LOGIN_PULL_REQUEST = pullRequest("login", 42)

/** The actions of a branch on the remote nowhere, with Patronus on the repository */
function unpushedActions(pullRequest: BranchReviewActions["pullRequest"] = null): BranchReviewActions {
  return { pullRequest, safePush: { reason: null }, push: { reason: null } }
}

test.describe("Branch review window", () => {
  test("should review a stacked branch on its parent, then the whole stack, by owner", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const data = await openReviewOf(page, "login")
    expect(data).toMatchObject({ chain: ["auth", "login"], range: "own", baselineBranch: "origin/master", group: null })

    // the branch on top of its parent: the files of its own two commits
    const segments = page.getByTestId("branch-review-segment")
    await expect(segments).toHaveText(["auth1", "login2"])
    await expect(segments.nth(1)).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js", "ui/login.css", "ui/login.js", "README.md"])
    // the owners by the OWNERSHIP files of the baseline, the most files first
    await expect.poll(() => ownerChips(page)).toEqual(["All4", "Auth Team1", "Design1", "Monorepo1", "UI Team1"])
    await expect(page.getByTestId("review-file-header").filter({ hasText: "ui/login.css" }).getByTestId("review-file-owner")).toHaveText("Design")

    await page.getByTestId("branch-review-owner").filter({ hasText: "UI Team" }).click()
    await expect.poll(() => reviewedPaths(page)).toEqual(["ui/login.js"])

    // all files again: the review stays at the file it showed
    await page.getByTestId("branch-review-owner").filter({ hasText: "All" }).click()
    await expect(page.getByTestId("branch-review-position")).toHaveText("File 3 / 4")

    // the parent alone, then extended to the reviewed branch: the whole stack from the baseline
    await segments.first().click()
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js"])
    await segments.nth(1).click({ modifiers: ["Shift"] })
    await expect(page.getByRole("button", { name: "Whole Stack" })).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js", "ui/login.css", "ui/login.js", "README.md"])

    // one commit of the part, under the name of its branch (the virtual commits have no prefix)
    const commits = page.getByTestId("branch-review-commits")
    await expect(commits.getByText("auth", { exact: true })).toBeVisible()
    await expect(commits.getByTestId("branch-review-commit")).toHaveText([/^All Changes3 commits$/, /^Style the form/, /^Add the login form/, /^Add the token/])
    await commits.getByTestId("branch-review-commit").nth(1).click()
    await expect.poll(() => reviewedPaths(page)).toEqual(["ui/login.css", "README.md"])
  })

  test("should read the stack again when a sync moves a tip of it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const data = await openReviewOf(page, "login")
    let reads = 0
    page.on("request", (request) => {
      if (request.url().endsWith("/invoke/get_branch_review")) {
        reads++
      }
    })
    const tips = { auth: "0".repeat(40), login: "1".repeat(40) }

    // another repository synced: nothing to read
    await emitTauriEvent(page, "branch-tips-changed", { repositoryPath: "/elsewhere", tips })
    await emitTauriEvent(page, "branch-tips-changed", { repositoryPath: data.repositoryPath, tips })
    await expect.poll(() => reads).toBe(1)
    // the part stays: the reviewed branch on its parent
    await expect(page.getByTestId("branch-review-segment").nth(1)).toHaveAttribute("aria-pressed", "true")
  })

  test("should act on the branch of the part from the footer, through the main window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership", { patronus: availableRobots(), pullRequests: availablePullRequests(LOGIN_PULL_REQUEST) })
    await syncAndWaitForBranches()
    // the robots and the pull requests load after the sync
    await expect(findBranchRow(page, "login").getByTestId("safe-push-slot")).toBeVisible()
    await expect(findBranchRow(page, "login").getByTestId("pull-request-chip")).toBeVisible()

    // the window opens with the actions of each branch of its chain
    const opened = await openReviewWindow(page, "login")
    const repositoryPath = opened.data.repositoryPath
    expect(opened.data.actions).toEqual({
      auth: unpushedActions(),
      login: unpushedActions({ number: 42, url: LOGIN_PULL_REQUEST.url }),
    })
    // a sync that rebuilds the branches as they were changes nothing the window offers: a push during it waits for it
    await page.getByTestId("sync-button").click()
    await expect(page.getByTestId("sync-button")).toBeEnabled()
    expect((await getEmittedEvents<BranchReviewActionsChanged>(page, "branch-review-actions-changed")).map(it => it.branches.login?.push?.reason ?? null)).toEqual([])

    await showReviewWindow(page, opened)
    const footer = page.getByTestId("branch-review-footer")
    await expect(page.getByTestId("branch-review-position")).toHaveText("File 1 / 4")
    await expect(footer).toContainText("JKFiles")
    await expect(footer).toContainText("NPChanges")
    await expect(footer).toContainText("EscClose")

    // O opens the pull request in the window itself; Safe Push and Push ask the main window
    await page.keyboard.press("o")
    await expect.poll(() => getOpenedUrls(page)).toEqual([LOGIN_PULL_REQUEST.url])
    await footer.getByRole("button", { name: "Safe Push…" }).click()
    // the parent alone: its actions, without a pull request
    await page.getByTestId("branch-review-segment").first().click()
    await expect(footer.getByRole("button", { name: "Open Pull Request" })).toHaveCount(0)
    await page.keyboard.press("s")
    // the whole stack: the reviewed branch
    await page.getByRole("button", { name: "Whole Stack" }).click()
    await expect(footer.getByRole("button", { name: "Open Pull Request" })).toBeVisible()
    await footer.getByRole("button", { name: "Push", exact: true }).click()
    const requests = await getEmittedEvents<BranchActionRequest>(page, "branch-action")
    expect(requests).toEqual([
      { repositoryPath, branchName: "login", action: "safe-push" },
      { repositoryPath, branchName: "auth", action: "safe-push" },
      { repositoryPath, branchName: "login", action: "push" },
    ])

    // what the main window says changed: another repository changes nothing, a running robot disables the Safe Push
    await emitTauriEvent(page, "branch-review-actions-changed", { repositoryPath: "/elsewhere", branches: {} } satisfies BranchReviewActionsChanged)
    await expect(footer.getByRole("button", { name: "Open Pull Request" })).toBeVisible()
    await emitTauriEvent(page, "branch-review-actions-changed", {
      repositoryPath,
      branches: { login: { pullRequest: null, safePush: { reason: "A Safe Push of the last commit is running" }, push: null } },
    } satisfies BranchReviewActionsChanged)
    await expect(footer.getByRole("button", { name: "Open Pull Request" })).toHaveCount(0)
    await expect(footer.getByRole("button", { name: "Push", exact: true })).toHaveCount(0)
    await expect(footer.getByRole("button", { name: "Safe Push…" })).toBeDisabled()
    await page.keyboard.press("s")
    expect(await getEmittedEvents(page, "branch-action")).toHaveLength(3)

    // the main window, in the page again: its listener runs what the window asked for
    await page.goto(`/?repoId=${new URL(page.url()).searchParams.get("repoId")}`)
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "login").getByTestId("safe-push-slot")).toBeVisible()
    const pushes = await routePush(page, {})
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    await emitTauriEvent(page, "branch-action", { ...requests[2]!, repositoryPath: "/elsewhere" })
    await emitTauriEvent(page, "branch-action", requests[2])
    // the push of the row: the stack, parent first
    await expect.poll(() => pushes.map(it => it.branchName)).toEqual(["auth", "login"])
    await emitTauriEvent(page, "branch-action", requests[0])
    await expect(page.getByTestId("patronus-start-form")).toBeVisible()
    // the window asked: the main window comes to the front, once per request it ran
    expect(await page.evaluate(() => (window as unknown as { __e2eWindowCalls?: string[] }).__e2eWindowCalls?.filter(it => it === "set_focus"))).toEqual(["set_focus", "set_focus"])
  })
})
