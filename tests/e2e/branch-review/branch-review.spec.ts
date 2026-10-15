// @spec spec/branch/branch-review.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { getOpenedSubWindows, openSubWindowPage } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import type { BranchReviewWindowData } from "../../../src/types/sub-window"

/** The paths of the diffs the review shows, in the order of the tree: directories first */
async function reviewedPaths(page: Page): Promise<string[]> {
  return page.getByTestId("review-file-header").evaluateAll(headers => headers.map(header => header.getAttribute("data-path")!))
}

/** `name:count` of the owner chips */
async function ownerChips(page: Page): Promise<string[]> {
  return page.getByTestId("branch-review-owner").evaluateAll(chips => chips.map(chip => chip.textContent!))
}

async function openReviewOf(page: Page, branch: string): Promise<BranchReviewWindowData> {
  await findBranchRow(page, branch).click({ button: "right" })
  await page.getByRole("menuitem", { name: "Review Changes" }).click()
  const [opened] = await getOpenedSubWindows<BranchReviewWindowData>(page)
  expect(opened).toMatchObject({ windowId: "branch-review", url: "/branch-review", title: `Review ${branch}` })
  await openSubWindowPage(page, opened!)
  await expect(page.getByTestId("review-file-header").first()).toBeVisible()
  return opened!.data
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

    // the parent alone, then extended to the reviewed branch: the whole stack from the baseline
    await page.getByTestId("branch-review-owner").filter({ hasText: "All" }).click()
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
})
