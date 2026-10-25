// @spec spec/repository/onboarding.spec.md
import { describe, expect, it } from "vite-plus/test"
import { describeSyncStep } from "@/lib/sync-step"

describe("describeSyncStep", () => {
  it("names what a running sync does, and only that it runs before a step is known", () => {
    expect(describeSyncStep("grouping")).toBe("Grouping the commits…")
    expect(describeSyncStep("building")).toBe("Building the branches…")
    expect(describeSyncStep("detection")).toBe("Checking which branches landed…")
    expect(describeSyncStep(null)).toBe("Syncing branches…")
  })
})
