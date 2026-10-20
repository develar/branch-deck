// @spec spec/branch/patronus.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import { robot, setupRobots } from "../helpers/patronus-helpers"
import { hoverStatusChip } from "../helpers/review-status-helpers"

// the `issue_links` template, none pushed: JIRA-456 has three commits, newest first these
const SUBJECTS = ["Add connection retry logic", "increase pool size to 50", "Fix database connection pool timeout"] as const

async function expandRobotBranch(page: Page): Promise<Locator[]> {
  const row = findBranchRow(page, "JIRA-456")
  await expect(row.getByTestId("patronus-chip")).toHaveAttribute("data-kind", "running")
  await expandBranch(page, row)
  return SUBJECTS.map(subject => getBranchCommitRow(page, "JIRA-456", subject))
}

test.describe("Commits a running Safe Push covers", () => {
  test("should draw the line of the robot over the commits it took, and mark the one added after it started", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the robot started before the newest commit
    await setupRobots(page, setupRepo, syncAndWaitForBranches, robot("JIRA-456", "running-robot", { sourceRevision: "~1" }))
    const [added, taken, oldest] = await expandRobotBranch(page)
    await expect(added!).toHaveAttribute("data-robot-dot", "uncovered")
    await expect(added!).not.toHaveAttribute("data-robot-line")
    // the commit the robot took: its line runs down from it to the oldest commit, and ends there
    await expect(taken!).toHaveAttribute("data-robot-dot", "tip")
    await expect(taken!).toHaveAttribute("data-robot-line", "down")
    await expect(oldest!).not.toHaveAttribute("data-robot-dot")
    await expect(oldest!).not.toHaveAttribute("data-robot-line")
    await expect(page.getByTestId("robot-commit-marker")).toHaveCount(1)

    const marker = added!.getByTestId("robot-commit-marker")
    await expect(marker).toHaveText("not in Safe Push")
    await expect(marker).toHaveAttribute("data-change", "added")
    const popover = await hoverStatusChip(page, marker, "robot-commit-popover")
    await expect(popover.getByTestId("robot-commit-title")).toHaveText(/^Added after the Safe Push started on [0-9a-f]{8}$/)
    await expect(popover).toContainText("The result won't cover this commit. Restart Safe Push to test the current branch.")
    // the actions of the robot, as its own details name them: the branch is not pushed, so the restart pushes first
    await expect(popover.getByRole("button", { name: "Restart Safe Push", exact: true })).toHaveAttribute("data-action", "push-and-restart-robot")
    await expect(popover.getByRole("button", { name: "Cancel Safe Push", exact: true })).toBeVisible()
  })

  test("should draw the line of the robot over every commit while it took the branch as it is", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, robot("JIRA-456", "running-robot"))
    const [newest, middle, oldest] = await expandRobotBranch(page)
    // from the top of the rail through every dot down to the last one
    await expect(newest!).toHaveAttribute("data-robot-line", "both")
    await expect(middle!).toHaveAttribute("data-robot-line", "down")
    await expect(oldest!).not.toHaveAttribute("data-robot-line")
    for (const row of [newest!, middle!, oldest!]) {
      await expect(row).not.toHaveAttribute("data-robot-dot")
    }
    await expect(page.getByTestId("robot-commit-marker")).toHaveCount(0)
  })
})
