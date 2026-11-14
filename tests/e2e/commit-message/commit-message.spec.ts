// @spec openspec/specs/commit/commit-row/spec.md
import type { Locator, Page } from "@playwright/test"
import { testWithIssueLinks as test, expect, expectErrorLog } from "../fixtures/test-fixtures"
import { activityEntry, openActivityWindow } from "../helpers/activity-helpers"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, expandBranch, getBranchDetailsRow, verifyBranchState } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { MIN_WINDOW } from "../helpers/header-helpers"
import { inlineBranchCreator } from "../helpers/inline-form-helpers"
import {
  emitTauriEvent,
  enableAutoSyncOnFocus,
  finishedSyncCount,
  nextAnswer,
  runAndWaitForAnswer,
  runAndWaitForSync,
  syncFromMenu,
  syncOnWindowFocus,
  waitForTauriListener,
} from "../helpers/sync-helpers"
import { holdCommand, holdRewrites, expectClosed } from "../helpers/wait-helpers"

// the `issue_links` template gives this commit a body with a wrapped paragraph, a list, indented code and a trailer
const BRANCH = "JIRA-456"
const SUBJECT = "Add connection retry logic"

/** Expands the branch and returns the row of the commit with the mark of its card. */
async function findCommitRow(page: Page): Promise<{ commitRow: Locator; toggle: Locator }> {
  await expandBranch(page, findBranchRow(page, BRANCH))
  const commitRow = getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: SUBJECT })
  return { commitRow, toggle: commitRow.getByTestId("commit-row-mark") }
}

function getPreview(page: Page): Locator {
  // not by the dialog role: the inline branch form has it too
  return page.getByTestId("commit-preview")
}

/** The preview takes the focus a frame after it opens; its keys only work from then on. */
async function expectPreviewFocused(preview: Locator) {
  await expect(preview.getByTestId("commit-preview-body")).toBeFocused()
}

/** Whether the card lies clear of its row, below it or above it: it never covers the meta line the reader was reading */
async function cardClearsRow(card: Locator, row: Locator): Promise<boolean> {
  const cardBox = await card.boundingBox()
  const rowBox = await row.boundingBox()
  return cardBox !== null && rowBox !== null && (cardBox.y >= rowBox.y + rowBox.height - 1 || cardBox.y + cardBox.height <= rowBox.y + 1)
}

/** Rewords the unassigned commit of CRITICAL-111 to `message` and waits for its row to show the new commit */
async function rewordEmergencyFix(page: Page, repo: { reword: (hash: string, message: string) => Promise<string> }, message: string): Promise<Locator> {
  await enableAutoSyncOnFocus(page)
  const unassignedRow = page.getByTestId("unassigned-commits-section").locator("tr").filter({ hasText: "Emergency fix for CRITICAL-111" })
  const hash = await unassignedRow.getAttribute("data-row-id")
  const newHash = await repo.reword(hash!, message)
  await syncOnWindowFocus(page)
  await expect(unassignedRow).toHaveAttribute("data-row-id", newHash)
  return unassignedRow
}

