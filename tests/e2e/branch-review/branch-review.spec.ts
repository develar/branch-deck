// @spec spec/branch/branch-review.spec.md
import type { Page } from "@playwright/test"
import type { PatronusStartOptions } from "../../../src/lib/bindings"
import type { BranchReviewFeedback, BranchReviewItem, BranchReviewRequest, BranchReviewWindowData, BranchReviewWindowState } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { NO_REVIEW_NEEDED, availableRobots } from "../helpers/patronus-helpers"
import { availablePullRequests, getOpenedUrls, pullRequest } from "../helpers/pull-request-helpers"
import { routePush } from "../helpers/push-helpers"
import { getEmittedEvents, getOpenedSubWindows, openSubWindowPage } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { routeCommand } from "../helpers/wait-helpers"

/** The paths of the diffs the review shows, in the order of the tree: directories first */
async function reviewedPaths(page: Page): Promise<string[]> {
  return page.getByTestId("review-file-header").evaluateAll(headers => headers.map(header => header.getAttribute("data-path")!))
}

/** `name:count` of the owner chips */
async function ownerChips(page: Page): Promise<string[]> {
  return page.getByTestId("branch-review-owner").evaluateAll(chips => chips.map(chip => chip.textContent!))
}

/** The last review window the page opened (`open_sub_window`), once there are `count` */
async function openedReview(page: Page, count: number) {
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(count)
  const opened = (await getOpenedSubWindows<BranchReviewWindowData>(page))[count - 1]!
  expect(opened).toMatchObject({ windowId: "branch-review", url: "/branch-review" })
  return opened
}

/** "Review Changes" of the branch menu: the review window, still in the main window */
async function openReviewWindow(page: Page, branch: string) {
  const count = (await getOpenedSubWindows(page)).length
  await findBranchRow(page, branch).click({ button: "right" })
  await page.getByRole("menuitem", { name: "Review Changes" }).click()
  const opened = await openedReview(page, count + 1)
  expect(opened.title).toBe(`Review ${branch}`)
  return opened
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

/** What the main window says of each branch: the ids of its actions, a disabled one with its reason */
function itemSummary(items: Readonly<Record<string, readonly BranchReviewItem[]>>): Record<string, string[]> {
  return Object.fromEntries(Object.entries(items).map(([name, list]) => [name, list.map(item => (item.disabled === true ? `${item.id}: ${item.description}` : item.id))]))
}

/** The window's requests without their ids */
async function windowRequests(page: Page): Promise<Omit<BranchReviewRequest, "id">[]> {
  return (await getEmittedEvents<BranchReviewRequest>(page, "branch-review/request")).map(({ id: _id, ...request }) => request)
}

test.describe("Opening the review", () => {
  test("should open the review of a branch in its window from its row, its menu and its preview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const preview = page.getByTestId("branch-preview")
    const plain = { range: "own", filter: { kind: "all" }, commitId: null, path: null }

    // D on the focused row: the window, no preview
    await findBranchRow(page, "login").focus()
    await page.keyboard.press("d")
    const byKey = await openedReview(page, 1)
    expect(byKey).toMatchObject({ title: "Review login", data: { chain: ["auth", "login"], focus: plain } })
    await expect(preview).toBeHidden()

    // Review Changes of the menu, the only review item
    await findBranchRow(page, "auth").click({ button: "right" })
    await expect(page.getByRole("menuitem", { name: "Open in Window" })).toHaveCount(0)
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    expect((await openedReview(page, 2)).title).toBe("Review auth")

    // D in the preview, and the button of its footer: the window, the preview closes
    await findBranchRow(page, "login").focus()
    await page.keyboard.press(" ")
    await expect(preview.getByTestId("branch-preview-body")).toBeFocused()
    await page.keyboard.press("d")
    expect((await openedReview(page, 3)).data.focus).toEqual(plain)
    await expect(preview).toBeHidden()
    await findBranchRow(page, "login").focus()
    await page.keyboard.press(" ")
    await preview.getByTestId("branch-preview-review").click()
    expect((await openedReview(page, 4)).title).toBe("Review login")
    await expect(preview).toBeHidden()

    // in the window P is the previous change, not a push: J moves to the next file, P back
    await showReviewWindow(page, byKey)
    const position = page.getByTestId("branch-review-position")
    await expect(position).toHaveText(/File 1 \/ \d+$/)
    await page.locator("[aria-label='Changes of the commit']").focus()
    await page.keyboard.press("j")
    await expect(position).toHaveText(/File 2 \/ \d+$/)
    await page.keyboard.press("p")
    await expect(position).toHaveText(/File 1 \/ \d+$/)
    expect(await windowRequests(page)).toEqual([])
  })
})

