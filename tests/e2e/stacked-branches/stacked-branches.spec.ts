// @spec spec/branch/stacked-branches.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getPushButton, getStackChip, verifyCommitCount } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { inlineBranchCreator } from "../helpers/inline-form-helpers"
import { AWAITING_CODE_OWNER, NO_REVIEW_NEEDED, availableRobots, robot, routeReviewRequirement } from "../helpers/patronus-helpers"
import { availablePullRequests, createdPullRequest, createPullRequestDialog, getOpenedUrls, pullRequest, reviewer, routeNewPullRequest } from "../helpers/pull-request-helpers"
import { confirmForcePush, expectPushedToast, PUSH_TIME, pushedToast, pushOutcome, routeHeldPush, routePush, routePushBranches, wholeStackResultKey } from "../helpers/push-helpers"
import { EARLIER_COMMIT, openBranchPreview } from "../helpers/review-status-helpers"
import {
  DEFAULT_CHECKS_ONLY,
  closeRefusal,
  expectLandedActions,
  expectPopoverAtSlot,
  openSafePushWithKey,
  openSafePushWithProfile,
  pathRows,
  popoverStart,
  safePushPopover,
  slotSafePush,
} from "../helpers/safe-push-helpers"
import { clickContextMenuItem, clickGroupIntoBranchButton, openContextMenu, selectCommit } from "../helpers/selection-helpers"
import { getOpenedSubWindows, openLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent, rewriteNextRemoteStatus, rewriteNextSyncEvents, runAndWaitForSync, syncBranches } from "../helpers/sync-helpers"
import { holdCommand, routeCommand, routeCommandError, waitForStorePersistence } from "../helpers/wait-helpers"

const GITHUB_COMPARE = "https://github.com/JetBrains/ultimate/compare"

