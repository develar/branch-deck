// @spec spec/branch/branch-review.spec.md
import type { Page } from "@playwright/test"
import type { PatronusProfile } from "../../../src/lib/bindings"
import type { ReviewRunRequest } from "../../../src/lib/review-runs"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, restOnBranchRow } from "../helpers/branch-helpers"
import { readClipboard, setupClipboardPermissions } from "../helpers/clipboard-helpers"
import { settledInvokes } from "../helpers/ipc-helpers"
import { AWAITING_CODE_OWNER, NO_REVIEW_NEEDED, availableRobots } from "../helpers/patronus-helpers"
import { getCommitPreview } from "../helpers/preview-helpers"
import { availablePullRequests, createPullRequestDialog, getOpenedUrls, gitHubRequests, pullRequest } from "../helpers/pull-request-helpers"
import { remotePushes, virtualBranch } from "../helpers/push-helpers"
import { addRecentProject, OTHER_REPOSITORY, switchRepository } from "../helpers/repository-switch-helpers"
import { reviewSidebar, turnOnReviewPlan } from "../helpers/review-plan-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { openReviewWindow, openedReview } from "../helpers/review-window-helpers"
import { popoverStart, safePushPopover } from "../helpers/safe-push-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import { getEmittedEvents, getOpenedSubWindows, openLinkedSubWindow, openSubWindowPage, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent, runAndWaitForAnswer, runAndWaitForSync } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { holdCommandCalls, recordCommand, recordRewrites } from "../helpers/wait-helpers"

/** The paths of the diffs the review shows, in the order of the tree: directories first */
async function reviewedPaths(page: Page): Promise<string[]> {
  return page.getByTestId("review-file-header").evaluateAll(headers => headers.map(header => header.getAttribute("data-path")!))
}

/** The header of the diff of `path` */
function fileHeader(page: Page, path: string) {
  return page.locator(`[data-testid=review-file-header][data-path='${path}']`)
}

/** `name:count` of the owner chips */
async function ownerChips(page: Page): Promise<string[]> {
  return page.getByTestId("branch-review-owner").evaluateAll(chips => chips.map(chip => chip.textContent!))
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
  return repo
}

/**
 * stacked_ownership with the robots and the pull request of login, no approval needed: the Safe Push of each part is
 * offered; with `otherRepository` the main window lists another repository to switch to
 */
async function setupLoginWithPullRequest(
  page: Page,
  setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
  { otherRepository = false }: { otherRepository?: boolean } = {},
) {
  const repo = await setupRepo("stacked_ownership", { patronus: availableRobots(), pullRequests: availablePullRequests(LOGIN_PULL_REQUEST), patronusProfiles: PROFILES })
  await repo.setReviewAnswers({ answers: { auth: NO_REVIEW_NEEDED, login: NO_REVIEW_NEEDED } })
  if (otherRepository) {
    await addRecentProject(page)
  }
  await syncAndWaitForBranches()
  // the robots, the pull requests and the approvals load after the sync: the steps wait for them
  for (const branch of ["auth", "login"]) {
    await expect(findBranchRow(page, branch).getByTestId("next-step-slot")).toHaveAttribute("data-step", "safePush")
  }
  await expect(findBranchRow(page, "login").getByTestId("pull-request-chip")).toBeVisible()
  return repo
}

/** What the window asked the main window to run at the rows (`review/run`), without the ids */
async function windowRuns(page: Page): Promise<Omit<ReviewRunRequest, "id">[]> {
  return (await getEmittedEvents<ReviewRunRequest>(page, "review/run")).map(({ id: _id, ...request }) => request)
}