test.describe("Branch review window", () => {
  test("should review a stacked branch on its parent, then the whole stack, by owner", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const data = await openReviewOf(page, "login")
    // its own part, all files, all changes, the first file
    expect(data).toMatchObject({
      chain: ["auth", "login"],
      baselineBranch: "origin/master",
      focus: { range: "own", filter: { kind: "all" }, commitId: null, path: null },
    })

    // the branch on top of its parent: the files of its own two commits
    const segments = page.getByTestId("branch-review-segment")
    await expect(segments).toHaveText(["auth1", "login2"])
    await expect(segments.nth(1)).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js", "ui/login.css", "ui/login.js", "README.md"])
    // the stack is the title bar of the window
    await expect(page.getByRole("banner").getByTestId("branch-review-stack")).toBeVisible()
    // the owners by the OWNERSHIP files of the baseline, the most files first, above the files they filter
    await expect.poll(() => ownerChips(page)).toEqual(["All4", "Auth Team1", "Design1", "Monorepo1", "UI Team1"])
    await expect(page.getByTestId("review-sidebar").getByTestId("branch-review-owners")).toBeVisible()
    await expect(page.getByTestId("review-file-header").filter({ hasText: "ui/login.css" }).getByTestId("review-file-owner")).toHaveText("Design")

    await page.getByTestId("branch-review-owner").filter({ hasText: "UI Team" }).click()
    await expect.poll(() => reviewedPaths(page)).toEqual(["ui/login.js"])

    // all files again: the review stays at the file it showed
    await page.getByTestId("branch-review-owner").filter({ hasText: "All" }).click()
    await expect(page.getByTestId("branch-review-position")).toHaveText("All Changes · File 3 / 4")

    // the parent alone, one commit: no column, the commit above its diffs
    await segments.first().click()
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js"])
    await expect(page.getByTestId("range-review-commits")).toHaveCount(0)
    await expect(page.getByTestId("range-review-commit-header")).toContainText("Add the token")
    // then extended to the reviewed branch: the whole stack from the baseline
    await segments.nth(1).click({ modifiers: ["Shift"] })
    await expect(page.getByRole("button", { name: "Whole Stack" })).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js", "ui/login.css", "ui/login.js", "README.md"])

    // one commit of the part, under the name of its branch (the virtual commits have no prefix)
    const commits = page.getByTestId("range-review-commits")
    await expect(commits.getByText("auth", { exact: true })).toBeVisible()
    await expect(commits.getByTestId("range-review-commit")).toHaveText([/^All Changes3 commits$/, /^Style the form/, /^Add the login form/, /^Add the token/])
    await commits.getByTestId("range-review-commit").nth(1).click()
    await expect.poll(() => reviewedPaths(page)).toEqual(["ui/login.css", "README.md"])
    await expect(commits.getByTestId("range-review-commit").nth(1)).toHaveAttribute("aria-selected", "true")
    // the commit above its diffs: its subject, its author
    const header = page.getByTestId("range-review-commit-header")
    await expect(header).toContainText("Style the form")
    await expect(header).toContainText("Test User")
    await expect(page.getByTestId("branch-review-position")).toHaveText(/^Commit 1 \/ 3 · File \d \/ 2$/)
    await expect(page.getByTestId("branch-review-footer")).toContainText("Commits")

    // ⌥↓ twice from the diffs: the second while the next commit loads, the review keeps the focus
    await page.getByTestId("commit-review-code").locator("[tabindex='0']").first().focus()
    await page.keyboard.press("Alt+ArrowDown")
    await page.keyboard.press("Alt+ArrowDown")
    await expect(commits.getByTestId("range-review-commit").nth(3)).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js"])
    await expect(page.getByTestId("branch-review-position")).toHaveText("Commit 3 / 3 · File 1 / 1")
    // ↑ in the column, to all changes
    await commits.focus()
    await page.keyboard.press("ArrowUp")
    await page.keyboard.press("ArrowUp")
    await page.keyboard.press("ArrowUp")
    await expect(commits.getByTestId("range-review-commit").first()).toHaveAttribute("aria-selected", "true")
    await expect(page.getByTestId("range-review-commit-header")).toHaveCount(0)
    // the commits keep the focus while the diffs of each choice mount beside them
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js", "ui/login.css", "ui/login.js", "README.md"])
    await expect(commits).toBeFocused()
  })

  test("should keep the chip of the filter on a part without its files, and show no chips when one owner has them all", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    await openReviewOf(page, "login")
    await page.getByTestId("branch-review-owner").filter({ hasText: "Design" }).click()
    await expect.poll(() => reviewedPaths(page)).toEqual(["ui/login.css"])

    // the parent alone has no file of Design: its chip stays, at 0, so the filter can be turned off
    await page.getByTestId("branch-review-segment").first().click()
    await expect.poll(() => ownerChips(page)).toEqual(["All1", "Auth Team1", "Design0"])
    await expect(page.getByText("No files of Design in this range")).toBeVisible()

    // all files: one owner has them all, so nothing filters; the header of the files names the owner
    await page.getByTestId("branch-review-owner").filter({ hasText: "All" }).click()
    await expect(page.getByTestId("branch-review-owners")).toHaveCount(0)
    await expect(page.getByTestId("branch-review-files")).toHaveText("1 file · Auth Team")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js"])
  })

  test("should read the stack again when a sync moves a tip of it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const data = await openReviewOf(page, "login")
    let reads = 0
    page.on("request", request => {
      if (request.url().endsWith("/invoke/get_branch_review")) {
        reads++
      }
    })
    const tips = { auth: "0".repeat(40), login: "1".repeat(40) }

    // another repository synced: nothing to read
    await emitTauriEvent(page, "branch-review/state", { ...data.state, repositoryPath: "/elsewhere", tips } satisfies BranchReviewWindowState)
    await emitTauriEvent(page, "branch-review/state", { ...data.state, tips } satisfies BranchReviewWindowState)
    await expect.poll(() => reads).toBe(1)
    // the part stays: the reviewed branch on its parent
    await expect(page.getByTestId("branch-review-segment").nth(1)).toHaveAttribute("aria-pressed", "true")
  })

  test("should act on the branch of the part from the title bar, through the main window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_ownership", { patronus: availableRobots(), pullRequests: availablePullRequests(LOGIN_PULL_REQUEST) })
    // no approval needed: the Safe Push of each part is offered
    await repo.setReviewAnswers({ answers: { auth: NO_REVIEW_NEEDED, login: NO_REVIEW_NEEDED } })
    await syncAndWaitForBranches()
    // the robots and the pull requests load after the sync
    await expect(findBranchRow(page, "login").getByTestId("next-step-slot")).toBeVisible()
    await expect(findBranchRow(page, "login").getByTestId("pull-request-chip")).toBeVisible()

    // the window opens with the actions of each branch of its chain, the items of its menu
    const opened = await openReviewWindow(page, "login")
    const repositoryPath = opened.data.repositoryPath
    expect(itemSummary(opened.data.state.items)).toEqual({ auth: ["safe-push", "push"], login: ["open-pull-request", "safe-push", "push"] })
    expect(opened.data.state.items.login![0]).toMatchObject({ label: "Open Pull Request", kbds: ["o"], url: LOGIN_PULL_REQUEST.url })
    // the push of login's menu and button, the stack; P is the previous change in the review: the push has no key there
    expect(opened.data.state.items.login![2]).toEqual({ id: "push", label: "Push Stack (2 branches)", description: "Pushes auth and login, parents first" })
    // a sync that rebuilds the branches as they were changes nothing the window offers: a push during it waits for it
    await page.getByTestId("sync-button").click()
    await expect(page.getByTestId("sync-button")).toBeEnabled()
    const pushStates = async () =>
      (await getEmittedEvents<BranchReviewWindowState>(page, "branch-review/state")).map(it => it.items.login?.find(item => item.id === "push")?.description ?? null)
    // no state with a reason the push is disabled
    expect((await pushStates()).filter(it => it !== null && it !== "Pushes auth and login, parents first")).toEqual([])

    await showReviewWindow(page, opened)
    const footer = page.getByTestId("branch-review-footer")
    // the actions on the branch are in the title bar, the keys of the review in the footer
    const actions = page.getByTestId("branch-review-actions")
    await expect(page.getByTestId("branch-review-position")).toHaveText("All Changes · File 1 / 4")
    await expect(footer).toContainText("JKFiles")
    await expect(footer).toContainText("NPChanges")
    await expect(footer).toContainText("EscClose")

    // O opens the pull request in the window itself; Safe Push and Push ask the main window
    await page.keyboard.press("o")
    await expect.poll(() => getOpenedUrls(page)).toEqual([LOGIN_PULL_REQUEST.url])
    await actions.getByRole("button", { name: "Safe Push…" }).click()
    // the parent alone: its actions, without a pull request
    await page.getByTestId("branch-review-segment").first().click()
    await expect(actions.getByRole("button", { name: "Open Pull Request" })).toHaveCount(0)
    await page.keyboard.press("s")
    // the whole stack: the reviewed branch
    await page.getByRole("button", { name: "Whole Stack" }).click()
    await expect(actions.getByRole("button", { name: "Open Pull Request" })).toBeVisible()
    await actions.getByRole("button", { name: "Push Stack (2 branches)", exact: true }).click()
    await expect
      .poll(() => windowRequests(page))
      .toEqual([
        { repositoryPath, branchName: "login", itemId: "safe-push" },
        { repositoryPath, branchName: "auth", itemId: "safe-push" },
        { repositoryPath, branchName: "login", itemId: "push" },
      ])
    const requests = await getEmittedEvents<BranchReviewRequest>(page, "branch-review/request")

    // the main window shows another repository: what it runs waits for it, the pull request opens from here
    await emitTauriEvent(page, "branch-review/state", { repositoryPath: "/elsewhere", selected: true, tips: {}, items: {} } satisfies BranchReviewWindowState)
    await expect(actions.getByRole("button", { name: "Safe Push…" })).toBeEnabled()
    await emitTauriEvent(page, "branch-review/state", { repositoryPath, selected: false, tips: {}, items: {} } satisfies BranchReviewWindowState)
    await expect(actions.getByRole("button", { name: "Safe Push…" })).toBeDisabled()
    // the tooltip of the button the mouse was on closes first
    await page.mouse.move(0, 0)
    await expect(page.getByRole("tooltip")).toHaveCount(0)
    await actions.getByRole("button", { name: "Push Stack (2 branches)", exact: true }).hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "in the main window to push login from here" })).toBeVisible()
    await expect(actions.getByRole("button", { name: "Open Pull Request" })).toBeEnabled()

    // a running robot disables the Safe Push; the pull request is gone
    await emitTauriEvent(page, "branch-review/state", {
      ...opened.data.state,
      items: { login: [{ id: "safe-push", label: "Safe Push…", kbds: ["s"], disabled: true, description: "A Safe Push of the last commit is running" }] },
    } satisfies BranchReviewWindowState)
    await expect(actions.getByRole("button", { name: "Open Pull Request" })).toHaveCount(0)
    await expect(actions.getByRole("button", { name: "Push Stack (2 branches)", exact: true })).toHaveCount(0)
    await expect(actions.getByRole("button", { name: "Safe Push…" })).toBeDisabled()
    await page.keyboard.press("s")
    expect(await getEmittedEvents(page, "branch-review/request")).toHaveLength(3)

    // the branch landed: the review stays, with what it can no longer do
    const { login: _login, ...tipsWithoutLogin } = opened.data.state.tips
    await emitTauriEvent(page, "branch-review/state", {
      ...opened.data.state,
      tips: tipsWithoutLogin,
      items: { auth: opened.data.state.items.auth! },
    } satisfies BranchReviewWindowState)
    await expect(page.getByTestId("branch-review-gone")).toHaveText("login is no longer a branch: it landed or was renamed")
    await expect(actions.getByRole("button", { name: "Safe Push…" })).toBeDisabled()
    await expect(page.getByTestId("review-file-header").first()).toBeVisible()

    // a refusal of the main window shows where the user is
    await emitTauriEvent(page, "branch-review/feedback", { kind: "error", title: "Cannot push login", message: "The remote branch is up to date" } satisfies BranchReviewFeedback)
    await expect(page.getByText("Cannot push login")).toBeVisible()

    // the main window, in the page again: its listener runs what the window asked for
    await page.goto(`/?repoId=${new URL(page.url()).searchParams.get("repoId")}`)
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "login").getByTestId("next-step-slot")).toBeVisible()
    const pushes = await routePush(page, {})
    await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
    // another repository: refused with the reason, in the window
    await emitTauriEvent(page, "branch-review/request", { ...requests[2]!, id: "elsewhere", repositoryPath: "/elsewhere" })
    await expect.poll(() => getEmittedEvents(page, "branch-review/reply")).toEqual([{ id: "elsewhere", ok: false }])
    expect(await getEmittedEvents(page, "branch-review/feedback")).toEqual([
      { kind: "error", title: "Cannot push login", message: "Select elsewhere in the main window to push login from here" },
    ])
    await emitTauriEvent(page, "branch-review/request", requests[2])
    // the push of the row: the stack, parent first
    await expect.poll(() => pushes.map(it => it.branchName)).toEqual(["auth", "login"])
    await emitTauriEvent(page, "branch-review/request", requests[0])
    await expect(page.getByTestId("patronus-start-form")).toBeVisible()
    await expect.poll(async () => (await getEmittedEvents<{ ok: boolean }>(page, "branch-review/reply")).map(it => it.ok)).toEqual([false, true, true])
    // the window asked: the main window comes to the front, once per request it ran
    expect(await page.evaluate(() => (window as unknown as { __e2eWindowCalls?: string[] }).__e2eWindowCalls?.filter(it => it === "set_focus"))).toEqual(["set_focus", "set_focus"])
  })
})
