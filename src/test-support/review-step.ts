import type { ReviewPlan, ReviewStep, StepCommit } from "@/lib/bindings"

/** A commit of a step by its title, which is its id too */
export function stepCommit(title: string, role: StepCommit["role"] = "starts"): StepCommit {
  return { id: title, title, role }
}

/** A step of the review plan: its commits, its stops by unit and path, the tests that reach its code, the rest of it */
export function reviewStep(id: string, commits: StepCommit[], stops: [string, string][], tests: Partial<ReviewStep["tests"]> = {}, more: Partial<ReviewStep> = {}): ReviewStep {
  return {
    id,
    commits,
    concern: null,
    stops: stops.map(([unit, path]) => ({ unit, path, blocks: null, uses: null, also: [], concern: null, joined: [], marks: [] })),
    paths: [...new Set(stops.map(([, path]) => path))],
    elsewhere: [],
    tests: { answered: true, proof: [], reached: 0, unreached: [], scenarios: [], ...tests },
    proves: [],
    newApi: [],
    moduleLinks: [],
    title: null,
    titleSource: "evidence",
    takenOutBy: null,
    shownElsewhere: [],
    ...more,
  }
}

/** A review plan of no files and no steps, the story told by its commits, with `more` of it */
export function reviewPlan(more: Partial<ReviewPlan> = {}): ReviewPlan {
  return {
    branch: "feature/layout",
    title: "feature/layout",
    base: "1111111111",
    head: "2222222222",
    interdiff: null,
    api: [],
    files: [],
    links: [],
    dependencies: [],
    repeatedEdits: [],
    units: [],
    concerns: [],
    steps: { source: { kind: "commits" }, told: true, steps: [], coverage: { blocks: 0, inStops: 0, mechanical: 0, missing: 0, twice: 0 }, commits: [] },
    brief: { issue: null, contract: [] },
    order: [],
    indexNote: null,
    ...more,
  }
}
