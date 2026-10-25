// The app's side of the scenario table (`crates/domain-policy/cases/*.json`, its README): each case's `given` seeded
// into the stores, then the app's own readers (`getBranchNextStepOf`, `getStackNextStepNow`, `readSafePush`), today's
// answer expected; `crates/bd-remote/src/scenario_test.rs` is the side of bd
import { describe, expect, it, vi } from "vite-plus/test"
import type { ExercisedCase, ScenarioCase } from "@/test-support/scenario-case"
import { appOutcome, bdOutcome, isExercised, scenarioCase } from "@/test-support/scenario-case"
import { UPDATE_FACTS, factsOf, nextStepOf, safePushReadOf, seedReadiness, seedScenario } from "@/test-support/scenario-facts"

vi.mock("@/stores/repository", () => import("@/test-support/scenario-repository-mock"))

const files = import.meta.glob<unknown>("/crates/domain-policy/cases/*.json", { eager: true, import: "default" })
/** The specs and the design docs a case may name, by their path from the repository root */
const documents = import.meta.glob<string>(["/spec/**/*.md", "/docs/**/*.md"], { eager: true, query: "?raw", import: "default" })

/** `given` of each case as the file has it, before the defaults: what its facts say they were computed from */
const rawGiven = new Map<string, unknown>()

