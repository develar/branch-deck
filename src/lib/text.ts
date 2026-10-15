/** More names than this are counted, not listed */
const NAMED = 3

/** "a", "a and b", "a, b and c", "a, b, c and 2 more" */
export function joinNames(names: readonly string[]): string {
  if (names.length <= 1) {
    return names.join("")
  }
  if (names.length > NAMED) {
    return `${names.slice(0, NAMED).join(", ")} and ${names.length - NAMED} more`
  }
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`
}
