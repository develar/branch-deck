/** The names listed of a list too long to name all: the rest is counted */
const NAMED = 3

/**
 * A list too long to give whole, cut at `limit`: all of its items up to one more than the limit, so the count of the
 * rest is never 1 where that one item would stand; from two more on the first `limit`. `total` counts the list the
 * items are the first of (the backend reports the first items of a long list and counts all, cut by the same rule:
 * `sync_utils::list_cut::kept`), so `more` is what is left out of it. Every cut list of the app and of a prompt for an
 * agent, and the names of a sentence (`joinNames`)
 */
export function cutList<T>(items: readonly T[], limit: number, total = items.length): { shown: readonly T[]; more: number } {
  const shown = items.length <= limit + 1 ? items : items.slice(0, limit)
  return { shown, more: total - shown.length }
}

/**
 * "a", "a and b", "a, b and c", "a, b, c and d", "a, b, c and 2 more": all of them up to four, so "and 1 more" never
 * stands where one name would; from five on the first three and the count of the rest (`cutList`)
 */
export function joinNames(names: readonly string[]): string {
  if (names.length <= 1) {
    return names.join("")
  }
  const { shown, more } = cutList(names, NAMED)
  if (more > 0) {
    return `${shown.join(", ")} and ${more} more`
  }
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`
}
