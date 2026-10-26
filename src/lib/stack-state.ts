import type { LandingNodeKind } from "@/lib/bindings"
import type { UIColor } from "@/lib/ui-types"
import { landingNodeLooks } from "@/presenters/landing-node"

/**
 * What the branches of a stack hold: how many rows have each node ([Landing](../../docs/design/landing.md)), the words
 * of the landing overview and of the filter chips
 */
export type StackState = Readonly<Record<LandingNodeKind, number>>

/** The kinds in the order of the chips: the user's moves, a fix first, then the way into master */
const STATE_ORDER: readonly LandingNodeKind[] = ["needsFix", "yourStep", "ready", "landing", "waiting", "parked", "inMaster"]

const NO_STATE: StackState = { needsFix: 0, yourStep: 0, ready: 0, landing: 0, waiting: 0, parked: 0, inMaster: 0 }

/**
 * What the rows of `names` hold by their nodes (`nodeOf`, `getLandingNodeNow`), and the node of the whole stack
 * (`stackNode`, of its stack branch while the stack forks) while it needs a fix: the robot of the whole stack failed on
 * the stack branch, which no row is. A branch without a node counts nothing.
 */
export function summarizeStackState(names: readonly string[], nodeOf: (name: string) => LandingNodeKind | null, stackNode: LandingNodeKind | null = null): StackState {
  const counts: Record<LandingNodeKind, number> = { ...NO_STATE }
  for (const name of names) {
    const kind = nodeOf(name)
    if (kind !== null) {
      counts[kind]++
    }
  }
  if (stackNode === "needsFix") {
    counts.needsFix++
  }
  return counts
}

/**
 * The first of `names` (in their order) with each node: where a click on its count goes; `root` for a fix of the whole
 * stack (`stackNode`), whose stack branch has no row, as the landing overview shows it
 */
export function firstBranchOfEachState(
  names: readonly string[],
  nodeOf: (name: string) => LandingNodeKind | null,
  stackNode: LandingNodeKind | null = null,
  root: string | null = null,
): Partial<Record<LandingNodeKind, string>> {
  const first: Partial<Record<LandingNodeKind, string>> = {}
  if (stackNode === "needsFix" && root !== null) {
    first.needsFix = root
  }
  for (const name of names) {
    const kind = nodeOf(name)
    if (kind !== null) {
      first[kind] ??= name
    }
  }
  return first
}

/** A state of a stack with its count, for its chips: "1 to fix" */
export interface StackStateChip {
  readonly state: LandingNodeKind
  readonly text: string
  /** The colour of the node (`landingNodeLooks`) */
  readonly color: UIColor
  /** A Safe Push lands the rows: the chip shows the ring of a running robot */
  readonly ring: boolean
}

/** "1 to fix", "2 your step", "3 ready", "1 landing", "2 waiting", "1 in master" */
function formatState(kind: LandingNodeKind, count: number): string {
  switch (kind) {
    case "needsFix":
      return `${count} to fix`
    case "yourStep":
      return `${count} your step`
    case "ready":
      return `${count} ready`
    case "landing":
      return `${count} landing`
    case "waiting":
      return `${count} waiting`
    case "parked":
      return `${count} parked`
    case "inMaster":
      return `${count} in master`
  }
}

/**
 * The states the rows hold, with their counts and the colours of their nodes, the strongest first: the user's moves (a
 * fix, a step, ready to land), then what lands, what waits, what is in master
 */
export function getStackStateChips(state: StackState): StackStateChip[] {
  return STATE_ORDER.filter(kind => state[kind] > 0).map(kind => ({
    state: kind,
    text: formatState(kind, state[kind]),
    color: landingNodeLooks[kind].color,
    ring: kind === "landing",
  }))
}
