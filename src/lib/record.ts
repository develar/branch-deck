/** The same keys with the same values (`===`): a record a patch built again that says what the shown one says */
export function sameFields<T extends object>(a: T, b: T): boolean {
  const keys = Object.keys(a) as (keyof T)[]
  return keys.length === Object.keys(b).length && keys.every(key => a[key] === b[key])
}

/** A copy of `record` without `keys` (a patch removes them), for the caller to add the changed ones to */
export function withoutKeys<T>(record: Readonly<Record<string, T>>, keys: readonly string[]): Record<string, T> {
  if (keys.length === 0) {
    return { ...record }
  }
  const removed = new Set(keys)
  return Object.fromEntries(Object.entries(record).filter(([key]) => !removed.has(key)))
}

/**
 * The same plain JSON value: a primitive by `Object.is`, an array element by element, a record key by key (the view
 * and its patches are JSON, so a part a patch replaced says the same in new objects); for a selector that answers an
 * object of a part, which the store would otherwise answer anew with every patch of the part (`useRowPart`)
 */
export function sameJson(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) {
    return true
  }
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null || Array.isArray(a) !== Array.isArray(b)) {
    return false
  }
  if (Array.isArray(a)) {
    const other = b as readonly unknown[]
    return a.length === other.length && a.every((item, index) => sameJson(item, other[index]))
  }
  const left = a as Readonly<Record<string, unknown>>
  const right = b as Readonly<Record<string, unknown>>
  const keys = Object.keys(left)
  return keys.length === Object.keys(right).length && keys.every(key => Object.hasOwn(right, key) && sameJson(left[key], right[key]))
}