const cases: ScenarioCase[] = Object.entries(files).flatMap(([file, content]) =>
  (content as unknown[]).map((raw, index) => {
    const parsed = scenarioCase.safeParse(raw)
    if (!parsed.success) {
      throw new Error(`${file} case ${index}: ${parsed.error.message}`)
    }
    rawGiven.set(parsed.data.id, (raw as { given?: unknown }).given)
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
  discrepancies: "the discrepancy cases name every id as the register does",
}

/** The ids of the registry of `docs/design/rust-domain-authority.md`: the group `discrepancies` has one entry of each */
const REGISTRY = ["D1", "D2", "D3a", "D3b", "D4", "D5", "D6", "D7", "D8", "D9", "D10", "D11", "D12", "D13", "D14", "D15"]

/** The text of a spec or a design doc by its path from the repository root; undefined without the file */
function documentOf(path: string): string | undefined {
  return documents[`/${path}`]
}

/** The GitHub slug of a Markdown heading: lower case, the letters, digits, spaces and hyphens kept, spaces as hyphens */
function headingSlug(heading: string): string {
  return heading
    .trim()
    .toLowerCase()
    .replaceAll(/[^\p{L}\p{N} _-]/gu, "")
    .replaceAll(" ", "-")
}

/** The slugs of the headings of a Markdown text */
function headingsOf(text: string): string[] {
  return text
    .split("\n")
    .flatMap(line => /^#+ (.+)$/.exec(line)?.[1] ?? [])
    .map(headingSlug)
}

/** The rows of the table of `spec/branch/next-step.spec.md` (`| <#> |` under "The table"; row 4 is removed) */
function nextStepRows(): string[] {
  const text = documentOf("spec/branch/next-step.spec.md")!
  const table = text.slice(text.indexOf("\n### The table\n"))
  const rows = [...table.matchAll(/^ {2}\| ([^|]+?) \| /gmu)].map(match => match[1]!).filter(row => row !== "#" && !row.startsWith("-"))
  return rows.filter(row => !table.includes(`  | ${row} | removed:`))
}

/** Why an anchor of a case goes nowhere: no file, no heading of its slug, no row `| <row> |` of a `#the-table:<row>`; null when it resolves */
function brokenSpec(spec: string): string | null {
  const [path, anchor] = spec.split("#") as [string, string | undefined]
  const text = documentOf(path)
  if (text === undefined) {
    return `${spec}: no file ${path}`
  }
  if (anchor === undefined) {
    return null
  }
  const [slug, row] = anchor.split(":") as [string, string | undefined]
  if (!headingsOf(text).includes(slug)) {
    return `${spec}: ${path} has no heading of the slug ${JSON.stringify(slug)}`
  }
  return row !== undefined && !text.includes(`\n  | ${row} |`) ? `${spec}: ${path} has no row ${JSON.stringify(row)} of the table` : null
}

/**
 * What the app answers today for the case, against its `app` column: the step as the row shows it, the decision of its
 * start where the case gives one, from the seeded stores; the failures, each named with the case
 */
async function appFailures(scenario: ExercisedCase): Promise<string[]> {
  const failures: string[] = []
  const check = (column: string, run: () => void): void => {
    try {
      run()
    } catch (error: unknown) {
      failures.push(`${scenario.id}: app.${column}: ${error instanceof Error ? error.message : String(error)} — ${scenario.title}`)
    }
  }
  // the facts of another `given` answer for another case: the Rust test rewrites them
  expect(factsOf(scenario.id).given, `${scenario.id}: the facts are of another given; ${UPDATE_FACTS}`).toEqual(rawGiven.get(scenario.id))
  await seedScenario(scenario.id, scenario.given)
  check("step", () => {
    const step = nextStepOf(scenario.given)
    if (scenario.app.step !== undefined) {
      expectSubset({ kind: step.kind, safePushBlocker: step.safePushBlocker, dryRunBlocker: step.dryRunBlocker }, scenario.app.step)
    }
  })
  check("decision", () => {
    if (scenario.app.decision !== undefined) {
      expectSubset(safePushReadOf(scenario.given).decision, scenario.app.decision)
    }
  })
  return failures
}

/** `actual` has every key of `expected` with its value; the failure names each key with both values */
function expectSubset(actual: object, expected: object): void {
  const differing = Object.entries(expected)
    .filter(([key, value]) => JSON.stringify((actual as Record<string, unknown>)[key]) !== JSON.stringify(value))
    .map(([key, value]) => `${key}: expected ${JSON.stringify(value)}, the app answers ${JSON.stringify((actual as Record<string, unknown>)[key])}`)
  if (differing.length > 0) {
    throw new Error(`${differing.join("; ")} (all: ${JSON.stringify(actual)})`)
  }
}

describe("scenario cases", () => {
  it("are well formed: unique ids, known groups, resolving anchors, a discrepancy or an asymmetry where the sides differ, an asymmetry only there", () => {
    expect(cases.length).toBeGreaterThan(0)
    const ids = cases.map(scenario => scenario.id)
    expect(ids.filter((id, index) => ids.indexOf(id) !== index)).toEqual([])
    expect([...new Set(cases.map(scenario => scenario.group))].filter(group => GROUPS[group] === undefined)).toEqual([])
    expect(
      cases.flatMap(scenario => scenario.spec.flatMap(spec => brokenSpec(spec) ?? [])),
      "the anchors of the cases",
    ).toEqual([])
    for (const scenario of cases.filter(isExercised)) {
      const differs = appOutcome(scenario) !== bdOutcome(scenario)
      const explained = scenario.discrepancy !== null || scenario.asymmetry !== undefined
      // a discrepancy may stand where the classes agree: both sides start, one of them for a wrong reason (D9 until s1-holes)
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
      for (const twin of entry.kind === "recorded" ? entry.twins : []) {
        const found = cases.find(scenario => scenario.id === twin)
        expect({ id: entry.id, twin, group: found?.group !== "discrepancies", discrepancy: found?.discrepancy }).toEqual({
          id: entry.id,
          twin,
          group: true,
          discrepancy: entry.discrepancy,
        })
      }
    }
    const twins = new Set(registry.flatMap(entry => (entry.kind === "recorded" ? entry.twins : [])))
    const outside = cases.filter(scenario => scenario.group !== "discrepancies" && scenario.discrepancy !== null)
    expect(outside.map(scenario => scenario.id).filter(id => !twins.has(id))).toEqual([])
    expect(cases.filter(scenario => scenario.kind === "recorded" && scenario.group !== "discrepancies").map(scenario => scenario.id)).toEqual([])
    // a closed discrepancy stays the key of its id in the registry only, with no twins
    expect(cases.filter(scenario => scenario.closed !== undefined && scenario.group !== "discrepancies").map(scenario => scenario.id)).toEqual([])
    expect(registry.filter(entry => entry.closed !== undefined && entry.kind === "recorded" && entry.twins.length > 0).map(entry => entry.id)).toEqual([])
    // a discrepancy whose classes agree says what differs (D9 until s1-holes: both started, the app by a failed read); a
    // closed one has nothing left to say
    const unexplained = cases
      .filter(isExercised)
      .filter(scenario => scenario.discrepancy !== null && scenario.closed === undefined && appOutcome(scenario) === bdOutcome(scenario) && scenario.today?.why === undefined)
    expect(unexplained.map(scenario => scenario.id)).toEqual([])
  })

  it("cover every row of the next step table", () => {
    const rows = nextStepRows()
    expect(rows).toContain("1")
    expect(rows).not.toContain("4")
    const named = new Set(cases.flatMap(scenario => scenario.spec).flatMap(spec => /^spec\/branch\/next-step\.spec\.md#the-table:(.+)$/.exec(spec)?.[1] ?? []))
    expect(rows.filter(row => !named.has(row))).toEqual([])
    expect([...named].filter(row => !rows.includes(row))).toEqual([])
  })

  // the facts of a readiness answer carry the tip and the pull request Rust asked with; the app's input of the branch is
  // computed by the app's own rule (`getPullRequestStart`): one that drifts from the other fails the seed instead of
  // filing Rust's answer as fresh under the app's key
  it("refuse the readiness facts asked with another start or tip than the app computes now", async () => {
    const scenario = cases.find(candidate => candidate.id === "row-6-draft")
    if (scenario === undefined || !isExercised(scenario)) {
      throw new Error("row-6-draft is an exercised case with a pull request start")
    }
    await seedScenario(scenario.id, scenario.given)
    const facts = factsOf(scenario.id)
    const auth = facts.readiness["auth"]!
    expect(auth.pullRequest).toBe(7)
    expect(() => seedReadiness({ ...facts, readiness: { auth: { ...auth, pullRequest: null } } })).toThrow(
      /auth: the facts asked another input .*"pullRequest":null.* than the app asks now .*"pullRequest":7/,
    )
    expect(() => seedReadiness({ ...facts, readiness: { auth: { ...auth, tip: "another-tip" } } })).toThrow(/auth: the facts asked another input .*"tip":"another-tip"/)
    // a key the app has no input of (no such branch) is refused too
    expect(() => seedReadiness({ ...facts, readiness: { nobody: auth } })).toThrow(/nobody: the facts asked another input .* than the app asks now \(null\)/)
  })

  for (const [group, title] of Object.entries(GROUPS)) {
    it(title, async () => {
      const ofGroup = cases.filter(scenario => scenario.group === group)
      expect(ofGroup.length, group).toBeGreaterThan(0)
      const failures: string[] = []
      for (const scenario of ofGroup.filter(isExercised)) {
        failures.push(...(await appFailures(scenario)))
      }
      expect(failures, `${failures.length} of ${ofGroup.length} cases of ${group} answer otherwise`).toEqual([])
    })
  }
})
