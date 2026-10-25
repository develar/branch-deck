// @spec spec/repository/onboarding.spec.md
import { describe, expect, it } from "vite-plus/test"
import { describeAgentsHere } from "@/lib/agents-hint"

describe("describeAgentsHere", () => {
  it("says where agents work by the rule of the repository, and that the Agent CLI lets them", () => {
    expect(describeAgentsHere(true, false)).toBe(
      "You work in this checkout. With the Agent CLI, coding agents such as Claude Code lease worktrees of their own and land each commit on your branch.",
    )
    expect(describeAgentsHere(true, true)).toBe(
      "You work in this checkout. Coding agents such as Claude Code lease worktrees of their own through bd and land each commit on your branch.",
    )
    expect(describeAgentsHere(false, false)).toBe("With the Agent CLI, coding agents such as Claude Code work in this checkout beside you.")
    expect(describeAgentsHere(false, true)).toBe("Coding agents such as Claude Code work in this checkout beside you, through bd.")
  })
})
