// @spec spec/commit/commit-row.spec.md
import type { CommitReviewWindowData } from "../../../src/lib/commit-review-window"
import type { ConflictingFilesWindowData, MissingCommitsWindowData } from "../../../src/types/sub-window"
import { testWithConflicts, expect } from "../fixtures/test-fixtures"
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
      await expect(file).toContainText("One side deleted the file, the other changed it: the change of the commit")
    })

    test("should load the contents of a conflict when its view shows it, once for all its files", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)
      // the first conflicting file is open: the change of the commit, from the contents
      const conflictingFile = bugFixDetails.getByTestId("conflicting-files").getByTestId("file-diff").first()
      await expect(conflictingFile).toContainText(COMMIT_CHANGE)

      // closed, the view drops the contents; shown again, one read gives the conflicting files and the missing commits
      await collapseBranch(page, bugFixBranch)
      // the only conflict of the repository: its view is gone
      await expect(page.getByTestId("conflict-title")).toHaveCount(0)
      const reads: string[] = []
      page.on("request", request => {
        if (request.url().endsWith("/invoke/get_conflict_contents")) {
          reads.push(request.postData() ?? "")
        }
      })
      await expandBranch(page, bugFixBranch)
      await expect(conflictingFile).toContainText(COMMIT_CHANGE)
      // the first missing commit, of feature-auth: "Add authentication to UserService"
      const missingFile = bugFixDetails.getByTestId("missing-commits").getByTestId("file-diff").first()
      await missingFile.getByRole("button", { name: USER_SERVICE }).click()
      await expect(missingFile).toContainText(MISSING_COMMIT_CHANGE)
      expect(reads).toHaveLength(1)
      const conflict = JSON.parse(reads[0]!) as { commit: string; parent: string; target: string }
      expect(conflict).toMatchObject({ commit: expect.stringMatching(/^[0-9a-f]{40}$/), parent: expect.stringMatching(/^[0-9a-f]{40}$/), target: expect.stringMatching(/^[0-9a-f]{40}$/) })
    })

    test("should say why the contents of a conflict did not load", async ({ page }) => {
      await routeCommandError(page, "get_conflict_contents", "Commit 1234 not found")
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await collapseBranch(page, bugFixBranch)
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      await expect(page.locator("[data-slot=toast]", { hasText: "Cannot load the conflict" })).toContainText("Commit 1234 not found")
      // the open file says it too; its header and conflict count came with the sync
      const conflictingFile = bugFixDetails.getByTestId("conflicting-files").getByTestId("file-diff").first()
      await expect(conflictingFile).toContainText("Cannot load the conflict: Commit 1234 not found")
      await expect(conflictingFile.getByTestId("file-conflict-count")).toHaveText("modify/delete")
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
      const fileAccordion = bugFixDetails.getByTestId("missing-commits").locator("button:has-text('src/main/kotlin/com/example/service/UserService.kt')").first()
      await expect(fileAccordion).toBeVisible()

      // Capture initial state (accordion collapsed) - capture the entire details row
      await captureHtmlSnapshot(bugFixDetails, "missing-commits-accordion-collapsed")

      // Click to expand the accordion
      await fileAccordion.click()

      // Wait for accordion content to be visible - look for the expanded state
      await expect(fileAccordion).toHaveAttribute("aria-expanded", "true")

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

    test("should persist the diff view mode", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const missingCommits = getBranchDetailsRow(page, "bug-fix").getByTestId("missing-commits")
      await waitForConflictViewer(page, getBranchDetailsRow(page, "bug-fix"))
      await missingCommits.getByRole("button", { name: "Split" }).click()
      await expect(missingCommits.getByRole("button", { name: "Split" })).toHaveAttribute("aria-pressed", "true")

      await waitForStorePersistence(page, "conflictViewer", settings => (settings as { conflictDiffViewMode?: string } | undefined)?.conflictDiffViewMode === "split")
      await page.reload()
      await syncBranches(page, ['[data-testid="branch-row"][data-branch-name="feature-auth"]', '[data-testid="branch-row"][data-branch-name="bug-fix"]'])
      const bugFixAfterReload = findBranchRow(page, "bug-fix")
      if ((await bugFixAfterReload.getAttribute("data-state")) !== "open") {
        await expandBranch(page, bugFixAfterReload)
      }
      const missingAfterReload = getBranchDetailsRow(page, "bug-fix").getByTestId("missing-commits")
      await expect(missingAfterReload.getByRole("button", { name: "Split" })).toHaveAttribute("aria-pressed", "true")
    })
  })
})
