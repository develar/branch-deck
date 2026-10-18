import type { PatronusRobot, RobotLanding } from "@/lib/bindings"

/**
 * A running Safe Push for tests, started 10 minutes before 2024-01-02 10:00Z, on the tip `${branchName}-tip` of its
 * branch (null: of no branch, its tip `${id}-tip`); `overrides` replace any field
 */
export function testRobot(id: string, branchName: string | null, overrides: Partial<PatronusRobot> = {}): PatronusRobot {
  return {
    id,
    name: `Safe Push of ${branchName ?? id}`,
    status: "running",
    pushMode: "rebaseAutosquash",
    branchName,
    sourceRevision: `${branchName ?? id}-tip`,
    targetBranch: "master",
    startTime: 1704189000,
    finishTime: null,
    cancellationReason: null,
    url: `https://patronus/robot/${id}`,
    ...overrides,
  }
}

/** The robots keyed by their id, as the Patronus store holds them */
export function robotsById(...robots: PatronusRobot[]): Record<string, PatronusRobot> {
  return Object.fromEntries(robots.map(it => [it.id, it]))
}

/**
 * What `get_robot_landings` answers for the robot `robotId`: by branch the commits added after the landed one, the
 * landed one `${branch}-1`
 */
export function testLanding(robotId: string, after: Readonly<Record<string, readonly string[]>>): RobotLanding {
  return { robotId, branches: Object.entries(after).map(([branch, commits]) => ({ branch, landed: [`${branch}-1`], after: [...commits] })) }
}
