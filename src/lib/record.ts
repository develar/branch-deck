/** The same keys with the same values (`===`) */
export function sameStringRecord(a: Readonly<Record<string, string>>, b: Readonly<Record<string, string>>): boolean {
  const keys = Object.keys(a)
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
