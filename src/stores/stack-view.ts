import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { buttonStackPushOrder, needsPush } from "@/lib/stack"
import { ancestorSegments } from "@/lib/stack-lanes"
import type { LaneSegmentRow, LaneSegments } from "@/lib/stack-lanes"
import { getBranch } from "@/stores/branch-sync"

/** The branch whose path the graph shows; `push`: from its push button, which marks the nodes the push sends */
export interface StackPathTip {
  readonly name: string
  readonly push: boolean
}

/**
 * The path the stack graph of the branch table highlights: from the root of a stack down to a branch, what its Safe
 * Push lands and what a Push Stack of it pushes along. The pointer wins over the keyboard focus, which wins over an
 * open Safe Push or Dry Run form (`pinned`).
 */
export interface StackPathState {
  /** The rows of the table in order (`setStackPathRows`) */
  readonly rows: readonly LaneSegmentRow[]
  readonly hovered: StackPathTip | null
  readonly focused: StackPathTip | null
  readonly pinned: string | null
  /** The segments of the path per row; interned, so a row whose part didn't change keeps its object */
  readonly segments: ReadonlyMap<string, LaneSegments>
  /** The branches the hovered push button sends */
  readonly pushNodes: ReadonlySet<string>
}

const NO_SEGMENTS: ReadonlyMap<string, LaneSegments> = new Map()
const NO_NODES: ReadonlySet<string> = new Set()

export const stackPathStore = createStore<StackPathState>(() => ({
  rows: [],
  hovered: null,
  focused: null,
  pinned: null,
  segments: NO_SEGMENTS,
  pushNodes: NO_NODES,
}))

function sameTip(a: StackPathTip | null, b: StackPathTip | null): boolean {
  return a === b || (a !== null && b !== null && a.name === b.name && a.push === b.push)
}

/** The branches a push of `name` sends: the stack its button pushes, else the branch while it has work */
function pushedBy(name: string): ReadonlySet<string> {
  const order = buttonStackPushOrder(name, getBranch)
  if (order.length > 0) {
    return new Set(order)
  }
  const branch = getBranch(name)
  return branch !== undefined && needsPush(branch) ? new Set([name]) : NO_NODES
}

function update(patch: Partial<Pick<StackPathState, "rows" | "hovered" | "focused" | "pinned">>) {
  const state = { ...stackPathStore.getState(), ...patch }
  const has = (tip: StackPathTip | null) => tip !== null && state.rows.some(row => row.name === tip.name)
  // a branch gone (renamed, merged, hidden) leaves the path
  const hovered = has(state.hovered) ? state.hovered : null
  const focused = has(state.focused) ? state.focused : null
  const pinned = state.pinned !== null && has({ name: state.pinned, push: false }) ? state.pinned : null
  const tip = hovered ?? focused ?? (pinned === null ? null : { name: pinned, push: false })
  const segments = ancestorSegments(state.rows, tip?.name ?? null)
  stackPathStore.setState({
    rows: state.rows,
    hovered,
    focused,
    pinned: state.pinned,
    segments: segments.size === 0 ? NO_SEGMENTS : segments,
    pushNodes: tip?.push === true ? pushedBy(tip.name) : NO_NODES,
  })
}

/** The rows of the table changed: the path is laid out over them again */
export function setStackPathRows(rows: readonly LaneSegmentRow[]) {
  if (rows !== stackPathStore.getState().rows) {
    update({ rows })
  }
}

/** The pointer is over a branch row (or its push button), null when it left the rows */
export function hoverStackPath(tip: StackPathTip | null) {
  if (!sameTip(tip, stackPathStore.getState().hovered)) {
    update({ hovered: tip })
  }
}

/** The keyboard focus is on a branch row (or its push button), null when it left the rows */
export function focusStackPath(tip: StackPathTip | null) {
  if (!sameTip(tip, stackPathStore.getState().focused)) {
    update({ focused: tip })
  }
}

/** The Safe Push or Dry Run form of `name` opened (its path stays shown while the user decides) or closed */
export function pinStackPath(name: string, pinned: boolean) {
  const current = stackPathStore.getState().pinned
  if (pinned ? current !== name : current === name) {
    update({ pinned: pinned ? name : null })
  }
}

/** The segments of the highlighted path in the row of `name`, null off the path */
export function useStackPathSegments(name: string): LaneSegments | null {
  return useStore(stackPathStore, state => state.segments.get(name) ?? null)
}

/** The hovered push button sends `name` */
export function useIsPushedOnPath(name: string): boolean {
  return useStore(stackPathStore, state => state.pushNodes.has(name))
}
