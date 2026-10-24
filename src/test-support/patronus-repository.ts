import type { BranchPatronus, PatronusRobot, PatronusView, RobotOf } from "@/lib/bindings"
import type { RepositoryPatronus } from "@/stores/patronus"

/**
 * What the Patronus store has of a repository on GitHub (`acme/air`) once Patronus answered with `robots` (by id) and
 * the backend with `view`: what they land, landed and hold
 */
export function testPatronusRepository(robots: Readonly<Record<string, PatronusRobot>> = {}, view: PatronusView | null = null): RepositoryPatronus {
  return {
    availability: { status: "available" },
    robots,
    listedFrom: 0,
    fetchedAt: 0,
    isRefreshing: false,
    needsReview: {},
    githubRepository: "acme/air",
    watched: {},
    failing: {},
    view,
    viewOf: view === null ? null : robots,
    // the facts of the rows are known: a test of a pending answer says so
    viewListed: true,
    problemsFailed: {},
    landingNeedsComputing: false,
  }
}

/** What a robot of `branch` (the root of a whole stack with `wholeStack`) is in a view */
export function robotOf(branch: string, robotId: string, wholeStack = false): RobotOf {
  return { branch, robotId, wholeStack }
}

/** The facts of a branch in a view, nothing but `patch` */
export function branchFacts(patch: Partial<BranchPatronus>): BranchPatronus {
  return { landedBy: null, span: null, landingAncestor: null, hold: null, startBlocked: false, start: null, dryRun: null, landingNeeds: { state: "unknown" }, ...patch }
}

/** The facts of a branch a merged robot landed whole */
export function landedWhole(by: RobotOf, own = false): BranchPatronus {
  return branchFacts({ landedBy: { by, own, landed: [], after: [] }, hold: { kind: "landed", by, carried: null } })
}
