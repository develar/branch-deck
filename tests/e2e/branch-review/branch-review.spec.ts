// @spec spec/branch/branch-review.spec.md
import type { Page } from "@playwright/test"
import type { PatronusProfile } from "../../../src/lib/bindings"
import type { BranchReviewFeedback, BranchReviewItem, BranchReviewRequest, BranchReviewWindowData, BranchReviewWindowState } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import { readClipboard, setupClipboardPermissions } from "../helpers/clipboard-helpers"
import { AWAITING_CODE_OWNER, NO_REVIEW_NEEDED, availableRobots } from "../helpers/patronus-helpers"
import { getCommitPreview } from "../helpers/preview-helpers"
import { availablePullRequests, createPullRequestDialog, getOpenedUrls, pullRequest } from "../helpers/pull-request-helpers"
import { routePush, routePushBranches } from "../helpers/push-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { popoverStart, safePushPopover } from "../helpers/safe-push-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import { getEmittedEvents, getOpenedSubWindows, openLinkedSubWindow, openSubWindowPage, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent, runAndWaitForSync } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { recordCommand, routeCommand } from "../helpers/wait-helpers"

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

/** The merge profiles Patronus offers on master (`patronusProfiles`) besides the default checks */
const PROFILES = { profiles: [{ id: "lsc", name: "Large Scale Change", description: "All the tests", allowedPushModes: [], skipsReview: null }] satisfies PatronusProfile[] }

const LOGIN_PULL_REQUEST = pullRequest("login", 42)

/** stacked_ownership with auth pushed, without a pull request: its code owner has to approve in one (row 7) */
async function setupAuthWithoutPullRequest(
  page: Page,
  setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
) {
  const repo = await setupRepo("stacked_ownership", {
    syncOverrides: { remote: { auth: { remoteExists: true, unpushedCommits: [], myUnpushedCount: 0, commitsBehind: 0, fastForward: true } } },
    patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
    // the code owner a collaborator; the next pull request created is #77
    pullRequests: { ...availablePullRequests(), users: [{ login: "ikorennoy", name: "Ilya Korennoy" }], nextNumber: 77 },
    patronusProfiles: PROFILES,
  })
  await repo.setReviewAnswers({ answers: { auth: AWAITING_CODE_OWNER } })
  await syncAndWaitForBranches()
  await expect(findBranchRow(page, "auth").getByTestId("next-step-slot")).toHaveAttribute("data-step", "createPullRequest")
}

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

test.describe("A commit of the branch preview", () => {
  test("should review a commit of the branch preview in the review of the branch, at the commit", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const preview = await openBranchPreview(page, "login")
    const commit = preview.getByTestId("branch-preview-commit").filter({ hasText: "Add the login form" })
    await commit.getByTestId("branch-preview-commit-review").click()
    const opened = await openedReview(page, 1)
    expect(opened.data).toMatchObject({ chain: ["auth", "login"], focus: { commitId: expect.stringMatching(/^[0-9a-f]{40}$/) } })
    expect(opened.title).toMatch(/^Review login · [0-9a-f]{8}$/)
    // the preview closes once the window opened
    await expect(preview).toBeHidden()
    await openSubWindowPage(page, opened)
    await expect(page.getByTestId("range-review-commit-header")).toContainText("Add the login form")
  })
})

test.describe("Reading the review ahead", () => {
  test("should read the review of a branch ahead once the pointer rests on its row or it has the focus, once per state", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const prefetches = await routeCommand(page, "prefetch_branch_review", () => ({ status: "ok", data: null }))

    // the pointer rests on the row: its own part on its parent, the stack from the root
    await findBranchRow(page, "login").hover()
    await expect.poll(() => prefetches.length).toBe(1)
    expect(prefetches[0]).toMatchObject({ review: { chain: ["auth", "login"] }, part: "own" })

    // read already: neither the focus of the row nor D reads it again; D opens the window
    await findBranchRow(page, "login").focus()
    await page.keyboard.press("d")
    expect((await openedReview(page, 1)).title).toBe("Review login")
    expect(prefetches).toHaveLength(1)

    // another row by the focus alone
    await findBranchRow(page, "auth").focus()
    await expect.poll(() => prefetches.length).toBe(2)
    expect(prefetches[1]).toMatchObject({ review: { chain: ["auth"] }, part: "own" })
  })
})