/** `name@depth` of the branch rows in display order */
async function tree(page: Page): Promise<string[]> {
  return page.getByTestId("branch-row").evaluateAll(rows => rows.map(row => `${row.getAttribute("data-branch-name")}@${row.getAttribute("data-depth") ?? 0}`))
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
    await verifyCommitCount(findBranchRow(page, "login"), "1 commit")

    expect(await tree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
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
    await expect(page.getByRole("tooltip")).toHaveText("Safe Push lands 2 branches: Patronus checks the approvals when the robot starts")
    await expect(findBranchRow(page, "login").getByTestId("branch-status-dot")).toHaveAttribute("data-color", "primary")
    // the graph shows what a branch is built on: the row doesn't say it
    await expect(findBranchRow(page, "oauth").getByTestId("stack-base")).toHaveCount(0)
    // the preview has no tree: it says what the branch is built on
    await findBranchRow(page, "oauth").hover()
    await page.keyboard.press("Space")
    await expect(page.getByTestId("branch-preview-body")).toBeVisible()
    await expect(page.getByRole("dialog").getByTestId("stack-base")).toHaveText("on login")
    await page.keyboard.press("Escape")
    // nor does the outgoing overview
    await page.getByTestId("outgoing-indicator").hover()
    const loginRow = page.getByTestId("outgoing-popover").locator("[data-testid=outgoing-branch][data-branch-name=login]")
    await expect(loginRow.getByTestId("stack-base")).toHaveText("on auth")
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
    await page.getByTestId("outgoing-indicator").hover()
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
    expect(await tree(page)).toEqual(["docs@0", "auth@0"])
    await expect(chip).toHaveAttribute("data-stack", "collapsed")
    // what the hidden rows hold: their branches to push, in the push colour
    await expect(chip.getByTestId("stack-chip-attention")).toHaveText("· 3 to push")
    await expect(findBranchRow(page, "auth").getByTestId("lane-collapsed")).toBeVisible()
    // the header still counts every branch
    await expect(page.getByTestId("outgoing-indicator")).toHaveAttribute("aria-label", /in 5 branches/)

    // the repository remembers it (`collapsedStacks`): a new window starts with the stack collapsed
    await waitForStorePersistence(page, "recentProjects", projects => (projects as { collapsedStacks?: string[] }[] | undefined)?.[0]?.collapsedStacks?.includes("auth") === true)
    await page.reload()
    await syncBranches(page, ['[data-testid="branch-row"][data-branch-name="auth"]'])
    expect(await tree(page)).toEqual(["docs@0", "auth@0"])
    await getStackChip(findBranchRow(page, "auth")).click()
    expect(await tree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
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
    await expect(page.getByTestId("robots-indicator")).toHaveText("1 failed")
    // the node of the failed branch needs a fix, where the eye starts reading a row; the ones fixed since are a Safe Push again
    await expect(findBranchRow(page, "session").getByTestId("lane-node")).toHaveAttribute("data-kind", "needsFix")
    await expect(findBranchRow(page, "session").getByTestId("lane-node")).toHaveAttribute("data-color", "error")
    await expect(findBranchRow(page, "session").getByTestId("lane-node")).toHaveAttribute("data-filled", "true")
    await expect(findBranchRow(page, "login").getByTestId("lane-node")).toHaveAttribute("data-kind", "yourStep")
    const chip = getStackChip(findBranchRow(page, "auth"))
    await chip.click()
    // the header's count: the failure the user has to act on, not the two fixed since
    await expect(chip.getByTestId("stack-chip-attention")).toHaveText("· 1 failed")
    await expect(chip.getByTestId("stack-chip-attention")).toHaveClass(/text-destructive/)
  })

  test("should collapse and expand a stack with Alt+arrows from any of its rows", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    await findBranchRow(page, "oauth").focus()

    await page.keyboard.press("Alt+ArrowLeft")
    expect(await tree(page)).toEqual(["docs@0", "auth@0"])
    // the focus moves from the hidden row to the root
    await expect(findBranchRow(page, "auth")).toBeFocused()
    await page.keyboard.press("Alt+ArrowRight")
    await expect.poll(() => tree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
    // a lone branch has no stack to collapse; the menu offers it on a branch of a stack
    await findBranchRow(page, "docs").focus()
    await page.keyboard.press("Alt+ArrowLeft")
    await expect(page.getByTestId("branch-row")).toHaveCount(5)
    await openContextMenu(page, findBranchRow(page, "login"))
    await clickContextMenuItem(page, "Collapse Stack")
    expect(await tree(page)).toEqual(["docs@0", "auth@0"])
  })

  test("should block the children of a parent that doesn't build", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_conflict")
    await syncAndWaitForBranches()

    const loginRow = findBranchRow(page, "login")
    await expect(loginRow.getByTestId("branch-status")).toHaveText("blocked by auth")
    await expect(findBranchRow(page, "auth").getByTestId("branch-status")).toHaveText("merge conflict")
    await expect(getPushButton(loginRow)).toBeDisabled()
    await expect(getPushButton(loginRow)).toHaveAccessibleName("Push")

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
    await runAndWaitForSync(page, async () => {
      await clickContextMenuItem(page, "oauth")
      await expect(page.getByText("Stacked docs on oauth")).toBeVisible()
    })
    await expect.poll(() => tree(page)).toEqual(["auth@0", "session@1", "login@1", "oauth@2", "docs@3"])

    await openContextMenu(page, findBranchRow(page, "docs"))
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "Unstack from oauth"))
    await expect.poll(() => tree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
  })

  test("should stack an issue branch without a rewrite and undo it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_issues")
    await syncAndWaitForBranches()
    // docs is stacked on ABC-1 by the stack parents, its prefix names no parent
    expect(await tree(page)).toEqual(["ABC-2@0", "ABC-1@0", "docs@1"])

    await openContextMenu(page, findBranchRow(page, "ABC-2"))
    await page.getByRole("menuitem", { name: "Stack On…" }).hover()
    await runAndWaitForSync(page, async () => {
      await clickContextMenuItem(page, "ABC-1")
      // no commit moved: the toast names no count
      await expect(page.getByText("Stacked ABC-2 on ABC-1", { exact: true })).toBeVisible()
    })
    await expect.poll(() => tree(page)).toEqual(["ABC-1@0", "docs@1", "ABC-2@1"])
    await expandBranch(page, findBranchRow(page, "ABC-2"))
    // the commit keeps its message; the row of ABC-2 shows the key
    await expect(getBranchCommitRow(page, "ABC-2", "Parse lists")).toBeVisible()

    await runAndWaitForSync(page, () => page.getByRole("button", { name: "Undo" }).click())
    await expect.poll(() => tree(page)).toEqual(["ABC-2@0", "ABC-1@0", "docs@1"])
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

    await expect.poll(() => tree(page)).toEqual(["docs@0", "identity@0", "session@1", "login@1", "oauth@2"])
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
    await expect.poll(() => tree(page)).toContain("oauth@0")
    await expect(findBranchRow(page, "oauth").getByTestId("stack-base")).toHaveCount(0)
    await expect(getBranchDetailsRow(page, "oauth").getByTestId("branch-base-changed")).toHaveCount(0)
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

    await expect.poll(() => tree(page)).toEqual(["auth@0", "guide@1", "session@1", "login@1", "oauth@2"])
    await expect(findBranchRow(page, "guide")).toHaveAttribute("data-state", "open")
  })

  test("should push a stack parents first and create each pull request into master", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // on GitHub, no pull request yet
    await setupRepo("stacked_branches", { pullRequests: availablePullRequests() })
    await syncAndWaitForBranches()
    // the E2E server has no remote: answer the pushes and record their order
    const pushes = await routePush(page, {})

    // login isn't pushed yet: the button of oauth pushes the stack, its description offers oauth alone
    const oauth = findBranchRow(page, "oauth")
    await expect(oauth.getByTestId("branch-status-dot")).toHaveAttribute("data-color", "primary")
    const button = getPushButton(oauth)
    // the menu names it alike, the arrow counts the commits
    await expect(button).toHaveAccessibleName("Push Stack (3 branches)")
    await expect(button).toHaveText("3")
    await expect(button).toBeEnabled()
    // the menu has the one push of the button, no second Push Stack
    await openContextMenu(page, oauth)
    await expect(page.getByRole("menuitem", { name: /^Push Stack/ })).toHaveCount(1)
    await expect(page.getByRole("menuitem", { name: "Push Stack (3 branches) P" })).toBeEnabled()
    await page.keyboard.press("Escape")
    await button.hover()
    const description = page.getByTestId("push-description")
    await expect(description).toContainText(
      "Pushes 3 branches, parents first: auth · 1 commit · creates the remote branch login · 1 commit · creates the remote branch oauth · 1 commit · creates the remote branch",
    )
    const alternative = description.getByTestId("push-alternative")
    await expect(alternative).toContainText("login isn't on the remote yet. Pushing oauth alone works: its pull request includes login's commits either way.")
    await expect(alternative.getByRole("button", { name: "Push oauth Only" })).toBeVisible()

    await button.click()
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["auth", "login", "oauth"])
    expect(pushes.map(push => push.parentBranch ?? null)).toEqual([null, "auth", "login"])
    // the stack is on the remote: the dots of their status are neutral
    for (const name of ["auth", "login", "oauth"]) {
      await expect(findBranchRow(page, name).getByTestId("branch-status-dot")).toHaveAttribute("data-color", "neutral")
    }
    // one toast for the stack, as Activity has an entry per branch; it creates the pull request of oauth
    const toast = page.locator("[data-slot=toast]", { hasText: "Pushed auth, login and oauth" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("3 commits · created 3 remote branches")
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
    const texts = await routeNewPullRequest(page)
    await toast.getByRole("button", { name: "Create Pull Request…" }).click()
    // into master, with the commits of the branches oauth is built on: what its Safe Push lands
    const dialog = createPullRequestDialog(page)
    await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("oauth → master")
    await expect(dialog.getByTestId("create-pull-request-includes")).toHaveText("Includes the commits of auth and login, which oauth is built on: 3 branches · 3 commits")
    expect(texts).toEqual([expect.objectContaining({ head: "user-name/virtual/oauth", base: "master" })])
    // GitHub's form, as "Continue on GitHub" opens it, takes the same base
    await dialog.getByTestId("create-pull-request-on-github").click()
    await expect.poll(() => getOpenedUrls(page)).toEqual([`${GITHUB_COMPARE}/master...user-name/virtual/oauth?expand=1&title=Add+auth&body=-+**Add+login**&assignees=user-name`])
    await expect(dialog).toHaveCount(0)

    // the root's pull request goes into master too, and holds only its own commits
    const creates = await routeCommand(page, "create_pull_request", () => createdPullRequest(pullRequest("auth", 900)))
    await openContextMenu(page, findBranchRow(page, "auth"))
    await clickContextMenuItem(page, "Create Pull Request…")
    await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("auth → master")
    await expect(dialog.getByTestId("create-pull-request-includes")).toHaveCount(0)
    await dialog.getByTestId("create-pull-request-submit").click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Created pull request #900" }).locator("[data-slot=toast-description]")).toHaveText("auth → master · assigned to you")
    expect(creates).toEqual([expect.objectContaining({ branchName: "auth", head: "user-name/virtual/auth", base: "master" })])
  })

  test("should push a stack in one push, refused whole when a remote branch of it moved", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    // someone else pushed to login since its row was shown: the remote refuses the one push, nothing of it goes out, and
    // every branch is fetched: auth and oauth are still not on the remote
    const theirs = { id: "c0ffee0", subject: "Their fix", author: "Ann", authorTime: PUSH_TIME }
    const { stackPushes, pushes } = await routePushBranches(page, {
      auth: { remoteExists: false, unpushedCommits: ["auth-1"] },
      login: [{ outcome: "remoteMoved", unpushedCommits: ["login-1"], commitsBehind: 1, droppedCommits: [theirs], remoteTip: "theirs" }, {}],
      oauth: { remoteExists: false, unpushedCommits: ["oauth-1"] },
    })
    const oauth = findBranchRow(page, "oauth")
    await expect(getPushButton(oauth)).toHaveAccessibleName("Push Stack (3 branches)")
    await getPushButton(oauth).click()

    const toast = page.locator("[data-slot=toast]", { hasText: "login changed on the remote" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("1 commit you don't have (by Ann). Nothing was pushed.")
    expect(stackPushes.map(push => push.map(branch => branch.branchName))).toEqual([["auth", "login", "oauth"]])
    // no toast of a push, and the branches below and above login are still not on the remote
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
    for (const name of ["auth", "oauth"]) {
      await expect(findBranchRow(page, name).getByTestId("branch-status-dot")).toHaveAttribute("data-color", "primary")
    }

    // the toast pushes login alone over what its remote has now, after the row shows the commit the push drops
    await toast.getByRole("button", { name: "Force Push" }).click()
    await confirmForcePush(page, "Force Push (1)")
    await expect.poll(() => pushes.length).toBe(4)
    expect([pushes[3]!.branchName, pushes[3]!.expectedRemote]).toEqual(["login", "theirs"])
    expect(stackPushes).toHaveLength(1)
  })

  test("should push a stacked branch alone from the description of its button", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    const pushes = await routePush(page, {})

    await getPushButton(findBranchRow(page, "oauth")).hover()
    await page.getByTestId("push-description").getByRole("button", { name: "Push oauth Only" }).click()
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["oauth"])
    expect(pushes[0]?.parentBranch).toBe("login")
    await expect(pushedToast(page, "oauth")).toBeVisible()
    // oauth is on the remote: its menu pushes nothing more, as its button; login's pushes the parents
    await openContextMenu(page, findBranchRow(page, "oauth"))
    await expect(page.getByRole("menuitem", { name: /^Push Stack/ })).toHaveCount(0)
    await expect(page.getByRole("menuitem", { name: "Push P" })).toHaveAttribute("aria-disabled", "true")
    await page.keyboard.press("Escape")
    await openContextMenu(page, findBranchRow(page, "login"))
    await expect(page.getByRole("menuitem", { name: "Push Stack (2 branches) P" })).toBeVisible()
  })

  test("should push a parent with its children", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    const { stackPushes } = await routePushBranches(page)

    // auth alone may go; its description offers its children too
    const button = getPushButton(findBranchRow(page, "auth"))
    await expect(button).toHaveAccessibleName("Create Remote (1)")
    await button.hover()
    const alternative = page.getByTestId("push-description").getByTestId("push-alternative")
    await expect(alternative).toHaveCount(1)
    await expect(alternative).toContainText("session and 2 more branches are stacked on auth and have commits to push.")

    await alternative.getByRole("button", { name: "Push with Children (4 branches)" }).click()
    // one push of the four, parents first; the stack branch of the whole stack, on no remote and in no pull request,
    // stays local
    await expect.poll(() => stackPushes.map(push => push.map(branch => branch.branchName))).toEqual([["auth", "session", "login", "oauth"]])
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
    await expect(findBranchRow(page, "auth").getByTestId("push-placeholder")).toHaveCount(1)
  })

  test("should offer Push with Children while a sync runs and when the root is pushed", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    const { stackPushes, pushes } = await routePushBranches(page)
    const auth = findBranchRow(page, "auth")
    const button = getPushButton(auth)
    await button.click()
    await expectPushedToast(page, "auth", "1 commit · created the remote branch")
    expect(pushes.map(push => push.branchName)).toEqual(["auth"])
    // auth is on the remote, its children are not: its push stays, disabled, its description offers them
    await expect(button).toHaveAccessibleName("Push")
    await expect(button).toBeDisabled()
    await button.hover()
    const alternative = page.getByTestId("push-description").getByTestId("push-alternative")
    await expect(alternative).toContainText("session and 2 more branches are stacked on auth and have commits to push.")
    await expect(alternative.getByRole("button", { name: "Push with Children (3 branches)" })).toBeEnabled()
    await page.keyboard.press("Escape")
    await expect(alternative).toBeHidden()

    // a sync runs (its statuses of the template's remote replaced by the push's): the menu keeps the item enabled, and
    // the push waits for the sync
    await rewriteNextRemoteStatus(page, status =>
      status.branchName === "auth" ? { ...status, remoteExists: true, remoteTip: "auth-pushed", unpushedCommits: [], myUnpushedCount: 0, fastForward: true } : status,
    )
    const releaseSync = await holdCommand(page, "sync_branches")
    await page.getByTestId("sync-button").click()
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    await openContextMenu(page, auth)
    const item = page.getByRole("menuitem", { name: "Push with Children (3 branches)" })
    await expect(item).toBeEnabled()
    await item.click()
    await expect(button).toHaveAttribute("aria-busy", "true")
    expect(stackPushes).toEqual([])

    releaseSync()
    await expect.poll(() => stackPushes.map(push => push.map(branch => branch.branchName))).toEqual([["session", "login", "oauth"]])
    await expect(page.locator("[data-slot=toast]", { hasText: "Pushed session, login and oauth" })).toBeVisible()
  })

  test("should take the whole stack branch along in Push with Children of the root once its pull request is open, from the push and the menu", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    // the pull request of the whole auth stack is open: its stack branch follows the branches
    const stackPullRequest = pullRequest("auth", 12, { title: "The whole auth stack", headRefName: "user-name/stack/auth" })
    await setupRepo("stacked_branches", { pullRequests: { ...availablePullRequests(), stackPullRequests: { auth: stackPullRequest } } })
    await syncAndWaitForBranches()
    const { stackPushes } = await routePushBranches(page)
    const auth = findBranchRow(page, "auth")

    // the description of the root's push: its children and the stack branch, one more branch of the push
    await getPushButton(auth).hover()
    const alternative = page.getByTestId("push-description").getByTestId("push-alternative")
    await expect(alternative).toContainText("session and 2 more branches are stacked on auth and have commits to push. The whole stack branch user-name/stack/auth goes along.")
    await alternative.getByRole("button", { name: "Push with Children (5 branches)" }).click()
    // one atomic push, the stack branch after the branches, leased as absent: the remote doesn't have it yet
    await expect.poll(() => stackPushes.length).toBe(1)
    expect(stackPushes[0]!.map(branch => [branch.branchName, branch.wholeStack === true])).toEqual([
      ["auth", false],
      ["session", false],
      ["login", false],
      ["oauth", false],
      ["auth", true],
    ])
    expect(stackPushes[0]!.at(-1)).toMatchObject({ expectedRemote: null })
    // one toast: the four branches named, the stack branch said
    const pushed = page.locator("[data-slot=toast]").filter({ hasText: "pushed the whole stack branch" })
    await expect(pushed.locator("[data-slot=toast-title]")).toHaveText("Pushed auth, session, login and oauth")
    await expect(pushed.locator("[data-slot=toast-description]")).toHaveText("4 commits · created 4 remote branches · pushed the whole stack branch")

    // everything is on the remote now, the stack branch too: nothing to push
    await openContextMenu(page, auth)
    const disabled = page.getByRole("menuitem", { name: "Push with Children", exact: true })
    await expect(disabled).toHaveAttribute("aria-disabled", "true")
    await disabled.hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "Nothing to push" })).toHaveText("Nothing to push: the stack is on the remote")
    await page.keyboard.press("Escape")

    // a sync rebuilds the stack branch with a commit its remote lacks, the branches stay as they are on the remote
    await rewriteNextSyncEvents(page, event => {
      if (event.type === "remoteStatusUpdate") {
        return { ...event, data: pushOutcome(event.data.branchName).remoteStatus }
      }
      if (event.type === "stackBranchUpdate") {
        return { ...event, data: { ...event.data, remote: pushOutcome("auth", { unpushedCommits: ["stack-1"], remoteTip: "stack-pushed" }).remoteStatus } }
      }
      return event
    })
    await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
    // the menu: the stack branch alone, named and leased on its remote head
    await openContextMenu(page, auth)
    const item = page.getByRole("menuitem", { name: "Push with Children (the whole stack branch)" })
    await expect(item).toBeEnabled()
    await item.hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "Also pushes" })).toHaveText("Also pushes the whole stack branch user-name/stack/auth, the head of #12")
    await item.click()
    await expect.poll(() => stackPushes.length).toBe(2)
    expect(stackPushes[1]).toEqual([expect.objectContaining({ branchName: "auth", wholeStack: true, expectedRemote: "stack-pushed" })])
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
    await setupRepo("stacked_branches", { pullRequests })
    await syncAndWaitForBranches()
    const pushes = await routePush(page, {})
    const retargets = await routeCommand(page, "retarget_pull_request", () => null)
    // GitHub listed them: the menu is built of what the rows show
    await expect(findBranchRow(page, "auth").getByTestId("pull-request-chip")).toHaveText("#802")
    await expect(page.getByTestId("pull-request-chip")).toHaveCount(3)

    await openContextMenu(page, findBranchRow(page, "auth"))
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
    const toast = page.locator("[data-slot=toast]", { hasText: "Pushed auth and login" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText(
      "2 commits · created 2 remote branches · retargeted pull request #802 of auth to master · retargeted pull request #801 of login to master",
    )
    expect(retargets).toEqual([expect.objectContaining({ pullRequestNumber: 802, base: "master" }), expect.objectContaining({ pullRequestNumber: 801, base: "master" })])
    expect(pushes.map(push => push.branchName)).toEqual(["auth", "login"])
    // Activity names each retarget, with its pull request
    await page.getByTestId("activity-button").click()
    const retargeted = page.getByTestId("activity-entry").filter({ hasText: "Retargeted #801 to master" })
    await expect(retargeted.getByTestId("activity-description")).toHaveText("login · it went into auth")

    // a pull request into a release branch is left as it is: that base may be meant
    await page.keyboard.press("Escape")
    await openContextMenu(page, findBranchRow(page, "session"))
    await expect(page.getByRole("menuitem", { name: "Copy Branch Name" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Copy Retarget Command" })).toHaveCount(0)
    await page.keyboard.press("Escape")
    await getPushButton(findBranchRow(page, "session")).click()
    await expect(pushedToast(page, "session").locator("[data-slot=toast-description]")).toHaveText("1 commit · created the remote branch")
    expect(retargets).toHaveLength(2)
  })

  test.describe("Safe Push", () => {
    const START_OPTIONS = DEFAULT_CHECKS_ONLY
    /** What the one robot of the whole stack of auth lands (`describeWholeStackLanding`) */
    const WHOLE_AUTH_STACK = "Lands 4 branches · 4 commits into master in one Safe Push: auth, login, oauth and session"

    /** stacked_branches on Space, synced: the pushes answered in the page (returned), no code-owner review needed */
    async function setupStack(page: Page, setupRepo: (template: string, options: object) => Promise<unknown>, syncAndWaitForBranches: () => Promise<unknown>) {
      await setupRepo("stacked_branches", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      const pushes = await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeReviewRequirement(page, () => NO_REVIEW_NEEDED)
      return pushes
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
     * it: its popover at the root's row, and Enter starts the robot from the stack branch (`starts`, the recorded starts)
     */
    async function startWholeAuthStack(page: Page, start: Locator, starts: readonly Record<string, unknown>[]): Promise<void> {
      await start.hover()
      await expect(page.getByRole("tooltip")).toHaveText(WHOLE_AUTH_STACK)
      await start.click()
      await expectWholeAuthStackPopover(page)
      const before = starts.length
      await page.keyboard.press("Enter")
      await expect.poll(() => starts.length).toBe(before + 1)
      expect(starts.at(-1)).toMatchObject({ branchName: "auth", stack: true, mode: "rebaseAutosquash" })
    }

    test("should give the focus to the start of the popover once it can start", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // fixme: the popup keeps the focus it took before the start could start (Enter on the popup still starts)
      await setupStack(page, setupRepo, syncAndWaitForBranches)
      await routeCommandError(page, "get_stack_independence", "The trial build failed")
      await slotSafePush(page, "oauth").click()
      await expect(safePushPopover(page)).toHaveAttribute("data-decision", "confirm")
      await expect(popoverStart(page)).toBeFocused()
      await page.keyboard.press("Escape")
      await openSafePushWithKey(page, "oauth")
      await expect(popoverStart(page)).toBeFocused()
    })

    test("should land a small stack with its side branch from the root's menu, its popover at the root", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // auth forks: login and oauth on one side, session on the other; one robot of its stack branch lands all four
      await setupStack(page, setupRepo, syncAndWaitForBranches)
      const { stackPushes, release } = await routeHeldPush(page, wholeStackResultKey("auth"), 45)
      const starts = await routeCommand(page, "start_patronus_robot", () => ({ status: "error", error: { message: "Patronus is restarting" } }))
      const popover = safePushPopover(page)
      const rootSlot = findBranchRow(page, "auth").getByTestId("next-step-slot")
      const openWholeStack = async () => {
        await findBranchRow(page, "auth").click({ button: "right" })
        await page.getByRole("menuitem", { name: "Whole Stack" }).click()
      }

      // the root's menu, "Whole Stack ›": its Safe Push says what it lands, its popover opens at the root's row
      await openWholeStack()
      const item = page.getByRole("menuitem", { name: "Safe Push Whole Stack" })
      await item.hover()
      await expect(page.getByRole("tooltip")).toHaveText(WHOLE_AUTH_STACK)
      await item.click()
      await expectWholeAuthStackPopover(page)
      // Enter: only the stack branch is pushed, its progress on the push of the root's row, where the pushes of a small
      // stack show; then the robot starts from it
      await page.keyboard.press("Enter")
      const rootPush = findBranchRow(page, "auth").getByTestId("push-button")
      await expect(rootPush).toHaveAccessibleName("Pushing the whole stack 45%")
      await expect(rootPush).toHaveAttribute("aria-busy", "true")
      await rootPush.hover()
      await expect(page.getByTestId("push-description").getByTestId("push-progress")).toHaveText("Writing objects:  45% (2/4)")
      await page.mouse.move(0, 0)
      // no branch of the stack is pushed
      await expect(findBranchRow(page, "login").getByTestId("push-button")).toBeVisible()
      await expect(findBranchRow(page, "login").getByTestId("push-button")).not.toHaveAttribute("aria-busy", "true")
      expect(starts).toEqual([])
      release()
      await expect.poll(() => starts.length).toBe(1)
      expect(starts[0]).toMatchObject({ branchName: "auth", stack: true, mode: "rebaseAutosquash" })
      expect(stackPushes).toEqual([[expect.objectContaining({ branchName: "auth", wholeStack: true, expectedRemote: null })]])
      await expect(rootPush).not.toHaveAttribute("aria-busy", "true")
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
      await popoverStart(page).click()
      await expect.poll(() => starts.length).toBe(2)
      expect(starts[1]).toMatchObject({ branchName: "auth", stack: true, mode: "dryRun" })
    })

    test("should offer the whole stack in the Safe Push popover of its root and of its branches, which opens its own popover", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      await setupStack(page, setupRepo, syncAndWaitForBranches)
      await routePushBranches(page, {})
      const starts = await routeCommand(page, "start_patronus_robot", () => ({ status: "error", error: { message: "Patronus is restarting" } }))
      const popover = safePushPopover(page)
      const rootSlot = findBranchRow(page, "auth").getByTestId("next-step-slot")

      // the root's popover lands auth alone, the branches built on it stay; the whole stack is its second way
      await openSafePushWithKey(page, "auth")
      await expect(popover).toHaveAttribute("aria-label", "Safe Push of auth")
      const option = popover.getByTestId("safe-push-whole-stack-option")
      await expect(option).toContainText("Land the whole stack:")
      // its own popover, at the root's row: what the whole stack lands, Enter starts it from its stack branch
      await startWholeAuthStack(page, option.getByRole("button", { name: "Safe Push Whole Stack" }), starts)
      await closeRefusal(page)

      // a side branch of the stack that forks offers it too
      await slotSafePush(page, "session").click()
      await expect(popover).toHaveAttribute("aria-label", "Safe Push of session")
      await popover.getByTestId("safe-push-whole-stack-option").getByRole("button", { name: "Safe Push Whole Stack" }).click()
      await expect(popover).toHaveAttribute("aria-label", "Safe Push of the whole auth stack")
      await expectPopoverAtSlot(popover, rootSlot)
      await popoverStart(page).click()
      await expect.poll(() => starts.length).toBe(2)
      expect(starts[1]).toMatchObject({ branchName: "auth", stack: true })
    })

    test("should ask for the approvals of the whole stack of a small stack while its root's row shows, and offer its pull request where they are missing", async ({
      page,
      setupRepo,
      syncAndWaitForBranches,
    }) => {
      const repo = await setupRepo("stacked_branches", { patronus: { ...availableRobots(), githubRepository: "acme/monorepo" }, pullRequests: availablePullRequests() })
      await repo.setReviewAnswers({ answers: { "stack:auth": AWAITING_CODE_OWNER } })
      const missing = "Code owners must approve in a pull request: IJPL Code Platform"
      // the row of the root asks Patronus for the whole stack, before any surface of it opens; that answer waits
      const asksForStack = (postData: string | null) => (postData ?? "").includes('"branchName":"stack:auth"')
      const asked = page.waitForRequest(request => request.url().endsWith("/invoke/get_branch_review_readiness") && asksForStack(request.postData()))
      let release!: () => void
      const held = new Promise<void>(resolve => (release = resolve))
      await page.route("**/invoke/get_branch_review_readiness", async route => {
        if (asksForStack(route.request().postData())) {
          await held
        }
        await route.fallback()
      })
      await syncAndWaitForBranches()
      await asked

      // a click before the answer waits for it, then says why, with the pull request where the approvals count
      await findBranchRow(page, "auth").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Whole Stack" }).click()
      await page.getByRole("menuitem", { name: "Safe Push Whole Stack" }).click()
      release()
      const notice = page.locator("[data-slot=toast]").filter({ hasText: "Safe Push of the whole auth stack needs a pull request" })
      await expect(notice.locator("[data-slot=toast-description]")).toHaveText("Code owners must approve in it: IJPL Code Platform")
      await notice.getByRole("button", { name: "Create Pull Request of the Stack…" }).click()
      const dialog = createPullRequestDialog(page)
      await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("Whole auth stack → master")
      await page.keyboard.press("Escape")
      await expect(dialog).toHaveCount(0)

      // now the menu knows: its Safe Push disabled with the reason, its pull request offered
      await findBranchRow(page, "auth").click({ button: "right" })
      await page.getByRole("menuitem", { name: "Whole Stack" }).click()
      const safePush = page.getByRole("menuitem", { name: "Safe Push Whole Stack" })
      await expect(safePush).toHaveAttribute("aria-disabled", "true")
      await safePush.hover()
      await expect(page.getByRole("tooltip")).toHaveText(missing)
      await page.getByRole("menuitem", { name: "Create Pull Request of the Stack…" }).click()
      await expect(dialog.getByTestId("create-pull-request-base")).toHaveText("Whole auth stack → master")
      await page.keyboard.press("Escape")

      // and the root's popover: the whole stack disabled, the reason under it
      await openSafePushWithKey(page, "auth")
      const option = safePushPopover(page).getByTestId("safe-push-whole-stack-option")
      await expect(option.getByRole("button", { name: "Safe Push Whole Stack" })).toBeDisabled()
      await expect(option.getByTestId("safe-push-whole-stack-reason")).toHaveText(missing)
    })

    test("should land a stacked branch with what it is built on in one Safe Push, Enter starts it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const pushes = await setupStack(page, setupRepo, syncAndWaitForBranches)
      // each start reaches Patronus and is refused: no robot holds the start of the next surface
      const starts = await routeCommand(page, "start_patronus_robot", () => ({ status: "error", error: { message: "Patronus is restarting" } }))
      // without the trial build of oauth alone, the landing is the only way
      await routeCommandError(page, "get_stack_independence", "The trial build failed")
      const oauthRow = findBranchRow(page, "oauth")
      /** Enter starts the robot of oauth, which lands auth and login with it; the popover closes, the refusal is told */
      const startWithEnter = async (count: number) => {
        const popover = await expectPopoverOf(page, "oauth")
        await page.keyboard.press("Enter")
        await expect.poll(() => starts.length).toBe(count)
        expect(starts[count - 1]).toMatchObject({ branchName: "oauth", mode: "rebaseAutosquash" })
        await expect(popover).toHaveCount(0)
        await closeRefusal(page)
      }

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
      await startWithEnter(1)
      // the stack is pushed first, root first, a step of the start
      expect(pushes.map(push => push.branchName)).toEqual(["auth", "login", "oauth"])

      // "⋯" of the row
      await oauthRow.getByRole("button", { name: "More actions for oauth" }).click()
      await page.getByRole("menuitem", { name: /^Safe Push · 3/ }).click()
      await startWithEnter(2)

      // Safe Push With Profile… of the branch menu: no profile but the default checks
      await openSafePushWithProfile(page, "oauth")
      await startWithEnter(3)

      // S on the focused row
      await openSafePushWithKey(page, "oauth")
      await startWithEnter(4)

      // the footer of the preview, which closes for the popover of the row
      let preview = await openBranchPreview(page, "oauth")
      await preview.getByRole("button", { name: /Safe Push · 3/ }).click()
      await expect(preview).toHaveCount(0)
      await startWithEnter(5)
      // S in the preview
      preview = await openBranchPreview(page, "oauth")
      await page.keyboard.press("s")
      await expect(preview).toHaveCount(0)
      await startWithEnter(6)

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
      await startWithEnter(7)
      // the stack is on the remote now: no start pushed again
      expect(pushes.map(push => push.branchName)).toEqual(["auth", "login", "oauth"])
    })

    test("should say in the Safe Push popover of a branch in the middle what lands and what stays", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupStack(page, setupRepo, syncAndWaitForBranches)
      await routeCommandError(page, "get_stack_independence", "The trial build failed")

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
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeReviewRequirement(page, () => NO_REVIEW_NEEDED)
      await routeCommandError(page, "get_stack_independence", "The trial build failed")
      const retargets = await routeCommand(page, "retarget_pull_request", () => null)
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
      await clickContextMenuItem(page, "Check Approvals Again")
      const slot = oauthRow.getByTestId("next-step-slot")
      await expect(slot).toHaveAttribute("data-step", "retarget")
      const retarget = slot.getByTestId("next-step")
      await expect(retarget).toHaveText("Retarget #8 to master")
      await retarget.hover()
      await expect(page.getByTestId("next-step-hover")).toHaveText(
        "Approvals in #8 don't count: its base is user-name/virtual/login, a virtual branch. Changes the base of #8 from user-name/virtual/login to master on GitHub: Patronus merges a pull request into its base",
      )
      // no start from it: S says why, the menu disables the start with the reason
      await oauthRow.focus()
      await page.keyboard.press("s")
      const notice = page.locator("[data-slot=toast]", { hasText: "Approvals in #8 of oauth don't count" })
      await expect(notice.locator("[data-slot=toast-description]")).toHaveText("Its base is user-name/virtual/login, not master")
      await expect(safePushPopover(page)).toHaveCount(0)
      await openContextMenu(page, oauthRow)
      const item = page.getByRole("menuitem", { name: /^Safe Push · 3/ })
      await expect(item).toHaveAttribute("aria-disabled", "true")
      await item.hover()
      await expect(page.getByRole("tooltip").filter({ hasText: "don't count" }).last()).toHaveText("Approvals in #8 don't count: its base is user-name/virtual/login")
      await page.keyboard.press("Escape")

      await retarget.click()
      await expect.poll(() => retargets).toEqual([expect.objectContaining({ pullRequestNumber: 8, base: "master" })])
      await expect(page.locator("[data-slot=toast]", { hasText: "Retargeted #8 to master" })).toBeVisible()
      // into master again: its approvals count, the start comments on it
      await expect(slot).toHaveAttribute("data-step", "safePush")
      await slotSafePush(page, "oauth").click()
      popover = await expectPopoverOf(page, "oauth")
      await expect(popover.getByTestId("safe-push-from-pull-request")).toHaveText("Comments /patronus merge on #8: Patronus reports there and closes it once merged.")
    })

    test("should wait for the Safe Push of an ancestor, which lands its commits", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRepo("stacked_branches", { patronus: availableRobots(robot("login", "login-robot")) })
      await syncAndWaitForBranches()
      const pushes = await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      const starts = await routeCommand(page, "start_patronus_robot", () => robot("oauth", "oauth-dry-run", { pushMode: "dryRun", sourceRevision: null }))
      await expect(findBranchRow(page, "login").getByTestId("patronus-chip")).toBeVisible()
      const reason = "A Safe Push of login is running: wait until it merges"

      await openContextMenu(page, findBranchRow(page, "oauth"))
      const item = page.getByRole("menuitem", { name: /^Safe Push · 3/ })
      await expect(item).toHaveAttribute("aria-disabled", "true")
      await item.hover()
      await expect(page.locator("[data-slot=tooltip-content]", { hasText: reason })).toBeVisible()
      // a dry run pushes nothing into master: it runs, with what oauth is built on
      await page.getByRole("menuitem", { name: "Dry Run" }).click()
      const popover = await expectPopoverOf(page, "oauth", "Dry Run")
      await expect(popover).toHaveAttribute("data-mode", "dryRun")
      await expect(popover.getByTestId("safe-push-lands")).toHaveText("Checks oauth with auth and login, which it is built on: 3 branches · 3 commits.")
      await page.keyboard.press("Enter")
      await expect.poll(() => starts.length).toBe(1)
      expect(starts[0]).toMatchObject({ branchName: "oauth", mode: "dryRun" })
      // the robot of login holds the pushes of auth and login: oauth goes alone, its branch has their commits
      expect(pushes.map(push => push.branchName)).toEqual(["oauth"])
      await expect(page.locator("[data-slot=toast]", { hasText: "Dry Run started" })).toBeVisible()

      // S says why the Safe Push waits, and opens nothing
      await findBranchRow(page, "oauth").focus()
      await page.keyboard.press("s")
      await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push of oauth" }).locator("[data-slot=toast-description]")).toHaveText(reason)
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
      await expect(page.getByRole("tooltip", { name: "Lands with oauth: its Safe Push runs" })).toBeVisible()
      await page.mouse.move(0, 0)
      for (const name of ["auth", "session", "login", "oauth"]) {
        await expect(findBranchRow(page, name).getByTestId("landing-lanes")).toHaveCount(1)
      }
      await expect(findBranchRow(page, "docs").getByTestId("landing-lanes")).toHaveCount(0)

      // a branch the Safe Push lands offers no push and no Safe Push of its own: the Status cell says why, the menu too
      const loginRow = findBranchRow(page, "login")
      const reason = "A Safe Push of oauth is running: it lands login with it"
      await expect(loginRow.getByTestId("next-step-slot")).toHaveAttribute("data-step", "none")
      await expect(getPushButton(loginRow)).toHaveCount(0)
      await openContextMenu(page, loginRow)
      for (const name of [/^Safe Push · 2/, /^Push P$/]) {
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
      // a dry run pushes nothing into master
      await openContextMenu(page, findBranchRow(page, "auth"))
      await expect(page.getByRole("menuitem", { name: /^Safe Push S/ })).toHaveAttribute("aria-disabled", "true")
      await expect(page.getByRole("menuitem", { name: "Dry Run" })).not.toHaveAttribute("aria-disabled", "true")
    })

    test("should say which branches a merged Safe Push landed until the update, and offer no Safe Push or push of them", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const repo = await setupRepo("stacked_branches", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeReviewRequirement(page, () => NO_REVIEW_NEEDED)
      await routeCommandError(page, "get_stack_independence", "The trial build failed")
      const started = robot("oauth", "oauth-robot", { sourceRevision: null })
      const starts = await routeCommand(page, "start_patronus_robot", () => started)
      const authRow = findBranchRow(page, "auth")
      const loginRow = findBranchRow(page, "login")

      // each branch offers its own Safe Push, with what it is built on
      await expect(slotSafePush(page, "auth")).toHaveText("Safe Push")
      await expect(slotSafePush(page, "login")).toHaveText("Safe Push · 2")
      await expect(slotSafePush(page, "session")).toHaveText("Safe Push · 2")
      // the Safe Push of oauth lands auth and login with it, in one robot
      await slotSafePush(page, "oauth").click()
      await expectPopoverOf(page, "oauth")
      await page.keyboard.press("Enter")
      await expect.poll(() => starts.length).toBe(1)
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

    test("should offer to land only a stacked branch that applies alone", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      const pushes = await setupStack(page, setupRepo, syncAndWaitForBranches)
      const starts = await routeCommand(page, "start_patronus_robot", () => robot("oauth", "oauth-robot", { sourceRevision: null }))

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
      await expect.poll(() => tree(page)).toContain("oauth@0")
      expect(starts).toHaveLength(0)
      await popoverStart(page).click()
      await expect.poll(() => starts.length).toBe(1)
      expect(starts[0]).toMatchObject({ branchName: "oauth", mode: "rebaseAutosquash" })
      await expect(popover).toHaveCount(0)
      // only oauth lands, so only oauth is pushed
      expect(pushes.map(push => push.branchName)).toEqual(["oauth"])
      // the toast of the start can cancel it
      const started = page.locator("[data-slot=toast]", { hasText: "Safe Push started" })
      await expect(started.getByRole("button", { name: "Open in Patronus" })).toBeVisible()
      await expect(started.getByRole("button", { name: "Cancel Safe Push" })).toBeVisible()
    })

    test("should warn that a stacked branch that applies alone uses a name its parent adds, the unstack still offered", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // lane-ui is stacked on lane-core by the stack parents, with the symbols on; its commit calls laneDocument()
      await setupRepo("stack_uses_parent", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)

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
      await setupRepo("stack_push_alone", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)

      await slotSafePush(page, "perf-part-2").click()
      const popover = await expectPopoverOf(page, "perf-part-2")
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
      await verifyCommitCount(findBranchRow(page, "perf-part-2"), "2 commits")
      // the toast of the move offers the unstack too, never does it
      const moved = page.locator("[data-slot=toast]", { hasText: "Moved 1 commit to perf-part-2" })
      await expect(moved.getByRole("button", { name: "Unstack from IJPL-257337" })).toBeVisible()
      await expect(alone).toHaveAttribute("data-action", "unstack")
      await expect(alone).toContainText("To land perf-part-2 alone: it applies on the baseline without IJPL-257337 and uses no name it adds. Not checked by a compiler.")
      expect(await tree(page)).toEqual(["IJPL-257337@0", "perf-part-2@1"])

      // the review of the new tip waits: until it answers, the popover says so and shows no review of the whole stack
      let releaseReview = () => {}
      const reviewHeld = new Promise<void>(resolve => {
        releaseReview = resolve
      })
      await page.route("**/invoke/get_branch_review_readiness", async route => {
        await reviewHeld
        await route.fallback()
      })
      await alone.getByRole("button", { name: "Unstack from IJPL-257337" }).click()
      // the popover shows itself again, without a start: a root with its 2 commits, reviewed alone
      await expect(popover.getByTestId("safe-push-lands")).toHaveText("Lands perf-part-2 into master: 2 commits.")
      await expect(popover.getByTestId("safe-push-status")).toHaveText("Checking the code-owner review…")
      await expect(review).toHaveCount(0)
      releaseReview()
      // two roots now, in the order of their latest commits
      expect((await tree(page)).toSorted()).toEqual(["IJPL-257337@0", "perf-part-2@0"])
      await verifyCommitCount(findBranchRow(page, "perf-part-2"), "2 commits")
      await expect(review).toContainText("Needs review by the code owners: a Safe Push fails without these approvals.")
      await expect(groups).toHaveText([/^UI Team/])
      await expect(popoverStart(page)).toBeEnabled()
    })
  })
})
