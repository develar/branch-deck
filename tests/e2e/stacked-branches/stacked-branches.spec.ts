// @spec spec/branch/stacked-branches.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getPushButton, verifyCommitCount } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { inlineBranchCreator } from "../helpers/inline-form-helpers"
import { clickContextMenuItem, clickGroupIntoBranchButton, openContextMenu, selectCommit } from "../helpers/selection-helpers"

/** `name@depth` of the branch rows in display order */
async function tree(page: Page): Promise<string[]> {
  return page.getByTestId("branch-row").evaluateAll(rows => rows.map(row => `${row.getAttribute("data-branch-name")}@${row.getAttribute("data-depth") ?? 0}`))
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

  test("should block the children of a parent that doesn't build", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_conflict")
    await syncAndWaitForBranches()

    const loginRow = findBranchRow(page, "login")
    await expect(loginRow).toContainText("blocked by auth")
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

  test("should confirm ungrouping a parent and list its children", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()

    await openContextMenu(page, findBranchRow(page, "login"))
    await clickContextMenuItem(page, "Ungroup")
    const note = page.getByTestId("stack-children-note")
    await expect(note).toContainText("1 branch is stacked on login")
    await expect(note).toContainText("oauth will be blocked until login has commits again")
    await page.getByRole("button", { name: "Ungroup" }).click()

    await expect(findBranchRow(page, "oauth")).toContainText("blocked by login")
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
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    // the E2E server has no remote: answer the pushes and record their order
    const pushes: { branchName: string, parentBranch?: string | null }[] = []
    await page.route("**/invoke/push_branch", async (route) => {
      const params = route.request().postDataJSON() as { branchName: string, parentBranch?: string | null }
      pushes.push({ branchName: params.branchName, parentBranch: params.parentBranch })
      await route.fulfill({ json: { branchName: params.branchName, remoteExists: true, unpushedCommits: [], commitsBehind: 0, myUnpushedCount: 0, fastForward: true, lastPushTime: 1704139200 } })
    })

    // not pushed yet: the parent goes first
    await getPushButton(findBranchRow(page, "oauth")).hover()
    await expect(page.getByText("Push login first: its commits aren't on the remote yet.")).toBeVisible()

    await openContextMenu(page, findBranchRow(page, "oauth"))
    await clickContextMenuItem(page, "Push Stack (3 branches)")
    await expect.poll(() => pushes.map(push => push.branchName)).toEqual(["auth", "login", "oauth"])
    expect(pushes.map(push => push.parentBranch ?? null)).toEqual([null, "auth", "login"])
    await expect(page.getByText("Pull request base: login")).toBeVisible()
    // the toasts of the stack pile up; hovering expands them
    const toast = page.locator("[data-slot=toast]", { hasText: "Pull request base: login" })
    await toast.hover()
    await toast.getByRole("button", { name: "Copy gh pr create" }).click()
    expect(await readClipboard(page)).toBe("gh pr create --base user-name/virtual/login --head user-name/virtual/oauth")

    await openContextMenu(page, findBranchRow(page, "auth"))
    await clickContextMenuItem(page, "Copy PR Base")
    await expect.poll(() => readClipboard(page)).toBe("master")
  })
})