/** The labels of the buttons of the title bar of the window: the actions of the part, then "⋯" */
async function titleBarActions(window: Page): Promise<string[]> {
  return (
    window
      .getByTestId("branch-review-actions")
      .getByRole("button")
      // the label of an action is its first text, beside its key; "⋯" is named by its label
      .evaluateAll(buttons => buttons.map(button => button.getAttribute("aria-label") ?? button.querySelector("span")?.textContent ?? ""))
  )
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

    // in the window P is the previous change, not a push: J moves to the next file, P back to the change of the first
    // once the diffs are read (before, N and P find no change)
    await showReviewWindow(page, byKey)
    const position = page.getByTestId("review-position")
    await expect(position).toHaveText(/File 1 \/ \d+$/)
    await expect(page.getByTestId("diff-view")).toBeVisible()
    await expect(page.getByTestId("diff-view")).not.toHaveAttribute("aria-busy")
    await page.locator("[aria-label='Changes of the commit']").focus()
    await page.keyboard.press("j")
    await expect(position).toHaveText(/File 2 \/ \d+$/)
    await page.keyboard.press("p")
    await expect(position).toHaveText(/File 1 \/ \d+$/)
    expect(await windowRuns(page)).toEqual([])
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
  test("should read the review of a branch ahead as its window opens, once per state, and nothing on a hover or a focus of its row", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    // the server reads it: the test only records the command
    const prefetches = await recordCommand(page, "prefetch_review")

    // the pointer rests on the row, the row gets the focus: nothing is read
    await restOnBranchRow(page, "login")
    await findBranchRow(page, "login").focus()
    await expect(findBranchRow(page, "login")).toBeFocused()
    await settledInvokes(page)
    expect(prefetches).toEqual([])

    // D opens the window and reads its own part on its parent, the stack from the root
    await page.keyboard.press("d")
    expect((await openedReview(page, 1)).title).toBe("Review login")
    await settledInvokes(page)
    expect(prefetches).toMatchObject([{ review: { target: { kind: "branch", chain: ["auth", "login"] } }, part: "own" }])

    // Review Changes of its menu: the window comes to the front, the branch is read already
    await openReviewWindow(page, "login")
    await settledInvokes(page)
    expect(prefetches).toHaveLength(1)

    // another branch: its own read
    await openReviewWindow(page, "auth")
    await settledInvokes(page)
    expect(prefetches).toMatchObject([
      { review: { target: { kind: "branch", chain: ["auth", "login"] } }, part: "own" },
      { review: { target: { kind: "branch", chain: ["auth"] } }, part: "own" },
    ])
  })
})

