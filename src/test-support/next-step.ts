// Next steps as Rust gives them (`NextStep` of the view's `nextSteps`) and the context the page presents them in, for
// the tests of the presenters: a step is data, so a test writes the one row it asks about
import type { MissingReviewGroup, NextStep, NextStepView, StepFact, StepKind, StepRow } from "@/lib/bindings"
import type { NextStepActions, StepContext } from "@/presenters/next-step"
import { branchSyncStore } from "@/stores/branch-sync"

/** The row of the table each kind is decided in, as Rust names it */
const ROWS: Readonly<Record<StepKind["kind"], StepRow>> = {
  hidden: "noSlot",
  settled: "1",
  pending: "0",
  unavailable: "0",
  needsBranch: "1′",
  restart: "2",
  robotPullRequest: "3",
  retarget: "5",
  otherBase: "5′",
  draft: "6",
  createPullRequest: "7",
  changesRequested: "8",
  requestReview: "9",
  awaiting: "10",
  changesRequestedUnjudged: "8′",
  awaitingUnjudged: "10′",
  safePush: "11",
}

/** The step of `kind`, nothing against its starts, of a lone branch, `extra` over it */
export function testStep(kind: StepKind, extra: Partial<NextStep> = {}): NextStep {
  return {
    row: ROWS[kind.kind],
    kind,
    safePush: null,
    dryRun: null,
    needsReadiness: false,
    staleReadiness: false,
    landed: null,
    retarget: null,
    withoutReview: null,
    lands: 1,
    unlisted: null,
    ...extra,
  }
}

/** The actions of a context: each records its name in `calls` */
export function testActions(calls: string[] = [], extra: Partial<NextStepActions> = {}): NextStepActions {
  const record = (name: string) => () => {
    calls.push(name)
  }
  return {
    openSafePush: record("openSafePush"),
    openPullRequest: record("openPullRequest"),
    retarget: record("retarget"),
    openStackPullRequest: root => calls.push(`openStackPullRequest ${root}`),
    fixLanding: fix => calls.push(`fixLanding ${fix.kind}`),
    copyLoginCommand: record("copyLoginCommand"),
    refreshPatronus: record("refreshPatronus"),
    restart: null,
    createPullRequest: { id: "create-pull-request", label: "Create Pull Request…", onSelect: record("createPullRequest") },
    requestReview: null,
    ...extra,
  }
}

/** The context of the branch auth, whose robots push into master, nothing running on its row */
export function testContext(extra: Partial<StepContext> = {}): StepContext {
  return { subject: "auth", target: "master", startDescription: null, busy: null, forStart: false, actions: testActions(), ...extra }
}

/** A missing group of code owners: `requested` asked, `needed` approvals still to come */
export function missingGroup(name: string, requested: readonly string[] = [], extra: Partial<MissingReviewGroup> = {}): MissingReviewGroup {
  return {
    name,
    needed: 1,
    approvedBy: [],
    requested: requested.map(fullName => ({ login: fullName.toLowerCase().replace(" ", "."), fullName })),
    suggested: [],
    changesRequestedBy: [],
    ...extra,
  }
}

/** The part of the view of a subject with `step`, its node ready to land */
export function testView(step: NextStep): NextStepView {
  return { step, node: { kind: "ready", label: { kind: "ready", lands: step.lands }, pending: false, robot: false, shows: null, moveShows: true } }
}

/** The next steps of the view, by subject, as the session sends them (`RepositoryView.nextSteps`) */
export function setNextSteps(steps: Readonly<Record<string, NextStep>>): void {
  branchSyncStore.setState({ nextSteps: Object.fromEntries(Object.entries(steps).map(([subject, step]) => [subject, testView(step)])) })
}

/** "Checking…" of row 0, waiting for `fact`, every Safe Push saying it */
export function pendingStep(fact: StepFact): NextStep {
  return testStep({ kind: "pending", fact }, { safePush: { code: "step" }, ...(fact === "patronus" && { dryRun: { code: "step" } }) })
}
