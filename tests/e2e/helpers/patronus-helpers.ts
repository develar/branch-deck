import type { PatronusAvailability, PatronusRobot, PatronusRobotsResult } from "../../../src/lib/bindings"

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
