// @spec spec/branch/branch-row.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getBranchCells } from "../helpers/branch-helpers"
import { expectColumnAligned } from "../helpers/column-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import { pressSyncHeld, releaseHeldSync, waitForBranchSyncComplete } from "../helpers/sync-helpers"
import { waitForStorePersistence } from "../helpers/wait-helpers"

/** The width of a track as the root of the branch view has it (`--bd-track-<track>`), in px */
async function trackWidth(page: Page, track: string): Promise<number> {
  return Number.parseFloat(await page.getByTestId("branch-view").evaluate((view, name) => (view as HTMLElement).style.getPropertyValue(name), `--bd-track-${track}`))
}

// https://github.com/develar/branch-deck/issues/72
test.describe("Branch table column widths", () => {
  test("gives the free width to the branch name and keeps the other columns at content width", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize({ width: 1400, height: 900 })
    await setupRepo("long_subjects")
    await syncAndWaitForBranches()

    const branchRow = findBranchRow(page, "IJPL-215166")
    await waitForBranchSyncComplete(page, "IJPL-215166")

    const { branchName, commits, status, actions } = getBranchCells(branchRow)
    const row = await branchRow.boundingBox()
    const name = await branchName.boundingBox()
    const commitsBox = await commits.boundingBox()
    const statusBox = await status.boundingBox()
    const actionsBox = await actions.boundingBox()
    expect(row && name && commitsBox && statusBox && actionsBox).toBeTruthy()

    // the name column takes more than half of the table
    expect(name!.width).toBeGreaterThan(row!.width / 2)
    // the right columns are no wider than their content plus padding (px-4 = 16px each side)
    const padding = 2 * 16 + 1
    for (const [cell, box] of [
      [commits, commitsBox],
      [status, statusBox],
      [actions, actionsBox],
    ] as const) {
      const content = await cell.locator("> *").first().boundingBox()
      expect(content).not.toBeNull()
      expect(box!.width).toBeLessThanOrEqual(content!.width + padding)
    }
    // the long subject is still truncated, not wrapped: the row stays a single line high
    expect(row!.height).toBeLessThan(80)
    const summary = branchName.locator(".truncate")
    await expect(summary).toContainText("add UI test that enables a plugin")
  })

  test("keeps the Actions column at its buttons: no place is held for a push, a step or a copy that no row shows", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // a running robot (no push, no step), a restart, Safe Push without a push of its own (the first push is a step of it)
    await page.setViewportSize({ width: 1400, height: 900 })
    await setupRepo("issue_links", {
      patronus: availableRobots(robot("JIRA-456", "running-robot"), robot("ABC-123", "failed-robot", { status: "failure", finishTime: 1704142800 })),
    })
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "ABC-123").getByRole("button", { name: "Restart Safe Push" })).toBeVisible()
    await expect(findBranchRow(page, "feature-auth").getByRole("button", { name: "Safe Push", exact: true })).toBeVisible()
    await expect(page.getByTestId("push-button")).toHaveCount(0)
    // "Restart Safe Push" and "⋯" with the padding, the gutter between them (201px): 328px when every row held a push
    // and a 10rem step
    const actions = getBranchCells(findBranchRow(page, "JIRA-456")).actions
    await expect.poll(async () => (await actions.boundingBox())!.width).toBeLessThan(12.75 * 16)
    await expectColumnAligned(page.getByTestId("next-step-slot"))
    await expectColumnAligned(page.getByTestId("branch-row").getByRole("button", { name: /^More actions for / }))
  })

  test("fits every label of a step in its column, so none truncates", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", { patronus: availableRobots(robot("ABC-123", "failed-robot", { status: "failure", finishTime: 1704142800 })) })
    await syncAndWaitForBranches()
    const step = findBranchRow(page, "ABC-123").getByRole("button", { name: "Restart Safe Push" })
    await expect(step).toBeVisible()
    // the labels a step shows in its column (`compactLabel`, else `label`), with long numbers: each in the button of a
    // step, with its icon, under the cap of the step column
    const labels = [
      "Restart Safe Push",
      "Restart Dry Run",
      "Create Pull Request…",
      "Request Review…",
      "Changes Requested",
      "Awaiting Review",
      "Awaiting 12 Reviews",
      "Open Pull Request",
      "Retarget #12345",
      "In #12345",
      "Draft",
      "Safe Push · 12",
    ]
    const truncated = await step.evaluate((button, texts) => {
      const text = button.querySelector<HTMLElement>("span")!
      return texts.filter(label => {
        text.textContent = label
        return text.scrollWidth > text.clientWidth
      })
    }, labels)
    expect(truncated).toEqual([])
  })

  test("holds the width of a column while a sync runs, and follows what the rows show once it ended", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const running = robot("JIRA-456", "running-robot")
    const repo = await setupRepo("issue_links", { patronus: availableRobots(running) })
    await syncAndWaitForBranches()
    const row = findBranchRow(page, "JIRA-456")
    const chip = row.getByTestId("patronus-chip")
    await expect(chip).toHaveAttribute("data-kind", "running")
    // the view settled (its widths kept for the next start): no late fact moves a column from now on
    await waitForStorePersistence(page, "repositoryState", states =>
      Object.values(states as Record<string, { settledLayout?: { tracks: Record<string, number> } }>).some(state => (state.settledLayout?.tracks["state"] ?? 0) > 0),
    )
    // the running robot is the widest state of the view
    const content = row.locator("[data-track=state] > div")
    const runningWidth = await trackWidth(page, "state")
    expect((await content.boundingBox())!.width).toBeCloseTo(runningWidth, 1)

    // the robot fails while a sync runs: its chip is narrower, the column keeps its width
    const syncs = await pressSyncHeld(page, repo)
    await repo.setPatronus(availableRobots({ ...running, status: "failure", finishTime: 1704189000 }))
    await expect
      .poll(async () => {
        // the session polls a running robot once a minute of its clock
        await repo.advanceBackendClock(61_000)
        return chip.getAttribute("data-kind")
      })
      .toBe("failed")
    const failedWidth = (await content.boundingBox())!.width
    expect(failedWidth).toBeLessThan(runningWidth)
    expect(await trackWidth(page, "state")).toBe(runningWidth)

    // the sync ended: the column follows its widest state once
    await releaseHeldSync(page, repo, syncs)
    await expect.poll(() => trackWidth(page, "state")).toBeLessThan(runningWidth)
    const widest = Math.max(...(await page.locator("[data-track=state] > div").evaluateAll(items => items.map(item => item.getBoundingClientRect().width))))
    expect(await trackWidth(page, "state")).toBeCloseTo(widest, 1)
  })

  test("drops a subject that has no room to be read", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the 768px minimum of the window at a zoom above 125% (`minWindowWidth`): the columns at their content leave every
    // name its subject
    await page.setViewportSize({ width: 768, height: 800 })
    await setupRepo("issue_links")
    await syncAndWaitForBranches()

    const summaries = page.getByTestId("branch-row").getByTestId("branch-summary")
    await expect(summaries.first()).toBeAttached()
    const visibleWidths = async () => {
      const widths: number[] = []
      for (const summary of await summaries.all()) {
        if (await summary.isVisible()) {
          widths.push((await summary.boundingBox())!.width)
        }
      }
      return widths
    }
    await expect(findBranchRow(page, "feature-auth").getByTestId("branch-summary")).toBeVisible()
    expect(await visibleWidths()).toHaveLength(await summaries.count())

    // narrower than the window gets: the issue chips of feature-auth and feature-db leave too little for their subjects,
    // which drop out; a subject that shows has at least 6rem, never a letter or two
    await page.setViewportSize({ width: 700, height: 800 })
    await expect(findBranchRow(page, "feature-auth").getByTestId("branch-summary")).toBeHidden()
    for (const width of await visibleWidths()) {
      expect(width).toBeGreaterThanOrEqual(6 * 16)
    }
  })
})
