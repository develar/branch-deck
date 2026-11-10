// @spec openspec/specs/branch/patronus/spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow, getBranchDetailsRow } from "../helpers/branch-helpers"
import { expectOneLineAcross } from "../helpers/lane-pixels"
import { availableRobots, robot, setupRobots, setupRobotsWith, willFail } from "../helpers/patronus-helpers"
import { hoverStatusChip } from "../helpers/review-status-helpers"

// the `issue_links` template, none pushed: JIRA-456 has three commits, newest first these
const SUBJECTS = ["Add connection retry logic", "increase pool size to 50", "Fix database connection pool timeout"] as const

/**
 * The colours the user sees of the robot of `branch`: its ring around the node, its line in the branch row
 * (`landing-lanes`), and on the rail of `commit` the dot and the line
 */
async function robotColors(page: Page, branch: string, commit: Locator): Promise<{ ring: string; node: string; dot: string; rail: string }> {
  const row = findBranchRow(page, branch)
  const ring = await row
    .getByTestId("lane-ring")
    .getByTestId("robot-ring")
    .evaluate(it => getComputedStyle(it).color)
  // the colour its lines and its curve take (`currentColor`)
  const node = await row.getByTestId("landing-lanes").evaluate(it => getComputedStyle(it).color)
  const [dot, rail] = await commit.locator("> td").evaluate(it => [getComputedStyle(it, "::before").backgroundColor, getComputedStyle(it, "::after").backgroundColor])
  return { ring, node, dot: dot!, rail: rail! }
}

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
    await expect(oldest!).toHaveAttribute("data-robot-dot", "covered")
    await expect(oldest!).not.toHaveAttribute("data-robot-line")
    await expect(page.getByTestId("robot-commit-marker")).toHaveCount(1)
    // the robot lacks the first row: its line doesn't start at the node, which keeps the plain line down
    await expect(findBranchRow(page, "JIRA-456").getByTestId("landing-lanes")).toHaveCount(0)

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
      await expect(row).toHaveAttribute("data-robot-dot", "covered")
    }
    await expect(page.getByTestId("robot-commit-marker")).toHaveCount(0)
  })

  test("should start the line of the robot at the ring of its node, in the colour of the ring", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, robot("JIRA-456", "running-robot"))
    const [newest] = await expandRobotBranch(page)
    await expect(newest!).toHaveAttribute("data-robot-line", "both")
    // the line of the branch row runs from the node down into the rail, one line of the robot from the ring
    const lines = findBranchRow(page, "JIRA-456").getByTestId("landing-lanes")
    await expect(lines).toHaveCount(1)
    await expect(lines).not.toHaveAttribute("data-failing")
    const colors = await robotColors(page, "JIRA-456", newest!)
    expect(colors).toEqual({ ring: colors.ring, node: colors.ring, dot: colors.ring, rail: colors.ring })
    // and in one width: the line of the branch row and the line of the rail are the same pixels at the border
    const row = findBranchRow(page, "JIRA-456")
    await expectOneLineAcross(page, row, "bottom", row.getByTestId("lane-node"))
  })

  test("should draw the line and the dots of a robot that will fail in the colour of its ring", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobotsWith(page, setupRepo, syncAndWaitForBranches, { patronusDetails: willFail("running-robot") }, robot("JIRA-456", "running-robot"))
    await expect(findBranchRow(page, "JIRA-456").getByTestId("robot-progress")).toHaveAttribute("data-failing", "true")
    const [newest] = await expandRobotBranch(page)
    await expect(findBranchRow(page, "JIRA-456").getByTestId("lane-ring").getByTestId("robot-ring")).toHaveClass(/text-destructive/)
    await expect(findBranchRow(page, "JIRA-456").getByTestId("landing-lanes")).toHaveAttribute("data-failing", "true")
    await expect(getBranchDetailsRow(page, "JIRA-456").locator("[data-robot-failing]")).toHaveCount(1)
    const colors = await robotColors(page, "JIRA-456", newest!)
    expect(colors).toEqual({ ring: colors.ring, node: colors.ring, dot: colors.ring, rail: colors.ring })
    // not the primary of a robot that runs on: the ring of another row's robot, the node of a branch whose step it is
    expect(colors.ring).not.toBe(
      await findBranchRow(page, "ABC-123")
        .getByTestId("lane-node")
        .evaluate(it => getComputedStyle(it).color),
    )
  })

  test("should run the line of the robot of a side branch from the fork through its node into its commits", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // stacked_branches: session forks off auth and has one commit
    await setupRepo("stacked_branches", { patronus: availableRobots(robot("session", "session-robot")) })
    await syncAndWaitForBranches()
    const row = findBranchRow(page, "session")
    await expect(row.getByTestId("lane-ring")).toBeVisible()
    await expandBranch(page, row)
    const commit = getBranchCommitRow(page, "session", "Add session store")
    // the one commit is the first and the last row: the line of the robot runs from the top of the rail to its dot
    await expect(commit).toHaveAttribute("data-robot-line", "up")
    await expect(commit).toHaveAttribute("data-robot-dot", "covered")
    // one line of the robot in the row: the curve from auth into the node, and on from the node down
    await expect(row.getByTestId("landing-lanes")).toHaveCount(1)
    const colors = await robotColors(page, "session", commit)
    expect(colors).toEqual({ ring: colors.ring, node: colors.ring, dot: colors.ring, rail: colors.ring })
    // the curve leaves the line of auth above in its pixels, and the line from the node goes on into the rail in them
    await expectOneLineAcross(page, row, "top", findBranchRow(page, "auth").getByTestId("lane-node"))
    await expectOneLineAcross(page, row, "bottom", row.getByTestId("lane-node"))
    // auth, which the robot lands with session: its line down to the fork, and its commit on the line of the robot
    await expect(findBranchRow(page, "auth").getByTestId("landing-lanes")).toHaveCount(1)
    await expect(findBranchRow(page, "login").getByTestId("landing-lanes")).toHaveCount(0)
  })

  test("should start the line of the robot of the whole stack at the node of each expanded branch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const whole = robot("air", "whole", { name: "Safe Push of the whole air stack", branchName: null, stackRoot: "air" })
    await setupRepo("stack_island", { patronus: availableRobots(whole) })
    await syncAndWaitForBranches()
    // the robot runs on the stack branch, no branch's: no ring and no landing span in the graph
    const row = findBranchRow(page, "bazel")
    await expect(row.getByTestId("lane-ring")).toHaveCount(0)
    await expect(row.getByTestId("landing-lanes")).toHaveCount(0)
    await expandBranch(page, row)
    const commit = getBranchCommitRow(page, "bazel", "Build the deploy jar, part 1")
    await expect(commit).toHaveAttribute("data-robot-line", "both")
    // its commits are on the line of the robot, which starts at the node, one line across the border of the row
    await expect(row.getByTestId("landing-lanes")).toHaveCount(1)
    await expectOneLineAcross(page, row, "bottom", row.getByTestId("lane-node"))
    await expect(findBranchRow(page, "air").getByTestId("landing-lanes")).toHaveCount(0)
  })
})
