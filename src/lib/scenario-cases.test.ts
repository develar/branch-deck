// The app's side of the scenario table (`crates/domain-policy/cases/*.json`, its README): each case's `given` through
// `getBranchNextStep` and `getSafePushDecision`, today's answer expected; `crates/bd-remote/src/scenario_test.rs` is
// the side of bd
import { describe, expect, it } from "vite-plus/test"
import { getSafePushDecision } from "@/lib/safe-push-decision"
import type { ScenarioCase } from "@/test-support/scenario-case"
import { appOutcome, bdOutcome, scenarioCase } from "@/test-support/scenario-case"
import { nextStepOf, safePushFactsOf } from "@/test-support/scenario-facts"

const files = import.meta.glob<unknown>("/crates/domain-policy/cases/*.json", { eager: true, import: "default" })

const cases: ScenarioCase[] = Object.entries(files).flatMap(([file, content]) =>
  (content as unknown[]).map((raw, index) => {
    const parsed = scenarioCase.safeParse(raw)
    if (!parsed.success) {
      throw new Error(`${file} case ${index}: ${parsed.error.message}`)
    }
    return parsed.data
  }),
)

/** The groups and the titles of their tests: the same groups as the test functions of the Rust side */
const GROUPS: Record<string, string> = {
  "modes-and-routes": "the safe push mode and route cases answer as the app answers today",
  profiles: "the safe push profile cases answer as the app answers today",
  review: "the safe push review cases answer as the app answers today",
  "next-step-rows": "the next step row cases answer as the app answers today",
  "stack-and-whole-stack": "the stack and whole stack cases answer as the app answers today",
}

/** The rows of the table of `spec/branch/next-step.spec.md` (row 4 is removed); a case names one as `#the-table:<row>` */
const NEXT_STEP_ROWS = ["1", "1′", "2", "3", "5", "5′", "6", "7", "8", "9", "10", "8′", "10′", "11"]

function assertAppAnswer(scenario: ScenarioCase): void {
  const { decision } = scenario.app
  const step = decision === undefined ? nextStepOf(scenario.given) : safePushFactsOf(scenario.given).step
  if (scenario.app.step !== undefined) {
    expect({ kind: step.kind, safePushBlocker: step.safePushBlocker, dryRunBlocker: step.dryRunBlocker }, `${scenario.id}: app.step`).toMatchObject(scenario.app.step)
  }
  if (decision !== undefined) {
    expect(getSafePushDecision(safePushFactsOf(scenario.given).facts), `${scenario.id}: app.decision`).toMatchObject(decision)
  }
}

describe("scenario cases", () => {
  it("are well formed: unique ids, known groups, a discrepancy or an asymmetry where the sides differ, an asymmetry only there", () => {
    expect(cases.length).toBeGreaterThan(0)
    const ids = cases.map(scenario => scenario.id)
    expect(ids.filter((id, index) => ids.indexOf(id) !== index)).toEqual([])
    expect([...new Set(cases.map(scenario => scenario.group))].filter(group => GROUPS[group] === undefined)).toEqual([])
    for (const scenario of cases) {
      const differs = appOutcome(scenario) !== bdOutcome(scenario)
      const explained = scenario.discrepancy !== null || scenario.asymmetry !== undefined
      // a discrepancy may stand where the classes agree: both sides start, one of them for a wrong reason (D9)
      expect({ id: scenario.id, differs, explained: differs && explained, asymmetry: scenario.asymmetry !== undefined && !differs }).toEqual({
        id: scenario.id,
        differs,
        explained: differs,
        asymmetry: false,
      })
    }
  })

  it("cover every row of the next step table", () => {
    const named = new Set(cases.flatMap(scenario => scenario.spec).flatMap(spec => /^spec\/branch\/next-step\.spec\.md#the-table:(.+)$/.exec(spec)?.[1] ?? []))
    expect(NEXT_STEP_ROWS.filter(row => !named.has(row))).toEqual([])
    expect([...named].filter(row => !NEXT_STEP_ROWS.includes(row))).toEqual([])
  })

  for (const [group, title] of Object.entries(GROUPS)) {
    it(title, () => {
      const ofGroup = cases.filter(scenario => scenario.group === group)
      expect(ofGroup.length, group).toBeGreaterThan(0)
      for (const scenario of ofGroup) {
        assertAppAnswer(scenario)
      }
    })
  }
})
