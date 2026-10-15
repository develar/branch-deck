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
export function laneCountOf(rows: readonly { readonly lanes?: StackLanes | null }[]): number {
  let count = 0
  for (const row of rows) {
    if (row.lanes != null) {
      count = Math.max(count, row.lanes.lane + 1)
    }
  }
  return count
}
