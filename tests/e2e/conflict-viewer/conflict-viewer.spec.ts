// @spec spec/commit/commit-row.spec.md
import type { CommitReviewWindowData } from "../../../src/lib/commit-review-window"
import type { ConflictingFilesWindowData, MissingCommitsWindowData } from "../../../src/types/sub-window"
import { test as simpleTest, testWithConflicts, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, expandBranch, collapseBranch, getBranchDetailsRow, verifyBranchState } from "../helpers/branch-helpers"
import { getOpenedSubWindows, openSubWindowPage } from "../helpers/sub-window-helpers"
import { syncBranches } from "../helpers/sync-helpers"
import { routeCommandError, waitForConflictViewer, waitForStorePersistence } from "../helpers/wait-helpers"

const test = testWithConflicts

/** The conflicting file of bug-fix, which also has its missing commits */
const USER_SERVICE = "src/main/kotlin/com/example/service/UserService.kt"
/** A line of the change of the commit of bug-fix in `UserService.kt`: what the contents of its conflict hold */
const COMMIT_CHANGE = "BCrypt.hashpw(password, BCrypt.gensalt())"
/** A line of the missing commit of feature-auth in `UserService.kt` */
const MISSING_COMMIT_CHANGE = "fun authenticate(email: String, password: String)"

