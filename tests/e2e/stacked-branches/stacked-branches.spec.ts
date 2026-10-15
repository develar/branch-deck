// @spec spec/branch/stacked-branches.spec.md
import type { Locator, Page } from "@playwright/test"
import type { PatronusReviewRequirement, PatronusStartOptions } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getPushButton, verifyCommitCount } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { inlineBranchCreator } from "../helpers/inline-form-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import { availablePullRequests, getOpenedUrls, pullRequest } from "../helpers/pull-request-helpers"
import { pushedToast, routePush } from "../helpers/push-helpers"
import { clickContextMenuItem, clickGroupIntoBranchButton, openContextMenu, selectCommit } from "../helpers/selection-helpers"
import { routeCommand } from "../helpers/wait-helpers"

const GITHUB_COMPARE = "https://github.com/JetBrains/ultimate/compare"

/** `name@depth` of the branch rows in display order */
async function tree(page: Page): Promise<string[]> {
  return page.getByTestId("branch-row").evaluateAll(rows => rows.map(row => `${row.getAttribute("data-branch-name")}@${row.getAttribute("data-depth") ?? 0}`))
}

/** The x of each tree rule in a row, outermost first */
async function ruleXs(row: Locator): Promise<number[]> {
  return row.getByTestId("nesting-rule").evaluateAll(rules => rules.map(rule => Math.round(rule.getBoundingClientRect().left)))
}

async function boxOf(locator: Locator): Promise<{ x: number, y: number, width: number, height: number }> {
  const box = await locator.boundingBox()
  expect(box).not.toBeNull()
  return box!
}

