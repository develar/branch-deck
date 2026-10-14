// @spec spec/repository/update-project.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import type { UpdateCommit } from "@/lib/bindings"
import { describeUpstreamCommit } from "@/lib/update-conflict"

const NOW = 1_760_400_000
const HOUR = 60 * 60
const DAY = 24 * HOUR

function upstreamCommit(time: number, committerTime: number): UpdateCommit {
  return { id: "c".repeat(40), subject: "Change line 2 upstream", author: "Dana Lee", time, committer: "Patronus", committerTime, files: [] }
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.setSystemTime(NOW * 1000)
})

afterEach(() => {
  vi.useRealTimers()
})

describe("describeUpstreamCommit", () => {
  it("names when the commit was written and when it landed, if that was more than an hour later", () => {
    expect(describeUpstreamCommit(upstreamCommit(NOW - 13 * DAY, NOW - 2 * HOUR))).toBe("Dana Lee · authored 13 days ago · landed 2 hours ago")
  })

  it("gives one time when the commit landed within an hour of being written", () => {
    expect(describeUpstreamCommit(upstreamCommit(NOW - 13 * DAY, NOW - 13 * DAY + HOUR))).toBe("Dana Lee, 13 days ago")
    expect(describeUpstreamCommit(upstreamCommit(NOW - 3 * HOUR, NOW - 3 * HOUR))).toBe("Dana Lee, 3 hours ago")
  })
})
