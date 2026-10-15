/**
 * The stack graph of the branch table, as Sapling ISL and jj draw it: a stack root on top, depth-first, a lane per line
 * that runs in parallel instead of an indent per level. A straight chain stays on one lane; only a fork opens one.
 */

/** Where a branch row sits in the stack graph: its node and the lines that cross the row */
export interface StackLanes {
  /** The column of the node, 0 at the left */
  readonly lane: number
  /** A side child: the lane of its parent, the curve into the node starts there; null on the main line and for a root */
  readonly forkFrom: number | null
  /** The line from the top of the row into the node: the parent is above, on the same lane */
  readonly up: boolean
  /** The line from the node to the bottom of the row: the branch has children, the last of them continues the lane */
  readonly down: boolean
  /** The other lanes that pass the row from top to bottom, ascending */
  readonly through: readonly number[]
}

/** A branch of one stack in table order */
export interface StackLayoutRow {
  readonly name: string
  /** The branch it stacks on; null for the root */
  readonly parent: string | null
  /** Levels below the root */
  readonly depth: number
  readonly lanes: StackLanes
}

/**
 * Lays out one stack, root first, depth-first. At a fork the children are sorted by their longest chain, ascending
 * (ties keep the given order): the last is the main line and continues the parent's lane; each other child takes the
 * lowest free lane, and its subtree follows the parent at once, so the parent's lane passes beside it to the main
 * child below, and lines never cross. A lane is free again after its subtree. Each stack starts at lane 0: separate
 * stacks share no line.
 */
export function layoutStack(root: string, childrenOf: (name: string) => readonly string[]): StackLayoutRow[] {
  const chains = new Map<string, number>()
  // the branches of the longest chain down from `name`, `name` included
  const chainLength = (name: string, path: Set<string>): number => {
    const known = chains.get(name)
    if (known !== undefined) {
      return known
    }
    path.add(name)
    let longest = 0
    for (const child of childrenOf(name)) {
      if (!path.has(child)) {
        longest = Math.max(longest, chainLength(child, path))
      }
    }
    path.delete(name)
    chains.set(name, longest + 1)
    return longest + 1
  }

  const rows: StackLayoutRow[] = []
  const placed = new Set<string>()
  const active = new Set<number>([0])
  const place = (name: string, parent: string | null, depth: number, lane: number, forkFrom: number | null) => {
    placed.add(name)
    const children = childrenOf(name)
      .filter(child => !placed.has(child))
      .map(child => ({ child, length: chainLength(child, new Set([name])) }))
      // stable: a tie keeps the given order
      .sort((a, b) => a.length - b.length)
      .map(it => it.child)
    const through = [...active].filter(it => it !== lane).sort((a, b) => a - b)
    rows.push({ name, parent, depth, lanes: { lane, forkFrom, up: parent !== null && forkFrom === null, down: children.length > 0, through } })
    for (const side of children.slice(0, -1)) {
      let sideLane = 0
      while (active.has(sideLane)) {
        sideLane++
      }
      active.add(sideLane)
      place(side, name, depth + 1, sideLane, lane)
      active.delete(sideLane)
    }
    const main = children.at(-1)
    if (main !== undefined) {
      place(main, name, depth + 1, lane, null)
    }
  }
  place(root, null, 0, 0, null)
  return rows
}

/** The lanes the table draws: the highest lane of a row plus one; 0 without stacked branches */
export function laneCountOf(lanes: readonly (StackLanes | null)[]): number {
  let count = 0
  for (const it of lanes) {
    if (it !== null) {
      count = Math.max(count, it.lane + 1)
    }
  }
  return count
}

/**
 * The lanes that run on below a branch row, beside its inline form and its expanded commits: the lanes passing it,
 * and its own while the branch has children or its commits are shown below
 */
export function lanesBelow(lanes: StackLanes, commitsBelow: boolean): readonly number[] {
  return lanes.down || commitsBelow ? [...lanes.through, lanes.lane].sort((a, b) => a - b) : lanes.through
}

/**
 * The padding of the content of a row below a branch row in a table with stacks (`--bd-row-inset`): right of the lanes
 * and the family guide, 1.5rem before the name, so the commit subjects and the form titles (`px-6`) start at the name
 */