test.describe("The scopes of the stack", () => {
  test("should move the review of a branch up to the whole stack, and down to a branch of it and to the branch of a commit", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the `stack_island` template: air, bazel, bridge, runtime, deploy, and side-fix, side-test off bazel; two commits each
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "bazel"))
    const whole = window.getByTestId("branch-review-whole-stack")
    await expect(window.getByRole("button", { name: "From Root" })).toHaveAttribute("aria-pressed", "false")
    await expect(whole).toHaveAttribute("aria-pressed", "false")

    // up: the stack branch, the chain the window showed as the way back
    await whole.click()
    const stack = await openedReview(page, 2)
    expect(stack).toMatchObject({
      title: "Review the whole air stack",
      data: {
        chain: ["air"],
        wholeStack: true,
        from: [
          { name: "air", commits: 2 },
          { name: "bazel", commits: 2 },
        ],
      },
    })
    await reopenLinkedSubWindow(window, stack)
    await expect(whole).toHaveAttribute("aria-pressed", "true")
    const segments = window.getByTestId("branch-review-segment")
    await expect(segments).toHaveText(["air2", "bazel2"])

    // each commit of the whole stack names its branch, whose review its heading opens at the commit
    const fix = window.getByTestId("range-review-commit").filter({ hasText: "Fix the jar manifest, part 2" })
    await expect(fix.getByTestId("range-review-commit-branch")).toHaveText("side-fix")
    await fix.click()
    await window.getByTestId("range-review-commit-header").getByRole("button", { name: "Review side-fix" }).click()
    const atCommit = await openedReview(page, 3)
    expect(atCommit.data).toMatchObject({ chain: ["air", "bazel", "side-fix"], focus: { commitId: expect.stringMatching(/^[0-9a-f]{40}$/) } })
    expect(atCommit.data.wholeStack).toBeUndefined()
    expect(atCommit.title).toMatch(/^Review side-fix · [0-9a-f]{8}$/)

    // down: a segment of the way back reviews its branch
    await segments.first().click()
    const back = await openedReview(page, 4)
    expect(back.data).toMatchObject({ chain: ["air"] })
    expect(back.data.wholeStack).toBeUndefined()
  })

  test("should go down from the review of the whole stack the island opens, along its main line", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island")
    await syncAndWaitForBranches()
    await page.getByTestId("stack-island").getByRole("button", { name: "More actions for the air stack" }).click()
    await page.getByRole("menuitem", { name: "Review Whole Stack" }).click()
    const opened = await openedReview(page, 1)
    expect(opened.data.from?.map(branch => branch.name)).toEqual(["air", "bazel", "bridge", "runtime", "deploy"])
    const window = await openLinkedSubWindow(page, opened)
    await window.getByTestId("branch-review-segment").filter({ hasText: "runtime" }).click()
    expect((await openedReview(page, 2)).data).toMatchObject({ chain: ["air", "bazel", "bridge", "runtime"] })
  })

  test("should offer the whole stack of a straight chain on a branch with branches built on it, as the last one from the root", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the `stack_push_alone` template: perf-part-2 stacked on IJPL-257337, a straight chain
    await setupRepo("stack_push_alone")
    await syncAndWaitForBranches()
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "IJPL-257337"))
    const whole = window.getByTestId("branch-review-whole-stack")
    await whole.hover()
    await expect(window.getByRole("tooltip")).toHaveText("IJPL-257337 to perf-part-2 on origin/master: 2 branches · 3 commits, what the Safe Push of perf-part-2 lands")
    await whole.click()
    expect((await openedReview(page, 2)).data).toMatchObject({ chain: ["IJPL-257337", "perf-part-2"], focus: { range: "all" } })

    // the last branch: From Root is the whole stack already
    await reopenLinkedSubWindow(window, await openReviewWindow(page, "perf-part-2"))
    await expect(window.getByRole("button", { name: "From Root" })).toBeVisible()
    await expect(whole).toHaveCount(0)
  })
})