test.describe("Conflict Viewer Feature", () => {
  test.describe("Branch Conflicts", () => {
    test.beforeEach(async ({ setupConflictRepo }) => {
      await setupConflictRepo("conflict_branches")
    })

    test("should show branches with missing commits", async ({ page }) => {
      // Check for good branch (feature-auth) - should sync cleanly
      const featureAuthBranch = findBranchRow(page, "feature-auth")
      await expect(featureAuthBranch).toBeVisible()

      // Feature-auth should NOT be expanded (synced successfully)
      await verifyBranchState(featureAuthBranch, "closed")

      // Check for bug-fix branch - should have conflicts due to missing commits
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expect(bugFixBranch).toBeVisible()

      // Expand the bug-fix branch to see conflicts
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")

      // Wait for conflict viewer to be visible
      await waitForConflictViewer(page, bugFixDetails)
      // the badge says "Merge conflict"; the title names the commit and what it does not apply onto
      await expect(bugFixDetails.getByTestId("conflict-title")).toHaveText(/^“Implement secure password hashing” does not apply cleanly onto (origin\/)?master$/)
      // the explanation names the branches of the missing commits and what the branch is built on
      const explanation = bugFixDetails.getByTestId("conflict-explanation")
      await expect(explanation).toContainText("1 commit of feature-auth changed these files first, as did 1 unassigned commit")
      await expect(explanation).toContainText(/bug-fix is built on (origin\/)?master, which doesn't have them\./)
      // the missing commits are grouped by their branch, the unassigned ones last
      await expect(bugFixDetails.getByTestId("missing-commits").locator("[data-missing-group]")).toHaveText(["feature-auth1 commit", "Unassigned1 commit"])
      // the modify/delete file shows the change of the commit, from its merge
      await expect(bugFixDetails.getByTestId("conflicting-files").getByTestId("file-diff").first()).toContainText(COMMIT_CHANGE)

      await captureHtmlSnapshot(bugFixDetails, "conflict-viewer")
    })

    test("should show the branch of the missing commits from the explanation", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      await bugFixDetails.getByTestId("missing-commit-owners").getByRole("button", { name: "feature-auth" }).click()
      // the branch opens with its missing commit
      await expect(getBranchDetailsRow(page, "feature-auth")).toContainText("Add authentication to UserService")
    })

    test("should review a missing commit in the review window from the missing commits window", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      // the window of the missing commits names the repository: its diffs and the reviews read it
      await bugFixDetails.getByTestId("missing-commits").getByRole("button", { name: "Open in Window" }).click()
      await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(1)
      const [missingCommits] = await getOpenedSubWindows<MissingCommitsWindowData>(page)
      expect(missingCommits).toMatchObject({ windowId: "missing-commits" })
      const repositoryPath = missingCommits!.data.repositoryPath
      expect(repositoryPath).toMatch(/\S/)

      // D on a commit of its list: the review window, from the sub-window (its page starts with no recorded opens)
      await openSubWindowPage(page, missingCommits!)
      const row = page.getByTestId("missing-commits-window").locator("tr[data-row-id]", { hasText: "Add authentication to UserService" })
      await row.hover()
      await page.keyboard.press("d")
      await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(1)
      const [review] = await getOpenedSubWindows<CommitReviewWindowData>(page)
      expect(review).toMatchObject({ windowId: "commit-review", url: "/commit-review" })
      expect(review!.data).toMatchObject({ repositoryPath, path: null })
      expect(review!.data.commits[review!.data.index]!.subject).toContain("Add authentication to UserService")
    })

    test("should list every conflicting file, the first open, and say that a file without conflict markers is a modify/delete", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      const conflictingFiles = bugFixDetails.getByTestId("conflicting-files")
      const badge = conflictingFiles.locator("[data-slot=badge]").first()
      await expect(badge).toHaveText(/^[1-9]\d* files?$/)
      const fileCount = Number.parseInt((await badge.textContent())!, 10)

      const files = conflictingFiles.getByTestId("file-diff")
      await expect(files).toHaveCount(fileCount)
      const file = files.first()
      await expect(file.getByTestId("file-conflict-count")).toHaveText("modify/delete")
      // open without a click: the change of the commit under the line that says what happened
      await expect(file.getByTestId("merge-one-sided")).toHaveText(/^(origin\/)?master does not have the file, the commit changes it: the change of the commit$/)
      await expect(file).toContainText(COMMIT_CHANGE)
    })

    test("should read the merges of a conflict when its view shows it, once for all its files", async ({ page }) => {
      const reads: string[] = []
      page.on("request", request => {
        if (request.url().endsWith("/invoke/get_file_merges")) {
          reads.push(request.postData() ?? "")
        }
      })
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)
      // the first conflicting file is open: the change of the commit, from its merge
      const conflictingFile = bugFixDetails.getByTestId("conflicting-files").getByTestId("file-diff").first()
      await expect(conflictingFile).toContainText(COMMIT_CHANGE)

      // closed and shown again: a merge of git's objects never changes, so the view reads it once
      await collapseBranch(page, bugFixBranch)
      // the only conflict of the repository: its view is gone
      await expect(page.getByTestId("conflict-title")).toHaveCount(0)
      await expandBranch(page, bugFixBranch)
      await expect(conflictingFile).toContainText(COMMIT_CHANGE)
      // the first missing commit, of feature-auth: "Add authentication to UserService"
      const missingFile = bugFixDetails.getByTestId("missing-commits").getByTestId("file-diff").first()
      await missingFile.getByRole("button", { name: USER_SERVICE }).click()
      await expect(missingFile).toContainText(MISSING_COMMIT_CHANGE)
      expect(reads).toHaveLength(1)
      const params = JSON.parse(reads[0]!) as { source: { kind: string; commit: string; parent: string; target: string }; priorityPaths: string[] }
      expect(params).toMatchObject({
        source: {
          kind: "cherryPick",
          commit: expect.stringMatching(/^[0-9a-f]{40}$/),
          parent: expect.stringMatching(/^[0-9a-f]{40}$/),
          target: expect.stringMatching(/^[0-9a-f]{40}$/),
        },
        priorityPaths: [USER_SERVICE],
      })
    })

    test("should say why the merges of a conflict did not load", async ({ page }) => {
      await routeCommandError(page, "get_file_merges", "Commit 1234 not found")
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await collapseBranch(page, bugFixBranch)
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      // the header and the conflict count of the open file came with the sync; its merge says why it is missing
      const conflictingFile = bugFixDetails.getByTestId("conflicting-files").getByTestId("file-diff").first()
      await expect(conflictingFile.getByTestId("file-conflict-count")).toHaveText("modify/delete")
      await expect(conflictingFile).toContainText("Cannot load the conflict: Commit 1234 not found")
    })

    test("should load the contents of the conflict in the windows of its files and missing commits", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      await bugFixDetails.getByTestId("conflicting-files").getByRole("button", { name: "Open in Window" }).click()
      await bugFixDetails.getByTestId("missing-commits").getByRole("button", { name: "Open in Window" }).click()
      await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(2)
      const [files, missing] = await getOpenedSubWindows<ConflictingFilesWindowData | MissingCommitsWindowData>(page)
      // the windows get the conflict without its contents, and the repository to read them from
      for (const window of [files!, missing!]) {
        expect(window.data.repositoryPath).toMatch(/\S/)
        expect(window.data.conflict.conflictingFiles).toEqual([{ file: USER_SERVICE, conflictCount: 0 }])
        expect(JSON.stringify(window.data)).not.toContain(COMMIT_CHANGE)
      }

      await openSubWindowPage(page, files!)
      const filesWindow = page.getByTestId("conflicting-files-window")
      await expect(filesWindow.getByTestId("file-diff").first()).toContainText(COMMIT_CHANGE)

      await openSubWindowPage(page, missing!)
      const missingWindow = page.getByTestId("missing-commits-window")
      const missingFile = missingWindow.getByTestId("file-diff").first()
      await missingFile.getByRole("button", { name: USER_SERVICE }).click()
      await expect(missingFile).toContainText(MISSING_COMMIT_CHANGE)
    })

    test("should expand/collapse file accordions in missing commits details", async ({ page }) => {
      // Find the bug-fix branch that has conflicts
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expect(bugFixBranch).toBeVisible()

      // Expand the bug-fix branch to see conflicts
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")

      // Wait for conflict viewer to be visible
      await waitForConflictViewer(page, bugFixDetails)

      // Find the file accordion button of a missing commit (the conflicting files above list the same file)
      const fileAccordion = bugFixDetails.getByTestId("missing-commits").getByRole("button", { name: USER_SERVICE }).first()
      await expect(fileAccordion).toBeVisible()
      // the conflicting file above opens with its diff: the snapshot waits for it, not for the time it takes
      await expect(bugFixDetails.getByTestId("conflicting-files").getByTestId("file-diff").first()).toContainText(COMMIT_CHANGE)

      // Capture initial state (accordion collapsed) - capture the entire details row
      await captureHtmlSnapshot(bugFixDetails, "missing-commits-accordion-collapsed")

      // Click to expand the accordion
      await fileAccordion.click()

      // Wait for accordion content to be visible - look for the expanded state
      await expect(fileAccordion).toHaveAttribute("aria-expanded", "true")
      // the diff of the missing commit is read when its item opens: the snapshot waits for it
      await expect(bugFixDetails.getByTestId("missing-commits").getByTestId("file-diff").first()).toContainText(MISSING_COMMIT_CHANGE)

      // Capture expanded state (the capture waits for the expansion to settle)
      await captureHtmlSnapshot(bugFixDetails, "missing-commits-accordion-expanded")

      // Click accordion again to collapse
      await fileAccordion.click()

      // Verify accordion is collapsed
      await expect(fileAccordion).toHaveAttribute("aria-expanded", "false")

      // Capture final collapsed state
      await captureHtmlSnapshot(bugFixDetails, "missing-commits-accordion-collapsed-final")
    })
  })

  test.describe("Commit Preview", () => {
    test.beforeEach(async ({ setupConflictRepo }) => {
      await setupConflictRepo("conflict_branches")
    })

    test("should open one preview for a missing commit nested in a conflict row", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      // the missing commit row sits inside the row of the conflicting commit; both match `:hover`
      const missingCommitRow = bugFixDetails.locator("tr[data-row-id] tr[data-row-id]").filter({ hasText: "Add UserService" })
      await missingCommitRow.hover()
      await page.keyboard.press("Space")

      const preview = page.getByTestId("commit-preview")
      await expect(preview).toHaveCount(1)
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText("Add UserService")
      await expect(preview.getByTestId("commit-preview-position")).toHaveText("2 / 2")
      // a missing commit is not on the current branch: its message is read-only
      await expect(preview.getByRole("button", { name: /Edit/ })).toHaveCount(0)
    })
  })

  test.describe("Conflict Viewer Settings", () => {
    test.beforeEach(async ({ setupConflictRepo }) => {
      await setupConflictRepo("conflict_branches")
    })

    test("should keep a stored diff view mode over the Split default", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const missingCommits = getBranchDetailsRow(page, "bug-fix").getByTestId("missing-commits")
      await waitForConflictViewer(page, getBranchDetailsRow(page, "bug-fix"))
      await expect(missingCommits.getByRole("button", { name: "Split" })).toHaveAttribute("aria-pressed", "true")
      await missingCommits.getByRole("button", { name: "Unified" }).click()
      await expect(missingCommits.getByRole("button", { name: "Unified" })).toHaveAttribute("aria-pressed", "true")

      await waitForStorePersistence(page, "diffView", settings => (settings as { mode?: string } | undefined)?.mode === "unified")
      await page.reload()
      await syncBranches(page, ['[data-testid="branch-row"][data-branch-name="feature-auth"]', '[data-testid="branch-row"][data-branch-name="bug-fix"]'])
      const bugFixAfterReload = findBranchRow(page, "bug-fix")
      if ((await bugFixAfterReload.getAttribute("data-state")) !== "open") {
        await expandBranch(page, bugFixAfterReload)
      }
      const missingAfterReload = getBranchDetailsRow(page, "bug-fix").getByTestId("missing-commits")
      await expect(missingAfterReload.getByRole("button", { name: "Unified" })).toHaveAttribute("aria-pressed", "true")
    })
  })
})

