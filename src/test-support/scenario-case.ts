// A case of the scenario table (`crates/domain-policy/cases/*.json`, its README): the TS twin of `Given` and the
// expectations in `crates/bd-remote/src/scenario_test.rs`. Strict: a key outside the closed vocabulary fails the case
import { z } from "zod"

const robotState = z.enum(["running", "failure", "canceled", "success"])

/** `tip`, `older`, `remote` (the remote branch, rebuilt since on a newer baseline), `stack`, `ancestor:<branch>`, `child:<branch>` */
const robotOn = z.union([z.enum(["tip", "older", "remote", "stack"]), z.string().regex(/^(ancestor|child):[\w./-]+$/)])

/** A robot of the branch, of a branch around it or of the whole stack; `reason: approvals` failed for the approvals */
const robot = z.strictObject({
  state: robotState,
  on: robotOn,
  reason: z.literal("approvals").optional(),
  /** Started by the comment on the pull request #7 */
  fromPullRequest: z.boolean().default(false),
})

const branchName = z.string().regex(/^[\w.-]+$/)

export const given = z.strictObject({
  mode: z.enum(["safePush", "dryRun"]).default("safePush"),
  /** The branch whose step and start the case asks */
  branch: branchName.default("auth"),
  via: z.enum(["click", "key"]).default("click"),
  subject: z.enum(["branch", "wholeStack"]).default("branch"),
  commits: z.array(z.string().regex(/^\([\w./-]+\) .+/)).default(["(auth) Add auth"]),
  target: z.enum(["protected", "unprotected"]).default("protected"),
  remote: z.enum(["none", "upToDate", "behind", "unpushed", "rebuiltOnNewerBaseline"]).default("none"),
  robot: z.union([robot, z.array(robot).min(2)]).optional(),
  /** The app's sync runs; `bd` runs its own before it plans */
  sync: z.literal("running").optional(),
  /** The first commit of this branch changes a file that only an unassigned commit creates: it fails to sync */
  conflicts: branchName.optional(),
  /** `origin` is not on Space */
  notOnSpace: z.literal(true).optional(),
  /** The open pull request #12 of the stack branch of the root */
  stackPullRequest: z.literal(true).optional(),
  pullRequest: z
    .strictObject({
      /** The branch of the pull request: the case's branch by default */
      of: branchName.optional(),
      base: z.string().default("master"),
      draft: z.boolean().default(false),
      approvers: z.array(z.string()).default([]),
      reviewers: z.array(z.string()).default([]),
      changesRequested: z.array(z.string()).default([]),
      state: z.enum(["open", "closed", "merged"]).default("open"),
    })
    .optional(),
  readiness: z.enum(["notRequired", "approved", "missing", "unknown", "unavailable", "unlisted", "space", "notAsked"]).default("notRequired"),
  profile: z
    .strictObject({
      chosen: z.string().optional(),
      lastRobot: z.literal("lsc").optional(),
      skipsReview: z.boolean().default(false),
      newApi: z.boolean().default(false),
    })
    .optional(),
  flags: z.array(z.enum(["withoutApprovals", "cancelRunning", "createPullRequest", "allowMissing", "allowDangling", "allowUnknownRules"])).default([]),
  /** What the GitHub rules of the target answer: `unknown` is a failed read, `loading` no answer yet (bd waits for its read: `keep`) */
  rules: z.enum(["keep", "dismiss", "unknown", "loading"]).default("keep"),
  /** `self`: the branch is parked; `builtOn`: its parent is; a branch of the stack: that one is */
  parked: z.union([z.enum(["self", "builtOn"]), branchName]).optional(),
  /** `notBuilt`: a member of the whole stack has a stack problem (a conflict is `conflicts` of a member) */
  stackBranch: z.literal("notBuilt").optional(),
  /** `inBaseline`: origin/master has every commit; `byContent`: the baseline landed the commits rebuilt, one added since */
  landed: z.enum(["inBaseline", "byContent"]).optional(),
  landingNeeds: z.literal(true).optional(),
  /** The commits of a fixture with a dangling reference instead of `commits`: `removed` of fake-agent, `changed` of suspend */
  dangling: z.enum(["removed", "changed"]).optional(),
})

export type Given = z.infer<typeof given>
export type GivenRobot = z.infer<typeof robot>

/** The robots of `given`, one or several */
export function robotsOf(given: Given): GivenRobot[] {
  return given.robot === undefined ? [] : Array.isArray(given.robot) ? given.robot : [given.robot]
}

