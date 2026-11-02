import type { ReviewStep, StepCommit } from "@/lib/bindings"

/** A commit of a step by its subject, which is its id too */
export function stepCommit(subject: string, role: StepCommit["role"] = "starts"): StepCommit {
  return { id: subject, subject, role }
}

/** A step of the review plan: its commits, its stops by unit and path, the tests that reach its code, the rest of it */
export function reviewStep(id: string, commits: StepCommit[], stops: [string, string][], tests: Partial<ReviewStep["tests"]> = {}, more: Partial<ReviewStep> = {}): ReviewStep {
  return {
    id,
    commits,
    concern: null,
    stops: stops.map(([unit, path]) => ({ unit, path, blocks: null, uses: null, also: [], concern: null })),
    paths: [...new Set(stops.map(([, path]) => path))],
    elsewhere: [],
    tests: { answered: true, reached: [], unreached: [], scenarios: [], ...tests },
    newApi: [],
    moduleLinks: [],
    takenOutBy: null,
    ...more,
  }
}