test.describe("Commit message", () => {
  test("should open the full message in a preview and reflow it into paragraphs, lists and code", async ({ page }) => {
    const { commitRow, toggle } = await findCommitRow(page)
    // rows don't render the body
    await expect(commitRow.getByTestId("commit-message-body")).toHaveCount(0)
    await expect(toggle).toHaveAccessibleName("Show full commit message")

    await toggle.click()
    const preview = getPreview(page)
    await expect(preview).toBeVisible()
    await expect(preview.getByRole("heading", { level: 2 })).toHaveText(`${BRANCH} ${SUBJECT}`)
    const body = preview.getByTestId("commit-message-body")
    await expect(body).toBeVisible()
    await expectPreviewFocused(preview)

    // the body shows as Rust read it (`commit_body`), which answers after the preview opened
    await expect(body.getByTestId("commit-body-lead")).toBeVisible()
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
    await expect(body.locator("[data-slot=commit-body-label]")).toHaveText(["Retry the connection instead", "Why"])
    await expect(paragraphs.last()).toHaveText("createPool in db/pool.js ignored POOL_RETRY_LIMIT, so retry never ran, unlike the JavaScript client.")

    // identifiers, paths and backtick spans are code; a product name is not
    const code = body.locator("[data-slot=inline-code]")
    await expect(code).toHaveText(["createPool", "db/pool.js", "POOL_RETRY_LIMIT", "retry"])
    await expect(code.last()).toHaveAttribute("data-explicit", "")

    await expect(body.getByText("Co-Authored-By: Jane Doe <jane@example.com>")).toBeVisible()

    await page.keyboard.press("Escape")
    await expectClosed(preview)
  })

  test("should open the preview of the hovered commit with Space and step through the list with the arrows", async ({ page }) => {
    const { commitRow } = await findCommitRow(page)
    await commitRow.hover()
    await page.keyboard.press("Space")

    const preview = getPreview(page)
    const title = preview.getByRole("heading", { level: 2 })
    const position = preview.getByTestId("commit-preview-position")
    await expect(title).toHaveText(`${BRANCH} ${SUBJECT}`)
    await expect(position).toHaveText("1 / 3")
    await expectPreviewFocused(preview)

    // the pointer now rests on the backdrop; the arrows page through the commits of the branch
    await page.keyboard.press("ArrowDown")
    await expect(title).toHaveText(`${BRANCH} perf db: increase pool size to 50`)
    await expect(position).toHaveText("2 / 3")
    // a commit without a body still shows its whole subject
    await expect(preview.getByText("No description")).toBeVisible()
    await page.keyboard.press("ArrowUp")
    await page.keyboard.press("ArrowUp")
    await expect(title).toHaveText(`${BRANCH} ${SUBJECT}`)
    await expect(position).toHaveText("1 / 3")

    await page.keyboard.press("Space")
    await expectClosed(preview)
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
    await expectClosed(preview)
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
    await expectClosed(getPreview(page))
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
    await expectClosed(getPreview(page))
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

    await expect(preview.getByRole("heading", { level: 2 })).toHaveText(`${BRANCH} perf db: increase pool size to 50`)
    await expect(preview.getByTestId("commit-preview-position")).toHaveText("2 / 3")
    await page.keyboard.press("ArrowDown")
    await expect(preview.getByTestId("commit-preview-position")).toHaveText("3 / 3")
  })

  test(
    "should open the commit card on the hover of the mark, of the subject and by I, with the menu of the commit, and give the focus back on Escape",
    { tag: "@webkit" },
    async ({ page }) => {
      const { commitRow, toggle } = await findCommitRow(page)
      const card = page.getByTestId("commit-card")
      const table = commitRow.locator("xpath=ancestor::table[1]")
      // a commit of a subject alone has no mark
      await expect(getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: "increase pool size to 50" }).getByTestId("commit-row-mark")).toHaveCount(0)

      // the hover of the mark opens the card: the whole subject over the meta line, then the body; the pointer leaving closes it
      await page.mouse.move(0, 0)
      await toggle.hover()
      await expect(card).toBeVisible()
      await expect(card.getByText(SUBJECT, { exact: true })).toBeVisible()
      // under the whole row: its hash, files and line counts stay in view
      await expect.poll(() => cardClearsRow(card, commitRow)).toBe(true)
      await expect(card.getByTestId("commit-message-body").locator("p").first()).toHaveText(
        "The pool gave up after the first failed connection attempt, so a short database restart surfaced as an error for every request that was in flight at that moment.",
      )
      await page.mouse.move(0, 0)
      await expectClosed(card)

      // the hover of the subject opens it too, after a longer rest; the menu of the commit at the end of the meta line of
      // the card; the pointer leaving the subject closes it
      await commitRow.getByText(SUBJECT).hover()
      await expect(card).toBeVisible()
      await expect(card.getByTestId("commit-header-meta").getByRole("button", { name: /^More actions for / })).toBeVisible()
      await page.mouse.move(0, 0)
      await expectClosed(card)
      // the pointer that moves from the subject onto the mark keeps the card the subject opened: the time stands still
      // past the close of the subject (150 ms) and before the mark could open it again (300 ms)
      await page.clock.pauseAt((await page.evaluate(() => Date.now())) + 1000)
      await commitRow.getByText(SUBJECT).hover()
      await page.clock.fastForward(700)
      await expect(card).toBeVisible()
      await toggle.hover()
      await page.clock.fastForward(200)
      await expect(card).toBeVisible()
      await page.clock.resume()
      await page.mouse.move(0, 0)
      await expectClosed(card)

      // I on the row Space would preview (the last clicked one) opens the card with the focus in it; Escape closes the
      // card alone and gives the focus back to the list, whose keys go on from that row
      await commitRow.getByText(SUBJECT).click()
      await page.mouse.move(0, 0)
      await page.keyboard.press("i")
      await expect(card).toBeFocused()
      await expect(card.getByTestId("commit-message-body").locator("ul > li")).toHaveCount(3)
      await page.keyboard.press("Escape")
      await expectClosed(card)
      await expect(table).toBeFocused()
      await expect(commitRow).toHaveAttribute("data-selected", "true")

      // the Escape of a card the hover opened, the focus on the list, closes the card alone: the selection stays
      await toggle.hover()
      await expect(card).toBeVisible()
      await page.keyboard.press("Escape")
      await expectClosed(card)
      await expect(commitRow).toHaveAttribute("data-selected", "true")
      await page.keyboard.press(" ")
      const preview = getPreview(page)
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(`${BRANCH} ${SUBJECT}`)
      await expectPreviewFocused(preview)
      await page.keyboard.press("Escape")
      await expectClosed(preview)

      // the window keeps one card: the hover of a mark in another list closes the card I opened in this one
      await page.mouse.move(0, 0)
      await page.keyboard.press("i")
      await expect(card).toBeFocused()
      await expect(card.getByText(SUBJECT, { exact: true })).toBeVisible()
      await page.getByTestId("unassigned-commits-section").locator("tr").filter({ hasText: "Emergency fix for CRITICAL-111" }).getByTestId("commit-row-mark").hover()
      await expect(card.getByText("Emergency fix for CRITICAL-111", { exact: true })).toBeVisible()
      // one card open: the one it replaced closes, its exit running until the window paints
      await expect(card.and(page.locator("[data-open]"))).toHaveCount(1)
    },
  )

  test("should act from the menu of the card on the selection as the right-click of its row does", async ({ page }) => {
    const { commitRow } = await findCommitRow(page)
    const otherRow = getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: "increase pool size to 50" })
    const hashes = [await otherRow.getAttribute("data-row-id"), await commitRow.getAttribute("data-row-id")]
    // the rows are selected after their cards' menus were rendered: the menu is built as it opens, from the selection then
    await otherRow.getByText("increase pool size to 50").click()
    await commitRow.getByText(SUBJECT).click({ modifiers: ["ControlOrMeta"] })
    await expect(getBranchDetailsRow(page, BRANCH).getByTestId("selection-actions")).toContainText("2 selected")

    // I opens the card of the last clicked row; its "⋯" copies the hashes of both selected commits
    await page.mouse.move(0, 0)
    await page.keyboard.press("i")
    const card = page.getByTestId("commit-card")
    await expect(card).toBeFocused()
    await card
      .getByTestId("commit-header-meta")
      .getByRole("button", { name: /^More actions for / })
      .click()
    await page.getByRole("menuitem", { name: "Copy 2 Commit Hashes" }).click()
    const byText = (a: string | null, b: string | null) => (a ?? "").localeCompare(b ?? "")
    await expect.poll(async () => (await readClipboard(page)).split("\n").toSorted(byText)).toEqual(hashes.toSorted(byText))
  })

  test("should fold a long body in the card as the review folds it, open each folded part, keep the Look closely, and fade the card while more is below", async ({
    page,
    issueLinksRepo: repo,
  }) => {
    const unassignedRow = await rewordEmergencyFix(
      page,
      repo,
      [
        "Emergency fix for CRITICAL-111",
        "The cache warm-up blocked every request for a second after a deploy, so the first users of a node waited. Now the warm-up runs in the background and a request reads the store until it is done.",
        "The cause:\n- the warm-up held the lock of the cache\n- every request waited for that lock\n- a deploy restarts every node at once",
        "The fix:\n- the warm-up fills a second cache\n- a request reads the store until the swap\n- the swap is one pointer write",
        "Look closely: a request during the warm-up reads the store, which is slower for that second. Decide whether a slower second is better than a blocked one, or the warm-up must stay in front of the traffic.",
        "Why not a lock per key:\nA lock per key would still block the keys a request asks first, and those are the hot ones the warm-up fills first too.",
        "Tests:\n- the warm-up test\n- the swap test",
        "Evidence:\n- 5 deploys on staging: no request waited",
      ].join("\n\n"),
    )

    await page.mouse.move(0, 0)
    await unassignedRow.getByTestId("commit-row-mark").hover()
    const card = page.getByTestId("commit-card")
    await expect(card).toBeVisible()
    await expect.poll(() => cardClearsRow(card, unassignedRow)).toBe(true)
    // the body shows up to its fold, the question of the author whole; the rest named on one line, verification too
    await expect(card.getByTestId("commit-body-look-closely")).toContainText("Decide whether a slower second is better than a blocked one")
    const folds = card.getByTestId("commit-body-fold")
    await expect(card.getByTestId("commit-body-folded")).toHaveText("More:Why not a lock per key,Tests,Evidence")
    await expect(folds).toHaveText(["Why not a lock per key", "Tests", "Evidence"])
    await expect(card).not.toContainText("the swap test")
    // a fold opens below the line, headed by its label, which leaves the line; the card scrolls, and fades at its
    // bottom while more is below
    await folds.nth(1).click()
    await expect(card.getByTestId("commit-body-unfold")).toHaveText("Tests")
    await expect(folds).toHaveText(["Why not a lock per key", "Evidence"])
    await expect(card.getByTestId("commit-body-part").filter({ hasText: "the swap test" })).toBeAttached()
    await folds.nth(0).click()
    await folds.nth(0).click()
    await expect(card.getByTestId("commit-body-folded")).toHaveCount(0)
    const fade = card.getByTestId("commit-card-fade")
    await expect(fade).toHaveAttribute("data-more", "")
    await card.evaluate(element => element.scrollTo({ top: element.scrollHeight }))
    await expect(fade).not.toHaveAttribute("data-more", "")
    await expect(card.getByText("5 deploys on staging: no request waited")).toBeInViewport()
  })

  test("should say No description above the trailers of a body of trailers alone, which has no mark", async ({ page, issueLinksRepo: repo }) => {
    await enableAutoSyncOnFocus(page)
    const unassignedRow = page.getByTestId("unassigned-commits-section").locator("tr").filter({ hasText: "Emergency fix for CRITICAL-111" })
    await expect(unassignedRow.getByTestId("commit-row-mark")).toBeVisible()
    const hash = await unassignedRow.getAttribute("data-row-id")
    const newHash = await repo.reword(hash!, "Emergency fix for CRITICAL-111\n\nCo-Authored-By: Jane Doe <jane@example.com>")
    await syncOnWindowFocus(page)
    await expect(unassignedRow).toHaveAttribute("data-row-id", newHash)
    await expect(unassignedRow.getByTestId("commit-row-mark")).toHaveCount(0)

    await unassignedRow.hover()
    await page.keyboard.press(" ")
    const preview = getPreview(page)
    await expectPreviewFocused(preview)
    const body = preview.getByTestId("commit-message-body")
    await expect(body.getByText("No description")).toBeVisible()
    await expect(body.getByText("Co-Authored-By: Jane Doe <jane@example.com>")).toBeVisible()
  })

  test("should close the preview when a sync removes its commit", async ({ page, issueLinksRepo: repo }) => {
    await enableAutoSyncOnFocus(page)
    const unassignedRow = page.getByTestId("unassigned-commits-section").locator("tr").filter({ hasText: "Emergency fix for CRITICAL-111" })
    const hash = await unassignedRow.getAttribute("data-row-id")
    await unassignedRow.getByTestId("commit-row-mark").click()
    const preview = getPreview(page)
    await expectPreviewFocused(preview)

    // the message is reworded in a terminal, so the commit gets a new hash
    const newHash = await repo.reword(hash!, "Emergency fix for CRITICAL-111\n\nSkip the cache warm-up until CRITICAL-112 is fixed upstream.")
    await syncOnWindowFocus(page)

    await expectClosed(preview)
    await expect(unassignedRow).toHaveAttribute("data-row-id", newHash)
    // the preview of the reworded commit shows the new body
    await unassignedRow.getByTestId("commit-row-mark").click()
    await expect(preview.getByTestId("commit-message-body")).toHaveText("Skip the cache warm-up until CRITICAL-112 is fixed upstream.")
  })

  test("should render a Markdown heading, a rule and a label glued to its list as sections of the body", async ({ page, issueLinksRepo: repo }) => {
    const unassignedRow = await rewordEmergencyFix(
      page,
      repo,
      "Emergency fix for CRITICAL-111\n\nSkip the cache warm-up until CRITICAL-112 is fixed.\n\n## Rollout\n\nEnable it on staging first.\n\n---\n\nTests:\n- the warm-up test",
    )

    await unassignedRow.getByTestId("commit-row-mark").click()
    const body = getPreview(page).getByTestId("commit-message-body")
    // the body shows as Rust read it (`commit_body`), which answers after the preview opened
    await expect(body.getByTestId("commit-body-lead")).toBeVisible()
    await captureHtmlSnapshot(body, "commit-message-markdown")

    // the heading heads its section without its marks, the rule is a line of that section, the label heads the list
    const heading = body.locator("[data-slot=commit-body-heading]")
    await expect(heading).toHaveText("Rollout")
    await expect(body.getByRole("separator")).toBeVisible()
    await expect(body.locator("[data-slot=commit-body-label]")).toHaveText("Tests")
    await expect(body.locator("ul > li")).toHaveText(["the warm-up test"])
    await expect(body).not.toContainText("##")
    await expect(body).not.toContainText("---")
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
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(`${BRANCH} perf db: increase pool size to 64`)
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
      await expectClosed(preview)
      await expect(page.getByTestId("inline-branch-target")).toBeVisible()
    })

    test("should cancel the edit with Escape and keep the preview open", async ({ page }) => {
      const { preview, editor } = await editPerfCommit(page)
      await editor.fill("Something else")

      await page.keyboard.press("Escape")
      await expect(editor).toHaveCount(0)
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(`${BRANCH} ${PERF_SUBJECT}`)
      await expectPreviewFocused(preview)

      await page.keyboard.press("Escape")
      await expectClosed(preview)
    })

    test("should save nothing when the commit changed meanwhile, say so, and close the preview of the commit that is gone", async ({ page, issueLinksRepo: repo }) => {
      expectErrorLog(page, /^Cannot edit the commit message: What the rewrite was decided on changed before it began/)
      const { preview, editor } = await editPerfCommit(page)
      const hash = await getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: PERF_SUBJECT }).getAttribute("data-row-id")
      await editor.fill(`(${BRANCH}) perf db: increase pool size to 64`)

      // the commit is reworded in a terminal meanwhile, so it left the branch
      await repo.reword(hash!, `(${BRANCH}) ${PERF_SUBJECT} connections`)
      await runAndWaitForAnswer(page, "execute", () => page.keyboard.press("ControlOrMeta+Enter"))

      // the session brings its table to HEAD to find the commit, and finds it neither by its id nor by its key: the plan
      // of the reword changed, nothing was done
      await expect(page.getByText("Cannot edit the commit message")).toBeVisible()
      await expect(page.getByText(/is no commit ahead of the baseline any more\); nothing was done/)).toBeVisible()
      // the table shows the commit as the terminal left it; the preview of the one that is gone closed
      await expectClosed(preview)
      await expect(
        getBranchDetailsRow(page, BRANCH)
          .locator("tr")
          .filter({ hasText: `${PERF_SUBJECT} connections` }),
      ).toBeVisible()
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
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(`${BRANCH} ${REWORDED}`)
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
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(`${BRANCH} ${PERF_SUBJECT}`)
      await expect(preview.getByTestId("commit-preview-position")).toHaveText("2 / 3")
      await expect(perfRow(page)).toHaveAttribute("data-row-id", hash)
      await expect(perfRow(page, REWORDED)).toHaveCount(0)
      await expectClosed(toast)
    })

    test("should undo a reword from Activity", async ({ page }) => {
      const { preview, hash } = await rewordPerfCommit(page)
      await expect(rewordToast(page, hash)).toBeVisible()
      // the native menu item, where the side panel has no room: the window opens, the preview stays open in the main
      // window
      await page.setViewportSize(MIN_WINDOW)
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
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(`${BRANCH} ${PERF_SUBJECT}`)
      await expect(perfRow(page)).toHaveAttribute("data-row-id", hash)
      // the Undo of the toast went with it
      await expectClosed(rewordToast(page, hash))
    })

    test("should refuse to undo a reword after the branch moved", async ({ page, issueLinksRepo: repo }) => {
      expectErrorLog(page, /^Cannot undo the reword: Branch master moved/)
      const { hash } = await rewordPerfCommit(page)
      await page.keyboard.press("Escape")
      // another commit of the branch is reworded in a terminal: HEAD is not what the reword wrote
      const other = await getBranchDetailsRow(page, BRANCH).locator("tr").filter({ hasText: SUBJECT }).getAttribute("data-row-id")
      await repo.reword(other!, `(${BRANCH}) ${SUBJECT} with backoff`)

      await runAndWaitForAnswer(page, "execute", () => rewordToast(page, hash).getByRole("button", { name: "Undo" }).click())
      const refusal = page.locator("[data-slot=toast]").filter({ hasText: "Cannot undo the reword" })
      // stale for good: the words say so, not that a retry may help
      await expect(refusal).toContainText(/moved: HEAD is .*; it can't be undone any more/)
      await expect(refusal).not.toContainText("retry")

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
      const { calls, release } = await holdRewrites(page, "reword")
      await page.keyboard.press("ControlOrMeta+Enter")
      await expect.poll(() => calls.length).toBe(1)
      const answered = nextAnswer(page, "execute")
      release()
      await answered

      // the save is over (its toast would come before the edit ends); the message reached the backend unchanged
      await expect(editor).toHaveCount(0)
      expect(calls[0]!.rewrite).toMatchObject({ rewords: [{ text: { message: `(${BRANCH}) ${PERF_SUBJECT}` } }] })
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText(`${BRANCH} ${PERF_SUBJECT}`)
      await expect(page.locator("[data-slot=toast]")).toHaveCount(0)
      expect(await finishedSyncCount(page)).toBe(syncs)
    })

    test("should keep the edit enabled while a sync runs: the save goes to the session at once", async ({ page }) => {
      const preview = await openPerfCommit(page)
      const editButton = preview.getByRole("button", { name: /Edit/ })
      await expect(editButton).toBeEnabled()

      // hold the sync until the checks are done
      const releaseSync = await holdCommand(page, "sync")
      const syncDone = syncFromMenu(page)
      await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")

      await expect(editButton).toBeEnabled()
      await page.keyboard.press("e")
      await expect(preview.getByTestId("commit-message-editor")).toBeFocused()

      releaseSync()
      await syncDone
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
      await expectClosed(preview)
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

    await unassignedRow.getByTestId("commit-row-mark").click()
    await expect(body).toHaveText("Skip the cache warm-up that hangs on startup until CRITICAL-112 is fixed.")
    await expect(unassignedRow).toHaveAttribute("data-selected", "false")
    // issue keys in the body are links, as in the subject
    await expect(body.getByRole("link", { name: "CRITICAL-112" })).toHaveAttribute("href", "https://jira.example.com/browse/CRITICAL-112")

    // reading or selecting the body text does not select the row either
    // near the start: the middle of the text is the CRITICAL-112 link
    await body.locator("p").click({ position: { x: 4, y: 4 } })
    await page.keyboard.press("Escape")
    await expectClosed(body)
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
