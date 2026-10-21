// @spec spec/branch/stack-parents.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow } from "../helpers/branch-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { expectDisabledItems } from "../helpers/safe-push-helpers"
import { clickContextMenuItem, openContextMenu, openSubmenu } from "../helpers/selection-helpers"
import { runAndWaitForSync } from "../helpers/sync-helpers"
import { holdCommandCalls } from "../helpers/wait-helpers"

/** The branch row above `name` one level up, the parent the table shows it under; null for a root */
async function shownParent(page: Page, name: string): Promise<string | null> {
  return page.getByTestId("branch-row").evaluateAll((rows, name) => {
    const depthOf = (row: Element) => Number(row.getAttribute("data-depth") ?? 0)
    const index = rows.findIndex(row => row.getAttribute("data-branch-name") === name)
    const depth = depthOf(rows[index]!)
    for (let above = index - 1; above >= 0; above--) {
      if (depthOf(rows[above]!) === depth - 1) {
        return rows[above]!.getAttribute("data-branch-name")
      }
    }
    return null
  }, name)
}

/** The alert of the stack problem in the expanded row of `name` */
async function problemAlert(page: Page, name: string) {
  const row = findBranchRow(page, name)
  await expandBranch(page, row)
  return getBranchDetailsRow(page, name).getByTestId("branch-stack-problem")
}

