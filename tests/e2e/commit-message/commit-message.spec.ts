// @spec spec/commit/commit-row.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, expandBranch, getBranchDetailsRow, verifyBranchState } from "../helpers/branch-helpers"
import { inlineBranchCreator } from "../helpers/inline-form-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"

// the `issue_links` template gives this commit a body with a wrapped paragraph, a list, indented code and a trailer
const BRANCH = "JIRA-456"
const SUBJECT = "Add connection retry logic"

/** Expands the branch and returns the row of the commit with its full-message button. */
async function findCommitRow(page: Page): Promise<{ commitRow: Locator, toggle: Locator }> {
  await expandBranch(page, findBranchRow(page, BRANCH))
  const commitRow = getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: SUBJECT })
  return { commitRow, toggle: commitRow.getByTestId("commit-message-toggle") }
}

function getPreview(page: Page): Locator {
  // not by the dialog role: the inline branch form has it too
  return page.getByTestId("commit-preview")
}

/** The preview takes the focus a frame after it opens; its keys only work from then on. */
async function expectPreviewFocused(preview: Locator) {
  await expect(preview.getByTestId("commit-preview-body")).toBeFocused()
}

test.describe("Commit message", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    await syncAndWaitForBranches()
    // the unassigned commits card can render after the branches and push the commit list down
    await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
  })

  test("should open the full message in a preview and reflow it into paragraphs, lists and code", async ({ page }) => {
    const { commitRow, toggle } = await findCommitRow(page)
    // rows don't render the body
    await expect(commitRow.getByTestId("commit-message-body")).toHaveCount(0)
    await expect(toggle).toHaveAccessibleName("Show full commit message")
    await expect(toggle).toHaveAttribute("aria-haspopup", "dialog")

    await toggle.click()
    const preview = getPreview(page)
    await expect(preview).toBeVisible()
    await expect(preview.getByRole("heading")).toHaveText(SUBJECT)
    const body = preview.getByTestId("commit-message-body")
    await expect(body).toBeVisible()
    await expectPreviewFocused(preview)

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

    await page.keyboard.press("Escape")
    await expect(preview).toHaveCount(0)
  })

  test("should open the preview of the hovered commit with Space and step through the list with the arrows", async ({ page }) => {
    const { commitRow } = await findCommitRow(page)
    await commitRow.hover()
    await page.keyboard.press("Space")

    const preview = getPreview(page)
    const title = preview.getByRole("heading")
    const position = preview.getByTestId("commit-preview-position")
    await expect(title).toHaveText(SUBJECT)
    await expect(position).toHaveText("1 / 3")
    await expectPreviewFocused(preview)

    // the pointer now rests on the backdrop; the arrows page through the commits of the branch
    await page.keyboard.press("ArrowDown")
    await expect(title).toHaveText("perf db: increase pool size to 50")
    await expect(position).toHaveText("2 / 3")
    // a commit without a body still shows its whole subject
    await expect(preview.getByText("No description")).toBeVisible()
    await page.keyboard.press("ArrowUp")
    await page.keyboard.press("ArrowUp")
    await expect(title).toHaveText(SUBJECT)
    await expect(position).toHaveText("1 / 3")

    await page.keyboard.press("Space")
    await expect(preview).toHaveCount(0)
  })

  test("should open the preview of the last clicked unassigned row with Space and keep the selection", async ({ page }) => {
    const unassignedRow = page.getByTestId("unassigned-commits-section").locator("tr").filter({ hasText: "Emergency fix for CRITICAL-111" })
    await unassignedRow.getByText("Emergency fix for").click()
    await expect(unassignedRow).toHaveAttribute("data-selected", "true")

    // away from every row: the focused list falls back to its last clicked row
    await page.mouse.move(0, 0)
    await page.keyboard.press("Space")
    const preview = getPreview(page)
    await expect(preview.getByRole("heading")).toHaveText("Emergency fix for CRITICAL-111")
    await expectPreviewFocused(preview)

    await page.keyboard.press("ArrowDown")
    await page.keyboard.press("Escape")
    await expect(preview).toHaveCount(0)
    await expect(unassignedRow).toHaveAttribute("data-selected", "true")
    await expect(page.getByTestId("selection-actions")).toContainText("1 selected")
  })

  test("should keep Space in a text field", async ({ page }) => {
    const unassignedRow = page.getByTestId("unassigned-commits-section").locator("tr").filter({ hasText: "Emergency fix for CRITICAL-111" })
    await unassignedRow.getByText("Emergency fix for").click()
    await page.keyboard.press("Enter")
    const input = inlineBranchCreator.getInput(page)
    await expect(input).toBeFocused()

    // the pointer rests on a commit row, yet Space types into the focused field
    await unassignedRow.hover()
    await input.fill("")
    await page.keyboard.type("hot fix")
    await expect(input).toHaveValue("hot fix")
    await expect(getPreview(page)).toHaveCount(0)
  })

  test("should leave Space to a button focused from the keyboard", async ({ page }) => {
    const { commitRow } = await findCommitRow(page)
    const branchRow = findBranchRow(page, BRANCH)
    const folderButton = branchRow.getByRole("button", { name: "Collapse" })
    await commitRow.hover()

    // the click that expanded the branch left the focus on the folder button; Tab away and back, without the pointer
    await folderButton.focus()
    await page.keyboard.press("Shift+Tab")
    await page.keyboard.press("Tab")
    await expect(folderButton).toBeFocused()

    await page.keyboard.press("Space")
    await verifyBranchState(branchRow, "closed")
    await expect(getPreview(page)).toHaveCount(0)
  })

  test("should keep the preview on its commit across a sync", async ({ page }) => {
    await page.getByTestId("sync-settings-button").click()
    await page.getByRole("switch").click()
    await expect(page.getByRole("switch")).toBeChecked()
    await page.keyboard.press("Escape")

    const { commitRow } = await findCommitRow(page)
    await commitRow.hover()
    await page.keyboard.press("Space")
    const preview = getPreview(page)
    await expectPreviewFocused(preview)
    await page.keyboard.press("ArrowDown")
    await expect(preview.getByTestId("commit-preview-position")).toHaveText("2 / 3")

    // auto-sync on focus replaces the commit objects while the preview is open
    const syncResponse = page.waitForResponse(response => response.url().endsWith("/invoke/sync_branches"))
    await emitTauriEvent(page, "tauri://focus")
    await syncResponse
    await expect(page.getByTestId("sync-button")).toBeEnabled()

    await expect(preview.getByRole("heading")).toHaveText("perf db: increase pool size to 50")
    await expect(preview.getByTestId("commit-preview-position")).toHaveText("2 / 3")
    await page.keyboard.press("ArrowDown")
    await expect(preview.getByTestId("commit-preview-position")).toHaveText("3 / 3")
  })

  test("should dim a conventional-commit prefix of the subject", async ({ page }) => {
    await expandBranch(page, findBranchRow(page, BRANCH))
    const commitRow = getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: "increase pool size to 50" })
    await expect(commitRow.locator("[data-slot=commit-subject-prefix]")).toHaveText("perf db:")
  })

  test("should not change the row selection", async ({ page }) => {
    const unassignedRow = page.getByTestId("unassigned-commits-section").locator("tr").filter({ hasText: "Emergency fix for CRITICAL-111" })
    const body = getPreview(page).getByTestId("commit-message-body")

    await unassignedRow.getByTestId("commit-message-toggle").click()
    await expect(body).toHaveText("Skip the cache warm-up that hangs on startup until CRITICAL-112 is fixed.")
    await expect(unassignedRow).toHaveAttribute("data-selected", "false")
    // issue keys in the body are links, as in the subject
    await expect(body.getByRole("link", { name: "CRITICAL-112" })).toHaveAttribute("href", "https://jira.example.com/browse/CRITICAL-112")

    // reading or selecting the body text does not select the row either
    // near the start: the middle of the text is the CRITICAL-112 link
    await body.locator("p").click({ position: { x: 4, y: 4 } })
    await page.keyboard.press("Escape")
    await expect(body).toHaveCount(0)
    await expect(unassignedRow).toHaveAttribute("data-selected", "false")
    await expect(page.getByTestId("selection-actions")).toHaveCount(0)

    // the rest of the row still selects it
    await unassignedRow.getByText("Emergency fix for").click()
    await expect(unassignedRow).toHaveAttribute("data-selected", "true")
  })

  test("should not select commit rows of a branch", async ({ page }) => {
    const { commitRow } = await findCommitRow(page)
    await expect(commitRow.locator("xpath=ancestor::table[1]")).toHaveAttribute("tabindex", "-1")
    await expect(commitRow).not.toHaveClass(/cursor-pointer/)

    await commitRow.getByText(SUBJECT).click()
    await expect(commitRow).toHaveAttribute("data-selected", "false")
    await expect(page.getByTestId("selection-actions")).toHaveCount(0)
  })
})
