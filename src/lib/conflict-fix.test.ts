// @spec spec/branch/conflict-fix.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { FixComponent, FixStep } from "@/lib/bindings"
import { componentFixing, describeFixGrouping, describeFixResult, describeFixStep, fixTitle } from "@/lib/conflict-fix"
import { buildConflictPrompt } from "@/lib/conflict-fix-prompt"

const subjects: Record<string, string> = { a1: "Add login", b1: "Add tokens" }
const subjectOf = (id: string) => subjects[id]

function step(kind: FixStep["kind"], commits: string[], from: string[], to: string, fixes: string): FixStep {
  return { kind, commits, from, to, fixes }
}

function component(overrides: Partial<FixComponent>): FixComponent {
  return { steps: [], plan: {}, fixed: [], unblocked: [], emptied: [], orphanedChildren: [], rewritten: 0, ...overrides }
}

describe("describeFixStep", () => {
  it("says a step as the Move action says it", () => {
    expect(describeFixStep(step("move", ["a1"], ["ui"], "api", "ui"), subjectOf)).toBe("Move “Add login” to api")
    expect(describeFixStep(step("pullIn", ["b1"], [], "auth", "auth"), subjectOf)).toBe("Move the unassigned “Add tokens” to auth")
    expect(describeFixStep(step("pullIn", ["a1", "b1"], ["x", "y"], "auth", "auth"), subjectOf)).toBe("Move 2 commits of x and y to auth")
    expect(describeFixStep(step("pullIn", ["a1", "b1"], [], "auth", "auth"), subjectOf)).toBe("Move 2 unassigned commits to auth")
    expect(describeFixStep(step("restack", [], ["core"], "api", "ui"), subjectOf)).toBe("Stack ui on api")
  })
})

describe("describeFixResult", () => {
  it("names what applies cleanly, syncs again and is emptied, and where the commits went", () => {
    const fix = component({
      steps: [step("move", ["a1"], ["docs"], "api", "ui")],
      fixed: ["ui", "api"],
      unblocked: ["web"],
      emptied: ["docs"],
      orphanedChildren: ["notes"],
    })
    expect(describeFixResult(fix)).toBe(
      "Afterwards ui and api apply cleanly, and web syncs again. docs has no commits left: its commits are in api. notes is built on the nearest branch below with commits.",
    )
  })

  it("names the pull requests the next push changes and keeps the one of an emptied branch open", () => {
    const fix = component({ steps: [step("move", ["a1"], ["docs"], "api", "docs")], fixed: ["api"], emptied: ["docs"] })
    expect(describeFixResult(fix, () => true)).toBe(
      "Afterwards api applies cleanly. docs has no commits left: its commits are in api; its pull request stays open. The next push updates the pull request of api.",
    )
  })
})

describe("describeFixGrouping", () => {
  it("says that the next commits of a whole issue follow it", () => {
    expect(describeFixGrouping(component({ plan: { issue_groups: { "IJAI-4047": ["IJAI-4085"] } } }))).toEqual(["Future IJAI-4085 commits go to IJAI-4047 too"])
  })
})

describe("componentFixing", () => {
  it("finds the component with a step for the branch, else one that fixes or unblocks it", () => {
    const first = component({ steps: [step("move", ["a1"], ["x"], "y", "x")], fixed: ["x"] })
    const second = component({ steps: [step("move", ["b1"], ["z"], "y", "z")], fixed: ["z"], unblocked: ["w"] })
    expect(componentFixing([first, second], "z")).toBe(second)
    expect(componentFixing([first, second], "w")).toBe(second)
    expect(componentFixing([first, second], "q")).toBeUndefined()
    expect(fixTitle(first)).toBe("Fixed the conflict of x")
    expect(fixTitle(component({ fixed: ["x", "y"] }))).toBe("Fixed the conflicts of 2 branches")
  })
})

describe("buildConflictPrompt", () => {
  const input = {
    repositoryPath: "/repo",
    branch: "ui",
    commit: { hash: "abc1234", subject: "Show the timeout" },
    builtOn: "origin/master",
    files: ["settings.js"],
    explanation: "2 commits of api and the branch it is built on changed these files first",
  }

  it("hands a conflict without a fix to bd's trial builds and asks before a rewrite", () => {
    const prompt = buildConflictPrompt({ ...input, bd: { command: "bd -C /repo", powershell: false } })
    expect(prompt).toContain("The commit “Show the timeout” (abc1234) does not apply cleanly onto `origin/master`. 2 commits of api")
    expect(prompt).toContain("`bd -C /repo check --fix`")
    expect(prompt).toContain("Only when I approve: `bd group --expect-head <head> --sync --plan plan.json`")
    expect(prompt).toContain("- settings.js")
  })

  it("works without bd", () => {
    expect(buildConflictPrompt({ ...input, bd: null })).toContain("Show it to me before you rewrite anything.")
  })
})
