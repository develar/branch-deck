// @spec spec/branch/stacked-branches.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { openActivityWindow } from "../helpers/activity-helpers"
import {
  branchTree,
  collapseBranch,
  expandBranch,
  expectNothingToPush,
  findBranchRow,
  getBranchCommitRow,
  getBranchDetailsRow,
  getMoreButton,
  getPushButton,
  getStackChip,
  unstackOauthFromMenuWaiting,
  verifyCommitCount,
} from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { inlineBranchCreator } from "../helpers/inline-form-helpers"
import {
  AWAITING_CODE_OWNER,
  availableRobots,
  expectPatronusStarts,
  mergeRunningRobot,
  mergeRunningRobots,
  robot,
  setupRunningRobot,
  setupTakenRobot,
  startRequest,
  wholeStackStartRequest,
} from "../helpers/patronus-helpers"
import { availablePullRequests, createPullRequestDialog, getOpenedUrls, gitHubRequests, gitHubWrites, pullRequest, repoPath, reviewer } from "../helpers/pull-request-helpers"
import {
  branchOf,
  confirmForcePush,
  expectPushedToast,
  expectPushedToRemote,
  fixesOfOthersOnRemote,
  holdRemotePushes,
  pushedBranches,
  pushedToast,
  remotePushes,
  stackBranchOf,
  virtualBranch,
} from "../helpers/push-helpers"
import { EARLIER_COMMIT, landingRow, openBranchPreview, openLandingOverview } from "../helpers/review-status-helpers"
import {
  closeRefusal,
  expectDisabledItems,
  expectLandedActions,
  expectPopoverAtSlot,
  openSafePushWithKey,
  openSafePushWithProfile,
  pathRows,
  popoverStart,
  safePushPopover,
  slotSafePush,
  waitForStepFacts,
} from "../helpers/safe-push-helpers"
import { clickContextMenuItem, clickGroupIntoBranchButton, closeMenus, dropAt, dropOn, openContextMenu, openSubmenu, selectCommit, startDrag } from "../helpers/selection-helpers"
import { getOpenedSubWindows, openLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent, finishedSyncCount, runAndWaitForAnswer, runAndWaitForSync, syncBranches, syncFromMenu } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { closeToast } from "../helpers/toast-helpers"
import type { RewriteCall } from "../helpers/wait-helpers"
import { holdCommand, recordCommand, recordRewrites, routeCommandError, waitForStorePersistence } from "../helpers/wait-helpers"

const GITHUB_COMPARE = "https://github.com/acme/monorepo/compare"

