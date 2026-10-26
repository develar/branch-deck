// @spec spec/branch/parked-branches.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { StepBlocker } from "@/lib/bindings"
import { formatParkedBlocker, formatParkedLabel } from "@/lib/parked"
import { presentNextStep } from "@/presenters/next-step"
import { testContext, testStep } from "@/test-support/next-step"

describe("parked branches", () => {
  it("says why nothing starts, of a branch and of one built on it", () => {
    expect(formatParkedBlocker({ note: "", with: null })).toBe("Parked: Unpark it to push")
    expect(formatParkedBlocker({ note: "", with: "auth" })).toBe("Built on parked auth: Unpark auth to push")
    expect(formatParkedLabel({ note: "waits for IJPL-1", with: null })).toBe("Parked, kept local: waits for IJPL-1")
    expect(formatParkedLabel({ note: "", with: "auth" })).toBe("Parked with auth, kept local")
  })

  it("starts no Safe Push and no Dry Run of a parked branch: its step is none, on every surface", () => {
    // Rust settles the step of a parked branch (row 1) with the reason of the rule (`domain_policy::parked`)
    const parked: StepBlocker = { code: "rule", reason: { code: "parked", branch: "login", with: "auth", note: "" } }
    const step = presentNextStep(testStep({ kind: "settled" }, { safePush: parked, dryRun: parked }), testContext({ subject: "login" }))
    expect(step.kind).toBe("none")
    expect(step.safePushBlocker).toBe("Built on parked auth: Unpark auth to push")
    expect(step.dryRunBlocker).toBe("Built on parked auth: Unpark auth to push")
    expect(step.needsReadiness).toBe(false)
  })
})
