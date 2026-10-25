import type { BranchLanding, BranchPatronus, PatronusRobot, PatronusView, RobotOf, StackLanding, StackPatronus } from "@/lib/bindings"
import type { RepositoryPatronus } from "@/stores/patronus"

/**
 * What the Patronus store has of a repository on GitHub (`acme/air`) once Patronus answered with `robots` (by id) and
 * the session derived `view` of them (none: a view of no facts): what they land, landed and hold, with the robot of
 * each row (`withRowRobots`)
 */
export function testPatronusRepository(robots: Readonly<Record<string, PatronusRobot>> = {}, view: PatronusView | null = null): RepositoryPatronus {
  return {
    availability: { status: "available" },
    robots,
    listed: true,
    fetchedAt: 0,
    isRefreshing: false,
    needsReview: {},
    githubRepository: "acme/air",
    watched: {},
    failing: {},
    // the facts of the rows are known: a test of a pending answer says so
    view: withRowRobots(view ?? { branches: {}, stacks: {}, landedByRobot: {}, restarts: {} }, robots),
    viewState: { kind: "fresh" },
    landingNeedsState: { kind: "fresh" },
    problemsFailed: {},
  }
}

/** Running, then canceling, then finished */
function activity(robot: PatronusRobot): number {
  return robot.status === "running" ? 2 : robot.status === "canceling" ? 1 : 0
}

/**
 * `view` with the robot of each row the session names (`robotId`) for the robots started on a branch or on the stack
 * branch of a whole stack, where the view names none: of several, the running one, then the canceling one, then the
 * newest. A robot of a temporary ref, which the session matches by the tips, a test names in its view itself
 */
export function withRowRobots(view: PatronusView, robots: Readonly<Record<string, PatronusRobot>>): PatronusView {
  const branches: Record<string, BranchPatronus> = { ...view.branches }
  const stacks: Record<string, StackPatronus> = { ...view.stacks }
  const chosen = new Map<string, PatronusRobot>()
  for (const robot of Object.values(robots)) {
    const key = robot.branchName !== null ? `branch\n${robot.branchName}` : robot.stackRoot != null ? `stack\n${robot.stackRoot}` : null
    const known = key === null ? undefined : chosen.get(key)
    if (key !== null && (known === undefined || activity(robot) > activity(known) || (activity(robot) === activity(known) && robot.startTime > known.startTime))) {
      chosen.set(key, robot)
    }
  }
  for (const [key, robot] of chosen) {
    const [kind, name] = key.split("\n") as ["branch" | "stack", string]
    if (kind === "branch") {
      branches[name] = { ...branchFacts({}), ...branches[name], robotId: branches[name]?.robotId ?? robot.id }
    } else {
      stacks[name] = { ...stackFacts({}), ...stacks[name], robotId: stacks[name]?.robotId ?? robot.id }
    }
  }
  return { ...view, branches, stacks }
}

/** What a robot of `branch` (the root of a whole stack with `wholeStack`) is in a view */
export function robotOf(branch: string, robotId: string, wholeStack = false): RobotOf {
  return { branch, robotId, wholeStack }
}

/** The facts of a branch in a view and what the rules decide of them (`landing`), nothing but `patch` */
export function branchFacts(patch: Partial<Omit<BranchPatronus, "landing">> & { readonly landing?: Partial<BranchLanding> }): BranchPatronus {
  const { landing, ...facts } = patch
  return {
    robotId: null,
    landedBy: null,
    span: null,
    landingAncestor: null,
    landingNeeds: { state: "unknown" },
    ...facts,
    landing: { hold: null, start: null, dryRun: null, replaces: null, push: null, ...landing },
  }
}

/** The facts of the whole stack of a root in a view and what the rules decide of them, nothing but `patch` */
export function stackFacts(patch: Partial<Omit<StackPatronus, "landing">> & { readonly landing?: Partial<StackLanding> }): StackPatronus {
  const { landing, ...facts } = patch
  return {
    robotId: null,
    landingMember: null,
    ...facts,
    landing: { hold: null, start: null, dryRun: null, replaces: null, rest: { members: [], commitCount: 0, landed: [], by: [], byOwnRobot: false }, ...landing },
  }
}

/** The landing of the whole stack of `root` whose own robot `robotId` runs on its stack branch */
export function stackHeld(root: string, robotId: string): Partial<StackLanding> {
  return { hold: { code: "stackHeld", root, robotId, mode: "rebaseAutosquash" } }
}

/** The facts of a branch a merged robot landed whole */
export function landedWhole(by: RobotOf, own = false): BranchPatronus {
  return branchFacts({ landedBy: { by, own, landed: [], after: [] }, landing: { hold: { kind: "landed", by, carried: null } } })
}
