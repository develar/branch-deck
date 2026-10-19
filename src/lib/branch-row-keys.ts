import type { BranchTableRow } from "@/stores/branch-sync"

/**
 * The branch row ↓ (`step` 1) or ↑ (-1) moves the focus to from `current` among `names`, the branch rows in table order:
 * the first or the last row from no row (the table itself, or a row the filter hid); null past the ends
 */
export function neighbourRow(names: readonly string[], current: string | null, step: 1 | -1): string | null {
  const index = current === null ? -1 : names.indexOf(current)
  if (index === -1) {
    return names.at(step === 1 ? 0 : -1) ?? null
  }
  return names[index + step] ?? null
}

/**
 * The row F2 (`step` 1) or Shift+F2 (-1) moves the focus to: the next or the previous of `moves` (the user's moves, the
 * most urgent first, `movesByUrgency`) after `current`, around the ends, as F2 steps through the problems in IntelliJ;
 * from no row or a row that is no move the first or the last one, the most urgent or the least; null without a move
 */
export function moveRow(moves: readonly string[], current: string | null, step: 1 | -1): string | null {
  if (moves.length === 0) {
    return null
  }
  const index = current === null ? -1 : moves.indexOf(current)
  if (index === -1) {
    return moves.at(step === 1 ? 0 : -1)!
  }
  return moves[(index + step + moves.length) % moves.length]!
}

/** The branch row of `name` among `rows` and its index; null for a branch without a row */
export function branchRowIn(rows: readonly BranchTableRow[], name: string): { index: number; row: Extract<BranchTableRow, { kind: "branch" }> } | null {
  const index = rows.findIndex(row => row.kind === "branch" && row.name === name)
  const row = rows[index]
  return row?.kind === "branch" ? { index, row } : null
}

/**
 * The row ← moves the focus to from a collapsed row, as in a tree: the nearest branch row above with a smaller depth,
 * the branch it stacks on or the branch of its issue; null at the top level and under an issue row
 */
export function parentRow(rows: readonly BranchTableRow[], name: string): string | null {
  const found = branchRowIn(rows, name)
  if (found === null) {
    return null
  }
  const { index, row } = found
  for (let i = index - 1; i >= 0; i--) {
    const above = rows[i]!
    if (above.kind === "issue") {
      return null
    }
    if (above.depth < row.depth) {
      return above.name
    }
  }
  return null
}
