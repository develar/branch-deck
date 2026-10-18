/** The names listed of a list too long to name all: the rest is counted */
const NAMED = 3

/**
 * "a", "a and b", "a, b and c", "a, b, c and d", "a, b, c and 2 more": all of them up to four, so "and 1 more" never
 * stands where one name would; from five on the first three and the count of the rest
 */
export function joinNames(names: readonly string[]): string {
  if (names.length <= 1) {
    return names.join("")
  }
  if (names.length > NAMED + 1) {
    return `${names.slice(0, NAMED).join(", ")} and ${names.length - NAMED} more`
  }
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`
}
