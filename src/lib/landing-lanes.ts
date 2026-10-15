import type { StackLanes } from "@/lib/stack-lanes"

/** The lane segments of a branch row that a running Safe Push lands: drawn in the colour of the robot */
export interface LaneHighlight {
  /** The line from the top of the row into the node (the parent is above, on the same lane) */
  readonly up: boolean
  /** The curve from the parent's lane into the node of a side child */
  readonly fork: boolean
  /** The line from the node down, towards the next branch of the span */
  readonly down: boolean
  /** The lanes that pass the row on the way from a branch of the span to the next one, ascending */
  readonly through: readonly number[]
}

export interface LandingLaneRow {
  readonly name: string
  /** null outside a stack graph */
  readonly lanes: StackLanes | null
}

/** One object per highlight, so a row whose segments didn't change keeps its props */
const internedHighlights = new Map<string, LaneHighlight>()

function intern(highlight: LaneHighlight): LaneHighlight {
  const key = `${Number(highlight.up)}${Number(highlight.fork)}${Number(highlight.down)}:${highlight.through.join(",")}`
  let known = internedHighlights.get(key)
  if (known === undefined) {
    known = highlight
    internedHighlights.set(key, known)
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
export function landingLanes(rows: readonly LandingLaneRow[], spans: ReadonlyMap<string, string>): Map<string, LaneHighlight> {
  const result = new Map<string, LaneHighlight>()
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
    result.set(rows[index]!.name, intern({ up: it.up, fork: it.fork, down: it.down, through: [...it.through].sort((a, b) => a - b) }))
  }
  return result
}

/** The highlighted lanes that run on below a branch row (its inline form, its expanded commits): as `lanesBelow` */
export function highlightedLanesBelow(lanes: StackLanes, highlight: LaneHighlight | null): readonly number[] {
  if (highlight === null) {
    return []
  }
  return highlight.down ? [...highlight.through, lanes.lane].sort((a, b) => a - b) : highlight.through
}
