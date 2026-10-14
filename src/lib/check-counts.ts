/** The checks of a pull request or a Patronus robot, counted */
export interface CheckCounts {
  readonly failed: number
  /** The names of the first failed checks */
  readonly failedNames: readonly string[]
  readonly running: number
  readonly canceled?: number
  readonly passed: number
}

/** "1 failing (tests), 2 running, 9 passed"; "All 4 passed" */
export function formatCheckCounts(checks: CheckCounts): string {
  const canceled = checks.canceled ?? 0
  const total = checks.failed + checks.running + canceled + checks.passed
  if (checks.failed === 0 && checks.running === 0 && canceled === 0) {
    return total === 1 ? "Passed" : `All ${total} passed`
  }
  const parts: string[] = []
  if (checks.failed > 0) {
    const names = checks.failedNames.length === 0 ? "" : ` (${checks.failedNames.join(", ")}${checks.failed > checks.failedNames.length ? ", …" : ""})`
    parts.push(`${checks.failed} failing${names}`)
  }
  if (checks.running > 0) {
    parts.push(`${checks.running} running`)
  }
  if (canceled > 0) {
    parts.push(`${canceled} canceled`)
  }
  if (checks.passed > 0) {
    parts.push(`${checks.passed} passed`)
  }
  return parts.join(", ")
}
