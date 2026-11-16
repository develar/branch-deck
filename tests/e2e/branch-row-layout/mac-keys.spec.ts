// @spec openspec/specs/branch/branch-row/spec.md
import { expect, test } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import { openContextMenu } from "../helpers/selection-helpers"

// Chromium of the other tests says it runs on Windows, and the page names the keys by the platform its user agent
// tells (`isMac`): here macOS, whose keys are symbols, a modifier beside its key
test.describe("The keys on macOS", () => {
  test.use({ userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)" })

  test("should name the keys as a Mac does: ⌘R and ⌘1 in the descriptions, ⌫ for the drop of a commit in its menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("drop_commits")
    await syncAndWaitForBranches()
    // the words of the tooltip of Sync and of a main view, their key as the Mac writes it, no "+" between
    await expect(page.getByTestId("sync-button")).toHaveAccessibleDescription("Sync branches · Shortcut: ⌘R")
    await expect(page.getByTestId("main-view-branches")).toHaveAccessibleDescription("Shortcut: ⌘1")

    const authRow = findBranchRow(page, "auth")
    await expandBranch(page, authRow)
    await openContextMenu(page, getBranchCommitRow(page, "auth", "Check the password"))
    // Delete by its symbol, not "Del" nor "DEL"
    await expect(page.getByRole("menuitem", { name: /^Drop Commit…/ })).toHaveText("Drop Commit…⌫")
  })
})
