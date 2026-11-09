// @spec openspec/specs/commit/fixups/spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { branchTree, expandBranch, findBranchRow, getBranchCommitRow, getUnassignedCommitRow, verifyCommitCount } from "../helpers/branch-helpers"
import { editCommitMessage } from "../helpers/preview-helpers"
import { clickContextMenuItem, openContextMenu } from "../helpers/selection-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"

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
  test("should mark fixups, squash those of a branch from the fixup badge of its row and undo it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("fixups")
    await syncAndWaitForBranches()
    const authRow = findBranchRow(page, "auth")
    // the fixup without a target is not counted; the unassigned `squash!` counts for the branch of its target
    await expect(authRow.getByTestId("branch-fixup-count")).toHaveText("1 fixup")
    await expect(findBranchRow(page, "docs").getByTestId("branch-fixup-count")).toHaveText("1 fixup")
    await expect(page.getByTestId("unassigned-commits-section").getByTestId("squash-fixups-button")).toHaveText("Squash 1 Fixup…")
    await verifyCommitCount(authRow, 3)

    await expandBranch(page, authRow)
    const fixup = fixupRow(getBranchCommitRow(page, "auth", "Add login form"), page)
    await fixup.getByTestId("fixup-marker").hover()
    // the "Collapse" tooltip of the expand button may still be closing
    await expect(page.getByRole("tooltip").filter({ hasText: "Squashes into" })).toHaveText("Squashes into “Add login form”")
    await page.mouse.move(0, 0)
    const orphan = getBranchCommitRow(page, "auth", "Drop the session cookie")
    await expect(orphan.getByTestId("fixup-marker")).toHaveAttribute("data-target", "none")

    // the badge names what its press does, and the press neither selects nor collapses the row
    const badge = authRow.getByRole("button", { name: "1 fixup" })
    await badge.hover()
    const badgeTooltip = page.getByRole("tooltip").filter({ hasText: /^Squash 1 Fixup…$/ })
    await expect(badgeTooltip).toBeVisible()
    await badge.click()
    const confirm = popover(page)
    await expect(confirm.getByTestId("squash-fixups-groups").locator("li")).toHaveText(["Add login formfixup!Add login form"])
    await expect(badgeTooltip).toHaveCount(0)
    await expect(authRow).toHaveAttribute("data-state", "open")
    await expect(authRow).toHaveAttribute("data-selected", "false")
    await expect(confirm.getByTestId("squash-fixups-orphans")).toContainText("Won't be squashed: no target in this range")
    await expect(confirm.getByTestId("squash-fixups-orphans")).toContainText("fixup!Drop the session cookie")
    await confirm.getByTestId("squash-fixups-confirm").click()

    await expect(confirm).toBeHidden()
    await expect(page.getByText("Squashed 1 fixup")).toBeVisible()
    await verifyCommitCount(authRow, 2)
    await expect(authRow.getByTestId("branch-fixup-count")).toHaveCount(0)

    await runAndWaitForSync(page, () => page.getByRole("button", { name: "Undo" }).click())
    await verifyCommitCount(authRow, 3)
    await expect(authRow.getByTestId("branch-fixup-count")).toHaveText("1 fixup")
  })

  test("should squash from the branch menu and one fixup from the commit menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("fixups")
    await syncAndWaitForBranches()
    await expect(unassignedRows(page)).toHaveCount(3)

    // the branch menu opens the confirmation at the fixup badge of the row, its commits left collapsed; it lists "Squash
    // Fixups" from the plan that loads after the sync, which the fixup count of the row shows
    const docsRow = findBranchRow(page, "docs")
    await expect(docsRow.getByTestId("branch-fixup-count")).toHaveText("1 fixup")
    await openContextMenu(page, docsRow)
    // one name for the button and the menu item
    await clickContextMenuItem(page, "Squash 1 Fixup…")
    const confirm = popover(page)
    await expect(confirm.getByTestId("squash-fixups-groups").locator("li")).toHaveText(["Write the guidesquash!Write the guide"])
    await expect(docsRow).not.toHaveAttribute("data-state", "open")
    await confirm.getByTestId("squash-fixups-confirm").click()
    await expect(confirm).toBeHidden()
    // the `squash!` was an unassigned commit; its body joins the message of the target
    await expect(unassignedRows(page)).toHaveCount(2)
    await expect(getUnassignedCommitRow(page, "Write the guide")).toHaveCount(0)
    await expandBranch(page, docsRow)
    await expect(getBranchCommitRow(page, "docs", "Write the guide").getByTestId("commit-row-mark")).toBeVisible()

    // the squash rewrote the unassigned commits: the menu offers "Squash into" once the plan of the new hashes is loaded,
    // which the marker shows with its target
    const buildFixup = fixupRow(getUnassignedCommitRow(page, "Add build script"), page)
    await expect(buildFixup.getByTestId("fixup-marker")).toHaveAttribute("data-target", /^[0-9a-f]+$/)
    await openContextMenu(page, buildFixup)
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "Squash into “Add build script”"))
    await expect(unassignedRows(page)).toHaveCount(1)
    await expect(page.getByTestId("unassigned-commits-section").getByTestId("squash-fixups-button")).toHaveCount(0)
  })
  test("should drop a target its fixup cancels, say so in the toast and bring it back with Undo", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("fixup_cancels")
    await syncAndWaitForBranches()
    const authRow = findBranchRow(page, "auth")
    await verifyCommitCount(authRow, 2)
    await authRow.getByTestId("branch-fixup-count").click()
    const confirm = popover(page)
    await expect(confirm).toHaveAccessibleDescription(
      "It is folded into its target, as with rebase --autosquash. A target it cancels leaves the history; your files are not touched.",
    )
    await runAndWaitForSync(page, () => confirm.getByTestId("squash-fixups-confirm").click())

    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Squashed 1 fixup" })
    await expect(toast).toContainText("“(auth) Rename the title” left the history: its fixups cancel it")
    // the branch had no other commit: it is gone, the fixup of docs is still there
    await expect(authRow).toHaveCount(0)
    await expect(findBranchRow(page, "docs").getByTestId("branch-fixup-count")).toHaveText("1 fixup")

    await runAndWaitForSync(page, () => toast.getByRole("button", { name: "Undo" }).click())
    await verifyCommitCount(findBranchRow(page, "auth"), 2)
  })
  // openspec/specs/branch/stacked-branches/spec.md: a rewrite of the commits of a branch moves no row
  test("should keep every row in its place when the fixup of the branch that began first is squashed", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("fixup_order")
    await syncAndWaitForBranches()
    // auth began first, so its fixup, the newest commit but one, does not lift it above docs and search
    const rows = ["search@0", "docs@0", "auth@0", "session@1"]
    expect(await branchTree(page)).toEqual(rows)
    const authRow = findBranchRow(page, "auth")
    // the badge of a collapsed row opens the confirmation as well
    await authRow.getByTestId("branch-fixup-count").click()
    const confirm = popover(page)
    // one fixup reads as one
    await expect(confirm).toHaveAccessibleName("Squash 1 fixup")
    await expect(confirm).toHaveAccessibleDescription(
      "It is folded into its target, as with rebase --autosquash. A target it cancels leaves the history; your files are not touched.",
    )
    // below the first row, so a header that the facts of the sync move moves both
    const offset = async () => (await authRow.boundingBox())!.y - (await findBranchRow(page, "search").boundingBox())!.y
    const before = await offset()
    await confirm.getByTestId("squash-fixups-confirm").click()

    await expect(page.getByText("Squashed 1 fixup")).toBeVisible()
    await verifyCommitCount(authRow, 1)
    // every commit above the target got a new committer time: no row moved under the pointer
    expect(await branchTree(page)).toEqual(rows)
    expect(await offset()).toBe(before)
  })
  // openspec/specs/commit/commit-row/spec.md: the reword in the app checks the fixups as `bd reword` does
  test("should refuse a reword that strands a fixup, and keep the draft", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("fixups")
    await syncAndWaitForBranches()
    const authRow = findBranchRow(page, "auth")
    await verifyCommitCount(authRow, 3)
    await expandBranch(page, authRow)
    const rows = getBranchCommitRow(page, "auth", "Add login form")
    const target = rows.filter({ hasNot: page.getByTestId("fixup-marker") })
    const fixupId = await fixupRow(rows, page).getAttribute("data-row-id")
    const editor = await editCommitMessage(page, target)

    // `fixup! (auth) Add login form` would fold into nothing
    const draft = "(auth) Add the login form"
    await editor.fill(draft)
    await page.keyboard.press("ControlOrMeta+Enter")

    const refusal = page.locator("[data-slot=toast]").filter({ hasText: "Cannot edit the commit message" })
    await expect(refusal).toContainText(`Fixups would fold into another target or none: ${fixupId!.slice(0, 8)} (squash the fixups into their target first)`)
    await expect(editor).toHaveValue(draft)
    await expect(editor).toBeEnabled()
    await page.keyboard.press("Escape")
    await page.keyboard.press("Escape")
    await expect(target).toHaveCount(1)
    await verifyCommitCount(authRow, 3)
  })
})