/** Whom a rewrite waits for while the test server holds the repository lock, as another process does */
const WAITING_FOR_HOLDER = /^Waiting for `[^`]+` \(pid \d+, for \d+ s\)…$/

/**
 * The unstack of oauth waits in the session for the repository lock the test holds (`repo.holdRepositoryLock`): the
 * rewrites of another row stay offered and say what their start waits behind; released, it ran once and its toast comes
 * with the sync
 */
async function expectRewritesWaitAndUnstackOnce(page: Page, repo: TestRepositoryBuilder, unstacks: readonly RewriteCall[]): Promise<void> {
  // the other rewrites stay offered: a start waits for the unstack in the queue of the session
  await openContextMenu(page, findBranchRow(page, "docs"))
  for (const name of [/^Move to…/, /^Stack On…/, /^Unapply…/]) {
    await expect(page.getByRole("menuitem", { name })).not.toHaveAttribute("aria-disabled", "true")
  }
  await page.getByRole("menuitem", { name: /^Unapply…/ }).hover()
  await expect(page.locator("[data-slot=tooltip-content][data-open]", { hasText: "Waits for Unstacking oauth…" })).toBeVisible()
  await page.keyboard.press("Escape")
  const syncs = await finishedSyncCount(page)
  await runAndWaitForSync(page, () => repo.holdRepositoryLock(false))
  // its toast comes once the sync showed the result
  await expect(page.getByText(/^Unstacked oauth/)).toBeVisible()
  // one rewrite reached the backend, and one sync showed it
  expect(unstacks).toHaveLength(1)
  expect(await finishedSyncCount(page)).toBe(syncs + 1)
}

/** `name@lane` of the branch rows in display order: the lane of the node in the stack graph */
async function lanes(page: Page): Promise<string[]> {
  return page.getByTestId("branch-row").evaluateAll(rows => rows.map(row => `${row.getAttribute("data-branch-name")}@${row.getAttribute("data-lane")}`))
}

/** The x of the middle of the node of a branch */
async function nodeX(page: Page, name: string): Promise<number> {
  const box = await boxOf(findBranchRow(page, name).getByTestId("lane-node"))
  return box.x + box.width / 2
}

/** The x of the middle of the commit dots of an expanded branch (the `::before` of the commit cells), one per commit */
async function commitDotXs(page: Page, name: string): Promise<number[]> {
  return getBranchDetailsRow(page, name)
    .locator("tr[data-row-id] > td")
    .evaluateAll(cells =>
      cells.map(cell => {
        const dot = getComputedStyle(cell, "::before")
        return cell.getBoundingClientRect().left + Number.parseFloat(dot.left) + Number.parseFloat(dot.width) / 2
      }),
    )
}

async function boxOf(locator: Locator): Promise<{ x: number; y: number; width: number; height: number }> {
  const box = await locator.boundingBox()
  expect(box).not.toBeNull()
  return box!
}

test.describe("Stacked branches", () => {
  test("should show the branches of a stack under their parents", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    await verifyCommitCount(findBranchRow(page, "login"), 1)

    expect(await branchTree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
    // a graph, not a staircase: the chain auth → login → oauth stays on lane 0, session forks off it on lane 1
    expect(await lanes(page)).toEqual(["docs@0", "auth@0", "session@1", "login@0", "oauth@0"])
    const authX = await nodeX(page, "auth")
    expect(Math.abs((await nodeX(page, "oauth")) - authX)).toBeLessThanOrEqual(1)
    expect(Math.round((await nodeX(page, "session")) - authX)).toBe(16)
    // every name starts at the same x, whatever the lane
    const nameX = async (name: string) => Math.round((await boxOf(findBranchRow(page, name).getByText(name, { exact: true }))).x)
    expect(await nameX("session")).toBe(await nameX("docs"))
    expect(await nameX("oauth")).toBe(await nameX("docs"))
    // the node says where the branch is on its way into master, its hover in words; the push is the dot of the status
    const node = findBranchRow(page, "login").getByTestId("lane-node")
    await expect(node).toHaveAttribute("data-kind", "yourStep")
    await expect(node).toHaveAttribute("data-filled", "true")
    await findBranchRow(page, "login").getByTestId("lane-node-target").hover()
    // GitHub doesn't list the pull requests here: nobody judges the approvals before the start, the start is the user's step
    await expect(page.getByTestId("lane-node-details")).toHaveText("Safe Push lands 2 branches: Patronus checks the approvals when the robot starts")
    await expect(findBranchRow(page, "login").getByTestId("branch-status-dot")).toHaveAttribute("data-color", "primary")
    // the graph shows what a branch is built on: the row doesn't say it
    await expect(findBranchRow(page, "oauth").getByTestId("stack-base")).toHaveCount(0)
    // the preview has no tree: it says what the branch is built on
    await findBranchRow(page, "oauth").hover()
    await page.keyboard.press("Space")
    await expect(page.getByTestId("branch-preview-body")).toBeVisible()
    await expect(page.getByRole("dialog").getByTestId("stack-base")).toHaveText("on login")
    await page.keyboard.press("Escape")
    // the landing overview has no tree either: the stack is one row, its root's, as it lands
    const overview = await openLandingOverview(page)
    await expect(landingRow(overview, "auth")).toContainText("4 branches")
    await expect(landingRow(overview, "login")).toHaveCount(0)
    await page.keyboard.press("Escape")
    // session belongs to an issue that its parent doesn't: the chip says so
    await expect(findBranchRow(page, "session").getByTestId("branch-issue-key")).toHaveText("ABC-1")
    await expect(findBranchRow(page, "login").getByTestId("branch-issue-key")).toHaveCount(0)
  })

  test("should carry the commits of a branch on its lane, into the lines of its children", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    await expandBranch(page, findBranchRow(page, "auth"))
    await expandBranch(page, findBranchRow(page, "oauth"))

    // the commits of auth sit on its lane, which runs on beside them to the bottom of the cell, into session and login
    const authX = await nodeX(page, "auth")
    for (const dotX of await commitDotXs(page, "auth")) {
      expect(Math.abs(dotX - authX)).toBeLessThanOrEqual(1)
    }
    const authLines = await boxOf(getBranchDetailsRow(page, "auth").getByTestId("lane-lines"))
    expect(Math.abs(authLines.x + 8 - authX)).toBeLessThanOrEqual(1)
    const authDetails = await boxOf(getBranchDetailsRow(page, "auth"))
    expect(Math.abs(authLines.y + authLines.height - (authDetails.y + authDetails.height))).toBeLessThanOrEqual(1)
    // oauth has no children: its lane ends at its last commit, no line runs on below it
    await expect(getBranchDetailsRow(page, "oauth").getByTestId("commit-rail").locator("xpath=..")).toHaveAttribute("data-lane-end", "")
    await expect(getBranchDetailsRow(page, "oauth").getByTestId("lane-lines")).toHaveCount(0)
    for (const dotX of await commitDotXs(page, "oauth")) {
      expect(Math.abs(dotX - authX)).toBeLessThanOrEqual(1)
    }
    // the commit subjects start at the branch name
    const name = await boxOf(findBranchRow(page, "oauth").getByText("oauth", { exact: true }))
    const subject = await boxOf(getBranchCommitRow(page, "oauth", "Add OAuth provider").getByText("Add OAuth provider", { exact: true }))
    expect(Math.abs(subject.x - name.x)).toBeLessThanOrEqual(1)
  })

  test("should draw the lanes beside the inline form of a stacked branch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    await openContextMenu(page, findBranchRow(page, "session"))
    await clickContextMenuItem(page, "Rename…")
    const formRow = page.locator("[data-branch-name=session] + tr")
    await expect(formRow.getByTestId("inline-rename-input")).toBeVisible()
    // the line of auth passes the form on lane 0 down to login; session has no children, its lane 1 ends at its node
    const lines = await boxOf(formRow.getByTestId("lane-lines"))
    expect(Math.abs(lines.x + 8 - (await nodeX(page, "auth")))).toBeLessThanOrEqual(1)
    expect(Math.round(lines.width)).toBe(16)
    const row = await boxOf(formRow)
    expect(Math.abs(lines.height - row.height)).toBeLessThanOrEqual(1)
    // the form starts right of the lanes, its title at the branch name
    const name = await boxOf(findBranchRow(page, "session").getByText("session", { exact: true }))
    const title = await boxOf(formRow.getByRole("heading", { name: "Rename session" }))
    expect(Math.abs(title.x - name.x)).toBeLessThanOrEqual(1)
  })

  test("should draw the path to the root of the hovered branch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    const onPath = () => pathRows(page)

    await expect(page.getByTestId("stack-path-lanes")).toHaveCount(0)
    await findBranchRow(page, "oauth").getByText("oauth", { exact: true }).hover()
    // auth → login → oauth; the line of auth passes session on its way to login; docs is another stack
    await expect.poll(onPath).toEqual(["auth", "session", "login", "oauth"])
    await findBranchRow(page, "session").getByText("session", { exact: true }).hover()
    await expect.poll(onPath).toEqual(["auth", "session"])
    // a root has no path, nor has a row outside the table body
    await findBranchRow(page, "docs").getByText("docs", { exact: true }).hover()
    await expect(page.getByTestId("stack-path-lanes")).toHaveCount(0)
    await page.getByTestId("landing-indicator").hover()
    await expect(page.getByTestId("stack-path-lanes")).toHaveCount(0)
  })

  test("should mark the branches a push sends when its button is hovered", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    const pushed = () => page.locator("[data-testid=lane-node][data-pushed]").evaluateAll(nodes => nodes.map(node => node.closest("tr")!.getAttribute("data-branch-name")))

    await getPushButton(findBranchRow(page, "oauth")).hover()
    // the button pushes the stack, parents first: its path and the nodes it sends
    await expect.poll(pushed).toEqual(["auth", "login", "oauth"])
    await expect(findBranchRow(page, "login").getByTestId("stack-path-lanes")).toHaveCount(1)
    await getPushButton(findBranchRow(page, "docs")).hover()
    await expect.poll(pushed).toEqual(["docs"])
  })

  test("should head a stack with its size and collapse it into its root, per repository", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    const chip = getStackChip(findBranchRow(page, "auth"))
    // the root heads its stack; a lone branch and the other branches of the stack have no chip
    await expect(chip).toHaveText("4 branches")
    await expect(chip).toHaveAttribute("aria-expanded", "true")
    await expect(page.getByTestId("stack-chip")).toHaveCount(1)

    await chip.click()
    expect(await branchTree(page)).toEqual(["docs@0", "auth@0"])
    await expect(chip).toHaveAttribute("data-stack", "collapsed")
    // where the hidden rows are on their way into master, in the colour of their nodes: neither Patronus nor GitHub, so
    // their push is the user's step
    await expect(chip.getByTestId("stack-chip-attention")).toHaveText("· 3 your step")
    await expect(chip.getByTestId("stack-chip-attention")).toHaveClass(/text-primary/)
    // the click closed its tooltip, the next hover opens it
    await page.mouse.move(0, 0)
    await chip.hover()
    await expect(page.getByRole("tooltip")).toContainText("Stack of auth · 4 branches · 4 commits · 3 your step")
    await expect(findBranchRow(page, "auth").getByTestId("lane-collapsed")).toBeVisible()
    // the header still has the whole stack, one row of its 4 branches
    await expect(landingRow(await openLandingOverview(page), "auth")).toContainText("4 branches")
    await page.keyboard.press("Escape")

    // the repository remembers it (`collapsedStacks`): a new window starts with the stack collapsed
    await waitForStorePersistence(page, "recentProjects", projects => (projects as { collapsedStacks?: string[] }[] | undefined)?.[0]?.collapsedStacks?.includes("auth") === true)
    await page.reload()
    await syncBranches(page, ['[data-testid="branch-row"][data-branch-name="auth"]'])
    expect(await branchTree(page)).toEqual(["docs@0", "auth@0"])
    await getStackChip(findBranchRow(page, "auth")).click()
    expect(await branchTree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
  })

  test("should name in the chip of a collapsed stack only the robots that failed on its branches as they are", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches", {
      patronus: availableRobots(
        robot("session", "session-failed", { status: "failure", finishTime: 1704142800 }),
        // failed on commits the branches are no longer at: fixed since, their rows offer a new Safe Push
        robot("login", "login-failed", { status: "failure", finishTime: 1704142800, sourceRevision: EARLIER_COMMIT }),
        robot("oauth", "oauth-failed", { status: "failure", finishTime: 1704142800, sourceRevision: EARLIER_COMMIT }),
      ),
    })
    await syncAndWaitForBranches()
    await expect(page.getByTestId("landing-indicator")).toHaveAttribute("data-urgent", "needsFix")
    // the node of the failed branch needs a fix, where the eye starts reading a row; the ones fixed since are a Safe Push again
    await expect(findBranchRow(page, "session").getByTestId("lane-node")).toHaveAttribute("data-kind", "needsFix")
    await expect(findBranchRow(page, "session").getByTestId("lane-node")).toHaveAttribute("data-color", "error")
    await expect(findBranchRow(page, "session").getByTestId("lane-node")).toHaveAttribute("data-filled", "true")
    await expect(findBranchRow(page, "login").getByTestId("lane-node")).toHaveAttribute("data-kind", "yourStep")
    const chip = getStackChip(findBranchRow(page, "auth"))
    await chip.click()
    // the header's count: the failure the user has to act on, not the two fixed since
    await expect(chip.getByTestId("stack-chip-attention")).toHaveText("· 1 to fix")
    await expect(chip.getByTestId("stack-chip-attention")).toHaveClass(/text-destructive/)
  })

  test("should collapse and expand a stack with Alt+arrows from any of its rows", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    await findBranchRow(page, "oauth").focus()

    await page.keyboard.press("Alt+ArrowLeft")
    expect(await branchTree(page)).toEqual(["docs@0", "auth@0"])
    // the focus moves from the hidden row to the root
    await expect(findBranchRow(page, "auth")).toBeFocused()
    await page.keyboard.press("Alt+ArrowRight")
    await expect.poll(() => branchTree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
    // a lone branch has no stack to collapse; the menu offers it on a branch of a stack
    await findBranchRow(page, "docs").focus()
    await page.keyboard.press("Alt+ArrowLeft")
    await expect(page.getByTestId("branch-row")).toHaveCount(5)
    await openContextMenu(page, findBranchRow(page, "login"))
    await clickContextMenuItem(page, "Collapse Stack")
    expect(await branchTree(page)).toEqual(["docs@0", "auth@0"])
  })

  test("should block the children of a parent that doesn't build", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_conflict")
    await syncAndWaitForBranches()

    const loginRow = findBranchRow(page, "login")
    await expect(loginRow.getByTestId("branch-status")).toHaveText("blocked by auth")
    await expect(findBranchRow(page, "auth").getByTestId("branch-status")).toHaveText("merge conflict")
    await expect(getPushButton(loginRow)).toBeDisabled()
    await expect(getPushButton(loginRow)).toHaveAccessibleName("Push Branch")
    const authRow = findBranchRow(page, "auth")

    // the hover of the node and the description of the push name the parent: both show it
    await loginRow.getByTestId("lane-node-target").hover()
    const node = page.getByTestId("lane-node-details")
    await expect(node).toContainText("Blocked by auth, which doesn't build")
    await node.getByRole("button", { name: "Show auth" }).click()
    await expect(authRow).toHaveAttribute("data-state", "open")
    await collapseBranch(page, authRow)
    await expect(loginRow).toHaveAttribute("data-state", "closed")
    await getPushButton(loginRow).hover()
    const description = page.getByTestId("push-description")
    await expect(description).toContainText("Blocked by auth: fix it first, then sync")
    await description.getByRole("button", { name: "Show auth" }).click()
    await expect(authRow).toHaveAttribute("data-state", "open")
    await collapseBranch(page, authRow)

    await expandBranch(page, loginRow)
    const alert = getBranchDetailsRow(page, "login").getByTestId("branch-blocked")
    await expect(alert).toContainText("Blocked: parent auth has a merge conflict")
    await alert.getByRole("button", { name: "Show auth" }).click()
    await expect(findBranchRow(page, "auth")).toHaveAttribute("data-state", "open")
  })

  test("should stack a branch on another one and unstack it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    await openContextMenu(page, findBranchRow(page, "docs"))
    // the submenu lists the other branches
    await page.getByRole("menuitem", { name: "Stack On…" }).hover()
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "oauth"))
    // the toast comes once the sync showed the result
    await expect(page.getByText("Stacked docs on oauth")).toBeVisible()
    await expect.poll(() => branchTree(page)).toEqual(["auth@0", "session@1", "login@1", "oauth@2", "docs@3"])

    await openContextMenu(page, findBranchRow(page, "docs"))
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "Unstack from oauth"))
    await expect.poll(() => branchTree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
  })

  test("should stack a dragged branch dropped on the Stack on chip of a row, refuse a cycle there, and unstack it on the unstack zone", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    expect(await branchTree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])

    // the chip shows over the row under the pointer, and a drop on it stacks instead of moving the commits
    await startDrag(page, findBranchRow(page, "docs"))
    await expect(page.getByTestId("stack-on-drop-chip")).toHaveCount(0)
    // a root has no unstack
    await expect(page.getByTestId("new-branch-drop-zone")).toBeVisible()
    await expect(page.getByTestId("unstack-drop-zone")).toHaveCount(0)
    await findBranchRow(page, "oauth").hover()
    const chip = findBranchRow(page, "oauth").getByTestId("stack-on-drop-chip")
    await expect(chip).toHaveText("Stack on oauth")
    await runAndWaitForSync(page, () => dropOn(page, chip))
    await expect(page.getByText("Stacked docs on oauth")).toBeVisible()
    await expect.poll(() => branchTree(page)).toEqual(["auth@0", "session@1", "login@1", "oauth@2", "docs@3"])

    // a branch stacked on the dragged one is refused, with the reason in the chip and in the drag bar
    await startDrag(page, findBranchRow(page, "auth"))
    await findBranchRow(page, "oauth").hover()
    const refused = findBranchRow(page, "oauth").getByTestId("stack-on-drop-chip")
    await expect(refused).toHaveAttribute("data-drop-target", "refused")
    await expect(refused).toHaveText("Cannot stack auth on oauth: oauth is stacked on it")
    await refused.hover()
    await expect(page.getByTestId("drag-target-status")).toHaveText("Cannot stack auth on oauth: oauth is stacked on it")
    await dropOn(page, refused)
    expect(await branchTree(page)).toEqual(["auth@0", "session@1", "login@1", "oauth@2", "docs@3"])

    // the unstack zone of the drag bar makes it a root again
    await startDrag(page, findBranchRow(page, "docs"))
    const unstack = page.getByTestId("drag-target-bar").getByTestId("unstack-drop-zone")
    await expect(unstack).toHaveText("Unstack docs")
    await runAndWaitForSync(page, () => dropAt(page, unstack))
    await expect(page.getByText("Unstacked docs")).toBeVisible()
    await expect.poll(() => branchTree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
  })

  test("should stack an issue branch without a rewrite and undo it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_issues")
    await syncAndWaitForBranches()
    // docs is stacked on ABC-1 by the stack parents, its prefix names no parent
    expect(await branchTree(page)).toEqual(["ABC-2@0", "ABC-1@0", "docs@1"])

    await openContextMenu(page, findBranchRow(page, "ABC-2"))
    await page.getByRole("menuitem", { name: "Stack On…" }).hover()
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "ABC-1"))
    // no commit moved: the toast names no count
    await expect(page.getByText("Stacked ABC-2 on ABC-1", { exact: true })).toBeVisible()
    await expect.poll(() => branchTree(page)).toEqual(["ABC-1@0", "docs@1", "ABC-2@1"])
    await expandBranch(page, findBranchRow(page, "ABC-2"))
    // the commit keeps its message; the row of ABC-2 shows the key
    await expect(getBranchCommitRow(page, "ABC-2", "Parse lists")).toBeVisible()

    await runAndWaitForSync(page, () => page.getByRole("button", { name: "Undo" }).click())
    await expect.poll(() => branchTree(page)).toEqual(["ABC-2@0", "ABC-1@0", "docs@1"])
  })

  test("should not offer a branch's own descendants to stack on", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    await openContextMenu(page, findBranchRow(page, "auth"))
    await page.getByRole("menuitem", { name: "Stack On…" }).hover()
    await expect(page.getByRole("menuitem", { name: "docs" })).toBeVisible()
    for (const descendant of ["login", "oauth", "session"]) {
      await expect(page.getByRole("menuitem", { name: descendant, exact: true })).toHaveCount(0)
    }
  })

  test("should rename a parent with its children following", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    await openContextMenu(page, findBranchRow(page, "auth"))
    await clickContextMenuItem(page, "Rename…")
    const input = page.getByTestId("inline-rename-input").locator('input[type="text"]')
    await input.fill("auth/core")
    // the message shows once the form was submitted
    await input.press("Enter")
    await expect(page.getByText("A branch name has no /")).toBeVisible()
    await input.fill("identity")
    await expect(page.getByText("session, login, oauth stay stacked on it")).toBeVisible()
    await runAndWaitForSync(page, () => input.press("Enter"))

    await expect.poll(() => branchTree(page)).toEqual(["docs@0", "identity@0", "session@1", "login@1", "oauth@2"])
  })

  test("should build a child on the baseline once its parent has no commits, and unstack it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    await openContextMenu(page, findBranchRow(page, "login"))
    await clickContextMenuItem(page, "Ungroup")
    const note = page.getByTestId("stack-children-note")
    await expect(note).toContainText("1 branch is stacked on login")
    await expect(note).toContainText("oauth stays stacked on login and is built on master until login has commits again")
    // a confirmation focuses its button: Enter confirms
    await expect(page.getByRole("button", { name: "Ungroup" })).toBeFocused()
    await runAndWaitForSync(page, () => page.keyboard.press("Enter"))

    // not blocked: built on the baseline, and the row says so
    const oauth = findBranchRow(page, "oauth")
    await expect(oauth.getByTestId("stack-base")).toHaveText("on master")
    await expect(oauth).not.toContainText("blocked by")
    await expect(getPushButton(oauth)).toBeEnabled()
    await expandBranch(page, oauth)
    const alert = getBranchDetailsRow(page, "oauth").getByTestId("branch-base-changed")
    await expect(alert).toContainText("Built on master: login has no commits.")
    // every pull request goes into master: the alert names no base
    await expect(alert).not.toContainText("pull request")

    // the user rewrites the prefixes, never the sync
    await runAndWaitForSync(page, () => alert.getByRole("button", { name: "Unstack from login" }).click())
    await expect.poll(() => branchTree(page)).toContain("oauth@0")
    await expect(findBranchRow(page, "oauth").getByTestId("stack-base")).toHaveCount(0)
    await expect(getBranchDetailsRow(page, "oauth").getByTestId("branch-base-changed")).toHaveCount(0)
  })

  test("should unstack once from the alert, its spinner on the button until the sync shows the branch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    await openContextMenu(page, findBranchRow(page, "login"))
    await clickContextMenuItem(page, "Ungroup")
    await expect(page.getByRole("button", { name: "Ungroup" })).toBeFocused()
    await runAndWaitForSync(page, () => page.keyboard.press("Enter"))
    await expandBranch(page, findBranchRow(page, "oauth"))
    const alert = getBranchDetailsRow(page, "oauth").getByTestId("branch-base-changed")
    const unstack = alert.getByTestId("unstack-from-parent")
    await expect(unstack).toHaveText("Unstack from login")

    // the unstack reaches the session and waits there for the repository lock another process holds
    const unstacks = await recordRewrites(page, "unstack")
    await repo.holdRepositoryLock(true)
    // a double click, as the triple click that once ran three unstacks
    await unstack.dblclick()
    await expect(unstack).toHaveAttribute("aria-busy", "true")
    await expect(unstack).toBeDisabled()
    await expect(unstack).toHaveText(WAITING_FOR_HOLDER)
    // the button that started it shows the spinner; the row's "⋯" is only disabled
    const more = findBranchRow(page, "oauth").getByRole("button", { name: "More actions for oauth" })
    await expect(more).toBeDisabled()
    await expect(more).not.toHaveAttribute("aria-busy")
    await expectRewritesWaitAndUnstackOnce(page, repo, unstacks)
    await expect(page.getByText("Cannot unstack oauth")).toHaveCount(0)
    await expect.poll(() => branchTree(page)).toContain("oauth@0")
    await expect(getBranchDetailsRow(page, "oauth").getByTestId("branch-base-changed")).toHaveCount(0)
  })

  test("should show an unstack started from the menu on the row's ⋯ and let the other rewrites queue behind it, from a key too", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    // the unstack reaches the session and waits there for the repository lock another process holds
    const unstacks = await recordRewrites(page, "unstack")
    await repo.holdRepositoryLock(true)
    // its "⋯" shows the wait, and its menu is the Stop Waiting of it
    const more = await unstackOauthFromMenuWaiting(page, WAITING_FOR_HOLDER)
    // a key runs the item it is the hint of: M on another row opens its form, whose move would wait for the unstack
    await findBranchRow(page, "docs").focus()
    await page.keyboard.press("m")
    await expect(page.getByTestId("inline-branch-target")).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(page.getByTestId("inline-branch-target")).toHaveCount(0)
    await expectRewritesWaitAndUnstackOnce(page, repo, unstacks)
    await expect(more).toBeEnabled()
    await expect(more).not.toHaveAttribute("aria-busy")
    // the rewrites run again
    await openContextMenu(page, findBranchRow(page, "docs"))
    await expect(page.getByRole("menuitem", { name: /^Move to…/ })).not.toHaveAttribute("aria-disabled", "true")
    await page.keyboard.press("Escape")
  })

  test("should unstack from the menu during a held sync once that sync ended, without a refusal, one sync for the rewrite", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    const unstacks = await recordRewrites(page, "unstack")
    // the sync of the session runs until the test releases it (a focus, another window)
    await repo.holdRemoteAnswers({ hold: ["sync"] })
    const syncs = await finishedSyncCount(page)
    const syncButton = page.getByTestId("sync-button")
    await syncButton.click()
    await expect(syncButton).toHaveAttribute("aria-busy", "true")

    // the unstack is offered during the sync and goes to the session at once, named on the table the row shows
    await openContextMenu(page, findBranchRow(page, "oauth"))
    await clickContextMenuItem(page, "Unstack from login")
    await expect.poll(() => unstacks.length).toBe(1)
    // its answer waits for the sync it asked, after the held one: no toast yet, nothing refused
    await expect(page.getByText(/^Unstacked oauth/)).toHaveCount(0)

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(page.getByText(/^Unstacked oauth/)).toBeVisible()
    await expect.poll(() => branchTree(page)).toContain("oauth@0")
    await expect(page.locator("[data-slot=toast]", { hasText: /^Cannot / })).toHaveCount(0)
    // the held sync and the one the rewrite asked
    expect(await finishedSyncCount(page)).toBe(syncs + 2)
    expect(unstacks).toHaveLength(1)
  })

  test("should move commits to a new branch on a parent with Move to…", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    await expandBranch(page, findBranchRow(page, "docs"))
    await selectCommit(getBranchCommitRow(page, "docs", "Document the login"))
    await clickGroupIntoBranchButton(page)
    await inlineBranchCreator.waitForVisible(page)
    const portal = inlineBranchCreator.getPortal(page)
    await inlineBranchCreator.getInput(page).fill("auth/guide")
    await expect(portal).toContainText("Creates guide on auth with 1 commit")
    await inlineBranchCreator.getInput(page).fill("login")
    await expect(portal).toContainText("Moves 1 commit to login (on auth)")
    await inlineBranchCreator.getInput(page).fill("auth/guide")
    await runAndWaitForSync(page, () => inlineBranchCreator.getInput(page).press("Enter"))

    await expect.poll(() => branchTree(page)).toEqual(["auth@0", "guide@1", "session@1", "login@1", "oauth@2"])
    await expect(findBranchRow(page, "guide")).toHaveAttribute("data-state", "open")
  })

  test("should push a stack parents first and create each pull request into master", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // on GitHub, no pull request yet; the next one created is #900
    const repo = await setupRepo("stacked_branches", { pullRequests: { ...availablePullRequests(), nextNumber: 900 } })
    await syncAndWaitForBranches()

    // login isn't pushed yet: the push of oauth pushes the stack, the menu offers oauth alone too. The remote lacks
    // oauth and its Create Pull Request pushes it: the row offers no push of its own
    const oauth = findBranchRow(page, "oauth")
    await expect(oauth.getByTestId("branch-status-dot")).toHaveAttribute("data-color", "primary")
    await expectNothingToPush(oauth)
    // the menu has the one push of the stack, no second Push Stack, and the push of oauth alone after it
    await openContextMenu(page, oauth)
    await expect(page.getByRole("menuitem", { name: /^Push Stack/ })).toHaveCount(1)
    await expect(page.getByRole("menuitem", { name: "Push oauth Only" })).toBeVisible()
    await page.getByRole("menuitem", { name: "Push Stack (3 branches) P" }).click()
    // one push of the stack
    await expect.poll(() => remotePushes(repo)).toEqual([["auth", "login", "oauth"]])
    await expectPushedToRemote(repo, ["auth", "login", "oauth"])
    // the stack is on the remote: up to date needs no action, so the rows that said "not pushed" say nothing
    for (const name of ["auth", "login", "oauth"]) {
      await expect(findBranchRow(page, name).getByTestId("branch-status")).toHaveCount(0)
    }
    // one toast for the stack, as Activity has an entry per branch; it creates the pull request of oauth
    const toast = page.locator("[data-slot=toast]", { hasText: "Pushed branches auth, login and oauth" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("3 commits · created 3 remote branches")
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
    const texts = await recordCommand(page, "get_new_pull_request")
    await toast.getByRole("button", { name: "Create Pull Request…" }).click()
    // into master, with the commits of the branches oauth is built on: what its Safe Push lands
    const dialog = createPullRequestDialog(page)
    await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("oauth → master")
    await expect(dialog.getByTestId("create-pull-request-includes")).toHaveText("Includes the commits of auth and login, which oauth is built on: 3 branches · 3 commits")
    // its text leaves the commits of the branches it is built on out of its own, and names them
    expect(texts).toEqual([expect.objectContaining({ head: "user-name/virtual/oauth", base: "master", stackAncestors: ["user-name/virtual/auth", "user-name/virtual/login"] })])
    // GitHub's form, as "Continue on GitHub" opens it, takes the same base; the text of the commits fills the form
    // first (under load it comes after the dialog)
    await expect(dialog.getByTestId("create-pull-request-title")).toHaveValue("Add OAuth provider")
    await dialog.getByTestId("create-pull-request-on-github").click()
    const body = "Built+on+auth%2C+then+login%3A+their+commits+are+in+this+pull+request+too.%0A%0A**auth**%0A-+Add+auth+module%0A%0A**login**%0A-+Add+login+form"
    await expect.poll(() => getOpenedUrls(page)).toEqual([`${GITHUB_COMPARE}/master...user-name/virtual/oauth?expand=1&title=Add+OAuth+provider&body=${body}&assignees=user-name`])
    await expect(dialog).toHaveCount(0)

    // the root's pull request goes into master too, and holds only its own commits
    await openContextMenu(page, findBranchRow(page, "auth"))
    await clickContextMenuItem(page, "Create Pull Request…")
    await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("auth → master")
    await expect(dialog.getByTestId("create-pull-request-includes")).toHaveCount(0)
    await dialog.getByTestId("create-pull-request-submit").click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Created pull request #900" }).locator("[data-slot=toast-description]")).toHaveText("auth → master · assigned to you")
    expect(gitHubWrites(await repo.getGitHubLog())).toEqual([
      ["POST", repoPath("pulls"), expect.objectContaining({ head: "user-name/virtual/auth", base: "master" })],
      ["POST", repoPath("issues", 900, "assignees"), { assignees: ["user-name"] }],
    ])
    expect(texts[1]).toEqual(expect.objectContaining({ head: "user-name/virtual/auth", stackAncestors: [] }))
  })

  test("should push a stack in one push, refused whole when a remote branch of it moved", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    // someone else pushed login since its row was shown, with a commit of theirs: the remote refuses the one push, nothing
    // of it goes out, and every branch is fetched: auth and oauth are still not on the remote
    await repo.pushInTerminal(virtualBranch("login"), { revision: virtualBranch("auth") })
    await repo.commitOnRemote(virtualBranch("login"), { subject: "Their fix", author: "Ann <ann@example.com>", timestamp: 1704139200, files: { "their.txt": "fix\n" } })
    const oauth = findBranchRow(page, "oauth")
    await expect(getPushButton(oauth)).toHaveAccessibleName("Push Stack (3 branches)")
    await getPushButton(oauth).click()

    const toast = page.locator("[data-slot=toast]", { hasText: "login changed on the remote" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("1 commit you don't have (by Ann). Nothing was pushed.")
    expect(await remotePushes(repo)).toEqual([])
    // no toast of a push, and the branches below and above login are still not on the remote
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
    for (const name of ["auth", "oauth"]) {
      await expect(findBranchRow(page, name).getByTestId("branch-status-dot")).toHaveAttribute("data-color", "primary")
    }
    expect(await pushedBranches(repo)).toEqual(["login"])

    // the toast pushes login alone over what its remote has now, after the row shows the commit the push drops
    await toast.getByRole("button", { name: "Force Push Branch" }).click()
    await confirmForcePush(page, "Force Push Branch (1)")
    await expectPushedToRemote(repo, ["login"])
    expect(await remotePushes(repo)).toEqual([["login"]])
    expect(await pushedBranches(repo)).toEqual(["login"])
  })

  test("should push a stacked branch alone from the description of its button", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    await getPushButton(findBranchRow(page, "oauth")).hover()
    await page.getByTestId("push-description").getByRole("button", { name: "Push oauth Only" }).click()
    await expect.poll(() => remotePushes(repo)).toEqual([["oauth"]])
    await expect(pushedToast(page, "oauth")).toBeVisible()
    // oauth is on the remote: its menu pushes nothing more, as its button; login's pushes the parents
    await openContextMenu(page, findBranchRow(page, "oauth"))
    await expect(page.getByRole("menuitem", { name: /^Push Stack/ })).toHaveCount(0)
    await expect(page.getByRole("menuitem", { name: "Push Branch P" })).toHaveAttribute("aria-disabled", "true")
    await page.keyboard.press("Escape")
    await openContextMenu(page, findBranchRow(page, "login"))
    await expect(page.getByRole("menuitem", { name: "Push Stack (2 branches) P" })).toBeVisible()
  })

  test("should push a stacked branch alone from its menu, the second way after the push of the stack", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    await openContextMenu(page, findBranchRow(page, "oauth"))
    await expect(page.getByRole("menuitem", { name: "Push Stack (3 branches) P" })).toBeVisible()
    await page.getByRole("menuitem", { name: "Push oauth Only" }).click()
    await expect.poll(() => remotePushes(repo)).toEqual([["oauth"]])
    await expect(pushedToast(page, "oauth")).toBeVisible()
  })

  test("should push a parent with its children", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    // auth alone may go; its description offers its children too
    const button = getPushButton(findBranchRow(page, "auth"))
    await expect(button).toHaveAccessibleName("Push Branch (1)")
    await button.hover()
    const alternative = page.getByTestId("push-description").getByTestId("push-alternative")
    await expect(alternative).toHaveCount(1)
    await expect(alternative).toContainText("session and 2 more branches are stacked on auth and have commits to push.")

    await alternative.getByRole("button", { name: "Push with Children (4 branches)" }).click()
    // one push of the four; the stack branch of the whole stack, on no remote and in no pull request, stays local
    await expect.poll(() => remotePushes(repo)).toEqual([["auth", "login", "oauth", "session"]])
    expect(await pushedBranches(repo)).toEqual(["auth", "login", "oauth", "session"])
    // every branch is on the remote: the menu keeps it beside the push, disabled with the reason, which says that the
    // stack branch isn't
    await openContextMenu(page, findBranchRow(page, "auth"))
    const item = page.getByRole("menuitem", { name: "Push with Children", exact: true })
    await expect(item).toHaveAttribute("aria-disabled", "true")
    await item.hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "on the remote" })).toHaveText(
      "The branches of the stack are on the remote. Its stack branch goes there with the pull request or the Safe Push of the whole stack",
    )
    // the root row has nothing left to push: its push leaves
    await page.keyboard.press("Escape")
    await expectNothingToPush(findBranchRow(page, "auth"))
  })

  test("should ask in the row of the branch before Push with Children drops its remote commits, and leave out a child whose drop nobody showed", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    // Ann pushed a fix onto the remote branch of auth and Bob one onto login's, both on master; a fetch brought them
    const theirs = await fixesOfOthersOnRemote(repo, { auth: "Ann <ann@example.com>", login: "Bob <bob@example.com>" })
    await syncFromMenu(page)
    const auth = findBranchRow(page, "auth")

    // Push with Children asks first in the push button of auth, which lists Ann's commit; nothing went yet
    await openContextMenu(page, auth)
    await page.getByRole("menuitem", { name: /^Push with Children/ }).click()
    await expect(getPushButton(auth)).toHaveAttribute("aria-expanded", "true")
    const description = page.getByTestId("push-description").filter({ has: page.getByTestId("push-confirm") })
    await expect(description.getByTestId("dropped-commits")).toContainText("Fix auth")
    await expect(description.getByTestId("dropped-commits")).not.toContainText("Fix login")
    expect(await remotePushes(repo)).toEqual([])

    // its Force Push runs Push with Children on that drop: one push of auth over Ann's commit and of the children that
    // drop nothing; login, whose drop nobody showed, keeps Bob's commit and asks in its own row
    await confirmForcePush(page, "Force Push Branch (1)")
    await expect.poll(() => remotePushes(repo)).toEqual([["auth", "oauth", "session"]])
    await expectPushedToRemote(repo, ["auth", "oauth", "session"])
    expect((await repo.getRemoteBranches())[virtualBranch("login")]).toBe(theirs.login)
    await expect(getPushButton(findBranchRow(page, "login"))).toHaveAccessibleName("Force Push Branch (1)")
  })

  test("should ask in the row of a stacked branch whose push drops remote commits by the time its button's push of the stack is planned, and push the stack over the remote head the row shows", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    // login is on the remote at master, auth is not: the push button of login pushes the stack, auth first
    await repo.pushInTerminal(virtualBranch("login"), { revision: "origin/master" })
    await syncFromMenu(page)
    const login = findBranchRow(page, "login")
    const button = getPushButton(login)
    await expect(button).toHaveAccessibleName(/^Push Stack/)

    // the click's plan waits while Bob pushes a fix onto the remote branch of login, which a fetch and a sync bring
    const releasePlan = await holdCommand(page, "prepare")
    await button.click()
    const bob = await repo.commitOnRemote(virtualBranch("login"), {
      subject: "Fix login",
      author: "Bob <bob@example.com>",
      timestamp: 1704139200,
      files: { "login-fix.txt": "fix\n" },
    })
    await repo.fetchInTerminal()
    await syncFromMenu(page)
    await expect(button).toHaveAccessibleName(/^Force Push/)
    releasePlan()

    // the plan drops Bob's commit: the push button of login asks first, listing it; nothing went
    await expect(button).toHaveAttribute("aria-expanded", "true")
    const description = page.getByTestId("push-description").filter({ has: page.getByTestId("push-confirm") })
    await expect(description.getByTestId("dropped-commits")).toContainText("Fix login")
    expect(await remotePushes(repo)).toEqual([])
    expect((await repo.getRemoteBranches())[virtualBranch("login")]).toBe(bob)

    // its Force Push runs the push of the stack again on that drop, over the remote head the row shows now (the one
    // the lease of the click predates): one push of auth and login
    await confirmForcePush(page, "Force Push Branch (1)")
    await expect.poll(() => remotePushes(repo)).toEqual([["auth", "login"]])
    await expectPushedToRemote(repo, ["auth", "login"])
  })

  test("should offer Push with Children while a sync runs and when the root is pushed", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    const auth = findBranchRow(page, "auth")
    const button = getPushButton(auth)
    await button.click()
    await expectPushedToast(page, "auth", "1 commit · created the remote branch")
    expect(await remotePushes(repo)).toEqual([["auth"]])
    // auth is on the remote, its children are not: its push is Push with Children, the commits they send and the words
    // that say whose they are; its hover lists the pushes and the graph circles the branches it sends
    await expect(button).toHaveAccessibleName("Push with Children (3 branches)")
    await expect(button).toBeEnabled()
    await expect(button).toHaveText(/3\s*with children/, { useInnerText: true })
    const pushed = () => page.locator("[data-testid=lane-node][data-pushed]").evaluateAll(nodes => nodes.map(node => node.closest("tr")!.getAttribute("data-branch-name")))
    await button.hover()
    const description = page.getByTestId("push-description")
    await expect(description).toContainText("Pushes 3 branches, parents first:")
    await expect.poll(pushed).toEqual(["session", "login", "oauth"])
    await page.keyboard.press("Escape")
    await expect(description).toBeHidden()

    // a sync runs: the menu keeps the item enabled, and the push waits for the sync
    const releaseSync = await holdCommand(page, "sync")
    await page.getByTestId("sync-button").click()
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    await openContextMenu(page, auth)
    const item = page.getByRole("menuitem", { name: "Push with Children (3 branches)" })
    await expect(item).toBeEnabled()
    await item.click()
    await expect(button).toHaveAttribute("aria-busy", "true")
    expect(await remotePushes(repo)).toEqual([["auth"]])

    releaseSync()
    const pushedChildren = page.locator("[data-slot=toast]", { hasText: "Pushed branches session, login and oauth" })
    await expect(pushedChildren).toBeVisible()
    expect(await remotePushes(repo)).toEqual([["auth"], ["login", "oauth", "session"]])
    await closeToast(pushedChildren)

    // the children of auth go back to the baseline on the remote (a push in a terminal), which a sync shows: the push of
    // the row of auth sends them again, its track as wide while it pushes (held at the remote) as before the click
    for (const child of ["session", "login", "oauth"]) {
      await repo.pushInTerminal(virtualBranch(child), { revision: "origin/master" })
    }
    await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
    await expect(button).toHaveAccessibleName("Push with Children (3 branches)")
    const track = auth.locator("[data-track=push]")
    const width = (await boxOf(track)).width
    const held = await holdRemotePushes(repo)
    await button.click()
    await held.arrived()
    await expect(button).toHaveAttribute("aria-busy", "true")
    await expect(button).toHaveText(/with children/, { useInnerText: true })
    expect((await boxOf(track)).width).toBe(width)
    await held.release()
    await expect(page.locator("[data-slot=toast]", { hasText: "Pushed branches session, login and oauth" })).toBeVisible()
    expect(await remotePushes(repo)).toEqual([["auth"], ["login", "oauth", "session"], ["login", "oauth", "session"]])
  })

  test("should take the whole stack branch along in Push with Children of the root once its pull request is open, from the push and the menu", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the pull request of the whole auth stack is open: its stack branch follows the branches
    const stackPullRequest = pullRequest("auth", 12, { title: "The whole auth stack", headRefName: "user-name/stack/auth" })
    const repo = await setupRepo("stacked_branches", { pullRequests: { ...availablePullRequests(), stackPullRequests: { auth: stackPullRequest } } })
    await syncAndWaitForBranches()
    const auth = findBranchRow(page, "auth")

    // the description of the root's push: its children and the stack branch, one more branch of the push
    await getPushButton(auth).hover()
    const alternative = page.getByTestId("push-description").getByTestId("push-alternative")
    await expect(alternative).toContainText("session and 2 more branches are stacked on auth and have commits to push. The whole stack branch user-name/stack/auth goes along.")
    await alternative.getByRole("button", { name: "Push with Children (5 branches)" }).click()
    // one atomic push, the stack branch with the branches
    await expect.poll(() => remotePushes(repo)).toEqual([["auth", "login", "oauth", "session", "stack:auth"]])
    await expectPushedToRemote(repo, ["auth", "login", "oauth", "session", stackBranchOf("auth")])
    // one toast: the four branches named, the stack branch said
    const pushed = page.locator("[data-slot=toast]").filter({ hasText: "pushed the whole stack branch" })
    await expect(pushed.locator("[data-slot=toast-title]")).toHaveText("Pushed branches auth, session, login and oauth")
    await expect(pushed.locator("[data-slot=toast-description]")).toHaveText("4 commits · created 4 remote branches · pushed the whole stack branch")

    // everything is on the remote now, the stack branch too: nothing to push
    await openContextMenu(page, auth)
    const disabled = page.getByRole("menuitem", { name: "Push with Children", exact: true })
    await expect(disabled).toHaveAttribute("aria-disabled", "true")
    await disabled.hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "Nothing to push" })).toHaveText("Nothing to push: the stack is on the remote")
    await page.keyboard.press("Escape")

    // the remote of the stack branch lacks its newest commit, the branches stay as they are on the remote
    await repo.pushInTerminal(branchOf(stackBranchOf("auth")), { revision: `${branchOf(stackBranchOf("auth"))}~1` })
    await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
    // the menu: the stack branch alone, named and leased on its remote head
    await openContextMenu(page, auth)
    const item = page.getByRole("menuitem", { name: "Push with Children (the whole stack branch)" })
    await expect(item).toBeEnabled()
    await item.hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "Also pushes" })).toHaveText("Also pushes the whole stack branch user-name/stack/auth, the head of #12")
    await item.click()
    await expect.poll(async () => (await remotePushes(repo)).at(-1)).toEqual(["stack:auth"])
    await expectPushedToRemote(repo, [stackBranchOf("auth")])
    // its toast opens the pull request of the whole stack
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Pushed the whole stack branch of auth" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("1 commit")
    await toast.getByRole("button", { name: "Open Pull Request" }).click()
    await expect.poll(() => getOpenedUrls(page)).toEqual([stackPullRequest.url])
  })

  test("should retarget a pull request into a parent's branch to master before the push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // auth's pull request goes into the branch it was stacked on before, login's into its parent's (approved), session's
    // into a release branch
    const pullRequests = availablePullRequests(
      pullRequest("auth", 802, { baseRefName: "user-name/virtual/core" }),
      pullRequest("login", 801, { baseRefName: "user-name/virtual/auth", reviewers: [reviewer("alice", "approved")] }),
      pullRequest("session", 803, { baseRefName: "release-252" }),
    )
    const repo = await setupRepo("stacked_branches", { pullRequests })
    await syncAndWaitForBranches()
    const retargets = [
      ["PATCH", repoPath("pulls", 802), { base: "master" }],
      ["PATCH", repoPath("pulls", 801), { base: "master" }],
    ]
    // GitHub listed them: the menu is built of what the rows show
    await expect(findBranchRow(page, "auth").getByTestId("pull-request-chip")).toHaveText("#802")
    await expect(page.getByTestId("pull-request-chip")).toHaveCount(3)

    await openContextMenu(page, findBranchRow(page, "auth"))
    await openSubmenu(page, "Copy")
    await clickContextMenuItem(page, "Copy Retarget Command")
    await expect.poll(() => readClipboard(page)).toBe("gh pr edit 802 --base master")

    // the description says it first, and that GitHub may dismiss the approvals of login's
    const button = getPushButton(findBranchRow(page, "login"))
    await expect(button).toHaveAccessibleName("Push Stack (2 branches)")
    await button.hover()
    await expect(page.getByTestId("push-description").getByTestId("push-retarget")).toHaveText(
      "Retargets #802 of auth to master. Retargets #801 of login to master: GitHub may dismiss its approvals.",
    )
    // the push retargets them first: every pull request of a virtual branch goes into master, with what it lands
    await button.click()
    const toast = page.locator("[data-slot=toast]", { hasText: "Pushed branches auth and login" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText(
      "2 commits · created 2 remote branches · retargeted pull request #802 of auth to master · retargeted pull request #801 of login to master",
    )
    expect(gitHubWrites(await repo.getGitHubLog())).toEqual(retargets)
    expect(await remotePushes(repo)).toEqual([["auth", "login"]])
    // Activity names each retarget, with its pull request
    const activity = await openActivityWindow(page)
    const retargeted = activity.getByTestId("activity-entry").filter({ hasText: "Retargeted #801 to master" })
    await expect(retargeted.getByTestId("activity-description")).toHaveText("login · it went into auth")

    // a pull request into a release branch is left as it is: that base may be meant
    await openContextMenu(page, findBranchRow(page, "session"))
    await openSubmenu(page, "Copy")
    await expect(page.getByRole("menuitem", { name: "Copy Branch Name" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Copy Retarget Command" })).toHaveCount(0)
    await closeMenus(page)
    await getPushButton(findBranchRow(page, "session")).click()
    await expect(pushedToast(page, "session").locator("[data-slot=toast-description]")).toHaveText("1 commit · created the remote branch")
    expect(gitHubWrites(await repo.getGitHubLog())).toEqual(retargets)
  })

  test.describe("Safe Push", () => {
    /** What the one robot of the whole stack of auth lands (`describeWholeStackLanding`) */
    const WHOLE_AUTH_STACK = "Lands 4 branches · 4 commits into master in one Safe Push: auth, login, oauth and session"

    /**
     * stacked_branches on Space, synced, the default checks, no code-owner review needed: the repository, whose fake
     * Patronus logs the starts and whose remote takes the pushes. `refuseStarts`: Patronus refuses every start, so no robot
     * holds the start of the next surface
     */
    async function setupStack(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
      syncAndWaitForBranches: () => Promise<unknown>,
      refuseStarts = false,
    ): Promise<{ repo: TestRepositoryBuilder }> {
      const repo = await setupRepo("stacked_branches", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      // the robots load after the sync, with the Safe Push slots: S before them finds no Patronus and does nothing
      await expect(page.getByTestId("next-step-slot").first()).toBeAttached()
      if (refuseStarts) {
        await repo.refusePatronusStarts("Patronus is restarting")
      }
      return { repo }
    }

    /**
     * Enter starts the popover: its start is enabled and the focus is in it. The start itself should take the focus once
     * it can start; the popup keeps it (see the fixme test below), and its own Enter starts too
     */
    async function expectEnterStarts(page: Page): Promise<void> {
      await expect(popoverStart(page)).toBeEnabled()
      await expect.poll(() => page.evaluate(() => document.activeElement?.closest("[data-testid=safe-push-popover]") != null)).toBe(true)
    }

    /** The one popover of the Safe Push of `branchName`, at the slot of its row (above or below it): Enter starts it */
    async function expectPopoverOf(page: Page, branchName: string, mode: "Safe Push" | "Dry Run" = "Safe Push"): Promise<Locator> {
      const popover = safePushPopover(page)
      await expect(popover).toHaveAttribute("aria-label", `${mode} of ${branchName}`)
      await expect(popover).toHaveCount(1)
      await expectEnterStarts(page)
      await expectPopoverAtSlot(popover, findBranchRow(page, branchName).getByTestId("next-step-slot"))
      return popover
    }

    /** The one popover of the Safe Push of the whole auth stack, at the root's row: what it lands, Enter starts it */
    async function expectWholeAuthStackPopover(page: Page): Promise<void> {
      const popover = safePushPopover(page)
      await expect(popover).toHaveAttribute("aria-label", "Safe Push of the whole auth stack")
      await expect(popover).toHaveCount(1)
      await expect(popover.getByTestId("safe-push-lands")).toHaveText(`${WHOLE_AUTH_STACK}.`)
      await expectEnterStarts(page)
      await expectPopoverAtSlot(popover, findBranchRow(page, "auth").getByTestId("next-step-slot"))
    }

    /**
     * Hovers `start` (an item or a button of the Safe Push of the whole auth stack), which says what it lands, and clicks
     * it: its popover at the root's row, and Enter starts the robot from the stack branch (one more start the fake
     * Patronus of `repo` logs)
     */
    async function startWholeAuthStack(page: Page, start: Locator, repo: TestRepositoryBuilder): Promise<void> {
      await start.hover()
      await expect(page.getByRole("tooltip")).toHaveText(WHOLE_AUTH_STACK)
      await start.click()
      await expectWholeAuthStackPopover(page)
      const before = (await repo.getPatronusLog()).starts
      await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Enter"))
      await expectPatronusStarts(repo, [...before, wholeStackStartRequest("auth")])
    }

    test("should give the focus to the start of the popover once it can start", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // fixme: the popup keeps the focus it took before the start could start (Enter on the popup still starts)
      await setupStack(page, setupRepo, syncAndWaitForBranches)
      await routeCommandError(page, "get_stack_independence", { code: "internal", message: "The trial build failed" })
      await slotSafePush(page, "oauth").click()
      await expect(safePushPopover(page)).toHaveAttribute("data-decision", "confirm")
      await expect(popoverStart(page)).toBeFocused()
      await page.keyboard.press("Escape")
      await openSafePushWithKey(page, "oauth")
      await expect(popoverStart(page)).toBeFocused()
    })

    test("should land a small stack with its side branch from the root's menu, its popover at the root", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // auth forks: login and oauth on one side, session on the other; one robot of its stack branch lands all four
      const { repo } = await setupStack(page, setupRepo, syncAndWaitForBranches, true)
      const held = await holdRemotePushes(repo)
      // the facts the start decides on, and the start itself, wait: what the slot shows meanwhile
      await repo.holdRemoteAnswers({ hold: ["profiles", "start"] })
      const popover = safePushPopover(page)
      const rootSlot = findBranchRow(page, "auth").getByTestId("next-step-slot")
      const openWholeStack = async () => {
        await findBranchRow(page, "auth").click({ button: "right" })
        await openSubmenu(page, "Whole Stack")
      }

      // the steps of the start take the place of the root's own step: "⋯" of the row stays where it is, measured once the
      // facts that place it answered (under load they come after the sync, and the step they answer moves it)
      await waitForStepFacts(rootSlot)
      const more = getMoreButton(findBranchRow(page, "auth"))
      // the measured tracks of the columns follow the labels of the slots ("Safe Push · 3") a frame after them: the place
      // stands once two reads agree
      let moreX = Number.NaN
      await expect
        .poll(async () => {
          const x = (await more.boundingBox())!.x
          const stands = x === moreX
          moreX = x
          return stands
        })
        .toBe(true)
      const expectMoreInPlace = async () => expect((await more.boundingBox())!.x).toBe(moreX)

      // the root's menu, "Whole Stack ›": its Safe Push says what it lands; its steps show where it was asked, in the
      // root's slot, as in the header of an island: the facts first, then its popover at the root's row
      await openWholeStack()
      const item = page.getByRole("menuitem", { name: "Safe Push Whole Stack" })
      await item.hover()
      await expect(page.getByRole("tooltip")).toHaveText(WHOLE_AUTH_STACK)
      await item.click()
      const start = rootSlot.getByTestId("safe-push-button")
      await expect(start).toHaveText("Checking…")
      await expect(start).toHaveAttribute("aria-busy", "true")
      await expectMoreInPlace()
      await repo.holdRemoteAnswers({ release: ["profiles"] })
      await expectWholeAuthStackPopover(page)
      // Enter: only the stack branch is pushed, its progress in the button of the start, the one indicator: the rows of
      // the root and of the branches show no push (the remote lacks them, their first push is a step of their landing);
      // then the robot starts from it
      await page.keyboard.press("Enter")
      await held.arrived()
      await expect(start).toHaveText(/^\d+%$/)
      await expect(start).toHaveAccessibleName(/^Pushing \d+%$/)
      await expect(start).toHaveAttribute("aria-busy", "true")
      await expectMoreInPlace()
      await expect(findBranchRow(page, "auth").getByTestId("push-button")).toHaveCount(0)
      // no branch of the stack is pushed
      await expect(findBranchRow(page, "login").getByTestId("push-button")).toHaveCount(0)
      await held.release()
      await expect(start).toHaveText("Starting…")
      expect((await repo.getPatronusLog()).starts).toEqual([])
      await runAndWaitForAnswer(page, "execute", () => repo.holdRemoteAnswers({ release: ["start"] }))
      await expectPatronusStarts(repo, [wholeStackStartRequest("auth")])
      expect(await remotePushes(repo)).toEqual([["stack:auth"]])
      // the root's own step again
      await expect(start).toHaveText("Safe Push")
      await expect(start).not.toHaveAttribute("aria-busy", "true")
      await expect(popover).toHaveCount(0)
      await closeRefusal(page)

      // Dry Run Whole Stack: the same robot checks it, the popover at the root's row too
      await openWholeStack()
      await page.getByRole("menuitem", { name: "Dry Run Whole Stack" }).click()
      await expect(popover).toHaveAttribute("aria-label", "Dry Run of the whole auth stack")
      await expect(popover.getByTestId("safe-push-lands")).toHaveText(
        `${WHOLE_AUTH_STACK.replace("Lands", "Checks").replace("into master in one Safe Push", "on master in one Dry Run")}.`,
      )
      await expectPopoverAtSlot(popover, rootSlot)
      await runAndWaitForAnswer(page, "execute", () => popoverStart(page).click())
      await expectPatronusStarts(repo, [wholeStackStartRequest("auth"), wholeStackStartRequest("auth", { mode: "DRY_RUN" })])
    })

    test("should offer the whole stack in the Safe Push popover of its root and of its branches, which opens its own popover", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      const { repo } = await setupStack(page, setupRepo, syncAndWaitForBranches, true)
      const popover = safePushPopover(page)
      const rootSlot = findBranchRow(page, "auth").getByTestId("next-step-slot")

      // the root's popover lands auth alone, the branches built on it stay; the whole stack is its second way
      await openSafePushWithKey(page, "auth")
      await expect(popover).toHaveAttribute("aria-label", "Safe Push of auth")
      const option = popover.getByTestId("safe-push-whole-stack-option")
      await expect(option).toContainText("Land the whole stack:")
      // its own popover, at the root's row: what the whole stack lands, Enter starts it from its stack branch
      await startWholeAuthStack(page, option.getByRole("button", { name: "Safe Push Whole Stack" }), repo)
      await closeRefusal(page)

      // a side branch of the stack that forks offers it too
      await slotSafePush(page, "session").click()
      await expect(popover).toHaveAttribute("aria-label", "Safe Push of session")
      await popover.getByTestId("safe-push-whole-stack-option").getByRole("button", { name: "Safe Push Whole Stack" }).click()
      await expect(popover).toHaveAttribute("aria-label", "Safe Push of the whole auth stack")
      await expectPopoverAtSlot(popover, rootSlot)
      await runAndWaitForAnswer(page, "execute", () => popoverStart(page).click())
      await expectPatronusStarts(repo, [wholeStackStartRequest("auth"), wholeStackStartRequest("auth")])
    })

    test("should ask for the approvals of the whole stack of a small stack while its root's row shows, and offer its pull request where they are missing", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      // the answers of Patronus wait
      const repo = await setupRepo("stacked_branches", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests() })
      await repo.holdRemoteAnswers({ hold: ["readiness"] })
      await repo.setReviewAnswers({ answers: { "stack:auth": AWAITING_CODE_OWNER } })
      const missing = "Code owners must approve in a pull request: IJPL Code Platform"
      await syncAndWaitForBranches()

      // before the answer the menu says what the whole stack waits for: no Safe Push a later answer refuses. Its ask may
      // still be in the queue behind the held asks of the branches (4 at a time): the rows of the branches can send the
      // demand before the row of the root shows the whole stack
      await findBranchRow(page, "auth").click({ button: "right" })
      await openSubmenu(page, "Whole Stack")
      const waiting = page.getByRole("menuitem", { name: "Safe Push Whole Stack" })
      await expect(waiting).toHaveAttribute("aria-disabled", "true")
      await waiting.hover()
      await expect(page.getByRole("tooltip")).toHaveText("Waits for Patronus to check the approvals")
      await page.keyboard.press("Escape")
      await page.keyboard.press("Escape")
      await expect(page.getByRole("menu")).toHaveCount(0)
      await repo.holdRemoteAnswers({ release: ["readiness"] })
      // the row of the root asks Patronus for the whole stack, before any surface of it asks (the menu asks nothing)
      await expect.poll(async () => (await repo.getReviewReadinessCalls()).some(call => call.branches.includes("stack:auth"))).toBe(true)

      // now the menu knows: its Safe Push disabled with the reason, its pull request offered
      await findBranchRow(page, "auth").click({ button: "right" })
      await openSubmenu(page, "Whole Stack")
      const safePush = page.getByRole("menuitem", { name: "Safe Push Whole Stack" })
      await expect(safePush).toHaveAttribute("aria-disabled", "true")
      await safePush.hover()
      await expect(page.getByRole("tooltip")).toHaveText(missing)
      await page.getByRole("menuitem", { name: "Create Pull Request of the Stack…" }).click()
      const dialog = createPullRequestDialog(page)
      await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("Whole auth stack → master")
      await page.keyboard.press("Escape")

      // and the root's popover: the whole stack disabled, the reason under it
      await openSafePushWithKey(page, "auth")
      const option = safePushPopover(page).getByTestId("safe-push-whole-stack-option")
      await expect(option.getByRole("button", { name: "Safe Push Whole Stack" })).toBeDisabled()
      await expect(option.getByTestId("safe-push-whole-stack-reason")).toHaveText(missing)
    })

    /**
     * The stack of `setupStack` whose starts Patronus refuses, without the trial build of oauth alone (the landing is the
     * only way): `startWithEnter` is Enter in the popover of oauth, the robot that lands auth and login with it, one more
     * start Patronus logs; the popover closes, the refusal is told and closed. A surface a test, two at most: each start
     * of a surface takes seconds under load
     */
    async function setupOauthStarts(
      page: Page,
      setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>,
      syncAndWaitForBranches: () => Promise<unknown>,
    ): Promise<{ repo: TestRepositoryBuilder; startWithEnter: () => Promise<void> }> {
      const { repo } = await setupStack(page, setupRepo, syncAndWaitForBranches, true)
      await routeCommandError(page, "get_stack_independence", { code: "internal", message: "The trial build failed" })
      let starts = 0
      const startWithEnter = async () => {
        const popover = await expectPopoverOf(page, "oauth")
        await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Enter"))
        starts++
        await expectPatronusStarts(
          repo,
          Array.from({ length: starts }, () => startRequest("oauth")),
        )
        await expect(popover).toHaveCount(0)
        await closeRefusal(page)
      }
      return { repo, startWithEnter }
    }

    test("should land a stacked branch with what it is built on in one Safe Push, Enter starts it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const { repo, startWithEnter } = await setupOauthStarts(page, setupRepo, syncAndWaitForBranches)
      const oauthRow = findBranchRow(page, "oauth")

      // the slot of oauth: its own Safe Push, which lands the two branches it is built on with it
      await expect(oauthRow.getByTestId("next-step-slot")).toHaveAttribute("data-step", "safePush")
      const slot = slotSafePush(page, "oauth")
      await expect(slot).toHaveText("Safe Push · 3")
      await slot.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: "Lands oauth" })).toContainText("Lands oauth with the 2 branches it is built on into master in one Safe Push")
      // a stack always says what lands first: the click opens the popover at the slot
      await slot.click()
      const popover = await expectPopoverOf(page, "oauth")
      await expect(popover).toHaveAttribute("data-decision", "confirm")
      await expect(popover.getByTestId("safe-push-lands")).toHaveText("Lands oauth with auth and login, which it is built on: 3 branches · 3 commits into master.")
      await expect(popover.getByTestId("safe-push-alone")).toHaveCount(0)
      // the path it lands is drawn while the popover is open, the pointer away
      await page.mouse.move(0, 0)
      await expect.poll(() => pathRows(page)).toEqual(["auth", "session", "login", "oauth"])
      await startWithEnter()
      // the stack is pushed first, in one push, a step of the start
      expect(await remotePushes(repo)).toEqual([["auth", "login", "oauth"]])

      // "⋯" of the row
      await oauthRow.getByRole("button", { name: "More actions for oauth" }).click()
      await page.getByRole("menuitem", { name: /^Safe Push · 3/ }).click()
      await startWithEnter()

      // Safe Push With Profile… of the branch menu: no profile but the default checks
      await openSafePushWithProfile(page, "oauth")
      await startWithEnter()
      // the stack is on the remote now: no start pushed again
      expect(await remotePushes(repo)).toEqual([["auth", "login", "oauth"]])
    })

    test("should land a stacked branch with what it is built on from S, the preview and the review window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const { repo, startWithEnter } = await setupOauthStarts(page, setupRepo, syncAndWaitForBranches)
      const oauthRow = findBranchRow(page, "oauth")

      // S on the focused row: the stack is pushed first, in one push
      await openSafePushWithKey(page, "oauth")
      await startWithEnter()
      expect(await remotePushes(repo)).toEqual([["auth", "login", "oauth"]])

      // the footer of the preview, which closes for the popover of the row
      let preview = await openBranchPreview(page, "oauth")
      await preview.getByRole("button", { name: /Safe Push · 3/ }).click()
      await expect(preview).toHaveCount(0)
      await startWithEnter()
      // S in the preview
      preview = await openBranchPreview(page, "oauth")
      await page.keyboard.press("s")
      await expect(preview).toHaveCount(0)
      await startWithEnter()

      // the review window of oauth asks the main window, which opens the popover of the row
      const count = (await getOpenedSubWindows(page)).length
      await openContextMenu(page, oauthRow)
      await clickContextMenuItem(page, "Review Changes")
      await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(count + 1)
      const reviewPage = await openLinkedSubWindow(page, (await getOpenedSubWindows(page))[count]!)
      await reviewPage
        .getByTestId("branch-review-actions")
        .getByRole("button", { name: /Safe Push · 3/ })
        .click()
      await expect(safePushPopover(page)).toBeVisible()
      await page.bringToFront()
      await startWithEnter()
      // the stack is on the remote now: no start pushed again
      expect(await remotePushes(repo)).toEqual([["auth", "login", "oauth"]])
    })

    test("should say in the Safe Push popover of a branch in the middle what lands and what stays", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupStack(page, setupRepo, syncAndWaitForBranches)
      await routeCommandError(page, "get_stack_independence", { code: "internal", message: "The trial build failed" })

      await slotSafePush(page, "login").click()
      const popover = await expectPopoverOf(page, "login")
      await expect(popover.getByTestId("safe-push-lands")).toHaveText("Lands login with auth, which it is built on: 2 branches · 2 commits into master.")
      await expect(popover.getByTestId("safe-push-children")).toHaveText("oauth is built on login: it stays, and after the merge the next sync builds it on master.")
      await page.keyboard.press("Escape")
      await expect(popover).toHaveCount(0)
      // the root lands alone; what is built on it stays (S: a click on a lone branch with nothing to say starts at once)
      const root = await openSafePushWithKey(page, "auth")
      await expect(root).toHaveAttribute("aria-label", "Safe Push of auth")
      await expect(root.getByTestId("safe-push-lands")).toHaveText("Lands auth into master: 1 commit.")
      await expect(root.getByTestId("safe-push-children")).toHaveText(/^.+ are built on auth: they stay, and after the merge the next sync builds them on master\.$/)
    })

    test("should offer the retarget of a pull request into the parent's branch as the next step, whose approvals don't count", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      const repo = await setupRepo("stacked_branches", {
        patronus: { ...availableRobots(), githubRepository: "acme/monorepo" },
        pullRequests: availablePullRequests(pullRequest("oauth", 8)),
      })
      await syncAndWaitForBranches()
      await routeCommandError(page, "get_stack_independence", { code: "internal", message: "The trial build failed" })
      const oauthRow = findBranchRow(page, "oauth")
      await expect(oauthRow.getByTestId("pull-request-chip")).toBeVisible()
      await slotSafePush(page, "oauth").click()
      let popover = await expectPopoverOf(page, "oauth")
      await expect(popover.getByTestId("safe-push-from-pull-request")).toHaveText("Comments /patronus merge on #8: Patronus reports there and closes it once merged.")
      await page.keyboard.press("Escape")
      await expect(popover).toHaveCount(0)

      // an older Branch Deck moved #8 onto the branch of login meanwhile: the next step says so once GitHub lists it
      await repo.setPullRequests(availablePullRequests(pullRequest("oauth", 8, { baseRefName: "user-name/virtual/login" })))
      await openContextMenu(page, oauthRow)
      await openSubmenu(page, "Patronus")
      await clickContextMenuItem(page, "Check Approvals Again")
      const slot = oauthRow.getByTestId("next-step-slot")
      await expect(slot).toHaveAttribute("data-step", "retarget")
      const retarget = slot.getByTestId("next-step")
      // short in its column: the hover and the menu name the target
      await expect(retarget).toHaveText("Retarget #8")
      await retarget.hover()
      await expect(page.getByTestId("next-step-hover")).toHaveText(
        "Approvals in #8 don't count: its base is user-name/virtual/login, a virtual branch. Changes the base of #8 from user-name/virtual/login to master on GitHub: Patronus merges a pull request into its base",
      )
      // its start retargets it first (D11): S says so in its popover, which comments on #8 once it goes into master
      await oauthRow.focus()
      await page.keyboard.press("s")
      popover = await expectPopoverOf(page, "oauth")
      await expect(popover.getByTestId("safe-push-retarget")).toHaveText(
        "Retargets #8 of oauth from user-name/virtual/login to master first: every pull request of a virtual branch goes into master.",
      )
      await expect(popover.getByTestId("safe-push-from-pull-request")).toHaveText("Comments /patronus merge on #8: Patronus reports there and closes it once merged.")
      await page.keyboard.press("Escape")
      await expect(popover).toHaveCount(0)

      // the retarget of the slot (its own test in next-step.spec.ts)
      await retarget.click()
      await expect.poll(async () => gitHubWrites(await repo.getGitHubLog())).toEqual([["PATCH", repoPath("pulls", 8), { base: "master" }]])
      await expect(page.locator("[data-slot=toast]", { hasText: "Retargeted #8 to master" })).toBeVisible()
      // into master again: its approvals count, the start comments on it
      await expect(slot).toHaveAttribute("data-step", "safePush")
      await slotSafePush(page, "oauth").click()
      popover = await expectPopoverOf(page, "oauth")
      await expect(popover.getByTestId("safe-push-from-pull-request")).toHaveText("Comments /patronus merge on #8: Patronus reports there and closes it once merged.")
    })

    test("should wait for the Safe Push of an ancestor, which lands its commits", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("stacked_branches", { patronus: availableRobots(robot("login", "login-robot")) })
      await syncAndWaitForBranches()
      await expect(findBranchRow(page, "login").getByTestId("patronus-chip")).toBeVisible()
      const reason = "A Safe Push of login is running: wait until it merges"

      await openContextMenu(page, findBranchRow(page, "oauth"))
      const item = page.getByRole("menuitem", { name: /^Safe Push · 3/ })
      await expect(item).toHaveAttribute("aria-disabled", "true")
      await item.hover()
      await expect(page.locator("[data-slot=tooltip-content]", { hasText: reason })).toBeVisible()
      await closeMenus(page)

      // S says why the Safe Push waits, and opens nothing
      const notice = page.locator("[data-slot=toast]", { hasText: "Safe Push of oauth" })
      const refusal = notice.locator("[data-slot=toast-description]")
      await findBranchRow(page, "oauth").focus()
      await page.keyboard.press("s")
      await expect(refusal).toHaveText(reason)
      await expect(safePushPopover(page)).toHaveCount(0)
      // Base UI hides the close button from the accessibility tree
      await notice.locator("[data-slot=toast-close]").click()
      await expect(notice).toHaveCount(0)

      // a dry run pushes nothing into master: it runs, with what oauth is built on
      await openContextMenu(page, findBranchRow(page, "oauth"))
      await openSubmenu(page, "Patronus")
      await page.getByRole("menuitem", { name: "Dry Run" }).click()
      const popover = await expectPopoverOf(page, "oauth", "Dry Run")
      await expect(popover).toHaveAttribute("data-mode", "dryRun")
      await expect(popover.getByTestId("safe-push-lands")).toHaveText("Checks oauth with auth and login, which it is built on: 3 branches · 3 commits.")
      await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Enter"))
      await expectPatronusStarts(repo, [startRequest("oauth", { mode: "DRY_RUN" })])
      // the robot of login holds the pushes of auth and login: oauth goes alone, its branch has their commits
      expect(await remotePushes(repo)).toEqual([["oauth"]])
      await expect(page.locator("[data-slot=toast]", { hasText: "Dry Run started" })).toBeVisible()

      // the Dry Run Patronus started runs on the last commit of oauth: S names it now
      await findBranchRow(page, "oauth").focus()
      await page.keyboard.press("s")
      await expect(refusal).toHaveText("A Dry Run of the last commit is running")
      await expect(safePushPopover(page)).toHaveCount(0)
    })

    test("should say which branches a running Safe Push lands, and not start them on their own", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRepo("stacked_branches", { patronus: availableRobots(robot("oauth", "oauth-robot")) })
      await syncAndWaitForBranches()
      await expect(findBranchRow(page, "oauth").getByTestId("patronus-chip")).toHaveAttribute("data-kind", "running")

      // oauth is built on login, built on auth: its Safe Push pushes their commits too
      await expect(findBranchRow(page, "auth").getByTestId("landing-span")).toHaveText("lands with oauth")
      const label = findBranchRow(page, "login").getByTestId("landing-span")
      await expect(label).toHaveText("lands with oauth")
      await label.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: "lands this branch" })).toHaveText("A Safe Push of oauth lands this branch with it")
      await page.mouse.move(0, 0)
      await expect(findBranchRow(page, "session").getByTestId("landing-span")).toHaveCount(0)
      await expect(findBranchRow(page, "docs").getByTestId("landing-span")).toHaveCount(0)
      // in the graph: the ring of the robot around the node of oauth, the lines from auth down to it in its colour,
      // past session, which forks off auth before login
      await expect(findBranchRow(page, "oauth").getByTestId("lane-node")).toHaveAttribute("data-ring", "true")
      await expect(findBranchRow(page, "oauth").getByTestId("lane-ring")).toBeVisible()
      await expect(findBranchRow(page, "login").getByTestId("lane-ring")).toHaveCount(0)
      // the nodes of the span are landing: the ring only around the branch the robot runs on
      for (const name of ["auth", "login", "oauth"]) {
        await expect(findBranchRow(page, name).getByTestId("lane-node")).toHaveAttribute("data-kind", "landing")
      }
      await findBranchRow(page, "login").getByTestId("lane-node-target").hover()
      await expect(page.getByTestId("lane-node-details")).toHaveText("Lands with oauth: its Safe Push runs")
      await page.mouse.move(0, 0)
      for (const name of ["auth", "session", "login", "oauth"]) {
        await expect(findBranchRow(page, name).getByTestId("landing-lanes")).toHaveCount(1)
      }
      await expect(findBranchRow(page, "docs").getByTestId("landing-lanes")).toHaveCount(0)

      // a branch the Safe Push lands offers no push and no Safe Push of its own: the Status cell says why, the menu too
      const loginRow = findBranchRow(page, "login")
      const reason = "A Safe Push of oauth is running: it lands 1 commit of login with it"
      await expect(loginRow.getByTestId("next-step-slot")).toHaveAttribute("data-step", "none")
      await expect(getPushButton(loginRow)).toHaveCount(0)
      await openContextMenu(page, loginRow)
      for (const name of [/^Safe Push · 2/, /^Push Branch P$/]) {
        const item = page.getByRole("menuitem", { name })
        await expect(item).toHaveAttribute("aria-disabled", "true")
        await item.hover()
        // the description of the item before may still be fading out
        await expect(page.getByRole("tooltip").filter({ hasText: "is running" }).last()).toHaveText(reason)
      }
      await page.keyboard.press("Escape")
      // S says it too, and opens nothing
      await loginRow.focus()
      await page.keyboard.press("s")
      await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push of login" }).locator("[data-slot=toast-description]")).toHaveText(reason)
      await expect(safePushPopover(page)).toHaveCount(0)
      // a dry run pushes nothing into master, but its push of auth would go into what the Safe Push of oauth lands and
      // fail it at the end: it waits too, and says why
      await openContextMenu(page, findBranchRow(page, "auth"))
      await expect(page.getByRole("menuitem", { name: /^Safe Push S/ })).toHaveAttribute("aria-disabled", "true")
      await openSubmenu(page, "Patronus")
      const dryRun = page.getByRole("menuitem", { name: "Dry Run" })
      await expect(dryRun).toHaveAttribute("aria-disabled", "true")
      await dryRun.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: "is running" }).last()).toHaveText("A Safe Push of oauth is running: it lands 1 commit of auth with it")
    })

    test("should push a branch a restack placed below a running Safe Push after it started, and hold what its commit has", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // on GitHub, no pull request yet
      const { repo } = await setupTakenRobot(page, setupRepo, syncAndWaitForBranches, "stacked_branches", robot("oauth", "oauth-robot"), "auth", {
        pullRequests: { ...availablePullRequests(), nextNumber: 901 },
      })
      await expect(findBranchRow(page, "docs").getByTestId("landing-span")).toHaveCount(0)

      // docs below auth, after the Safe Push of oauth took its commit: the robot lands none of docs
      await openContextMenu(page, findBranchRow(page, "auth"))
      await page.getByRole("menuitem", { name: "Stack On…" }).hover()
      await runAndWaitForSync(page, () => clickContextMenuItem(page, "docs"))
      await expect.poll(() => branchTree(page)).toEqual(["docs@0", "auth@1", "session@2", "login@2", "oauth@3"])
      // auth and login stay held: the robot's commit has their commits, the reason counts them
      await expect(findBranchRow(page, "auth").getByTestId("landing-span")).toHaveText("lands with oauth")
      const authRow = findBranchRow(page, "auth")
      await openContextMenu(page, authRow)
      const push = page.getByRole("menuitem", { name: /^Push Branch P$/ })
      await expect(push).toHaveAttribute("aria-disabled", "true")
      await push.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: "is running" }).last()).toHaveText("A Safe Push of oauth is running: it lands 1 commit of auth with it")
      await page.keyboard.press("Escape")
      const docsRow = findBranchRow(page, "docs")
      await expect(docsRow.getByTestId("landing-span")).toHaveCount(0)
      // its next step is its own Safe Push, as of any root
      await expect(docsRow.getByTestId("next-step-slot")).toHaveAttribute("data-step", "safePush")

      // its push from the menu and by the key pushes it, and so does the push of Create Pull Request; between them its
      // remote branch goes back to master in a terminal, so each finds its commit to push
      const unpushed = async () => {
        await repo.pushInTerminal(virtualBranch("docs"), { revision: "origin/master" })
        await syncFromMenu(page)
      }
      await openContextMenu(page, docsRow)
      await page.getByRole("menuitem", { name: /^Push Branch \(1\)/ }).click()
      await expectPushedToast(page, "docs")
      await expect.poll(() => remotePushes(repo)).toEqual([["docs"]])
      await unpushed()
      await docsRow.focus()
      await page.keyboard.press("p")
      await expect.poll(() => remotePushes(repo)).toEqual([["docs"], ["docs"]])
      await unpushed()
      await openContextMenu(page, docsRow)
      await clickContextMenuItem(page, "Create Pull Request…")
      const dialog = createPullRequestDialog(page)
      await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("docs → master")
      await dialog.getByTestId("create-pull-request-submit").click()
      await expect(page.locator("[data-slot=toast]", { hasText: "Created pull request #901" })).toBeVisible()
      expect(await remotePushes(repo)).toEqual([["docs"], ["docs"], ["docs"]])
      expect(await gitHubRequests(repo, "POST", "pulls")).toEqual([expect.objectContaining({ head: "user-name/virtual/docs", base: "master" })])
    })

    test("should say which branches a merged Safe Push landed until the update, and offer no Safe Push or push of them", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("stacked_branches", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      await routeCommandError(page, "get_stack_independence", { code: "internal", message: "The trial build failed" })
      // the robot the fake Patronus starts
      const started = robot("oauth", "started-1")
      const authRow = findBranchRow(page, "auth")
      const loginRow = findBranchRow(page, "login")

      // each branch offers its own Safe Push, with what it is built on
      await expect(slotSafePush(page, "auth")).toHaveText("Safe Push")
      await expect(slotSafePush(page, "login")).toHaveText("Safe Push · 2")
      await expect(slotSafePush(page, "session")).toHaveText("Safe Push · 2")
      // the Safe Push of oauth lands auth and login with it, in one robot
      await slotSafePush(page, "oauth").click()
      await expectPopoverOf(page, "oauth")
      await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("Enter"))
      await expectPatronusStarts(repo, [startRequest("oauth")])
      await expect(loginRow.getByTestId("landing-span")).toHaveText("lands with oauth")

      // it merged: the branches it landed say so, until the update archives them
      await repo.setPatronus(availableRobots({ ...started, sourceRevision: "", status: "success", finishTime: 1704142800 }))
      await emitTauriEvent(page, "tauri://focus")
      await expect(page.locator("[data-slot=toast]").filter({ hasText: "oauth merged into master" })).toBeVisible()
      for (const row of [authRow, loginRow]) {
        await expect(row.getByTestId("landed-with")).toHaveText("Landed with oauth")
        await expect(row.getByTestId("next-step-slot")).toHaveAttribute("data-step", "none")
        await expect(row.getByTestId("branch-status")).toHaveText("landed")
        await expect(row.getByTestId("landing-span")).toHaveCount(0)
      }
      // oauth is its robot's: Merged; session, built on auth too, lands only itself now
      await expect(findBranchRow(page, "oauth").getByTestId("patronus-chip")).toHaveAttribute("data-kind", "merged")
      await expect(findBranchRow(page, "oauth").getByTestId("landed-with")).toHaveCount(0)
      await expect(slotSafePush(page, "session")).toHaveText("Safe Push")
      await expect(findBranchRow(page, "session").getByTestId("landed-with")).toHaveCount(0)

      // its hover says when it merged, a click opens the robot
      const chip = loginRow.getByTestId("landed-with")
      await chip.hover()
      await expect(page.getByTestId("landed-with-popover")).toContainText("In master with the Safe Push of oauth · the update removes the local copies")
      await chip.click()
      await expect.poll(() => getOpenedUrls(page)).toContain(started.url)
      await page.mouse.move(0, 0)

      // no Safe Push, no Dry Run, no push of commits that landed in the menu, each saying why; S says it too
      const reason = "Landed in master with oauth"
      await expectLandedActions(page, "login", reason)
      await expectLandedActions(page, "auth", reason)
      // the preview names it too, and its Safe Push says why it is disabled
      const preview = await openBranchPreview(page, "login")
      await expect(preview.getByTestId("landed-with")).toHaveText("Landed with oauth")
      const footer = preview.getByTestId("safe-push-button")
      await expect(footer).toHaveAttribute("aria-disabled", "true")
      await footer.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: reason })).toHaveText(reason)
    })

    test("should hold the branch its merged Safe Push ran on, and keep what it landed by content after a change below its commit: a commit added since is the only work left", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      const running = robot("oauth", "oauth-robot")
      const repo = await setupRunningRobot(page, setupRepo, syncAndWaitForBranches, "stacked_branches", running, "login")
      // it merged on oauth as the table has it
      await mergeRunningRobot(page, repo, running)
      await expect(page.locator("[data-slot=toast]").filter({ hasText: "oauth merged into master" })).toBeVisible()
      const authRow = findBranchRow(page, "auth")
      const loginRow = findBranchRow(page, "login")
      const oauthRow = findBranchRow(page, "oauth")

      // oauth, the branch it ran on, landed whole too: its row says Merged and offers no Safe Push and no push
      await expect(oauthRow.getByTestId("patronus-chip")).toHaveAttribute("data-kind", "merged")
      await expect(oauthRow.getByTestId("next-step-slot")).toHaveAttribute("data-step", "none")
      await expect(getPushButton(oauthRow)).toHaveCount(0)
      await expectLandedActions(page, "oauth", "Merged by Patronus")
      await expect(authRow.getByTestId("landed-with")).toHaveText("Landed with oauth")

      // a new commit of auth behind the app's back: the sync rebuilds login and oauth on it, no hash is the robot's now
      await runAndWaitForSync(page, async () => {
        await repo.commit("(auth) Check the token", { "token.js": "export const token = null\n" }, 1704128400)
        await page.getByTestId("sync-button").click()
      })
      // by content: auth landed but for the new commit, the only work left of it, which its Safe Push sends: the remote
      // lacks auth, its first push is a step of it, in the menu only
      const authChip = authRow.getByTestId("landed-with")
      await expect(authChip).toHaveText("Landed with oauth · 1 commit after it")
      await expect(slotSafePush(page, "auth")).toHaveText("Safe Push")
      await expect(getPushButton(authRow)).toHaveCount(0)
      await openContextMenu(page, authRow)
      await expect(page.getByRole("menuitem", { name: "Push Branch (2) P" })).toBeEnabled()
      await page.keyboard.press("Escape")
      await authChip.hover()
      await expect(page.getByTestId("landed-with-after")).toHaveText("1 commit added since, not in master yet")
      await page.mouse.move(0, 0)
      // login and oauth, copies of the same changes, landed whole
      await expect(loginRow.getByTestId("landed-with")).toHaveText("Landed with oauth")
      await expectLandedActions(page, "login", "Landed in master with oauth")
      // oauth, rebuilt with the same changes, stays Merged and held (auth above shows the push its new commit needs)
      await expect(oauthRow.getByTestId("patronus-chip")).toHaveAttribute("data-kind", "merged")
      await expect(oauthRow.getByTestId("next-step-slot")).toHaveAttribute("data-step", "none")
      await expect(getPushButton(oauthRow)).toHaveCount(0)
      // session, built on auth: its Safe Push lands the new commit of auth with it
      await expect(slotSafePush(page, "session")).toHaveText("Safe Push · 2")
    })

    test("should close by content the pull requests of the branches a Safe Push landed after a change below its commit, not of one with work left", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      const running = robot("oauth", "oauth-robot")
      const repo = await setupRepo("stacked_branches", {
        patronus: availableRobots(running),
        pullRequests: availablePullRequests(pullRequest("auth", 4, { headRefOid: "remote-auth" }), pullRequest("login", 5, { headRefOid: "remote-login" })),
      })
      // auth and login are on the remote as their pull requests show them; the robot takes oauth as this sync builds it
      const onRemote = () =>
        repo.setSyncOverrides({
          remote: Object.fromEntries(["auth", "login"].map(name => [name, { remoteExists: true, remoteTip: `remote-${name}`, commitsBehind: 0, fastForward: false }])),
        })
      await onRemote()
      await syncAndWaitForBranches()
      const taken = await repo.revision("user-name/virtual/oauth")
      await expect(findBranchRow(page, "login").getByTestId("landing-span")).toHaveText("lands with oauth")

      // a new commit of auth behind the app's back: the sync rebuilds login and oauth on it, no hash is the robot's now
      await onRemote()
      await runAndWaitForSync(page, async () => {
        await repo.commit("(auth) Check the token", { "token.js": "export const token = null\n" }, 1704128400)
        await page.getByTestId("sync-button").click()
      })
      // it merged on oauth as it was: the content tells what it landed, the toast waits for it
      await repo.setPatronus(availableRobots({ ...running, sourceRevision: taken, status: "success", finishTime: 1704142800 }))
      await emitTauriEvent(page, "tauri://focus")
      await expect(page.locator("[data-slot=toast]").filter({ hasText: "oauth merged into master" })).toContainText("closes the pull request of login: it landed with oauth")
      // login landed whole and its pull request holds nothing it lacks; auth has the new commit left, its pull request stays
      const closes = async () => gitHubWrites(await repo.getGitHubLog())
      await expect.poll(closes).toEqual([
        ["POST", repoPath("issues", 5, "comments"), { body: expect.stringContaining("the Safe Push of user-name/virtual/oauth") }],
        ["PATCH", repoPath("pulls", 5), { state: "closed" }],
      ])
      await expect(findBranchRow(page, "auth").getByTestId("landed-with")).toHaveText("Landed with oauth · 1 commit after it")
    })

    test("should refuse the restart of the failed robot of a branch a merged Safe Push of a branch built on it landed, in its menu, its robot's details and its preview", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      // the Safe Push of auth failed; then the Safe Push of oauth runs, which lands auth and login with it
      const failed = robot("auth", "auth-failed", { status: "failure", finishTime: 1704139300 })
      const running = robot("oauth", "oauth-robot")
      const repo = await setupRepo("stacked_branches", { patronus: availableRobots(failed, running) })
      await syncAndWaitForBranches()
      const authRow = findBranchRow(page, "auth")
      await expect(authRow.getByTestId("landing-span")).toHaveText("lands with oauth")
      // while it runs, the restart of auth waits for it
      await authRow.click({ button: "right" })
      await expectDisabledItems(page, [["Restart Safe Push", "A Safe Push of oauth is running: it lands 1 commit of auth with it"]])
      await page.keyboard.press("Escape")
      await expect(page.getByRole("menu")).toHaveCount(0)

      // it merged: auth is in master, nothing of it is left to restart
      await mergeRunningRobots(page, repo, [running], 1704142800, [failed])
      await expect(authRow.getByTestId("landed-with")).toHaveText("Landed with oauth")
      const reason = "Landed in master with oauth"
      await authRow.click({ button: "right" })
      await expectDisabledItems(page, [["Restart Safe Push", reason]])
      await page.keyboard.press("Escape")
      await expect(page.getByRole("menu")).toHaveCount(0)
      // the details of its robot in the preview say it too
      const preview = await openBranchPreview(page, "auth")
      const restart = preview.getByTestId("branch-preview-patronus").getByRole("button", { name: "Restart Safe Push" })
      await expect(restart).toHaveAttribute("aria-disabled", "true")
      await restart.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: reason })).toHaveText(reason)
      await restart.click({ force: true })
      expect((await repo.getPatronusLog()).restarts).toEqual([])
    })

    test("should disable Create Pull Request… of a branch a merged Safe Push landed, in its menu, its preview and its review window: nothing of it is left to review", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      // on GitHub, no pull request yet; the Safe Push of oauth runs, which lands auth and login with it
      const running = robot("oauth", "oauth-robot")
      const repo = await setupRepo("stacked_branches", { patronus: availableRobots(running), pullRequests: availablePullRequests() })
      await syncAndWaitForBranches()
      const authRow = findBranchRow(page, "auth")
      await expect(authRow.getByTestId("landing-span")).toHaveText("lands with oauth")
      // auth has work to review while it runs
      await authRow.click({ button: "right" })
      await expect(page.getByRole("menuitem", { name: "Create Pull Request…" })).not.toHaveAttribute("aria-disabled", "true")
      await page.keyboard.press("Escape")
      await expect(page.getByRole("menu")).toHaveCount(0)

      // it merged: auth and login are in master, oauth is its robot's; session, built on auth, has its own work
      await mergeRunningRobot(page, repo, running)
      await expect(authRow.getByTestId("landed-with")).toHaveText("Landed with oauth")
      for (const [name, reason] of [
        ["auth", "Landed in master with oauth"],
        ["login", "Landed in master with oauth"],
        ["oauth", "Merged by Patronus"],
      ] as const) {
        await findBranchRow(page, name).click({ button: "right" })
        await expectDisabledItems(page, [["Create Pull Request…", reason]])
        await page.keyboard.press("Escape")
        await expect(page.getByRole("menu")).toHaveCount(0)
      }
      // the footer of the preview says it too, and its click opens no dialog
      let preview = await openBranchPreview(page, "auth")
      const create = preview.getByRole("button", { name: "Create Pull Request…" })
      await expect(create).toHaveAttribute("aria-disabled", "true")
      await create.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: "Landed in master with oauth" })).toHaveText("Landed in master with oauth")
      await create.click({ force: true })
      await expect(preview).toBeVisible()
      await expect(createPullRequestDialog(page)).toHaveCount(0)
      await page.keyboard.press("Escape")
      await expect(preview).toHaveCount(0)
      // and the title bar of its review window
      const count = (await getOpenedSubWindows(page)).length
      await openContextMenu(page, authRow)
      await clickContextMenuItem(page, "Review Changes")
      await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(count + 1)
      const reviewPage = await openLinkedSubWindow(page, (await getOpenedSubWindows(page))[count]!)
      const reviewCreate = reviewPage.getByTestId("branch-review-actions").getByRole("button", { name: "Create Pull Request…" })
      await expect(reviewCreate).toHaveAttribute("aria-disabled", "true")
      await reviewCreate.hover()
      await expect(reviewPage.getByRole("tooltip").filter({ hasText: "Landed in master with oauth" })).toHaveText("Landed in master with oauth")
      await page.bringToFront()

      // session lands without auth: its pull request opens from the same footer
      preview = await openBranchPreview(page, "session")
      await preview.getByRole("button", { name: "Create Pull Request…" }).click()
      await expect(createPullRequestDialog(page)).toBeVisible()
    })

    test("should offer to land only a stacked branch that applies alone", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const { repo } = await setupStack(page, setupRepo, syncAndWaitForBranches)

      // oauth adds a file of its own: it applies on the baseline without login, the second way after the landing
      await slotSafePush(page, "oauth").click()
      const popover = await expectPopoverOf(page, "oauth")
      await expect(popover.getByTestId("safe-push-lands")).toHaveText("Lands oauth with auth and login, which it is built on: 3 branches · 3 commits into master.")
      const alone = popover.getByTestId("safe-push-alone")
      await expect(alone).toHaveAttribute("data-action", "unstack")
      await expect(alone).toContainText("To land oauth alone: it applies on the baseline without login and uses no name it adds. Not checked by a compiler.")
      // the landing stays the start: Enter would start it
      await expectEnterStarts(page)

      await alone.getByRole("button", { name: "Unstack from login" }).click()
      // the popover shows itself again, of oauth alone, a root now: no start after the unstack
      await expect(popover.getByTestId("safe-push-lands")).toHaveText("Lands oauth into master: 1 commit.")
      await expect(alone).toHaveCount(0)
      await expect.poll(() => branchTree(page)).toContain("oauth@0")
      expect((await repo.getPatronusLog()).starts).toEqual([])
      await runAndWaitForAnswer(page, "execute", () => popoverStart(page).click())
      await expectPatronusStarts(repo, [startRequest("oauth")])
      await expect(popover).toHaveCount(0)
      // only oauth lands, so only oauth is pushed
      expect(await remotePushes(repo)).toEqual([["oauth"]])
      // the toast of the start can cancel it
      const started = page.locator("[data-slot=toast]", { hasText: "Safe Push started" })
      await expect(started.getByRole("button", { name: "Open in Patronus" })).toBeVisible()
      await expect(started.getByRole("button", { name: "Cancel Safe Push" })).toBeVisible()
    })

    test("should warn that a stacked branch that applies alone uses a name its parent adds, the unstack still offered", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // lane-ui is stacked on lane-core by the stack parents, with the symbols on; its commit calls laneDocument()
      await setupRepo("stack_uses_parent", { patronus: availableRobots() })
      await syncAndWaitForBranches()

      await slotSafePush(page, "lane-ui").click()
      const popover = await expectPopoverOf(page, "lane-ui")
      // the landing leads, Enter starts it
      await expect(popover.getByTestId("safe-push-lands")).toHaveText(/^Lands lane-ui with lane-core, which it is built on: 2 branches · \d+ commits into master\.$/)
      const alone = popover.getByTestId("safe-push-alone")
      await expect(alone).toHaveAttribute("data-action", "unstack")
      await expect(alone).toHaveAttribute("data-warning", "true")
      await expect(alone).toContainText(
        '"Show the lane document" uses laneDocument, which "Add the lane document" of lane-core adds. lane-ui applies on the baseline without lane-core, but needs it to compile. Found by names, not by a compiler.',
      )
      // the unstack is offered, never decided: names, not a compiler, found the use
      await expect(alone.getByRole("button", { name: "Unstack from lane-core" })).toBeEnabled()
      await expectEnterStarts(page)
    })

    test("should offer the move of the commit a stacked branch needs to land it alone, then its unstack, and review the branch alone", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      // perf-part-2 is stacked on IJPL-257337 by the stack parents; its commit needs one commit of IJPL-257337
      const repo = await setupRepo("stack_push_alone", { patronus: availableRobots() })
      await syncAndWaitForBranches()

      // on Space the merge request misses the approvals of the code owners: the click opens the popover, which names
      // them and refuses the start a robot would fail (D14)
      await slotSafePush(page, "perf-part-2").click()
      const popover = safePushPopover(page)
      await expect(popover).toHaveAttribute("aria-label", "Safe Push of perf-part-2")
      await expect(popover.getByTestId("safe-push-status")).toHaveText("Patronus checks the approvals when the robot starts and fails without them")
      // the landing with what it is built on leads, the way alone is the second one
      await expect(popover.getByTestId("safe-push-lands")).toHaveText(/^Lands perf-part-2 with IJPL-257337, which it is built on: 2 branches · \d+ commits into master\.$/)
      const alone = popover.getByTestId("safe-push-alone")
      await expect(alone).toHaveAttribute("data-action", "move")
      await expect(alone).toContainText(
        'To land perf-part-2 alone, move what it needs here: "Cache the parsed icon" changes lines that "Parse the icon off the EDT" of IJPL-257337 adds.',
      )
      // the review covers the whole stack
      const review = popover.getByTestId("safe-push-review")
      const groups = popover.getByTestId("safe-push-review-groups").getByRole("listitem")
      await expect(review).toContainText("Needs review by the code owners: a Safe Push fails without these approvals.")
      await expect(groups).toHaveText([/^Icons Team/, /^UI Team/])

      await alone.getByRole("button", { name: "Move 1 Commit Here" }).click()
      await verifyCommitCount(findBranchRow(page, "perf-part-2"), 2)
      // the toast of the move offers the unstack too, never does it
      const moved = page.locator("[data-slot=toast]", { hasText: "Moved 1 commit to perf-part-2" })
      await expect(moved.getByRole("button", { name: "Unstack from IJPL-257337" })).toBeVisible()
      await expect(alone).toHaveAttribute("data-action", "unstack")
      await expect(alone).toContainText("To land perf-part-2 alone: it applies on the baseline without IJPL-257337 and uses no name it adds. Not checked by a compiler.")
      expect(await branchTree(page)).toEqual(["IJPL-257337@0", "perf-part-2@1"])

      // the review of the new tip waits: until it answers, the popover says so and shows no review of the whole stack
      await repo.holdRemoteAnswers({ hold: ["readiness"] })
      await alone.getByRole("button", { name: "Unstack from IJPL-257337" }).click()
      // the popover shows itself again, without a start: a root with its 2 commits, reviewed alone
      await expect(popover.getByTestId("safe-push-lands")).toHaveText("Lands perf-part-2 into master: 2 commits.")
      await expect(popover.getByTestId("safe-push-status")).toHaveText("Checking the code-owner review…")
      await expect(review).toHaveCount(0)
      await repo.holdRemoteAnswers({ release: ["readiness"] })
      // two roots now, in the order of their latest commits
      expect((await branchTree(page)).toSorted()).toEqual(["IJPL-257337@0", "perf-part-2@0"])
      await verifyCommitCount(findBranchRow(page, "perf-part-2"), 2)
      await expect(review).toContainText("Needs review by the code owners: a Safe Push fails without these approvals.")
      await expect(groups).toHaveText([/^UI Team/])
      await expect(popoverStart(page)).toBeDisabled()
    })
  })
})
