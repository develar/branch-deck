// The app's side of the scenario table (`crates/domain-policy/cases/*.json`, its README): each case's `given` through
// `getBranchNextStep` and `getSafePushDecision`, today's answer expected; `crates/bd-remote/src/scenario_test.rs` is
// the side of bd
import { afterEach, describe, expect, it } from "vite-plus/test"
import { getStackNextStepNow } from "@/hooks/useStackNextStep"
import { getSafePushDecision } from "@/lib/safe-push-decision"
import { branchSyncStore, getStackDescendants, stackMainLineIn } from "@/stores/branch-sync"
import { testBranch } from "@/stores/test-branch"
import type { ExercisedCase, ScenarioCase } from "@/test-support/scenario-case"
import { appOutcome, bdOutcome, isExercised, scenarioCase } from "@/test-support/scenario-case"
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
  landing: "the landing cases answer as the app answers today",
  discrepancies: "the discrepancy cases name d1 to d10 as the register does",
}

/** The ids of the registry of `docs/design/rust-domain-authority.md`: the group `discrepancies` has one entry of each */
const REGISTRY = ["D1", "D2", "D3a", "D3b", "D4", "D5", "D6", "D7", "D8", "D9", "D10"]

/** The rows of the table of `spec/branch/next-step.spec.md` (row 4 is removed); a case names one as `#the-table:<row>` */
const NEXT_STEP_ROWS = ["1", "1′", "2", "3", "5", "5′", "6", "7", "8", "9", "10", "8′", "10′", "11"]

function assertAppAnswer(scenario: ExercisedCase): void {
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
    for (const scenario of cases.filter(isExercised)) {
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

  it("name every discrepancy once in the registry, the others as the twins of its entries, and say why where the classes agree", () => {
    const registry = cases.filter(scenario => scenario.group === "discrepancies")
    const byId = (a: string, b: string) => a.localeCompare(b)
    expect(registry.map(entry => entry.discrepancy ?? "").toSorted(byId)).toEqual(REGISTRY.toSorted(byId))
    for (const entry of registry) {
      if (entry.kind === "recorded" && entry.twin !== undefined) {
        const twin = cases.find(scenario => scenario.id === entry.twin)
        expect({ id: entry.id, group: twin?.group !== "discrepancies", discrepancy: twin?.discrepancy }).toEqual({ id: entry.id, group: true, discrepancy: entry.discrepancy })
      }
    }
    const twins = new Set(registry.flatMap(entry => (entry.kind === "recorded" && entry.twin !== undefined ? [entry.twin] : [])))
    const outside = cases.filter(scenario => scenario.group !== "discrepancies" && scenario.discrepancy !== null)
    expect(outside.map(scenario => scenario.id).filter(id => !twins.has(id))).toEqual([])
    expect(cases.filter(scenario => scenario.kind === "recorded" && scenario.group !== "discrepancies").map(scenario => scenario.id)).toEqual([])
    // a discrepancy whose classes agree says what differs (D9: both start, the app by a failed read of the rules)
    const unexplained = cases
      .filter(isExercised)
      .filter(scenario => scenario.discrepancy !== null && appOutcome(scenario) === bdOutcome(scenario) && scenario.today?.why === undefined)
    expect(unexplained.map(scenario => scenario.id)).toEqual([])
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
      for (const scenario of ofGroup.filter(isExercised)) {
        assertAppAnswer(scenario)
      }
    })
  }
})

describe("the whole stack of D8", () => {
  afterEach(() => {
    branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false, stackBranches: {} })
  })

  // the table the sync of `the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8` answers (its
  // registry entry `d8-whole-stack-parent`): A its own parent, C declared on B, which has no commits, blocked by A. bd
  // answers the chain A → C (`whole_stack_of` follows `blocked_by`); the app follows `effectiveBase ?? declaredParent`,
  // B, which no row has: C is a root of its own, and the island of A has no whole stack
  it("the whole stack of a blocked branch with an inactive declared parent is d8", () => {
    const a = testBranch("A", null, {
      stack: {
        ...testBranch("A", null).stack!,
        problems: [
          {
            problem: { SelfParent: ["a2"] },
            label: "its own parent",
            reason: "A is declared as its own parent",
            message: "A prefix makes the branch its own parent: a2",
            commits: [],
            fixes: ["Unstack"],
          },
        ],
      },
    })
    const c = testBranch("C", "B", {
      stack: { ...testBranch("C", "B").stack!, parentState: "Missing", effectiveBase: null, blockedBy: { parent: "A", reason: "Problem" }, prBaseChanged: true },
    })
    branchSyncStore.setState({ branches: { C: c, A: a }, branchOrder: ["C", "A"], stacksEnabled: true, baselineBranch: "origin/master", stackBranches: {} })

    const state = branchSyncStore.getState()
    expect({ a: stackMainLineIn(state, "A"), c: stackMainLineIn(state, "C") }).toEqual({ a: ["A"], c: ["C"] })
    expect(getStackDescendants("A")).toEqual([])
    expect(getStackNextStepNow("A")).toBeNull()
  })
})
