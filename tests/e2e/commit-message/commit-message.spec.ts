// @spec spec/commit/commit-row.spec.md
import type { Locator, Page } from "@playwright/test"
import { testWithIssueLinks as test, expect } from "../fixtures/test-fixtures"
import { activityEntry, openActivityWindow } from "../helpers/activity-helpers"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, expandBranch, getBranchDetailsRow, verifyBranchState } from "../helpers/branch-helpers"
import { inlineBranchCreator } from "../helpers/inline-form-helpers"
import { emitTauriEvent, enableAutoSyncOnFocus, finishedSyncCount, runAndWaitForSync, syncFromMenu, syncOnWindowFocus, waitForTauriListener } from "../helpers/sync-helpers"
import { holdCommand, holdCommandCalls } from "../helpers/wait-helpers"

// the `issue_links` template gives this commit a body with a wrapped paragraph, a list, indented code and a trailer
const BRANCH = "JIRA-456"
const SUBJECT = "Add connection retry logic"

/** Expands the branch and returns the row of the commit with its full-message button. */
async function findCommitRow(page: Page): Promise<{ commitRow: Locator; toggle: Locator }> {
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
  test("should open the full message in a preview and reflow it into paragraphs, lists and code", async ({ page }) => {
    const { commitRow, toggle } = await findCommitRow(page)
    // rows don't render the body
    await expect(commitRow.getByTestId("commit-message-body")).toHaveCount(0)
    await expect(toggle).toHaveAccessibleName("Show full commit message")
    await expect(toggle).toHaveAttribute("aria-haspopup", "dialog")

    await toggle.click()
    const preview = getPreview(page)
    await expect(preview).toBeVisible()
    await expect(preview.getByRole("heading", { level: 2 })).toHaveText(SUBJECT)
    const body = preview.getByTestId("commit-message-body")
    await expect(body).toBeVisible()
    await expectPreviewFocused(preview)

    await captureHtmlSnapshot(body, "commit-message-body")

    // hard-wrapped lines are joined into one paragraph
    const paragraphs = body.locator("p")
    await expect(paragraphs).toHaveCount(3)
    const firstParagraph = await paragraphs.first().textContent()
    expect(firstParagraph).toBe(
      "The pool gave up after the first failed connection attempt, so a short database restart surfaced as an error for every request that was in flight at that moment.",
    )
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
    const title = preview.getByRole("heading", { level: 2 })
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
    await expect(preview.getByRole("heading", { level: 2 })).toHaveText("Emergency fix for CRITICAL-111")
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
    const { commitRow } = await findCommitRow(page)
    await commitRow.hover()
    await page.keyboard.press("Space")
    const preview = getPreview(page)
    await expectPreviewFocused(preview)
    await page.keyboard.press("ArrowDown")
    await expect(preview.getByTestId("commit-preview-position")).toHaveText("2 / 3")

    // a sync replaces the commit objects while the preview is open
    await syncFromMenu(page)

    await expect(preview.getByRole("heading", { level: 2 })).toHaveText("perf db: increase pool size to 50")
    await expect(preview.getByTestId("commit-preview-position")).toHaveText("2 / 3")
    await page.keyboard.press("ArrowDown")
    await expect(preview.getByTestId("commit-preview-position")).toHaveText("3 / 3")
  })

  test("should close the preview when a sync removes its commit", async ({ page, issueLinksRepo: repo }) => {
    await enableAutoSyncOnFocus(page)
    const unassignedRow = page.getByTestId("unassigned-commits-section").locator("tr").filter({ hasText: "Emergency fix for CRITICAL-111" })
    const hash = await unassignedRow.getAttribute("data-row-id")
    await unassignedRow.getByTestId("commit-message-toggle").click()
    const preview = getPreview(page)
    await expectPreviewFocused(preview)

    // the message is reworded in a terminal, so the commit gets a new hash
    const newHash = await repo.reword(hash!, "Emergency fix for CRITICAL-111\n\nSkip the cache warm-up until CRITICAL-112 is fixed upstream.")
    await syncOnWindowFocus(page)

    await expect(preview).toHaveCount(0)
    await expect(unassignedRow).toHaveAttribute("data-row-id", newHash)
    // the preview of the reworded commit shows the new body
    await unassignedRow.getByTestId("commit-message-toggle").click()
    await expect(preview.getByTestId("commit-message-body")).toHaveText("Skip the cache warm-up until CRITICAL-112 is fixed upstream.")
  })

  test.describe("editing", () => {
    const PERF_SUBJECT = "perf db: increase pool size to 50"

    /** Opens the preview of the second JIRA-456 commit (no body) with Space. */
    async function openPerfCommit(page: Page): Promise<Locator> {
      await expandBranch(page, findBranchRow(page, BRANCH))
      await getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: PERF_SUBJECT }).hover()
      await page.keyboard.press("Space")
      const preview = getPreview(page)
      await expectPreviewFocused(preview)
      return preview
    }

    /** Opens the preview of the second JIRA-456 commit and starts editing it with E. */
    async function editPerfCommit(page: Page): Promise<{ preview: Locator; editor: Locator }> {
      const preview = await openPerfCommit(page)
      await page.keyboard.press("e")
      const editor = preview.getByTestId("commit-message-editor")
      await expect(editor).toBeFocused()
      return { preview, editor }
    }

    test("should save the edited message and stay on the reworded commit", async ({ page }) => {
      const { preview, editor } = await editPerfCommit(page)
      // the raw message, the grouping prefix included
      await expect(editor).toHaveValue(`(${BRANCH}) ${PERF_SUBJECT}`)

      await editor.fill(`(${BRANCH}) perf db: increase pool size to 64\n\nThe load test saturated 50 connections.`)
      // the reword, then the sync that brings the reworded commit
      await runAndWaitForSync(page, () => page.keyboard.press("ControlOrMeta+Enter"))

      await expect(editor).toHaveCount(0)
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText("perf db: increase pool size to 64")
      await expect(preview.getByTestId("commit-message-body")).toHaveText("The load test saturated 50 connections.")
      await expect(preview.getByTestId("commit-preview-position")).toHaveText("2 / 3")
      await expectPreviewFocused(preview)

      await page.keyboard.press("Escape")
      await expect(getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: "perf db: increase pool size to 64" })).toBeVisible()
    })

    test("should reword a commit from the menu of its row, in its preview", async ({ page }) => {
      await expandBranch(page, findBranchRow(page, BRANCH))
      await getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: PERF_SUBJECT }).click({ button: "right" })
      await page.getByRole("menuitem", { name: "Reword…" }).click()
      // the preview opens with the message in edit, as E does there
      const editor = getPreview(page).getByTestId("commit-message-editor")
      await expect(editor).toBeFocused()
      await expect(editor).toHaveValue(`(${BRANCH}) ${PERF_SUBJECT}`)
      // Escape ends the edit, the preview stays; the request doesn't start it again
      await page.keyboard.press("Escape")
      await expect(editor).toHaveCount(0)
      await expectPreviewFocused(getPreview(page))
    })

    test("should offer the menu of the commit's row in its preview", async ({ page }) => {
      const preview = await openPerfCommit(page)
      await preview.getByRole("button", { name: /^More actions for / }).click()
      await page.getByRole("menuitem", { name: "Move to…" }).click()
      // a form of the row: the preview closes for it
      await expect(preview).toHaveCount(0)
      await expect(page.getByTestId("inline-branch-target")).toBeVisible()
    })

    test("should cancel the edit with Escape and keep the preview open", async ({ page }) => {
      const { preview, editor } = await editPerfCommit(page)
      await editor.fill("Something else")

      await page.keyboard.press("Escape")
      await expect(editor).toHaveCount(0)
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(PERF_SUBJECT)
      await expectPreviewFocused(preview)

      await page.keyboard.press("Escape")
      await expect(preview).toHaveCount(0)
    })

    test("should keep the edit open with an error when the commit changed meanwhile", async ({ page, issueLinksRepo: repo }) => {
      const { preview, editor } = await editPerfCommit(page)
      const hash = await getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: PERF_SUBJECT }).getAttribute("data-row-id")
      const draft = `(${BRANCH}) perf db: increase pool size to 64`
      await editor.fill(draft)

      // the commit is reworded in a terminal meanwhile, so it left the branch
      await repo.reword(hash!, `(${BRANCH}) ${PERF_SUBJECT} connections`)
      await page.keyboard.press("ControlOrMeta+Enter")

      await expect(page.getByText("Cannot edit the commit message")).toBeVisible()
      await expect(page.getByText(/is no longer on the current branch, sync and try again/)).toBeVisible()
      await expect(editor).toHaveValue(draft)
      await expect(editor).toBeEnabled()
      await expect(preview).toBeVisible()
    })

    const REWORDED = "perf db: increase pool size to 64"

    function perfRow(page: Page, subject = PERF_SUBJECT): Locator {
      return getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: subject })
    }

    /** Rewords the second JIRA-456 commit in its preview, which stays open on the new commit: the old id */
    async function rewordPerfCommit(page: Page): Promise<{ preview: Locator; hash: string }> {
      const { preview, editor } = await editPerfCommit(page)
      const hash = (await perfRow(page).getAttribute("data-row-id"))!
      await editor.fill(`(${BRANCH}) ${REWORDED}`)
      await runAndWaitForSync(page, () => page.keyboard.press("ControlOrMeta+Enter"))
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(REWORDED)
      return { preview, hash }
    }

    function rewordToast(page: Page, hash: string): Locator {
      return page.locator("[data-slot=toast]").filter({ hasText: `Reworded ${hash.slice(0, 8)}` })
    }

    test("should undo a reword from its toast and follow the preview back", async ({ page }) => {
      const { preview, hash } = await rewordPerfCommit(page)
      const toast = rewordToast(page, hash)
      await expect(toast).toContainText(`(${BRANCH}) ${REWORDED}`)
      await expect(perfRow(page, REWORDED)).not.toHaveAttribute("data-row-id", hash)

      // a click on the toast keeps the preview, which follows the commit back
      await runAndWaitForSync(page, () => toast.getByRole("button", { name: "Undo" }).click())
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(PERF_SUBJECT)
      await expect(preview.getByTestId("commit-preview-position")).toHaveText("2 / 3")
      await expect(perfRow(page)).toHaveAttribute("data-row-id", hash)
      await expect(perfRow(page, REWORDED)).toHaveCount(0)
      await expect(toast).toHaveCount(0)
    })

    test("should undo a reword from Activity", async ({ page }) => {
      const { preview, hash } = await rewordPerfCommit(page)
      await expect(rewordToast(page, hash)).toBeVisible()
      // the native menu item: the preview stays open in the main window
      const window = await openActivityWindow(page, async () => {
        await waitForTauriListener(page, "open_activity")
        await emitTauriEvent(page, "open_activity")
      })
      const entry = activityEntry(window, `Reworded ${hash.slice(0, 8)}`)
      await expect(entry).toHaveAttribute("data-kind", "rewrite")
      await expect(entry.getByTestId("activity-description")).toHaveText(`(${BRANCH}) ${REWORDED}`)

      await runAndWaitForSync(page, () => entry.getByTestId("activity-undo").click())
      await expect(entry.getByTestId("activity-undone")).toHaveText("undone")
      await expect(entry.getByTestId("activity-undo")).toHaveCount(0)
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(PERF_SUBJECT)
      await expect(perfRow(page)).toHaveAttribute("data-row-id", hash)
      // the Undo of the toast went with it
      await expect(rewordToast(page, hash)).toHaveCount(0)
    })

    test("should refuse to undo a reword after the branch moved", async ({ page, issueLinksRepo: repo }) => {
      const { hash } = await rewordPerfCommit(page)
      await page.keyboard.press("Escape")
      // another commit of the branch is reworded in a terminal: HEAD is not what the reword wrote
      const other = await getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: SUBJECT }).getAttribute("data-row-id")
      await repo.reword(other!, `(${BRANCH}) ${SUBJECT} with backoff`)

      await rewordToast(page, hash).getByRole("button", { name: "Undo" }).click()
      const refusal = page.locator("[data-slot=toast]").filter({ hasText: "Cannot undo the reword" })
      await expect(refusal).toContainText(/moved: HEAD is/)

      // nothing was undone: the next sync shows both rewords
      await syncFromMenu(page)
      await expect(perfRow(page, REWORDED)).toBeVisible()
      await expect(
        getBranchDetailsRow(page, BRANCH)
          .locator("tr")
          .filter({ hasText: `${SUBJECT} with backoff` }),
      ).toBeVisible()
      await expect(perfRow(page)).toHaveCount(0)
    })

    test("should save an unchanged message without a toast", async ({ page }) => {
      const { preview, editor } = await editPerfCommit(page)
      const syncs = await finishedSyncCount(page)
      const { calls, release } = await holdCommandCalls(page, "reword")
      await page.keyboard.press("ControlOrMeta+Enter")
      await expect.poll(() => calls.length).toBe(1)
      release()

      // the save is over (its toast would come before the edit ends); the message reached the backend unchanged
      await expect(editor).toHaveCount(0)
      expect(calls[0]).toMatchObject({ rewords: [{ text: { message: `(${BRANCH}) ${PERF_SUBJECT}` } }] })
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(PERF_SUBJECT)
      await expect(page.locator("[data-slot=toast]")).toHaveCount(0)
      expect(await finishedSyncCount(page)).toBe(syncs)
    })

    test("should disable the edit while a sync runs", async ({ page }) => {
      const preview = await openPerfCommit(page)
      const editButton = preview.getByRole("button", { name: /Edit/ })
      await expect(editButton).toBeEnabled()

      // hold the sync until the checks are done
      const releaseSync = await holdCommand(page, "sync_branches")
      const syncDone = syncFromMenu(page)

      await expect(editButton).toBeDisabled()
      await page.keyboard.press("e")
      await expect(preview.getByTestId("commit-message-editor")).toHaveCount(0)

      releaseSync()
      await syncDone
      await expect(editButton).toBeEnabled()
      await page.keyboard.press("e")
      await expect(preview.getByTestId("commit-message-editor")).toBeFocused()
    })

    test("should warn that a changed grouping prefix moves the commit, and move it", async ({ page }) => {
      const { preview, editor } = await editPerfCommit(page)
      const hint = preview.getByTestId("commit-prefix-change")
      await expect(hint).toHaveCount(0)

      await editor.fill(`(JIRA-789) ${PERF_SUBJECT}`)
      await expect(hint).toHaveText("Saving moves the commit to the branch JIRA-789")
      await editor.fill(PERF_SUBJECT)
      await expect(hint).toHaveText("Saving moves the commit to the unassigned commits")

      await runAndWaitForSync(page, () => preview.getByRole("button", { name: /Save/ }).click())
      // the commit left the branch list, so its preview closes
      await expect(preview).toHaveCount(0)
      await expect(page.getByTestId("unassigned-commits-section").locator("tr").filter({ hasText: PERF_SUBJECT })).toBeVisible()
    })
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

  test("should select commit rows of a branch, one list at a time", async ({ page }) => {
    const { commitRow } = await findCommitRow(page)
    await expect(commitRow.locator("xpath=ancestor::table[1]")).toHaveAttribute("tabindex", "0")

    await commitRow.getByText(SUBJECT).click()
    await expect(commitRow).toHaveAttribute("data-selected", "true")
    await expect(getBranchDetailsRow(page, BRANCH).getByTestId("selection-actions")).toContainText("1 selected")

    // selecting in another list clears this selection
    const unassignedRow = page.getByTestId("unassigned-commits-section").locator("[data-row-id]").first()
    await unassignedRow.getByText("Emergency fix for").click()
    await expect(unassignedRow).toHaveAttribute("data-selected", "true")
    await expect(commitRow).toHaveAttribute("data-selected", "false")
  })
})
