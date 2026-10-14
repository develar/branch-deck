import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { describeIntegrationCommit } from "@/lib/integration-commit"

describe("describeIntegrationCommit", () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date("2026-10-14T12:00:00Z"))
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it("names the short hash, the committer and how long ago", () => {
    const time = Date.parse("2026-10-14T10:00:00Z") / 1000
    const commit = { id: "0123456789abcdef0123456789abcdef01234567", author: "Jane Doe", committer: "Safe Push Robot", time }
    expect(describeIntegrationCommit(commit)).toBe("Merged as 01234567 by Safe Push Robot, 2 hours ago")
  })
})
