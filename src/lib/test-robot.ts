import type { PatronusRobot } from "@/lib/bindings"

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