export function laneContentInsetClass(familyIndent: 0 | 1): string {
  return familyIndent === 1
    ? "pl-(--bd-row-inset) [--bd-row-inset:calc(var(--bd-tree-x)+0.5rem+var(--bd-family-step))]"
    : "pl-(--bd-row-inset) [--bd-row-inset:calc(var(--bd-tree-x)+0.5rem)]"
}

/** The lane segments of a branch row that a running Safe Push lands: drawn in the colour of the robot */
export interface LandingSegments {
  /** The line from the top of the row into the node (the parent is above, on the same lane) */
  readonly up: boolean
  /** The curve from the parent's lane into the node of a side child */
  readonly fork: boolean
  /** The line from the node down, towards the next branch of the span */
  readonly down: boolean
  /** The lanes that pass the row on the way from a branch of the span to the next one, ascending */
  readonly through: readonly number[]
}

export interface LandingSegmentRow {
  readonly name: string
  /** null outside a stack graph */
  readonly lanes: StackLanes | null
}

/** One object per highlight, so a row whose segments didn't change keeps its props */
const internedSegments = new Map<string, LandingSegments>()

function internSegments(highlight: LandingSegments): LandingSegments {
  const key = `${Number(highlight.up)}${Number(highlight.fork)}${Number(highlight.down)}:${highlight.through.join(",")}`
  let known = internedSegments.get(key)
  if (known === undefined) {
    known = highlight
    internedSegments.set(key, known)
  }
  return known
}

/**
 * The lane segments from the root of a landing span down to the branch whose running robot lands it (`useLandingSpan`:
 * ancestor → K), per row in table order: K's line up (or its fork curve), each branch of the span's line down, and its
 * lane through the rows between it and the next branch of the span (the side branches drawn between them). The parent
 * of a row is the nearest row above on its lane (`up`) or on the lane it forks from: the stack graph is depth-first,
 * and the lanes between belong to the side branches. A span stops where the graph doesn't connect its branches.
 */
export function landingSegments(rows: readonly LandingSegmentRow[], spans: ReadonlyMap<string, string>): Map<string, LandingSegments> {
  const result = new Map<string, LandingSegments>()
  if (spans.size === 0) {
    return result
  }
  const indexOf = new Map(rows.map((row, index) => [row.name, index]))
  const parentIndex = (index: number): number | null => {
    const lanes = rows[index]!.lanes
    const lane = lanes === null ? null : lanes.up ? lanes.lane : lanes.forkFrom
    if (lane === null) {
      return null
    }
    for (let above = index - 1; above >= 0; above--) {
      if (rows[above]!.lanes?.lane === lane) {
        return above
      }
    }
    return null
  }

  const parts = new Map<number, { up: boolean, fork: boolean, down: boolean, through: Set<number> }>()
  const partsOf = (index: number) => {
    let it = parts.get(index)
    if (it === undefined) {
      it = { up: false, fork: false, down: false, through: new Set() }
      parts.set(index, it)
    }
    return it
  }
  for (const landing of new Set(spans.values())) {
    let child = indexOf.get(landing)
    while (child !== undefined) {
      const parent = parentIndex(child)
      if (parent === null || spans.get(rows[parent]!.name) !== landing) {
        break
      }
      const childLanes = rows[child]!.lanes!
      const parentLane = rows[parent]!.lanes!.lane
      if (childLanes.up) {
        partsOf(child).up = true
      }
      else {
        partsOf(child).fork = true
      }
      partsOf(parent).down = true
      for (let between = parent + 1; between < child; between++) {
        if (rows[between]!.lanes?.through.includes(parentLane) === true) {
          partsOf(between).through.add(parentLane)
        }
      }
      child = parent
    }
  }
  for (const [index, it] of parts) {
    result.set(rows[index]!.name, internSegments({ up: it.up, fork: it.fork, down: it.down, through: [...it.through].sort((a, b) => a - b) }))
  }
  return result
}

/** The highlighted lanes that run on below a branch row (its inline form, its expanded commits): as `lanesBelow` */
export function landingLanesBelow(lanes: StackLanes, highlight: LandingSegments | null): readonly number[] {
  if (highlight === null) {
    return []
  }
  return highlight.down ? [...highlight.through, lanes.lane].sort((a, b) => a - b) : highlight.through
}
