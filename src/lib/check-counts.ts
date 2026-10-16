import { joinNames } from "@/lib/text"

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

/** "Fast Tests", "Fast Tests, Smoke, Compilation and 2 more", "2 checks" without names; null while none failed */
export function formatFailedCheckNames(checks: Pick<CheckCounts, "failed" | "failedNames">): string | null {
  if (checks.failed === 0) {
    return null
  }
  const names = checks.failedNames
  if (names.length === 0) {
    return checks.failed === 1 ? "1 check" : `${checks.failed} checks`
  }
  const more = checks.failed - names.length
  return more > 0 ? `${names.join(", ")} and ${more} more` : joinNames(names)
}

/** "Failed: Fast Tests", for the row of a running robot; null while none failed */
export function formatFailedChecks(checks: Pick<CheckCounts, "failed" | "failedNames">): string | null {
  const names = formatFailedCheckNames(checks)
  return names === null ? null : `Failed: ${names}`
}