test.describe("Stack problems", () => {
  test("should name conflicting parents, keep the commits and stack on a parent from the alert", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_problems")
    await syncAndWaitForBranches()

    const row = findBranchRow(page, "publish-viewer")
    // the badge names the problem, not an internal error
    await expect(row).toContainText("conflicting parents")
    await expect(row).not.toContainText("internal error")
    const alert = await problemAlert(page, "publish-viewer")
    await expect(alert).toContainText("Commits declare different parents: ci-traces (")
    await expect(alert).toContainText("trace-tools (its stack parent)")
    // the app offers the fixes as its actions, never as commands of bd
    await expect(alert).not.toContainText("bd ")
    await expect(alert.getByTestId("stack-problem-fix")).toHaveText(["Stack On ci-traces", "Stack On trace-tools", "Unstack"])
    // the commits stay, among them the one whose prefix declares ci-traces
    await expect(getBranchCommitRow(page, "publish-viewer", "Publish the viewer")).toBeVisible()
    // the child is blocked by it and says why
    await expect(findBranchRow(page, "viewer-docs")).toContainText("blocked by publish-viewer")

    const restack = await holdCommandCalls(page, "restack_branch")
    // the button says the running action instead of its name: take it by its place
    const fix = alert.getByTestId("stack-problem-fix").first()
    await fix.click()
    await expect(fix).toHaveAttribute("aria-busy", "true")
    await expect(fix).toHaveText("Stacking publish-viewer on ci-traces…")
    await runAndWaitForSync(page, async () => restack.release())
    expect(restack.calls).toHaveLength(1)

    await expect(page.getByText(/^Stacked publish-viewer on ci-traces/)).toBeVisible()
    await expect(getBranchDetailsRow(page, "publish-viewer").getByTestId("branch-stack-problem")).toHaveCount(0)
    await expect(findBranchRow(page, "publish-viewer")).not.toContainText("conflicting parents")
    await expect.poll(() => shownParent(page, "publish-viewer")).toBe("ci-traces")
    await expect.poll(() => shownParent(page, "viewer-docs")).toBe("publish-viewer")
    await expect(findBranchRow(page, "viewer-docs")).not.toContainText("blocked by")
  })

  test("should offer the fixes in the menu and block the other rewrites with the problem", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_problems")
    await syncAndWaitForBranches()

    await openContextMenu(page, findBranchRow(page, "publish-viewer"))
    await expectDisabledItems(page, [
      [/^Move to…/, "publish-viewer has conflicting parents"],
      ["Rename…", "publish-viewer has conflicting parents"],
      ["Unapply…", "publish-viewer has conflicting parents"],
      // the review waits for a build, the problem says why there is none
      [/^Review Changes/, "publish-viewer has conflicting parents"],
    ])
    await expect(page.getByRole("menuitem", { name: "Unstack", exact: true })).not.toHaveAttribute("aria-disabled", "true")
    // Stack On… lists the parents its fixes name: the stack parent too, which no prefix declares
    await openSubmenu(page, "Stack On…")
    await expect(page.getByRole("menu").last().getByRole("menuitem")).toHaveText(["ci-traces", "trace-tools"])
    await runAndWaitForSync(page, () => page.getByRole("menuitem", { name: "trace-tools", exact: true }).click())

    await expect.poll(() => shownParent(page, "publish-viewer")).toBe("trace-tools")
    await expect(findBranchRow(page, "publish-viewer")).not.toContainText("conflicting parents")
    // the prefix that declared ci-traces lost its parent level
    await expandBranch(page, findBranchRow(page, "publish-viewer"))
    await expect(getBranchCommitRow(page, "publish-viewer", "Publish the viewer")).toBeVisible()
    await expect(getBranchDetailsRow(page, "publish-viewer")).not.toContainText("ci-traces")
  })

  test("should unstack a branch with conflicting parents from the menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_problems")
    await syncAndWaitForBranches()

    await openContextMenu(page, findBranchRow(page, "publish-viewer"))
    await runAndWaitForSync(page, () => clickContextMenuItem(page, "Unstack"))
    await expect(page.getByText(/^Unstacked publish-viewer/)).toBeVisible()
    await expect.poll(() => shownParent(page, "publish-viewer")).toBeNull()
    await expect(findBranchRow(page, "publish-viewer")).not.toContainText("conflicting parents")
  })

  test("should cut a branch out of a parent cycle from its alert", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_problems")
    await syncAndWaitForBranches()

    await expect(findBranchRow(page, "lane-a")).toContainText("parent cycle")
    await expect(findBranchRow(page, "lane-b")).toContainText("parent cycle")
    const alert = await problemAlert(page, "lane-a")
    await expect(alert).toContainText("The parents form a cycle: lane-a -> lane-b -> lane-a")
    await runAndWaitForSync(page, () => alert.getByRole("button", { name: "Unstack from lane-b" }).click())

    await expect(findBranchRow(page, "lane-a")).not.toContainText("parent cycle")
    await expect(findBranchRow(page, "lane-b")).not.toContainText("parent cycle")
    await expect.poll(() => shownParent(page, "lane-b")).toBe("lane-a")
  })

  test("should open the rename of a branch whose name differs from another only in case", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_problems")
    await syncAndWaitForBranches()

    await expect(findBranchRow(page, "Notes")).toContainText("case clash")
    const alert = await problemAlert(page, "Notes")
    await expect(alert).toContainText("Branch notes has the same name ignoring case")
    await alert.getByRole("button", { name: "Rename…" }).click()
    const input = page.getByTestId("inline-rename-input").locator('input[type="text"]')
    await expect(input).toBeFocused()
    await input.fill("release-notes")
    await runAndWaitForSync(page, () => input.press("Enter"))

    await expect(findBranchRow(page, "release-notes")).not.toContainText("case clash")
    await expect(findBranchRow(page, "notes")).not.toContainText("case clash")
  })

  test("should show the problem with its fixes in the preview, a fix closing the preview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_problems")
    await syncAndWaitForBranches()

    const preview = await openBranchPreview(page, "publish-viewer")
    const alert = preview.getByTestId("branch-stack-problem")
    await expect(alert).toContainText("Commits declare different parents")
    await expect(preview.getByText("Sync failed")).toHaveCount(0)
    await runAndWaitForSync(page, () => alert.getByRole("button", { name: "Unstack" }).click())
    await expect(preview).toBeHidden()
    await expect.poll(() => shownParent(page, "publish-viewer")).toBeNull()
  })
})
