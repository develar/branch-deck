// @spec spec/commit/fixups.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow, getUnassignedCommitRow, verifyCommitCount } from "../helpers/branch-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"

function popover(page: Page): Locator {
  return page.getByTestId("squash-fixups-popover")
}

function unassignedRows(page: Page): Locator {
  return page.getByTestId("unassigned-commits-section").locator("[data-row-id]")
}

/** The row of a fixup commit (its subject is shown without the marker, like its target's) */
function fixupRow(rows: Locator, page: Page): Locator {
  return rows.filter({ has: page.getByTestId("fixup-marker") })
}

test.describe("Squash fixups", () => {
  test("should mark fixups, squash those of a branch from its commits and undo it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("fixups")
    await syncAndWaitForBranches()
    const authRow = findBranchRow(page, "auth")
    // the fixup without a target is not counted; the unassigned `squash!` counts for the branch of its target
    await expect(authRow.getByTestId("branch-fixup-count")).toHaveText("1 fixup")
    await expect(findBranchRow(page, "docs").getByTestId("branch-fixup-count")).toHaveText("1 fixup")
    await expect(page.getByTestId("unassigned-commits-section").getByTestId("squash-fixups-button")).toHaveText("Squash 1 Fixup")
    await verifyCommitCount(authRow, "3 commits")

    await expandBranch(page, authRow)
    const fixup = fixupRow(getBranchCommitRow(page, "auth", "Add login form"), page)
    await fixup.getByTestId("fixup-marker").hover()
    // the "Collapse" tooltip of the expand button may still be closing
    await expect(page.getByRole("tooltip").filter({ hasText: "Squashes into" })).toHaveText("Squashes into “Add login form”")
    await page.mouse.move(0, 0)
    const orphan = getBranchCommitRow(page, "auth", "Drop the session cookie")
    await expect(orphan.getByTestId("fixup-marker")).toHaveAttribute("data-target", "none")

    await getBranchDetailsRow(page, "auth").getByTestId("squash-fixups-button").click()
    const confirm = popover(page)
    await expect(confirm.getByTestId("squash-fixups-groups").locator("li")).toHaveText(["Add login formfixup!Add login form"])
    await expect(confirm.getByTestId("squash-fixups-orphans")).toContainText("Won't be squashed: no target in this range")
    await expect(confirm.getByTestId("squash-fixups-orphans")).toContainText("fixup!Drop the session cookie")
    await confirm.getByTestId("squash-fixups-confirm").click()

    await expect(confirm).toBeHidden()
    await expect(page.getByText("Squashed 1 fixup")).toBeVisible()
    await verifyCommitCount(authRow, "2 commits")
    await expect(authRow.getByTestId("branch-fixup-count")).toHaveCount(0)

    await page.getByRole("button", { name: "Undo" }).click()
    await verifyCommitCount(authRow, "3 commits")
    await expect(authRow.getByTestId("branch-fixup-count")).toHaveText("1 fixup")
  })

  test("should squash from the branch menu and one fixup from the commit menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("fixups")
    await syncAndWaitForBranches()
    await expect(unassignedRows(page)).toHaveCount(3)

    // the branch menu expands the branch and opens the confirmation there; it lists "Squash Fixups" from the plan that
    // loads after the sync, which the fixup count of the row shows
    const docsRow = findBranchRow(page, "docs")
    await expect(docsRow.getByTestId("branch-fixup-count")).toHaveText("1 fixup")
    await openContextMenu(page, docsRow)
    await clickContextMenuItem(page, "Squash Fixups (1)…")
    const confirm = popover(page)
    await expect(confirm.getByTestId("squash-fixups-groups").locator("li")).toHaveText(["Write the guidesquash!Write the guide"])
    await confirm.getByTestId("squash-fixups-confirm").click()
    await expect(confirm).toBeHidden()
    // the `squash!` was an unassigned commit; its body joins the message of the target
    await expect(unassignedRows(page)).toHaveCount(2)
    await expect(getUnassignedCommitRow(page, "Write the guide")).toHaveCount(0)
    await expect(getBranchCommitRow(page, "docs", "Write the guide").getByTestId("commit-message-toggle")).toBeVisible()

    // the squash rewrote the unassigned commits: the menu offers "Squash into" once the plan of the new hashes is loaded,
    // which the marker shows with its target
    const buildFixup = fixupRow(getUnassignedCommitRow(page, "Add build script"), page)
    await expect(buildFixup.getByTestId("fixup-marker")).toHaveAttribute("data-target", /^[0-9a-f]+$/)
    await openContextMenu(page, buildFixup)
    await clickContextMenuItem(page, "Squash into “Add build script”")
    await expect(unassignedRows(page)).toHaveCount(1)
    await expect(page.getByTestId("unassigned-commits-section").getByTestId("squash-fixups-button")).toHaveCount(0)
  })
})
