// @spec spec/commit/commit-row.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, expandBranch, getBranchDetailsRow } from "../helpers/branch-helpers"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"

// the `issue_links` template gives this commit a body with a wrapped paragraph, a list, indented code and a trailer
const BRANCH = "JIRA-456"
const SUBJECT = "Add connection retry logic"

/** Expands the branch and returns the row of the commit with its full-message toggle. */
async function findCommitRow(page: Page): Promise<{ commitRow: Locator, toggle: Locator }> {
  await expandBranch(page, findBranchRow(page, BRANCH))
  const commitRow = getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: SUBJECT })
  return { commitRow, toggle: commitRow.getByTestId("commit-message-toggle") }
}

test.describe("Commit message", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    await syncAndWaitForBranches()
    // the unassigned commits card can render after the branches and push the commit list down
    await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
  })

  test("should expand the body inline and reflow it into paragraphs, lists and code", async ({ page }) => {
    const { commitRow, toggle } = await findCommitRow(page)
    const body = commitRow.getByTestId("commit-message-body")
    await expect(body).toHaveCount(0)

    await toggle.click()
    await expect(toggle).toHaveAttribute("aria-expanded", "true")
    await expect(toggle).toHaveAccessibleName("Hide full commit message")
    await expect(body).toBeVisible()

    await captureHtmlSnapshot(body, "commit-message-body")

    // hard-wrapped lines are joined into one paragraph
    const paragraphs = body.locator("p")
    await expect(paragraphs).toHaveCount(3)
    const firstParagraph = await paragraphs.first().textContent()
    expect(firstParagraph).toBe("The pool gave up after the first failed connection attempt, so a short database restart surfaced as an error for every request that was in flight at that moment.")
    expect(firstParagraph).not.toContain("\n")

    // list items lose their marker, and a continuation line stays in its item
    const items = body.locator("ul > li")
    await expect(items).toHaveCount(3)
    await expect(items.first()).toHaveText("wait with exponential backoff, starting at 100 ms and capped at 5 s, for up to 6 attempts")

    // indented code is kept verbatim without the indent
    await expect(body.locator("pre")).toHaveText("retry: { attempts: 6, maxDelayMs: 5000 }")

    // a paragraph that is only "Label:" and a run-in "Why:" become section labels; the long one stacks above its blocks
    await expect(body.locator("[data-slot=commit-section-label]")).toHaveText(["Retry the connection instead", "Why"])
    await expect(paragraphs.last()).toHaveText("createPool in db/pool.js ignored POOL_RETRY_LIMIT, so retry never ran, unlike the JavaScript client.")

    // identifiers, paths and backtick spans are code; a product name is not
    const code = body.locator("[data-slot=inline-code]")
    await expect(code).toHaveText(["createPool", "db/pool.js", "POOL_RETRY_LIMIT", "retry"])
    await expect(code.last()).toHaveAttribute("data-explicit", "")

    await expect(body.getByText("Co-Authored-By: Jane Doe <jane@example.com>")).toBeVisible()

    await toggle.click()
    await expect(toggle).toHaveAttribute("aria-expanded", "false")
    await expect(body).toHaveCount(0)
  })

  test("should dim a conventional-commit prefix of the subject", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, BRANCH))
    const commitRow = getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: "increase pool size to 50" })
    await expect(commitRow.locator("[data-slot=commit-subject-prefix]")).toHaveText("perf db:")
  })

  test("should not change the row selection", async ({ page }) => {
    const unassignedRow = page.getByTestId("unassigned-commits-section").locator("tr").filter({ hasText: "Emergency fix for CRITICAL-111" })
    const body = unassignedRow.getByTestId("commit-message-body")

    await unassignedRow.getByTestId("commit-message-toggle").click()
    await expect(body).toHaveText("Skip the cache warm-up that hangs on startup until CRITICAL-112 is fixed.")
    await expect(unassignedRow).toHaveAttribute("data-selected", "false")

    // reading or selecting the body text does not select the row either
    // near the start: the middle of the text is the CRITICAL-112 link
    await body.locator("p").click({ position: { x: 4, y: 4 } })
    await expect(unassignedRow).toHaveAttribute("data-selected", "false")
    await expect(page.getByTestId("selection-actions")).toHaveCount(0)

    // the rest of the row still selects it
    await unassignedRow.getByText("Emergency fix for").click()
    await expect(unassignedRow).toHaveAttribute("data-selected", "true")
  })
})
