import type { LandingNode, LandingNodeKind } from "@/lib/landing-node"
import { landingNodeLooks } from "@/lib/landing-node"
import type { RobotOverview, RobotOverviewEntry } from "@/lib/robot-overview"
import { isFailingEntry } from "@/lib/robot-overview"
import type { UIColor } from "@/lib/ui-types"

/**
 * What lands into master in one move: a lone branch, or a stack, whose node and next step are those of the whole stack
 * (its island header's); its branches that need a fix of their own come with it
 */
export interface LandingUnit {
  /** The branch, or the root of the stack */
  readonly name: string
  /** The branches of the stack, the root included; 1 for a lone branch */
  readonly branchCount: number
  readonly commitCount: number
  readonly node: LandingNode
  /** The branches of a stack whose own node needs a fix (a conflict, a failed robot): each is a move of its own */
  readonly fixes: readonly { readonly name: string; readonly node: LandingNode }[]
}

/** The kinds that are the user's move, the most urgent first */
const MOVE_KINDS: readonly LandingNodeKind[] = ["needsFix", "yourStep", "ready"]

export interface LandingOverview {
  /** The units on the way to master, in the table order: the bar of the header button */
  readonly onTheWay: readonly LandingUnit[]
  /** The units whose next step is the user's, the most urgent first, then in the table order */
  readonly yourMove: readonly LandingUnit[]
  /** The running robots, Safe Push and Dry Run, of the branches, the whole stacks and the watched ones */
  readonly running: readonly RobotOverviewEntry[]
  /** The units that wait for others; those that wait for the first answer of Patronus are `checking` */
  readonly waiting: readonly LandingUnit[]
  /** The units the baseline or a merge has, until the update archives them */
  readonly inMaster: readonly LandingUnit[]
  /** A failed robot of no branch row (a temporary ref, a watched pull request): the user acts on the robot */
  readonly failedRobots: readonly RobotOverviewEntry[]
  /** The robots that failed on a branch of a row as it is (not for approvals): its row in the overview is the robot's */
  readonly robotFailures: readonly RobotOverviewEntry[]
  /** The other robots that finished in the last day, without those of the units in master */
  readonly finished: readonly RobotOverviewEntry[]
  /**
   * The moves of the header: the user's units, their branches that need a fix, the failed robots of no row and the
   * running Safe Pushes that will fail; none of a unit that waits for the first answer of Patronus
   */
  readonly moves: number
  /** The running Safe Pushes, a Dry Run lands nothing */
  readonly landing: number
  /** The units that wait for the first answer of Patronus: the popover asks for them while it is open */
  readonly checking: number
  /** The most urgent move: the node of the header button; null without one */
  readonly urgent: LandingNodeKind | null
  /** A Safe Push runs: the ring around the node of the header button */
  readonly safePushRuns: boolean
}

/** Nothing on the way to master, nothing that ran in the last day: the header button is hidden */
export function isLandingOverviewEmpty(overview: LandingOverview): boolean {
  return (
    overview.yourMove.length === 0 &&
    overview.running.length === 0 &&
    overview.waiting.length === 0 &&
    overview.inMaster.length === 0 &&
    overview.failedRobots.length === 0 &&
    overview.finished.length === 0
  )
}

function isMove(kind: LandingNodeKind): boolean {
  return MOVE_KINDS.includes(kind)
}

/**
 * Groups the units (in the table order) by where they are on their way into master, beside the robots
 * (`collectRobotOverview`): the moves are the user's units, not branches, so a ready stack of 22 branches is one move
 */
