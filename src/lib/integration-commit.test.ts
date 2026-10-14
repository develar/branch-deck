import { describe, expect, it } from "vitest"
import { describeIntegrationCommit } from "@/lib/integration-commit"

describe("describeIntegrationCommit", () => {
  it("names the short hash and the committer, not the time the row already shows", () => {
    const time = Date.parse("2026-10-14T10:00:00Z") / 1000
    const commit = { id: "0123456789abcdef0123456789abcdef01234567", author: "Jane Doe", committer: "Safe Push Robot", time }
    expect(describeIntegrationCommit(commit)).toBe("Merged as 01234567 by Safe Push Robot")
  })
})
