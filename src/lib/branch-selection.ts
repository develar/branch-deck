/** Selection of branch rows: pure helpers of the commit move store (`src/stores/commit-move.ts`) */

/** `selected` with `name` added at the end, or removed */
export function toggleBranch(selected: readonly string[], name: string): readonly string[] {
  return selected.includes(name) ? selected.filter(it => it !== name) : [...selected, name]
}

/** The rows of `order` from `anchor` to `name`, both included, in `order`; just `name` when either is not a row */
export function selectBranchRange(order: readonly string[], anchor: string, name: string): readonly string[] {
  const from = order.indexOf(anchor)
  const to = order.indexOf(name)
  if (from === -1 || to === -1) {
    return [name]
  }
  return order.slice(Math.min(from, to), Math.max(from, to) + 1)
}

/** `selected` without the branches that are gone (renamed, merged, moved away); the same array when none is */
export function pruneSelection(selected: readonly string[], names: ReadonlySet<string>): readonly string[] {
  return selected.every(name => names.has(name)) ? selected : selected.filter(name => names.has(name))
}

/** `selected` in table order */
export function inTableOrder(selected: readonly string[], order: readonly string[]): string[] {
  return order.filter(name => selected.includes(name))
}
