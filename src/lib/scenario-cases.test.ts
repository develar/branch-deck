// The app's side of the scenario table (`crates/domain-policy/cases/*.json`, its README): each case's `given` through
// `getBranchNextStep` and `getSafePushDecision`, today's answer expected; `crates/bd-remote/src/scenario_test.rs` is
// the side of bd
import { describe, expect, it } from "vite-plus/test"
import { getSafePushDecision } from "@/lib/safe-push-decision"
import type { ScenarioCase } from "@/test-support/scenario-case"
import { appOutcome, bdOutcome, scenarioCase } from "@/test-support/scenario-case"
import { safePushFactsOf } from "@/test-support/scenario-facts"

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
}

function assertAppAnswer(scenario: ScenarioCase): void {
  const { step, facts } = safePushFactsOf(scenario.given)
  if (scenario.app.step !== undefined) {
    expect({ kind: step.kind, safePushBlocker: step.safePushBlocker, dryRunBlocker: step.dryRunBlocker }, `${scenario.id}: app.step`).toMatchObject(scenario.app.step)
  }
  expect(getSafePushDecision(facts), `${scenario.id}: app.decision`).toMatchObject(scenario.app.decision)
}

describe("scenario cases", () => {
  it("are well formed: unique ids, known groups, and a discrepancy or an asymmetry exactly where the sides differ", () => {
    expect(cases.length).toBeGreaterThan(0)
    const ids = cases.map(scenario => scenario.id)
    expect(ids.filter((id, index) => ids.indexOf(id) !== index)).toEqual([])
    expect([...new Set(cases.map(scenario => scenario.group))].filter(group => GROUPS[group] === undefined)).toEqual([])
    for (const scenario of cases) {
      const differs = appOutcome(scenario) !== bdOutcome(scenario)
      const explained = scenario.discrepancy !== null || scenario.asymmetry !== undefined
      expect({ id: scenario.id, differs, explained }).toEqual({ id: scenario.id, differs, explained: differs })
    }
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