const appExpectation = z.strictObject({
  step: z
    .strictObject({
      kind: z.string().optional(),
      safePushBlocker: z.string().nullable().optional(),
      dryRunBlocker: z.string().nullable().optional(),
    })
    .optional(),
  /** Where the row feeds a start; without it the class is the step's */
  decision: z
    .strictObject({
      kind: z.enum(["wait", "refuse", "choose", "confirm", "start"]),
      reason: z.string().optional(),
      reasons: z.array(z.string()).optional(),
      what: z.string().optional(),
      offersPush: z.boolean().optional(),
    })
    .optional(),
})

const bdCheck = z.union([
  z.strictObject({ error: z.string() }),
  z.strictObject({
    refusals: z.array(z.string()).optional(),
    push_needed: z.boolean().optional(),
    profile: z.string().nullable().optional(),
    pull_request: z.number().nullable().optional(),
    readiness: z.string().nullable().optional(),
    /** `rules.dismiss_stale_reviews_on_push`: null when GitHub didn't answer or bd didn't ask */
    rules: z.boolean().nullable().optional(),
    lands: z.array(z.string()).optional(),
    retargeted: z.array(z.number()).optional(),
  }),
])

const bdStart = z.union([
  /** `canceled`: it canceled the running Safe Push of an older commit first (`--cancel-running`) */
  z.strictObject({ started_via: z.enum(["api", "pull_request"]), canceled: z.boolean().optional() }),
  z.strictObject({ error: z.string() }),
])

const discrepancy = z.enum(["D1", "D2", "D3a", "D3b", "D4", "D5", "D6", "D7", "D8", "D9", "D10", "D11", "D12", "D13", "D14", "D15"])

/** Where each side does it today: `path#text` or `path:line#text` (a test, a function, a line), from the repository root */
const today = z.strictObject({
  app: z.array(z.string()).min(1),
  bd: z.array(z.string()).min(1),
  /** Why the sides differ although their classes agree (D9), or what the pointers show */
  why: z.string().optional(),
})

const header = {
  id: z.string().regex(/^[a-z0-9]+(-[a-z0-9]+)*$/),
  group: z.string(),
  title: z.string(),
  spec: z.array(z.string()).min(1),
  /**
   * Of an entry of the registry: the stage of the design doc that closed the discrepancy; the entry stays the key of its
   * id, `today` says where each side does it now, and no case is its twin any longer
   */
  closed: z.string().optional(),
}

/** Both sides answer `given` */
const exercisedCase = z.strictObject({
  ...header,
  kind: z.literal("exercised").default("exercised"),
  discrepancy: discrepancy.nullable(),
  /** Of an entry of the registry whose sides agree by class */
  today: today.optional(),
  /** Why the sides differ by design (a spec, a doc comment) without an id of the registry; every other difference carries one */
  asymmetry: z.strictObject({ kind: z.literal("byDesign"), why: z.string() }).optional(),
  given,
  app: appExpectation,
  bd: z.strictObject({ check: bdCheck, start: bdStart.optional() }),
})

/** An entry of the registry that no decision of either side reaches: `today` says where each side does it */
const recordedCase = z.strictObject({
  ...header,
  kind: z.literal("recorded"),
  discrepancy,
  /** The exercised cases of other groups that record the same discrepancy */
  twins: z.array(z.string()).default([]),
  today,
})

export const scenarioCase = z.union([exercisedCase, recordedCase])

export type ScenarioCase = z.infer<typeof scenarioCase>
export type ExercisedCase = z.infer<typeof exercisedCase>

export function isExercised(scenario: ScenarioCase): scenario is ExercisedCase {
  return scenario.kind === "exercised"
}

/** Whether a side starts, refuses or waits: the class the README compares the sides by */
export type OutcomeClass = "start" | "refuse" | "wait"

export function appOutcome(scenario: ExercisedCase): OutcomeClass {
  const { decision, step } = scenario.app
  if (decision === undefined) {
    // a step alone: nothing offered, or the reason every surface disables the start of the mode with
    const blocker = scenario.given.mode === "dryRun" ? step?.dryRunBlocker : step?.safePushBlocker
    if (blocker === undefined) {
      throw new Error(`${scenario.id}: app.step names the blocker of the mode when app.decision is not given`)
    }
    return step?.kind === "hidden" || blocker !== null ? "refuse" : "start"
  }
  switch (decision.kind) {
    case "refuse":
      return "refuse"
    case "wait":
      return "wait"
    case "choose":
    case "confirm":
    case "start":
      return "start"
  }
}

export function bdOutcome(scenario: ExercisedCase): OutcomeClass {
  const { start, check } = scenario.bd
  if (start !== undefined) {
    return "error" in start ? "refuse" : "start"
  }
  return "error" in check || (check.refusals?.length ?? 0) > 0 ? "refuse" : "start"
}