test.describe("Branch review window", () => {
  test("should mark the groups that need approval as the code-owner check answers while the window is open", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_ownership", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests() })
    await repo.setReviewAnswers({ answers: { login: AWAITING_CODE_OWNER } })
    await repo.holdRemoteAnswers({ hold: ["readiness"] })
    await syncAndWaitForBranches()
    await expect.poll(() => repo.getArrivedAnswers()).toContain("readiness")
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "login"))
    await expect(window.getByTestId("review-file-header").first()).toBeVisible()
    // the check hasn't answered: the owners of the files, no group that needs approval
    await expect.poll(() => ownerChips(window)).toEqual(["All4", "Auth Team1", "Design1", "Monorepo1", "UI Team1"])

    // it answers: the window marks its group without being opened again
    await repo.holdRemoteAnswers({ release: ["readiness"] })
    const group = window.getByTestId("branch-review-owner").filter({ hasText: "IJPL Code Platform" })
    await expect(group).toHaveText("IJPL Code Platform4")
    await group.hover()
    await expect(window.getByRole("tooltip", { name: "Needs approval for a Safe Push" })).toBeVisible()
  })

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
    await expect(page.locator("[data-testid=review-file-header][data-path='ui/login.css']").getByTestId("review-file-owner")).toHaveText("Design")

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
    await expect(page.getByRole("button", { name: "From Root" })).toHaveAttribute("aria-pressed", "true")
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
    await expect(page.getByTestId("branch-review-files")).toHaveText("Files1Auth Team")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js"])
  })

  test("should save the diff view the window chooses through the writer of the backend", async ({ page, setupRepo, syncAndWaitForBranches, testStore }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const reviewPage = await openLinkedSubWindow(page, await openReviewWindow(page, "login"))
    const unified = reviewPage.getByRole("button", { name: "Unified" })
    await expect(unified).toHaveAttribute("aria-pressed", "false")
    await unified.click()
    // the review window sends its change to the writer of the backend (`set_setting`), which keeps what differs from
    // the defaults
    await expect.poll(() => testStore.get("diffView")).toEqual({ mode: "unified" })
    // the writer's answer keeps the choice
    await expect(unified).toHaveAttribute("aria-pressed", "true")
  })

  test("should read the stack again from the window's own session when a sync of the main window moves a tip of it, and keep a branch that left", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const opened = await openReviewWindow(page, "login")
    // the link of the main window says nothing of the heads: the window reads them from its own session
    expect(Object.keys(opened.data.state).sort()).toEqual(["commitMenus", "createPullRequest", "groups", "items", "menus", "repositoryPath", "selected", "wholeStack"])
    const window = await openLinkedSubWindow(page, opened)
    const segments = window.getByTestId("branch-review-segment")
    await expect(segments).toHaveText(["auth1", "login2"])
    // the window holds the view of its own subscription to the session: the version the main window holds
    const review = window.getByTestId("branch-review")
    const mainVersion = async () => (await page.getByTestId("sync-button").getAttribute("data-view-version"))!
    await expect(review).toHaveAttribute("data-view-version", await mainVersion())
    let reads = 0
    window.on("request", request => {
      if (request.url().endsWith("/invoke/get_branch_review")) {
        reads++
      }
    })

    // a commit of login in a terminal, then Sync of the main window: its patches reach the window on its own channel
    await repo.commit("(login) Add the logout link", { "ui/logout.js": "export function logout() {}\n" }, 1704124800)
    await syncAndWaitForBranches()
    await expect(review).toHaveAttribute("data-view-version", await mainVersion())
    // the stack read again once, at the new head of login; the part stays: the reviewed branch on its parent
    await expect(segments).toHaveText(["auth1", "login3"])
    await expect.poll(() => reviewedPaths(window)).toContain("ui/logout.js")
    await expect(segments.nth(1)).toHaveAttribute("aria-pressed", "true")
    expect(reads).toBe(1)
    const push = window.getByTestId("branch-review-actions").getByRole("button", { name: /^Push/ })
    await expect(push).toBeEnabled()

    // login renamed in the main window: the sync of the rename leaves it out of the session's table
    await openContextMenu(page, findBranchRow(page, "login"))
    await clickContextMenuItem(page, "Rename…")
    const input = page.getByTestId("inline-rename-input").locator('input[type="text"]')
    await input.fill("signin")
    await runAndWaitForSync(page, () => input.press("Enter"))
    await expect(findBranchRow(page, "signin")).toBeVisible()
    await expect(review).toHaveAttribute("data-view-version", await mainVersion())
    // the review keeps its last view, says why, and is not read again
    await expect(window.getByTestId("branch-review-gone")).toHaveText("login is no longer a branch: it landed or was renamed")
    await expect(window.getByTestId("review-file-header").first()).toBeVisible()
    await expect(segments).toHaveText(["auth1", "login3"])
    // the push of login, which the main window can no longer run, stays, disabled
    await expect(push).toBeDisabled()
    expect(reads).toBe(1)
  })

  test("should act on the branch of the part from the title bar, through the main window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_ownership", { patronus: availableRobots(), pullRequests: availablePullRequests(LOGIN_PULL_REQUEST), patronusProfiles: PROFILES })
    // no approval needed: the Safe Push of each part is offered
    await repo.setReviewAnswers({ answers: { auth: NO_REVIEW_NEEDED, login: NO_REVIEW_NEEDED } })
    await syncAndWaitForBranches()
    // the robots, the pull requests and the approvals load after the sync: the steps wait for them
    for (const branch of ["auth", "login"]) {
      await expect(findBranchRow(page, branch).getByTestId("next-step-slot")).toHaveAttribute("data-step", "safePush")
    }
    await expect(findBranchRow(page, "login").getByTestId("pull-request-chip")).toBeVisible()

    // the window opens with the actions of each branch of its chain, the items of its menu
    const opened = await openReviewWindow(page, "login")
    const repositoryPath = opened.data.repositoryPath
    // auth has no pull request: its Create pushes it first, on the remote or not; the remote lacks auth, so its first
    // push is a step of Create and Safe Push: disabled with why, in its place, "⋯" pushes it alone. login's push
    // updates its pull request
    expect(itemSummary(opened.data.state.items)).toEqual({
      auth: ["create-pull-request", "safe-push", 'push: Safe Push and Create Pull Request… push auth first: "⋯" pushes it alone'],
      login: ["open-pull-request", "safe-push", "push"],
    })
    expect(opened.data.state.items.login![0]).toMatchObject({ label: "Open Pull Request", kbds: ["o"], url: LOGIN_PULL_REQUEST.url })
    // the Safe Push of login lands auth with it: the count of the branches it lands
    expect(opened.data.state.items.login![1]).toMatchObject({ label: "Safe Push · 2", kbds: ["s"] })
    expect(opened.data.state.items.auth![1]).toMatchObject({ label: "Safe Push", kbds: ["s"] })
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
    await actions.getByRole("button", { name: "Safe Push · 2" }).click()
    // the parent alone: its actions, without a pull request
    await page.getByTestId("branch-review-segment").first().click()
    await expect(actions.getByRole("button", { name: "Open Pull Request" })).toHaveCount(0)
    // the remote lacks auth: its first push is a step of its Safe Push, the title bar keeps the push in its place,
    // disabled with why
    await expect(actions.getByRole("button", { name: /^Safe Push/ })).toBeVisible()
    const firstPush = actions.getByRole("button", { name: /^Push/ })
    await expect(firstPush).toBeDisabled()
    await firstPush.hover()
    await expect(page.getByRole("tooltip", { name: 'Safe Push and Create Pull Request… push auth first: "⋯" pushes it alone' })).toBeVisible()
    await page.mouse.move(0, 0)
    await page.keyboard.press("s")
    // the whole stack: the reviewed branch
    await page.getByRole("button", { name: "From Root" }).click()
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
    const emptyState = (repository: string, selected: boolean): BranchReviewWindowState => ({
      repositoryPath: repository,
      selected,
      items: {},
      menus: {},
      createPullRequest: null,
      wholeStack: null,
      commitMenus: {},
      groups: null,
    })
    await emitTauriEvent(page, "branch-review/state", emptyState("/elsewhere", true))
    await expect(actions.getByRole("button", { name: "Safe Push · 2" })).toBeEnabled()
    await emitTauriEvent(page, "branch-review/state", emptyState(repositoryPath, false))
    // they stay: a click switches the main window to this repository first, as the hover says
    await expect(actions.getByRole("button", { name: "Safe Push · 2" })).toBeEnabled()
    // the tooltip of the button the mouse was on closes first
    await page.mouse.move(0, 0)
    await expect(page.getByRole("tooltip")).toHaveCount(0)
    await actions.getByRole("button", { name: "Push Stack (2 branches)", exact: true }).hover()
    await expect(page.getByRole("tooltip").filter({ hasText: /^Switches the main window to / })).toBeVisible()
    await expect(actions.getByRole("button", { name: "Open Pull Request" })).toBeEnabled()

    // a running robot disables the Safe Push; the pull request is gone
    await emitTauriEvent(page, "branch-review/state", {
      ...opened.data.state,
      items: { login: [{ id: "safe-push", label: "Safe Push · 2", kbds: ["s"], disabled: true, description: "A Safe Push of the last commit is running" }] },
    } satisfies BranchReviewWindowState)
    await expect(actions.getByRole("button", { name: "Open Pull Request" })).toHaveCount(0)
    await expect(actions.getByRole("button", { name: "Push Stack (2 branches)", exact: true })).toHaveCount(0)
    await expect(actions.getByRole("button", { name: "Safe Push · 2" })).toBeDisabled()
    await page.keyboard.press("s")
    expect(await getEmittedEvents(page, "branch-review/request")).toHaveLength(3)

    // a refusal of the main window shows where the user is
    await emitTauriEvent(page, "branch-review/feedback", { kind: "error", title: "Cannot push login", message: "The remote branch is up to date" } satisfies BranchReviewFeedback)
    await expect(page.getByText("Cannot push login")).toBeVisible()

    // the main window, in the page again: its listener runs what the window asked for
    await page.goto(`/?repoId=${new URL(page.url()).searchParams.get("repoId")}`)
    await syncAndWaitForBranches()
    // the Safe Push the window asks for runs once its step is known
    await expect(findBranchRow(page, "login").getByTestId("next-step-slot")).toHaveAttribute("data-step", "safePush")
    const pushes = await routePush(page, {})
    // a repository the main window can't switch to: refused with the reason, in the window
    await emitTauriEvent(page, "branch-review/request", { ...requests[2]!, id: "elsewhere", repositoryPath: "/elsewhere" })
    await expect.poll(() => getEmittedEvents(page, "branch-review/reply")).toEqual([{ id: "elsewhere", ok: false }])
    expect(await getEmittedEvents(page, "branch-review/feedback")).toEqual([
      { kind: "error", title: "Cannot push login", message: "elsewhere is no longer among the repositories of the main window" },
    ])
    await emitTauriEvent(page, "branch-review/request", requests[2])
    // the push of the row: the stack, parent first
    await expect.poll(() => pushes.map(it => it.branchName)).toEqual(["auth", "login"])
    // the Safe Push of login lands auth with it: something to say, the popover at login's row
    await emitTauriEvent(page, "branch-review/request", requests[0])
    await expect(safePushPopover(page)).toHaveAttribute("data-decision", "confirm")
    await expect(safePushPopover(page).getByTestId("safe-push-lands")).toContainText("Lands login with auth")
    await expect.poll(async () => (await getEmittedEvents<{ ok: boolean }>(page, "branch-review/reply")).map(it => it.ok)).toEqual([false, true, true])
    // the window asked: the main window comes to the front, once per request it ran
    expect(await page.evaluate(() => (window as unknown as { __e2eWindowCalls?: string[] }).__e2eWindowCalls?.filter(it => it === "set_focus"))).toEqual(["set_focus", "set_focus"])
  })

  test("should run an item of the branch's menu from the window's ⋯, in the main window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupAuthWithoutPullRequest(page, setupRepo, syncAndWaitForBranches)
    const reviewPage = await openLinkedSubWindow(page, await openReviewWindow(page, "auth"))
    // the whole menu of the row, not only the buttons of the title bar
    await reviewPage.getByTestId("branch-review-actions").getByRole("button", { name: "More actions for auth" }).click()
    await reviewPage.getByRole("menuitem", { name: "Rename…" }).click()
    // the main window runs it: the rename form of the row
    await expect(page.getByTestId("inline-rename-input")).toBeVisible()
    expect(await windowRequests(reviewPage)).toEqual([expect.objectContaining({ branchName: "auth", menuItemId: "rename" })])
  })

  test("should create the pull request in the window it was asked in, through the main window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupAuthWithoutPullRequest(page, setupRepo, syncAndWaitForBranches)
    const created = await recordCommand(page, "create_pull_request")

    const reviewPage = await openLinkedSubWindow(page, await openReviewWindow(page, "auth"))
    const actions = reviewPage.getByTestId("branch-review-actions")
    const windowCalls = () => reviewPage.evaluate(() => (window as unknown as { __e2eWindowCalls?: string[] }).__e2eWindowCalls ?? [])

    // the dialog opens here, with the code owner Patronus suggests; the main window stays where it is
    await actions.getByRole("button", { name: "Create Pull Request…" }).click()
    const dialog = createPullRequestDialog(reviewPage)
    await expect(dialog.getByTestId("create-pull-request-title")).toHaveValue("Add the token")
    await expect(dialog.getByTestId("create-pull-request-reviewers").locator("[data-slot=combobox-chip]")).toHaveText(["ikorennoy"])
    await expect(createPullRequestDialog(page)).toHaveCount(0)
    expect(await page.evaluate(() => (window as unknown as { __e2eWindowCalls?: string[] }).__e2eWindowCalls ?? [])).not.toContain("set_focus")

    // the title takes the focus with the caret after its text, a frame after the open ("Focus" in docs/design/e2e-testing.md)
    // S and O are letters of the title while the dialog is open, not the keys of the review
    await expect(dialog.getByTestId("create-pull-request-title")).toBeFocused()
    await reviewPage.keyboard.type(" so")
    await expect(dialog.getByTestId("create-pull-request-title")).toHaveValue("Add the token so")
    expect(await windowRequests(reviewPage)).toEqual([expect.objectContaining({ branchName: "auth", dialog: "open" })])
    expect(await getOpenedUrls(reviewPage)).toEqual([])

    // Escape closes the dialog, not the window
    await reviewPage.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
    expect(await windowCalls()).not.toContain("close")

    // the main window creates it; the toast is in the window, which opens the pull request
    await actions.getByRole("button", { name: "Create Pull Request…" }).click()
    await dialog.getByTestId("create-pull-request-submit").click()
    await expect(dialog).toBeHidden()
    const toast = reviewPage.locator("[data-slot=toast]", { hasText: "Created pull request #77" })
    await expect(toast).toBeVisible()
    expect(created).toEqual([expect.objectContaining({ branchName: "auth", title: "Add the token", reviewers: ["ikorennoy"] })])
    await toast.getByRole("button", { name: "Open Pull Request" }).click()
    await expect.poll(() => getOpenedUrls(reviewPage)).toEqual([pullRequest("auth", 77).url])

    // a main window that shows another repository refuses the creation, in the window
    const input = { title: "Add auth", body: "", reviewers: [], assignSelf: true, viewerLogin: null }
    await emitTauriEvent(page, "branch-review/request", {
      id: "elsewhere",
      repositoryPath: "/elsewhere",
      branchName: "auth",
      dialog: "create",
      input,
    } satisfies BranchReviewRequest)
    await expect
      .poll(async () => (await getEmittedEvents<{ id: string; ok: boolean }>(page, "branch-review/reply")).filter(it => it.id === "elsewhere"))
      .toEqual([{ id: "elsewhere", ok: false }])
    expect(created).toHaveLength(1)
  })

  test("should keep the Create pull request dialog of the window while a commit is reviewed, and move to the commit once it closes", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupAuthWithoutPullRequest(page, setupRepo, syncAndWaitForBranches)
    const reviewPage = await openLinkedSubWindow(page, await openReviewWindow(page, "login"))
    await expect(reviewPage.getByTestId("range-review-commit-header")).toHaveCount(0)
    await reviewPage.getByTestId("branch-review-actions").getByRole("button", { name: "Create Pull Request…" }).click()
    const dialog = createPullRequestDialog(reviewPage)
    const title = dialog.getByTestId("create-pull-request-title")
    await expect(title).toBeFocused()
    await reviewPage.keyboard.type(" so")
    const typed = await title.inputValue()
    expect(typed).toMatch(/ so$/)

    // D on a commit of the chain on screen: a move, which waits for the dialog
    await expandBranch(page, findBranchRow(page, "login"))
    await getBranchCommitRow(page, "login", "Add the login form").hover()
    await page.keyboard.press("d")
    const moved = await openedReview(page, 2)
    expect(moved.data).toMatchObject({ move: true, chain: ["auth", "login"] })
    await reopenLinkedSubWindow(reviewPage, moved)
    await expect(title).toHaveValue(typed)
    await expect(reviewPage.getByTestId("range-review-commit-header")).toHaveCount(0)

    // the review of another chain would start the window over: refused, the dialog keeps what was typed
    await findBranchRow(page, "auth").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Cannot review auth" })).toBeVisible()
    expect(await getOpenedSubWindows(page)).toHaveLength(2)
    await expect(title).toHaveValue(typed)

    // the dialog closes: the window shows the commit
    await reviewPage.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
    await expect(reviewPage.getByTestId("range-review-commit-header")).toContainText("Add the login form")
  })

  test("should run the menu of a commit of the review in the main window, its forms at the row, its copies in the window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the `stacked_ownership` template: login, two commits, stacked on auth
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const { stackPushes } = await routePushBranches(page)
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "login"))
    const column = window.getByTestId("range-review-commit")
    const older = column.filter({ hasText: "Add the login form" })

    // the right-click of a commit: the menu of its row, without key hints; Push Up to Here pushes in the main window
    await older.click({ button: "right" })
    await expect(window.getByRole("menuitem")).toHaveText([
      "Reword…",
      "Move to…",
      "Move to Unassigned",
      "Push Up to Here",
      "Copy Commit Hash",
      "Copy Commit Message",
      "Copy Changed Paths",
    ])
    await window.getByRole("menuitem", { name: "Push Up to Here" }).click()
    await expect.poll(() => stackPushes.map(push => push.map(branch => branch.branchName))).toEqual([["auth", "login"]])

    // the copies run in the window: the message of the commit as it shows it, in the virtual branch
    await setupClipboardPermissions(page.context())
    await older.click({ button: "right" })
    await window.getByRole("menuitem", { name: "Copy Commit Message" }).click()
    await expect.poll(() => readClipboard(window)).toBe("Add the login form")
    // the menu fades out: its items are there until it is gone, beside those of the next menu
    await expect(window.getByRole("menu")).toHaveCount(0)

    // "⋯" of the heading of the shown commit: Move to… opens the form of the row in the main window, at the commit
    await older.click()
    await window
      .getByTestId("range-review-commit-header")
      .getByRole("button", { name: /^More actions for / })
      .click()
    await window.getByRole("menuitem", { name: "Move to…" }).click()
    await expect(page.getByTestId("inline-branch-target")).toBeVisible()
    await expect(getBranchCommitRow(page, "login", "Add the login form")).toHaveAttribute("data-selected", "true")
    await page.keyboard.press("Escape")
    await expect(window.getByRole("menu")).toHaveCount(0)

    // Reword…: the preview of the commit in the main window, its message in edit
    await older.click({ button: "right" })
    await window.getByRole("menuitem", { name: "Reword…" }).click()
    await expect(getCommitPreview(page).getByRole("textbox", { name: "Commit message" })).toBeVisible()
  })

  test("should start the Safe Push without review the window's dialog offers in the main window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupAuthWithoutPullRequest(page, setupRepo, syncAndWaitForBranches)

    const reviewPage = await openLinkedSubWindow(page, await openReviewWindow(page, "auth"))
    await reviewPage.getByTestId("branch-review-actions").getByRole("button", { name: "Create Pull Request…" }).click()
    const dialog = createPullRequestDialog(reviewPage)
    await dialog.getByRole("button", { name: "Safe Push Without Review…" }).click()
    await expect(dialog).toBeHidden()
    // the popover of the row, in the main window, which comes to the front: without review, its profile to choose
    const popover = safePushPopover(page)
    await expect(popover).toHaveAttribute("data-decision", "choose")
    await expect(popover.getByTestId("safe-push-lands")).toHaveText("Lands auth into master: 1 commit.")
    await expect(popover.getByTestId("safe-push-status")).toHaveText("Choose the profile that skips the code-owner review")
    await expect(popoverStart(page)).toHaveText("Safe Push Without Review")
    await expect.poll(() => page.evaluate(() => (window as unknown as { __e2eWindowCalls?: string[] }).__e2eWindowCalls ?? [])).toContain("set_focus")
  })
})
