// @spec spec/branch/parked-branches.spec.md
import { describe, expect, it } from "vite-plus/test"
import { getBranchNextStep } from "@/lib/branch-next-step"
import { formatParkedBlocker, formatParkedLabel, stackParkedState } from "@/lib/parked"
import { facts } from "@/lib/test-next-step-facts"

describe("parked branches", () => {
  it("says why nothing starts, of a branch, of one built on it and of the whole stack", () => {
    expect(formatParkedBlocker({ note: "", with: null })).toBe("Parked: Unpark it to push")
    expect(formatParkedBlocker({ note: "", with: "auth" })).toBe("Built on parked auth: Unpark auth to push")
    const whole = stackParkedState({ member: "side", parked: { note: "later", with: null } })
    expect(whole).toEqual({ note: "later", with: "side", wholeStack: true })
    expect(formatParkedBlocker(whole!)).toBe("Holds parked side: land the stack in pieces")
    expect(formatParkedBlocker(stackParkedState({ member: "oauth", parked: { note: "", with: "login" } })!)).toBe("Holds parked login: land the stack in pieces")
    expect(stackParkedState(null)).toBeNull()
    expect(formatParkedLabel({ note: "waits for IJPL-1", with: null })).toBe("Parked, kept local: waits for IJPL-1")
    expect(formatParkedLabel({ note: "", with: "auth" })).toBe("Parked with auth, kept local")
  })

  it("starts no Safe Push and no Dry Run of a parked branch: its step is none, on every surface", () => {
    const step = getBranchNextStep(facts({ parked: { note: "", with: "auth" } }))
    expect(step.kind).toBe("none")
    expect(step.safePushBlocker).toBe("Built on parked auth: Unpark auth to push")
    expect(step.dryRunBlocker).toBe("Built on parked auth: Unpark auth to push")
    expect(step.needsReadiness).toBe(false)
  })
})
