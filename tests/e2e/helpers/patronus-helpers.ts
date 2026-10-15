import type { Page } from "@playwright/test"
import type { PatronusAvailability, PatronusRobot, PatronusRobotsResult } from "../../../src/lib/bindings"
import { expect } from "../fixtures/test-fixtures"

/**
 * A running Safe Push of a virtual branch. The empty `sourceRevision` makes the test server answer with the local tip
 * of the branch, so the robot tests the branch as it is unless a test sets another commit.
 */
export function robot(branchName: string, id: string, patch: Partial<PatronusRobot> = {}): PatronusRobot {
  return {
    id,
    name: `Safe Push of ${branchName}`,
    status: "running",
    pushMode: "rebaseAutosquash",
    branchName,
    sourceRevision: "",
    targetBranch: "master",
    startTime: 1704139200,
    finishTime: null,
    cancellationReason: null,
    url: `https://patronus.labs.jb.gg/robot/${id}`,
    ...patch,
  }
}

/** The answer of `get_patronus_robots` for these robots */
export function availableRobots(...robots: PatronusRobot[]): PatronusRobotsResult {
  return { availability: { status: "available" }, robots }
}

export function unavailableRobots(availability: PatronusAvailability): PatronusRobotsResult {
  return { availability, robots: [] }
}

/**
 * The issue_links repository with these robots; the unassigned commits card renders after the branches and would move
 * the rows away from the pointer
 */
export async function setupRobots(page: Page, setupRepo: (template: string, options: { patronus: unknown }) => Promise<unknown>, syncAndWaitForBranches: () => Promise<unknown>, ...robots: PatronusRobot[]): Promise<void> {
  await setupRepo("issue_links", { patronus: availableRobots(...robots) })
  await syncAndWaitForBranches()
  await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
  // the robots load after the sync, with the answer that shows the Safe Push slots: a push before they did would not
  // know that one runs
  await expect(page.getByTestId("safe-push-slot").first()).toBeVisible()
}
