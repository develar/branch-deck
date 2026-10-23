/**
 * JSON serialization with sorted object keys, so that two structurally equal values
 * produce the same string regardless of property order (Zod output order vs. spread order).
 * `undefined` properties are dropped, as `JSON.stringify` does.
 */
export function stableStringify(value: unknown): string {
  return JSON.stringify(value ?? null, (_key, it: unknown) => {
    if (it !== null && typeof it === "object" && !Array.isArray(it)) {
      const sorted: Record<string, unknown> = {}
      for (const key of Object.keys(it).sort()) {
        sorted[key] = (it as Record<string, unknown>)[key]
      }
      return sorted
    }
    return it
  })
}
