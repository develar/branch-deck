// @spec spec/branch/parked-branches.spec.md
import { describe, expect, it } from "vite-plus/test"
import { getBranchNextStep } from "@/lib/branch-next-step"
import { collectParkedStates, formatParkedBlocker, formatParkedLabel, wholeStackParkedState } from "@/lib/parked"
import { facts } from "@/lib/test-next-step-facts"

describe("parked branches", () => {
  it("parks a branch by its line, and a branch built on it with the nearest parked ancestor", () => {
    // auth <- login <- oauth, docs alone; sso blocked on oauth
    const parents: Record<string, string> = { login: "auth", oauth: "login", sso: "oauth" }
    const names = ["auth", "login", "oauth", "sso", "docs"]
    const states = collectParkedStates(names, { auth: "waits for IJPL-1" }, name => parents[name] ?? null)
    expect(Object.fromEntries(states)).toEqual({
      auth: { note: "waits for IJPL-1", with: null },
      login: { note: "waits for IJPL-1", with: "auth" },
      oauth: { note: "waits for IJPL-1", with: "auth" },
      sso: { note: "waits for IJPL-1", with: "auth" },
    })
    const nearest = collectParkedStates(names, { auth: "", login: "mine" }, name => parents[name] ?? null)
    expect(nearest.get("oauth")).toEqual({ note: "mine", with: "login" })
    // no line, no state; a cycle of parents ends
    expect(collectParkedStates(names, {}, name => parents[name] ?? null).size).toBe(0)
    expect(collectParkedStates(["a", "b"], { c: "" }, name => (name === "a" ? "b" : "a")).size).toBe(0)
  })

  it("says why nothing starts, of a branch, of one built on it and of the whole stack", () => {
    expect(formatParkedBlocker({ note: "", with: null })).toBe("Parked: Unpark it to push")
    expect(formatParkedBlocker({ note: "", with: "auth" })).toBe("Built on parked auth: Unpark auth to push")
    const whole = wholeStackParkedState(["auth", "login", "side"], name => (name === "side" ? { note: "later", with: null } : null))
    expect(whole).toEqual({ note: "later", with: "side", wholeStack: true })
    expect(formatParkedBlocker(whole!)).toBe("Holds parked side: land the stack in pieces")
    expect(wholeStackParkedState(["auth"], () => null)).toBeNull()
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