export function collectLandingOverview(units: readonly LandingUnit[], robots: RobotOverview): LandingOverview {
  const byUrgency = (kind: LandingNodeKind) => MOVE_KINDS.indexOf(kind)
  const yourMove = units.filter(unit => isMove(unit.node.kind) || unit.fixes.length > 0).toSorted((a, b) => byUrgency(urgentKindOf(a)) - byUrgency(urgentKindOf(b)))
  const inMove = new Set(yourMove)
  const waiting = units.filter(unit => !inMove.has(unit) && unit.node.kind === "waiting")
  const inMaster = units.filter(unit => !inMove.has(unit) && unit.node.kind === "inMaster")
  const inMasterNames = new Set(inMaster.map(unit => unit.name))
  const failedRobots = robots.attention.filter(entry => !entry.hasRow)
  const failingSafePushes = robots.running.filter(isFailingEntry).length
  const moves =
    yourMove.reduce((sum, unit) => sum + (isMove(unit.node.kind) && unit.node.pending !== true ? 1 : 0) + unit.fixes.length, 0) + failedRobots.length + failingSafePushes
  const kinds = [...yourMove.map(urgentKindOf), ...(failedRobots.length > 0 || failingSafePushes > 0 ? ["needsFix" as const] : [])]
  return {
    onTheWay: units.filter(unit => urgentKindOf(unit) !== "inMaster"),
    yourMove,
    running: robots.running,
    waiting,
    inMaster,
    failedRobots,
    robotFailures: robots.attention.filter(entry => entry.hasRow && entry.chip.kind === "failed"),
    finished: robots.finished.filter(entry => !inMasterNames.has(entry.stackRoot ?? entry.branchName)),
    moves,
    landing: robots.running.filter(entry => entry.robot.pushMode !== "dryRun").length,
    checking: units.filter(unit => unit.node.pending === true).length,
    urgent: MOVE_KINDS.find(kind => kinds.includes(kind)) ?? null,
    safePushRuns: robots.running.some(entry => entry.robot.pushMode !== "dryRun"),
  }
}

/** The kind a unit is listed by: a fix of one of its branches is more urgent than the step of the whole stack */
export function urgentKindOf(unit: LandingUnit): LandingNodeKind {
  return unit.fixes.length > 0 ? "needsFix" : unit.node.kind
}

/** "On the way to master: 3 your move · 1 landing · 3 waiting"; the accessible name of the header button too */
export function formatLandingTitle(overview: LandingOverview): string {
  const parts = [
    overview.moves > 0 ? `${overview.moves} your move` : null,
    overview.landing > 0 ? `${overview.landing} landing` : null,
    overview.waiting.length > 0 ? `${overview.waiting.length} waiting` : null,
    overview.inMaster.length > 0 ? `${overview.inMaster.length} in master` : null,
  ].filter(part => part !== null)
  return parts.length === 0 ? "Nothing on the way to master" : `On the way to master: ${parts.join(" · ")}`
}

/** A segment of the bar of the header button: a kind, its weight the commits of its units */
export interface LandingBarSegment {
  readonly kind: LandingNodeKind
  readonly weight: number
}

/** The bar shows the way to master from the left: landing first, then waiting, the user's moves last */
const BAR_ORDER: readonly LandingNodeKind[] = ["landing", "waiting", "ready", "yourStep", "needsFix"]

/**
 * The bar of the header button: a segment per kind of the units on the way to master, its width the commits of its
 * units, in the order of the way; the units in master are not on the way
 */
export function getLandingBarSegments(units: readonly LandingUnit[]): LandingBarSegment[] {
  const weights = new Map<LandingNodeKind, number>()
  for (const unit of units) {
    const kind = urgentKindOf(unit)
    if (kind !== "inMaster") {
      weights.set(kind, (weights.get(kind) ?? 0) + Math.max(1, unit.commitCount))
    }
  }
  return BAR_ORDER.flatMap(kind => {
    const weight = weights.get(kind)
    return weight === undefined ? [] : [{ kind, weight }]
  })
}

/** The fill of a segment: the colour of the node, light for a hollow node (not the user's move) */
export function landingBarFill(kind: LandingNodeKind): string {
  const { color, filled } = landingNodeLooks[kind]
  return (filled ? solidFill : lightFill)[color]
}

// whole class names, as Tailwind finds them in the source
const solidFill: Readonly<Record<UIColor, string>> = {
  primary: "bg-primary",
  secondary: "bg-muted-foreground",
  success: "bg-success",
  info: "bg-info",
  warning: "bg-warning",
  error: "bg-destructive",
  neutral: "bg-muted-foreground",
}

const lightFill: Readonly<Record<UIColor, string>> = {
  primary: "bg-primary/40",
  secondary: "bg-muted-foreground/40",
  success: "bg-success/40",
  info: "bg-info/40",
  warning: "bg-warning/40",
  error: "bg-destructive/40",
  neutral: "bg-muted-foreground/40",
}