simpleTest.describe("Merge of a conflicting file", () => {
  simpleTest.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the `conflict_words` template: the commit of ui changes the retries on the line whose timeout core raised first,
    // and adds a line
    await setupRepo("conflict_words")
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "ui").getByTestId("branch-status")).toHaveText("merge conflict")
  })

  simpleTest("shows the merge in three columns, with the result the engine proposes and the base", async ({ page }) => {
    const uiRow = findBranchRow(page, "ui")
    if ((await uiRow.getAttribute("data-state")) !== "open") {
      await expandBranch(page, uiRow)
    }
    const file = getBranchDetailsRow(page, "ui").getByTestId("conflicting-files").getByTestId("file-diff")
    await expect(file.getByTestId("file-conflict-count")).toHaveText("1 conflict")
    // ours, the middle, theirs
    const columns = file.getByTestId("merge-columns")
    await expect(columns).toContainText(/^(origin\/)?master/)
    await expect(columns).toContainText("“Retry three times and log it”")
    // git stops at the line, but the changes of the two sides touch other words of it
    await expect(file.getByTestId("merge-notice")).toHaveText("Git stops here; the conflict resolves by words")

    const conflict = file.locator(".dv-row[data-kind=merge]", { hasText: "limits" })
    const half = (row: typeof conflict, side: number) => row.locator(`.dv-half[data-side="${side}"]`)
    await expect(half(conflict, 0)).toHaveAttribute("data-cell", "merge-conflict")
    await expect(half(conflict, 0)).toContainText("{ timeout: 10, retries: 1 }")
    await expect(half(conflict, 2)).toContainText("{ timeout: 30, retries: 3 }")
    // Result (proposed): the conflict resolved by its words, marked as resolved
    await expect(file.getByRole("button", { name: "Result (proposed)" })).toHaveAttribute("aria-pressed", "true")
    await expect(half(conflict, 1)).toContainText("{ timeout: 10, retries: 3 }")
    await expect(half(conflict, 1)).toHaveAttribute("data-resolved", "")
    // the line the commit added: inserted on its side and in the result, a gap on the left
    const inserted = file.locator(".dv-row[data-kind=merge]", { hasText: "verbose" })
    await expect(half(inserted, 2)).toHaveAttribute("data-cell", "merge-inserted")
    await expect(half(inserted, 1)).toHaveAttribute("data-cell", "merge-inserted")
    await expect(half(inserted, 0)).toHaveAttribute("data-cell", "filler")

    // Base: the lines before both changes; the base has no line where the commit added one
    await file.getByRole("button", { name: "Base" }).click()
    await expect(file.getByRole("button", { name: "Base" })).toHaveAttribute("aria-pressed", "true")
    await expect(half(conflict, 1)).toContainText("{ timeout: 30, retries: 1 }")
    await expect(half(conflict, 1)).not.toHaveAttribute("data-resolved")
    await expect(half(inserted, 1)).toHaveAttribute("data-cell", "filler")
  })
})