test.describe("Stacked branches", () => {
  test("should show the branches of a stack under their parents", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    await verifyCommitCount(findBranchRow(page, "login"), "1 commit")

    expect(await tree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
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

  test("should continue the commit rail of a parent into the rule of its children", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    await expandBranch(page, findBranchRow(page, "auth"))
    await expandBranch(page, findBranchRow(page, "login"))

    const authRail = await boxOf(getBranchDetailsRow(page, "auth").getByTestId("commit-rail"))
    const loginRail = await boxOf(getBranchDetailsRow(page, "login").getByTestId("commit-rail"))
    const railX = Math.round(authRail.x)
    const loginRailX = Math.round(loginRail.x)
    // session and login hang from the rail of auth, oauth from the rail of login, one step further
    expect(await ruleXs(findBranchRow(page, "session"))).toEqual([railX])
    expect(await ruleXs(findBranchRow(page, "login"))).toEqual([railX])
    expect(await ruleXs(findBranchRow(page, "oauth"))).toEqual([railX, loginRailX])
    // the commits of login keep the rule of its level beside them
    expect(await ruleXs(getBranchDetailsRow(page, "login"))).toEqual([railX])
    expect(loginRailX - railX).toBe(28)
    // a rail runs to the bottom of its cell, into the rule of the first child
    const session = await boxOf(findBranchRow(page, "session"))
    expect(Math.abs(authRail.y + authRail.height - session.y)).toBeLessThanOrEqual(1)
    const oauth = await boxOf(findBranchRow(page, "oauth"))
    expect(Math.abs(loginRail.y + loginRail.height - oauth.y)).toBeLessThanOrEqual(1)
    // the commit subjects start at the branch name
    const name = await boxOf(findBranchRow(page, "login").getByText("login", { exact: true }))
    const subject = await boxOf(getBranchCommitRow(page, "login", "Add login form").getByText("Add login form", { exact: true }))
    expect(Math.abs(subject.x - name.x)).toBeLessThanOrEqual(1)
  })

  test("should block the children of a parent that doesn't build", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_conflict")
    await syncAndWaitForBranches()

    const loginRow = findBranchRow(page, "login")
    await expect(loginRow.getByTestId("branch-status")).toHaveText("blocked by auth")
    await expect(findBranchRow(page, "auth").getByTestId("branch-status")).toHaveText("merge conflict")
    await expect(getPushButton(loginRow)).toBeDisabled()
    await expect(getPushButton(loginRow)).toHaveText("Push")

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
    await clickContextMenuItem(page, "oauth")
    await expect(page.getByText("Stacked docs on oauth")).toBeVisible()
    await expect.poll(() => tree(page)).toEqual(["auth@0", "session@1", "login@1", "oauth@2", "docs@3"])

    await openContextMenu(page, findBranchRow(page, "docs"))
    await clickContextMenuItem(page, "Unstack from oauth")
    await expect.poll(() => tree(page)).toEqual(["docs@0", "auth@0", "session@1", "login@1", "oauth@2"])
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
    const input = page.getByTestId("inline-rename-input").locator("input[type=\"text\"]")
    await input.fill("auth/core")
    // the message shows once the form was submitted
    await input.press("Enter")
    await expect(page.getByText("A branch name has no /")).toBeVisible()
    await input.fill("identity")
    await expect(page.getByText("session, login, oauth stay stacked on it")).toBeVisible()
    await input.press("Enter")

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
    await page.keyboard.press("Enter")

    // not blocked: built on the baseline, and the row says so
    const oauth = findBranchRow(page, "oauth")
    await expect(oauth.getByTestId("stack-base")).toHaveText("on master")
    await expect(oauth).not.toContainText("blocked by")
    await expect(getPushButton(oauth)).toBeEnabled()
    await expandBranch(page, oauth)
    const alert = getBranchDetailsRow(page, "oauth").getByTestId("branch-base-changed")
    await expect(alert).toContainText("Built on master: login has no commits. The pull request base is master.")

    // the user rewrites the prefixes, never the sync
    await alert.getByRole("button", { name: "Unstack from login" }).click()
    await expect.poll(() => tree(page)).toContain("oauth@0")
    await expect(findBranchRow(page, "oauth").getByTestId("stack-base")).toHaveCount(0)
    await expect(getBranchDetailsRow(page, "oauth").getByTestId("branch-base-changed")).toHaveCount(0)
  })

  test("should move commits to a new branch on a parent with Move to…", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    await expandBranch(page, findBranchRow(page, "docs"))
    await selectCommit(page, getBranchCommitRow(page, "docs", "Document the login"))
    await clickGroupIntoBranchButton(page)
    await inlineBranchCreator.waitForVisible(page)
    const portal = inlineBranchCreator.getPortal(page)
    await inlineBranchCreator.getInput(page).fill("auth/guide")
    await expect(portal).toContainText("Creates guide on auth with 1 commit")
    await inlineBranchCreator.getInput(page).fill("login")
    await expect(portal).toContainText("Moves 1 commit to login (on auth)")
    await inlineBranchCreator.getInput(page).fill("auth/guide")
    await inlineBranchCreator.getInput(page).press("Enter")

    await expect.poll(() => tree(page)).toEqual(["auth@0", "guide@1", "session@1", "login@1", "oauth@2"])
    await expect(findBranchRow(page, "guide")).toHaveAttribute("data-state", "open")
  })

  test("should push a stack parents first and name the pull request base", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // on GitHub, no pull request yet
    await setupRepo("stacked_branches", { pullRequests: availablePullRequests() })
    await syncAndWaitForBranches()
    // the E2E server has no remote: answer the pushes and record their order
    const pushes = await routePush(page, {})

    // login isn't pushed yet: the button of oauth pushes the stack, its description offers oauth alone; the row says what it is built on
    const oauth = findBranchRow(page, "oauth")
    await expect(oauth.getByTestId("stack-base")).toHaveText("on login")
    await expect(findBranchRow(page, "auth").getByTestId("stack-base")).toHaveCount(0)
    const button = getPushButton(oauth)
    await expect(button).toHaveText("Push Stack (3)")
    await expect(button).toBeEnabled()
    await button.hover()
    const description = page.getByTestId("push-description")
    await expect(description).toContainText("Pushes 3 branches, parents first: auth · 1 commit · creates the remote branch login · 1 commit · creates the remote branch oauth · 1 commit · creates the remote branch")
    const alternative = description.getByTestId("push-alternative")
    await expect(alternative).toContainText("login isn't on the remote yet. Pushing oauth alone works; a pull request includes login's commits until login is pushed.")
    await expect(alternative.getByRole("button", { name: "Push oauth Only" })).toBeVisible()

    await button.click()
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["auth", "login", "oauth"])
    expect(pushes.map(push => push.parentBranch ?? null)).toEqual([null, "auth", "login"])
    // the parents are on the remote: nothing to choose any more
    await expect(oauth.getByTestId("stack-base")).toHaveCount(0)
    // one toast for the stack, as Activity has an entry per branch; it creates the pull request of oauth into login
    const toast = page.locator("[data-slot=toast]", { hasText: "Pushed auth, login and oauth" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("3 commits · created 3 remote branches")
    await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
    await toast.getByRole("button", { name: "Create Pull Request" }).click()
    await expect.poll(() => getOpenedUrls(page)).toEqual([`${GITHUB_COMPARE}/user-name/virtual/login...user-name/virtual/oauth?expand=1`])

    await openContextMenu(page, findBranchRow(page, "auth"))
    await clickContextMenuItem(page, "Copy PR Base")
    await expect.poll(() => readClipboard(page)).toBe("master")
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
    // the parents still have work for the remote: the menu pushes them with oauth
    await openContextMenu(page, findBranchRow(page, "oauth"))
    await expect(page.getByRole("menuitem", { name: "Push Stack (3 branches)" })).toBeVisible()
  })

  test("should push a parent with its children", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    const pushes = await routePush(page, {})

    // auth alone may go; its description offers its children too
    const button = getPushButton(findBranchRow(page, "auth"))
    await expect(button).toHaveText("Create Remote (1)")
    await button.hover()
    const alternative = page.getByTestId("push-description").getByTestId("push-alternative")
    await expect(alternative).toHaveCount(1)
    await expect(alternative).toContainText("session and 2 more branches are stacked on auth and have commits to push.")

    await alternative.getByRole("button", { name: "Push with Children (4 branches)" }).click()
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["auth", "session", "login", "oauth"])
    // every branch is on the remote: the menu offers nothing more to push with auth
    await openContextMenu(page, findBranchRow(page, "auth"))
    await expect(page.getByRole("menuitem", { name: "Stack On…" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: /^Push with Children/ })).toHaveCount(0)
  })

  test("should retarget a pull request whose base is not the expected one before the push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the pull request of auth targets the branch it was stacked on before; the one of login targets master
    const pullRequests = availablePullRequests(
      pullRequest("auth", 802, { baseRefName: "user-name/virtual/core" }),
      pullRequest("login", 801, { baseRefName: "master" }),
    )
    await setupRepo("stacked_branches", { pullRequests })
    await syncAndWaitForBranches()
    const pushes = await routePush(page, {})
    const retargets = await routeCommand(page, "retarget_pull_request", () => null)

    // auth isn't on the remote: master is the right base of login until auth is pushed, GitHub has no auth to target
    await openContextMenu(page, findBranchRow(page, "login"))
    await expect(page.getByRole("menuitem", { name: "Copy PR Base" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Copy Retarget Command" })).toHaveCount(0)
    await page.keyboard.press("Escape")

    await openContextMenu(page, findBranchRow(page, "auth"))
    await clickContextMenuItem(page, "Copy Retarget Command")
    await expect.poll(() => readClipboard(page)).toBe("gh pr edit 802 --base master")

    // the push retargets it first: pushed onto master, a pull request into core would show the wrong commits
    await getPushButton(findBranchRow(page, "auth")).click()
    const toast = page.locator("[data-slot=toast]", { hasText: "Pushed auth" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText("1 commit · created the remote branch · retargeted pull request #802 to master")
    expect(retargets).toEqual([expect.objectContaining({ pullRequestNumber: 802, base: "master" })])
    expect(pushes.map(push => push.branchName)).toEqual(["auth"])
    await expect(toast.getByRole("button", { name: "Open Pull Request" })).toBeVisible()
  })

  test.describe("Safe Push", () => {
    const START_OPTIONS: PatronusStartOptions = { targetBranch: "master", supported: true, profiles: [], defaultProfileId: null }
    const NO_REVIEW: PatronusReviewRequirement = { required: false, mergeRequest: null, groups: [], skipped: false }

    async function openSafePush(page: Page, branchName: string): Promise<Locator> {
      await openContextMenu(page, findBranchRow(page, branchName))
      await clickContextMenuItem(page, "Safe Push…")
      return page.getByTestId("patronus-start-form")
    }

    test("should lead a stacked branch root first, and land the stack in one Safe Push when told to", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRepo("stacked_branches", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      const pushes = await routePush(page, {})
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeCommand(page, "get_patronus_review_requirement", () => NO_REVIEW)
      const starts = await routeCommand(page, "start_patronus_robot", () => robot("oauth", "oauth-robot", { sourceRevision: null }))

      const form = await openSafePush(page, "oauth")
      await expect(form.getByTestId("patronus-start-stack-lead")).toContainText("oauth is built on auth and login, which aren't in master yet. Safe Push auth first, then the next: after a merge, the next sync moves the branches built on it onto master.")
      await expect(form.getByTestId("patronus-start-stack-note")).toHaveText("Or start oauth now: the Safe Push lands auth and login with it.")
      await expect(form.getByTestId("patronus-start-push-note")).toHaveText("auth, login and oauth have commits the remote lacks: they are pushed first, parents first, then Patronus starts from the remote branch of oauth.")

      await form.getByRole("button", { name: "Push Stack and Start Safe Push" }).click()
      await expect.poll(() => starts.length).toBe(1)
      expect(pushes.map(push => push.branchName)).toEqual(["auth", "login", "oauth"])
      expect(starts[0]).toMatchObject({ branchName: "oauth", mode: "rebaseAutosquash" })
      // the pushes are a step of the start: one action, one toast
      await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push started" })).toBeVisible()
      await expect(page.locator("[data-slot=toast]")).toHaveCount(1)
    })

    test("should open the Safe Push of the root from the lead, and say what its merge does to its children", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRepo("stacked_branches", { patronus: availableRobots() })
      await syncAndWaitForBranches()
      await routeCommand(page, "get_patronus_start_options", () => START_OPTIONS)
      await routeCommand(page, "get_patronus_review_requirement", () => NO_REVIEW)

      const form = await openSafePush(page, "oauth")
      await form.getByRole("button", { name: "Safe Push auth…" }).click()
      await expect(form).toContainText("Safe Push of auth")
      await expect(form.getByTestId("patronus-start-stack-lead")).toHaveCount(0)
      await expect(form.getByTestId("patronus-start-stack-note")).toHaveText(/^.+ are built on auth: after the merge, the next sync moves them onto master\.$/)
      await expect(form.getByRole("button", { name: "Push and Start Safe Push" })).toBeVisible()
    })

    test("should wait for the Safe Push of an ancestor, which lands its commits", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      await setupRepo("stacked_branches", { patronus: availableRobots(robot("login", "login-robot")) })
      await syncAndWaitForBranches()
      await expect(findBranchRow(page, "login").getByTestId("patronus-chip")).toBeVisible()

      await openContextMenu(page, findBranchRow(page, "oauth"))
      const item = page.getByRole("menuitem", { name: /^Safe Push…/ })
      await expect(item).toHaveAttribute("aria-disabled", "true")
      await item.hover()
      await expect(page.locator("[data-slot=tooltip-content]", { hasText: "A Safe Push of login is running: wait until it merges" })).toBeVisible()
      // a dry run pushes nothing into master
      await expect(page.getByRole("menuitem", { name: /^Dry Run…/ })).not.toHaveAttribute("aria-disabled", "true")
    })
  })
})
