import { describe, expect, it } from "vite-plus/test"
import type { Concern, ReviewPlan, ReviewStep, ReviewUnit, StepCommit } from "@/lib/bindings"
import { bodyParagraphs, formatStepTitle, presentBrief, presentChapters, WITHOUT_A_STORY } from "@/presenters/review-steps"
import { presentNarrative } from "@/presenters/review-units"

function commit(subject: string, body: string, role: StepCommit["role"] = "starts"): StepCommit {
  return { id: subject, subject, body, role }
}

function step(id: string, commits: StepCommit[], stops: [string, string][], tests: Partial<ReviewStep["tests"]> = {}): ReviewStep {
  return {
    id,
    commits,
    concern: null,
    stops: stops.map(([unit, path]) => ({ unit, path, blocks: null, uses: null, also: [], concern: null })),
    paths: [...new Set(stops.map(([, path]) => path))],
    elsewhere: [],
    tests: { named: [], unnamed: [], scenarios: [], ...tests },
  }
}

function plan(steps: ReviewStep[], issue: string | null = null, concerns: Concern[] = []): Pick<ReviewPlan, "steps" | "concerns" | "brief"> {
  return {
    steps: { source: { kind: "commits" }, steps, coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 } },
    concerns,
    brief: { issue, contract: [] },
  }
}

describe("review steps", () => {
  it("names a step by the subject of the commit that starts it, without the issue the brief names, a step of no story by what it is", () => {
    expect(formatStepTitle("IJAI-1 the open asks the layout", "IJAI-1")).toBe("The open asks the layout")
    expect(formatStepTitle("IJAI-1: the open asks", "IJAI-1")).toBe("The open asks")
    expect(formatStepTitle("the open asks", null)).toBe("The open asks")
    const chapters = presentChapters(
      plan([step("a", [commit("IJAI-1 the rule", "Why.")], [["u:rule", "src/Rule.kt"]]), step("no-story", [commit("wip", "", "noStory")], [["u:wip", "src/Wip.kt"]])], "IJAI-1"),
    )
    expect(chapters.map(chapter => [chapter.number, chapter.label, chapter.heading, chapter.reference, chapter.parts.length])).toEqual([
      [1, "The rule", "1 · The rule", "step 1", 1],
      [2, WITHOUT_A_STORY, `2 · ${WITHOUT_A_STORY}`, "step 2", 1],
    ])
  })

  it("names a step of a concern by its concern and keeps its main rows", () => {
    const concern: Concern = {
      id: "c",
      name: { source: "token", token: "proxy" },
      parts: [{ unit: "u:a", path: "a.json", blocks: null }],
      paths: ["a.json"],
      elsewhere: ["a.json"],
      evidence: [{ kind: "token", detail: "ij-proxy", joins: 2 }],
      fingerprint: "f",
    }
    const of = plan([{ ...step("c", [commit("one", "")], [["u:a", "a.json"]]), concern: "c" }], null, [concern])
    const [chapter] = presentChapters(of)
    expect([chapter!.label, chapter!.heading, chapter!.hover, chapter!.elsewhere]).toEqual(["proxy", "proxy", "Joined by the identifier ij-proxy", ["a.json"]])
  })

  it("words the brief: the why of each step its first paragraph, the rest folded, its rounds, the tests that name its code, a step no test proves", () => {
    const brief = presentBrief(
      plan([
        step(
          "a",
          [commit("the rule", "One rule decides.\n\nThe product decision is\nto keep the layout."), commit("the rule again", "Renamed.", "followsUp")],
          [["u:rule", "src/Rule.kt"]],
          { named: [{ name: "allowed", tests: [] }], unnamed: ["show"] },
        ),
        step("b", [commit("the paint", "It paints first.")], [["u:paint", "src/Frame.kt"]], { unnamed: ["paint", "create", "run", "open", "close"] }),
        step("c", [commit("the story", "It restarts.")], [["u:spec", "spec/a.spec.md"]], { scenarios: ["tests/restart.json"] }),
        step("d", [commit("the try", "It tried.")], []),
      ]),
    )
    expect(brief.steps.map(it => [it.title, it.why, it.more, it.rounds, it.named, it.unnamed, it.unproven, it.scenarios, it.landsNothing])).toEqual([
      [
        "The rule",
        "One rule decides.",
        ["The product decision is to keep the layout."],
        ["Follows up: The rule again"],
        "Tests that name it: allowed",
        "Not named by a test: show",
        null,
        null,
        false,
      ],
      ["The paint", "It paints first.", [], [], null, null, "No test in this change names paint, create, run, open +1", null, false],
      ["The story", "It restarts.", [], [], null, null, null, "Proven by restart.json", false],
      ["The try", "It tried.", [], [], null, null, null, null, true],
    ])
    expect(bodyParagraphs("a\nb\n\n\nc")).toEqual(["a b", "c"])
    // the only step what no evidence joined: no step to tell
    const rest: Concern = { id: "r", name: { source: "rest" }, parts: [], paths: [], elsewhere: [], evidence: [], fingerprint: "r" }
    const restStep = { ...step("r", [commit("fix", "", "noStory")], [["u:a", "a.kt"]]), concern: "r" }
    const untold = presentBrief({
      ...plan([restStep], null, [rest]),
      steps: { source: { kind: "concerns", reason: "noStory" }, steps: [restStep], coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 } },
    })
    expect([untold.steps, untold.source]).toEqual([[], null])
  })

  it("lays a declaration without a block of its own out in its step as the row of the block another step shows", () => {
    const block = { index: 0, oldStart: 4, oldEnd: 6, newStart: 4, newEnd: 9 }
    const unit = (id: string, kind: ReviewUnit["kind"], name: string, blocks: (typeof block)[]): ReviewUnit => ({
      id,
      kind,
      name,
      key: `a.Manager.${name}`,
      signature: "",
      public: true,
      line: 5,
      slices: [{ path: "src/Manager.kt", oldPath: null, whole: false, blocks, fingerprint: id }],
      fingerprint: id,
    })
    const units = [unit("u:hooks", { kind: "removed" }, "hooks", []), unit("u:allowed", { kind: "added" }, "allowed", [block])]
    const chapters = presentChapters(
      plan([step("a", [commit("drop the hooks", "Why.")], [["u:hooks", "src/Manager.kt"]]), step("b", [commit("the rule", "Why.")], [["u:allowed", "src/Manager.kt"]])]),
    )
    const sections = presentNarrative(units, [{ path: "src/Manager.kt", blocks: [block] }], [], [], chapters)
    expect(sections.map(section => [section.key, section.concernHeading, [...(section.units?.marks ?? [])], [...(section.units?.elsewhere ?? [])]])).toEqual([
      ["file:src/Manager.kt", "1 · Drop the hooks", [], [[0, "hooks — removed · shown in step 2"]]],
      ["file:src/Manager.kt#1", "2 · The rule", [[0, "allowed — new · hooks — removed"]], []],
    ])
  })
})