test.describe("The scopes of the stack", () => {
  test("should keep an unavailable Whole Stack in the scopes, announced as unavailable, its reason on hover, a press changing nothing", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the `stack_island` template, its stack branch not built: the review of the whole air stack can't open
    await setupRepo("stack_island", { syncOverrides: { stacks: { air: { tip: null } } } })
    await syncAndWaitForBranches()
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "bazel"))
    const whole = window.getByTestId("branch-review-whole-stack")
    await expect(whole).toHaveAttribute("aria-disabled", "true")
    await whole.hover()
    await expect(window.getByRole("tooltip")).toHaveText("The stack branch is not built yet: sync first")
    // a press (forced: Playwright waits for an enabled element) opens no review and presses nothing; From Root after it
    // still changes the scope
    await whole.click({ force: true })
    await expect(whole).toHaveAttribute("aria-pressed", "false")
    const fromRoot = window.getByRole("button", { name: "From Root" })
    await fromRoot.click()
    await expect(fromRoot).toHaveAttribute("aria-pressed", "true")
    await expect(whole).toHaveAttribute("aria-pressed", "false")
    expect(await getOpenedSubWindows(page)).toHaveLength(1)
  })

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
    // the window opens it itself, as the menus of the rows would
    await whole.click()
    const stack = await openedReview(window, 1)
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
    await expect(fix.getByTestId("review-commit-branch")).toHaveText("side-fix")
    await fix.click()
    await window.getByTestId("range-review-commit-header").getByRole("button", { name: "Review side-fix" }).click()
    const atCommit = await openedReview(window, 2)
    expect(atCommit.data).toMatchObject({ chain: ["air", "bazel", "side-fix"], focus: { commitId: expect.stringMatching(/^[0-9a-f]{40}$/) } })
    expect(atCommit.data.wholeStack).toBeUndefined()
    expect(atCommit.title).toMatch(/^Review side-fix · [0-9a-f]{8}$/)

    // down: a segment of the way back reviews its branch
    await segments.first().click()
    const back = await openedReview(window, 3)
    expect(back.data).toMatchObject({ chain: ["air"] })
    expect(back.data.wholeStack).toBeUndefined()
  })

  test("should name the branch of each commit row of the plan's tree, from the root and in the whole stack, and review the branch of a commit shown alone", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    turnOnReviewPlan(await setupRepo("stack_island"))
    await syncAndWaitForBranches()
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "bazel"))
    const { commitRow, column } = reviewSidebar(window)

    // From Root, a part of two branches: the tree orders the commits by step, so each row says its branch, which the
    // column said by its headings
    await window.getByRole("button", { name: "From Root" }).click()
    await expect(commitRow("Add the agent, part 1").getByTestId("review-commit-branch")).toHaveText("air")
    await expect(commitRow("Build the deploy jar, part 2").getByTestId("review-commit-branch")).toHaveText("bazel")
    await expect(commitRow("Build the deploy jar, part 2")).toHaveAttribute("aria-description", /bazel, commit [0-9a-f]{8}$/)
    await expect(column).toHaveCount(1)

    // the whole stack: each commit row names the branch it came from; Enter shows it alone, whose heading reviews its branch
    await window.getByTestId("branch-review-whole-stack").click()
    await reopenLinkedSubWindow(window, await openedReview(window, 1))
    const fix = commitRow("Fix the jar manifest, part 2")
    await expect(fix.getByTestId("review-commit-branch")).toHaveText("side-fix")
    await expect(column).toHaveCount(1)
    await fix.click()
    await fix.press("Enter")
    const header = window.getByTestId("range-review-commit-header")
    await expect(header).toContainText("Fix the jar manifest, part 2")
    await header.getByRole("button", { name: "Review side-fix" }).click()
    const atCommit = await openedReview(window, 2)
    expect(atCommit.data).toMatchObject({ chain: ["air", "bazel", "side-fix"], focus: { commitId: expect.stringMatching(/^[0-9a-f]{40}$/) } })
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
    expect((await openedReview(window, 1)).data).toMatchObject({ chain: ["air", "bazel", "bridge", "runtime"] })
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
    expect((await openedReview(window, 1)).data).toMatchObject({ chain: ["IJPL-257337", "perf-part-2"], focus: { range: "all" } })

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
    // the owners by the OWNERSHIP files of the baseline, the most files first, above the files they mark
    await expect.poll(() => ownerChips(page)).toEqual(["All4", "Auth Team1", "Design1", "Monorepo1", "UI Team1"])
    await expect(page.getByTestId("review-sidebar").getByTestId("branch-review-owners")).toBeVisible()
    await expect(fileHeader(page, "ui/login.css").getByTestId("review-file-owner")).toHaveText("Design")

    // a chip hides no file: the review stays at the file it shows, the other owners' files muted
    await page.getByRole("tree", { name: "Changed files" }).getByRole("treeitem", { name: "login.js" }).click()
    await expect(page.getByTestId("review-position")).toHaveText("All Changes · File 3 / 4")
    await page.getByTestId("branch-review-owner").filter({ hasText: "UI Team" }).click()
    await expect(fileHeader(page, "README.md")).toHaveAttribute("data-muted", "")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js", "ui/login.css", "ui/login.js", "README.md"])
    // the footer counts the part of the chip too: login.js is the one file of UI Team
    await expect(page.getByTestId("review-position")).toHaveText("All Changes · File 3 / 4 · UI Team 1 / 1")

    // all files again: nothing muted, the review still at the file it showed
    await page.getByTestId("branch-review-owner").filter({ hasText: "All" }).click()
    await expect(fileHeader(page, "README.md")).not.toHaveAttribute("data-muted")
    await expect(page.getByTestId("review-position")).toHaveText("All Changes · File 3 / 4")

    // the parent alone, one commit: the bar shows the part at once, the review keeps the files on screen until the files
    // of the part are read, then no column, the commit above its diffs
    const rangeFiles = await holdCommandCalls(page, "get_range_files")
    await segments.first().click()
    await expect(segments.first()).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => rangeFiles.calls.length).toBe(1)
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js", "ui/login.css", "ui/login.js", "README.md"])
    await expect(page.getByTestId("branch-review-body")).toHaveAttribute("aria-busy", "true")
    rangeFiles.release()
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js"])
    await expect(page.getByTestId("branch-review-body")).not.toHaveAttribute("aria-busy")
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
    await expect(page.getByTestId("review-position")).toHaveText(/^Commit 1 \/ 3 · File \d \/ 2$/)
    await expect(page.getByTestId("review-window-footer")).toContainText("Commits")

    // ⌥↓ twice from the diffs: the second while the next commit loads, the review keeps the focus
    await page.getByTestId("commit-review-code").locator("[tabindex='0']").first().focus()
    await page.keyboard.press("Alt+ArrowDown")
    await page.keyboard.press("Alt+ArrowDown")
    await expect(commits.getByTestId("range-review-commit").nth(3)).toHaveAttribute("aria-selected", "true")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js"])
    await expect(page.getByTestId("review-position")).toHaveText("Commit 3 / 3 · File 1 / 1")
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

  test("should mark the files of an owner chip, mute the others, and move J and K, N and P through its files only", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    await openReviewOf(page, "login")
    const tree = page.getByRole("tree", { name: "Changed files" })
    const position = page.getByTestId("review-position")
    const owner = (name: string) => page.getByTestId("branch-review-owner").filter({ hasText: name })
    const focusDiffs = () => page.getByTestId("commit-review-code").locator("[tabindex='0']").first().focus()
    await expect(position).toHaveText("All Changes · File 1 / 4")

    // Monorepo has README.md, the last file: every file stays, the rows and the headers of the others muted
    await owner("Monorepo").click()
    await expect(owner("Monorepo")).toHaveAttribute("aria-pressed", "true")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js", "ui/login.css", "ui/login.js", "README.md"])
    for (const muted of ["auth", "token.js", "ui", "login.js"]) {
      await expect(tree.getByRole("treeitem", { name: muted, exact: true })).toHaveAttribute("data-muted", "")
    }
    await expect(tree.getByRole("treeitem", { name: "README.md" })).not.toHaveAttribute("data-muted")
    await expect(fileHeader(page, "auth/token.js")).toHaveAttribute("data-muted", "")
    await expect(fileHeader(page, "README.md")).not.toHaveAttribute("data-muted")
    // the map of the files plays the others down too
    const map = page.getByTestId("review-file-map")
    await expect(map).toHaveAttribute("data-muted", "3")
    // the footer counts the files of Monorepo, the file shown being another owner's
    await expect(position).toHaveText("All Changes · File 1 / 4 · Monorepo 1")

    // N from the first file passes the blocks of the others to README.md; P finds none of Monorepo before it, and moves
    // nothing: with All it goes back to ui/login.js
    await focusDiffs()
    await page.keyboard.press("n")
    await expect(position).toHaveText("All Changes · File 4 / 4 · Monorepo 1 / 1")
    await page.keyboard.press("p")
    await owner("All").click()
    await expect(map).toHaveAttribute("data-muted", "0")
    await focusDiffs()
    await page.keyboard.press("p")
    await expect(position).toHaveText("All Changes · File 3 / 4")

    // J from auth/token.js passes the files of the others to README.md, K finds none of Monorepo before it: with All it
    // goes back to ui/login.js
    await tree.getByRole("treeitem", { name: "token.js" }).click()
    await expect(position).toHaveText("All Changes · File 1 / 4")
    await owner("Monorepo").click()
    await focusDiffs()
    await page.keyboard.press("j")
    await expect(position).toHaveText("All Changes · File 4 / 4 · Monorepo 1 / 1")
    await page.keyboard.press("k")
    await owner("All").click()
    await expect(tree.getByRole("treeitem", { name: "auth", exact: true })).not.toHaveAttribute("data-muted")
    await focusDiffs()
    await page.keyboard.press("k")
    await expect(position).toHaveText("All Changes · File 3 / 4")
  })

  test("should name at the end of a long card the next file of the chosen owner chip, which a click shows as J does", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_ownership")
    // a long file of Auth Team first in the tree, before the files of the others
    const lines = Array.from({ length: 40 }, (_, index) => `export const step${index + 1} = ${index + 1}\n`).join("")
    await repo.commit("(login) Keep the session", { "auth/session.js": lines }, 1704124800)
    await syncAndWaitForBranches()
    await openReviewOf(page, "login")
    const position = page.getByTestId("review-position")
    const owner = (name: string) => page.getByTestId("branch-review-owner").filter({ hasText: name })
    const session = page.locator("[data-testid=diff-view-file][data-path='auth/session.js']")
    await expect(position).toHaveText("All Changes · File 1 / 5")
    // the next file of all
    await expect(session.getByRole("button", { name: "Next: token.js auth", exact: true })).toBeVisible()

    // Monorepo has README.md, the last file: J and the line at the end of the card pass the files of the others
    await owner("Monorepo").click()
    const next = session.getByRole("button", { name: "Next: README.md", exact: true })
    await expect(next).toBeVisible()
    await next.click()
    await expect(position).toHaveText("All Changes · File 5 / 5 · Monorepo 1 / 1")
    await expect(page.getByRole("tree", { name: "Changed files" }).getByRole("treeitem", { name: "README.md" })).toHaveAttribute("aria-selected", "true")
    await expect(page.locator("[data-testid=diff-view-file][data-path='README.md']").getByTestId("review-card-end")).toHaveText("End of the review")
  })

  test("should keep the chip of an owner on a part without its files, muting nothing, and show no chips when one owner has them all", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    await openReviewOf(page, "login")
    await page.getByTestId("branch-review-owner").filter({ hasText: "Design" }).click()
    await expect(fileHeader(page, "README.md")).toHaveAttribute("data-muted", "")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js", "ui/login.css", "ui/login.js", "README.md"])
    await expect(page.getByTestId("branch-review-owner-empty")).toHaveCount(0)

    // the parent alone has no file of Design: its chip stays, at 0, so it can be turned off; nothing is muted
    await page.getByTestId("branch-review-segment").first().click()
    await expect.poll(() => ownerChips(page)).toEqual(["All1", "Auth Team1", "Design0"])
    await expect(page.getByTestId("branch-review-owner-empty")).toHaveText("No files of Design in this range")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js"])
    await expect(fileHeader(page, "auth/token.js")).not.toHaveAttribute("data-muted")
    await expect(page.getByRole("tree", { name: "Changed files" }).getByRole("treeitem", { name: "token.js" })).not.toHaveAttribute("data-muted")

    // all files: one owner has them all, so no chip would mark less; the header of the files names the owner
    await page.getByTestId("branch-review-owner").filter({ hasText: "All" }).click()
    await expect(page.getByTestId("branch-review-owners")).toHaveCount(0)
    await expect(page.getByTestId("branch-review-files")).toHaveText("Files1Auth Team")
    await expect.poll(() => reviewedPaths(page)).toEqual(["auth/token.js"])
  })

  test("should open on the owner chip last chosen in a review window of the repository where it marks files, else on All", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
    testStore,
  }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const owner = (window: Page, name: string) => window.locator(`[data-testid=branch-review-owner][data-owner='${name}']`)
    const first = await openLinkedSubWindow(page, await openReviewWindow(page, "login"))
    await expect.poll(() => ownerChips(first)).toEqual(["All4", "Auth Team1", "Design1", "Monorepo1", "UI Team1"])
    await expect(owner(first, "All")).toHaveAttribute("aria-pressed", "true")
    await owner(first, "Design").click()
    await expect(fileHeader(first, "README.md")).toHaveAttribute("data-muted", "")
    // remembered for the repository by the writer of the settings
    await expect.poll(() => testStore.get("recentProjects")).toMatchObject([{ reviewOwnerChip: { kind: "owner", name: "Design" } }])
    await first.close()

    // the next review window opens on it, its files marked
    const second = await openLinkedSubWindow(page, await openReviewWindow(page, "login"))
    await expect(owner(second, "Design")).toHaveAttribute("aria-pressed", "true")
    await expect(fileHeader(second, "README.md")).toHaveAttribute("data-muted", "")
    await expect(fileHeader(second, "ui/login.css")).not.toHaveAttribute("data-muted")
    await second.close()

    // auth has no file of Design: the window opens on All, which needs no chip here (one owner has every file)
    const third = await openLinkedSubWindow(page, await openReviewWindow(page, "auth"))
    await expect(third.getByTestId("branch-review-files")).toHaveText("Files1Auth Team")
    await expect(third.getByTestId("branch-review-owners")).toHaveCount(0)
    await expect(fileHeader(third, "auth/token.js")).not.toHaveAttribute("data-muted")
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
    // the open says what to review; the window reads the heads and the actions from its own session
    const opened = await openReviewWindow(page, "login")
    const window = await openLinkedSubWindow(page, opened)
    const segments = window.getByTestId("branch-review-segment")
    await expect(segments).toHaveText(["auth1", "login2"])
    // the window holds the view of its own subscription to the session: the version the main window holds. Each window
    // hears a patch on its own channel, either one first (a fact the remote answers after the sync): both are read
    // until they agree
    const review = window.getByTestId("review-window")
    const sameVersion = async () => {
      const shown = await review.getAttribute("data-view-version")
      return shown !== null && shown === (await page.getByTestId("sync-button").getAttribute("data-view-version"))
    }
    const expectMainVersion = () => expect.poll(sameVersion, { message: "the review window holds the version of the main window" }).toBe(true)
    await expectMainVersion()
    let reads = 0
    window.on("request", request => {
      if (request.url().endsWith("/invoke/get_review")) {
        reads++
      }
    })

    // a commit of login in a terminal, then Sync of the main window: its patches reach the window on its own channel
    await repo.commit("(login) Add the logout link", { "ui/logout.js": "export function logout() {}\n" }, 1704124800)
    await syncAndWaitForBranches()
    await expectMainVersion()
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
    await expectMainVersion()
    // the review keeps its last view, says why, and is not read again
    await expect(window.getByTestId("branch-review-gone")).toHaveText("login is no longer a branch: it landed or was renamed")
    await expect(window.getByTestId("review-file-header").first()).toBeVisible()
    await expect(segments).toHaveText(["auth1", "login3"])
    // the push of login, which no longer is a branch, stays, disabled
    await expect(push).toBeDisabled()
    expect(reads).toBe(1)
  })

  test("should build the actions of the title bar of each part from the window's own session, and open the pull request itself", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupLoginWithPullRequest(page, setupRepo, syncAndWaitForBranches)
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "login"))
    const actions = window.getByTestId("branch-review-actions")
    // login: its pull request (O), its Safe Push, which lands auth with it (S), the push of its stack, without a key
    // (P is the previous change in the review); then its whole menu
    await expect.poll(() => titleBarActions(window)).toEqual(["Open Pull Request", "Safe Push · 2", "Push Stack (2 branches)", "More actions for login"])
    await actions.getByRole("button", { name: "Push Stack (2 branches)", exact: true }).hover()
    await expect(window.getByRole("tooltip", { name: "Pushes auth and login, parents first" })).toBeVisible()
    await window.mouse.move(0, 0)
    const footer = window.getByTestId("review-window-footer")
    await expect(window.getByTestId("review-position")).toHaveText("All Changes · File 1 / 4")
    await expect(footer).toContainText("JKFiles")
    await expect(footer).toContainText("NPChanges")
    await expect(footer).toContainText("EscClose")

    // O opens the pull request in the window itself, nothing asked of the main window
    await window.locator("[aria-label='Changes of the commit']").focus()
    await window.keyboard.press("o")
    await expect.poll(() => getOpenedUrls(window)).toEqual([LOGIN_PULL_REQUEST.url])
    // the parent alone: its actions, without a pull request; the remote lacks auth, so its first push is a step of its
    // Create and its Safe Push: disabled with why, in its place, "⋯" pushes it alone
    await window.getByTestId("branch-review-segment").first().click()
    await expect.poll(() => titleBarActions(window)).toEqual(["Create Pull Request…", "Safe Push", "Push Branch (1)", "More actions for auth"])
    const firstPush = actions.getByRole("button", { name: /^Push/ })
    await expect(firstPush).toBeDisabled()
    await firstPush.hover()
    await expect(window.getByRole("tooltip", { name: 'Safe Push and Create Pull Request… push auth first: "⋯" pushes it alone' })).toBeVisible()
    expect(await windowRuns(window)).toEqual([])
  })

  test("should run the Safe Push and the push of the title bar at the rows of the main window, switched to the repository of the review first", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupLoginWithPullRequest(page, setupRepo, syncAndWaitForBranches, { otherRepository: true })
    const opened = await openReviewWindow(page, "login")
    const { repositoryPath } = opened.data
    const window = await openLinkedSubWindow(page, opened)
    const actions = window.getByTestId("branch-review-actions")
    const push = actions.getByRole("button", { name: "Push Stack (2 branches)", exact: true })
    await expect(push).toBeEnabled()

    // the main window shows another repository: an action it runs at a row says that it switches back first
    await switchRepository(page, OTHER_REPOSITORY)
    await push.hover()
    await expect(window.getByRole("tooltip").filter({ hasText: /^Switches the main window to / })).toBeVisible()
    await window.mouse.move(0, 0)
    // the push asks first at the row when it would drop remote commits: the main window runs it there, switched back,
    // the stack in one push
    await push.click()
    await expect.poll(() => remotePushes(repo)).toEqual([["auth", "login"]])
    await expect(findBranchRow(page, "login")).toBeVisible()

    // S: the Safe Push of login lands auth with it, something to say: the popover at login's row of the main window
    await window.locator("[aria-label='Changes of the commit']").focus()
    await window.keyboard.press("s")
    await expect(safePushPopover(page)).toHaveAttribute("data-decision", "confirm")
    await expect(safePushPopover(page).getByTestId("safe-push-lands")).toContainText("Lands login with auth")
    expect(await windowRuns(window)).toEqual([
      { repositoryPath, subject: "login", itemId: "push" },
      { repositoryPath, subject: "login", itemId: "safe-push" },
    ])
    // the main window comes to the front, once per action it ran
    expect(await page.evaluate(() => (window as unknown as { __e2eWindowCalls?: string[] }).__e2eWindowCalls?.filter(it => it === "set_focus"))).toEqual(["set_focus", "set_focus"])

    // a repository the main window can't switch to: refused with the reason, shown in the window
    await emitTauriEvent(page, "review/run", { id: "elsewhere", repositoryPath: "/elsewhere", subject: "login", itemId: "push" } satisfies ReviewRunRequest)
    await expect
      .poll(async () => (await getEmittedEvents<{ id: string; ok: boolean }>(page, "review/reply")).filter(it => it.id === "elsewhere"))
      .toEqual([{ id: "elsewhere", ok: false }])
    await expect(window.getByText("elsewhere is no longer among the repositories of the main window")).toBeVisible()
  })

  test("should run an item of the branch's menu from the window's ⋯, in the main window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupAuthWithoutPullRequest(page, setupRepo, syncAndWaitForBranches)
    const reviewPage = await openLinkedSubWindow(page, await openReviewWindow(page, "auth"))
    // the whole menu of the row, not only the buttons of the title bar
    await reviewPage.getByTestId("branch-review-actions").getByRole("button", { name: "More actions for auth" }).click()
    await reviewPage.getByRole("menuitem", { name: "Rename…" }).click()
    // the main window runs it: the rename form of the row
    await expect(page.getByTestId("inline-rename-input")).toBeVisible()
    expect(await windowRuns(reviewPage)).toEqual([expect.objectContaining({ subject: "auth", itemId: "rename" })])
  })

  test("should show the Unstack the row runs as running in the window's menu, from the window's own session, and run it once", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "oauth"))
    await expect(window.getByTestId("review-file-header").first()).toBeVisible()
    const unstacks = await recordRewrites(page, "unstack")
    // the row's menu starts it: it waits in the session for the repository lock another process holds
    await repo.holdRepositoryLock(true)
    await openContextMenu(page, findBranchRow(page, "oauth"))
    await clickContextMenuItem(page, "Unstack from login")
    await expect.poll(() => unstacks.length).toBe(1)
    // the window's session runs it: the item of its menu is running, as on the row, so a click there starts nothing
    await window.getByTestId("branch-review-actions").getByRole("button", { name: "More actions for oauth" }).click()
    await expect(window.getByRole("menuitem", { name: "Unstack from login" })).toBeDisabled()
    await window.keyboard.press("Escape")
    await runAndWaitForSync(page, () => repo.holdRepositoryLock(false))
    await expect(page.getByText(/^Unstacked oauth/)).toBeVisible()
    expect(unstacks).toHaveLength(1)
    expect(await windowRuns(window)).toEqual([])
  })

  test("should create the pull request in the window, with the context of its own session", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupAuthWithoutPullRequest(page, setupRepo, syncAndWaitForBranches)

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
    expect(await windowRuns(reviewPage)).toEqual([])
    expect(await getOpenedUrls(reviewPage)).toEqual([])

    // Escape closes the dialog, not the window
    await reviewPage.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
    expect(await windowCalls()).not.toContain("close")

    // the window creates it (the push, then the pull request: the answer first); the toast is in the window, which
    // opens the pull request
    await actions.getByRole("button", { name: "Create Pull Request…" }).click()
    await runAndWaitForAnswer(reviewPage, "execute", () => dialog.getByTestId("create-pull-request-submit").click())
    await expect(dialog).toBeHidden()
    const toast = reviewPage.locator("[data-slot=toast]", { hasText: "Created pull request #77" })
    await expect(toast).toBeVisible()
    expect(await gitHubRequests(repo, "POST", "pulls")).toEqual([expect.objectContaining({ head: "user-name/virtual/auth", title: "Add the token" })])
    expect(await gitHubRequests(repo, "POST", "pulls", 77, "requested_reviewers")).toEqual([{ reviewers: ["ikorennoy"] }])
    await toast.getByRole("button", { name: "Open Pull Request" }).click()
    await expect.poll(() => getOpenedUrls(reviewPage)).toEqual([pullRequest("auth", 77).url])

    expect(await windowRuns(reviewPage)).toEqual([])
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
    expect(moved.data).toMatchObject({ branchName: "login", chain: ["auth", "login"], focus: { commitId: expect.stringMatching(/^[0-9a-f]{40}$/) } })
    await reopenLinkedSubWindow(reviewPage, moved)
    await expect(title).toHaveValue(typed)
    await expect(reviewPage.getByTestId("range-review-commit-header")).toHaveCount(0)

    // the review of another chain would start the window over: the window refuses it, the dialog keeps what was typed
    await findBranchRow(page, "auth").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    await reopenLinkedSubWindow(reviewPage, await openedReview(page, 3))
    await expect(reviewPage.locator("[data-slot=toast]", { hasText: "Cannot review auth" })).toBeVisible()
    await expect(title).toHaveValue(typed)

    // the dialog closes: the window shows the commit
    await reviewPage.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
    await expect(reviewPage.getByTestId("range-review-commit-header")).toContainText("Add the login form")
  })

  test("should push up to a commit of the review from its menu in the main window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the `stacked_ownership` template: login, two commits, stacked on auth
    const repo = await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "login"))
    const column = window.getByTestId("range-review-commit")
    const older = column.filter({ hasText: "Add the login form" })
    // the actions of the main window came, those of the title bar and of the menus of the commits alike
    await expect(window.getByTestId("branch-review-actions").getByRole("button", { name: /^Push / })).toBeVisible()

    // the right-click of a commit: the menu of its row, without key hints; Push Up to Here pushes in the main window
    await older.click({ button: "right" })
    await expect(window.getByRole("menuitem")).toHaveText([
      "Reword…",
      "Move to…",
      "Move to Unassigned",
      "Push Up to Here",
      "Drop Commit…",
      "Copy Commit Hash",
      "Copy Commit Message",
      "Copy Changed Paths",
    ])
    // the main window pushes: its answer, however long the push takes
    await runAndWaitForAnswer(page, "execute", () => window.getByRole("menuitem", { name: "Push Up to Here" }).click())
    // one push of the stack: login up to the copy of that commit, its newer commit left out
    await expect.poll(() => remotePushes(repo)).toEqual([["auth", "login"]])
    expect((await repo.getRemoteBranches())[virtualBranch("login")]).toBe(await repo.revision(`${virtualBranch("login")}~1`))
  })

  test("should run the menu of a commit of the review in the main window, its forms at the row, its copies in the window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "login"))
    const older = window.getByTestId("range-review-commit").filter({ hasText: "Add the login form" })

    // the copies run in the window: the message of the commit as it shows it, in the virtual branch
    await setupClipboardPermissions(page.context())
    await older.click({ button: "right" })
    await window.getByRole("menuitem", { name: "Copy Commit Message" }).click()
    await expect.poll(() => readClipboard(window)).toBe("Add the login form")
    // the menu fades out: its items are there until it is gone, beside those of the next menu
    await expect(window.getByRole("menu")).toHaveCount(0)

    // "⋯" of the heading of the shown commit: Move to… opens the form of the row in the main window, at the commit. The
    // menu opened while the diffs of the commit load stays open as they come and the heading moves into the diffs
    const diffs = await holdCommandCalls(window, "get_file_diffs")
    await older.click()
    await window
      .getByTestId("range-review-commit-header")
      .getByRole("button", { name: /^More actions for / })
      .click()
    await expect(window.getByRole("menuitem", { name: "Move to…" })).toBeVisible()
    await expect.poll(() => diffs.calls.length).toBeGreaterThan(0)
    diffs.release()
    await expect(window.getByTestId("diff-view")).not.toHaveAttribute("aria-busy")
    await expect(window.getByRole("menuitem", { name: "Move to…" })).toBeVisible()
    await window.getByRole("menuitem", { name: "Move to…" }).click()
    await expect(page.getByTestId("inline-branch-target")).toBeVisible()
    await expect(getBranchCommitRow(page, "login", "Add the login form")).toHaveAttribute("data-selected", "true")
    await page.keyboard.press("Escape")
    await expect(window.getByRole("menu")).toHaveCount(0)

    // Drop Commit…: the confirmation below the row in the main window, at the commit
    await older.click({ button: "right" })
    await window.getByRole("menuitem", { name: "Drop Commit…" }).click()
    await expect(page.getByTestId("drop-commits-form").getByRole("button", { name: "Drop Commit" })).toBeFocused()
    await expect(getBranchCommitRow(page, "login", "Add the login form")).toHaveAttribute("data-selected", "true")
    await page.keyboard.press("Escape")
    await expect(page.getByTestId("drop-commits-form")).toHaveCount(0)

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
