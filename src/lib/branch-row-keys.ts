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
 * The row ← moves the focus to from a collapsed row, as in a tree: the nearest branch row above with a smaller depth,
 * the branch it stacks on or the branch of its issue; null at the top level and under an issue row
 */
export function parentRow(rows: readonly BranchTableRow[], name: string): string | null {
  const index = rows.findIndex(row => row.kind === "branch" && row.name === name)
  const row = rows[index]
  if (row?.kind !== "branch") {
    return null
  }
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
