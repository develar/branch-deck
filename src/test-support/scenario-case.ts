// A case of the scenario table (`crates/domain-policy/cases/*.json`, its README): the TS twin of `Given` and the
// expectations in `crates/bd-remote/src/scenario_test.rs`. Strict: a key outside the closed vocabulary fails the case
import { z } from "zod"

const robotState = z.enum(["running", "failure", "canceled", "success"])

/** `tip`, `older`, `stack`, `ancestor:<branch>`, `child:<branch>` */
const robotOn = z.union([z.enum(["tip", "older", "stack"]), z.string().regex(/^(ancestor|child):[\w./-]+$/)])

export const given = z.strictObject({
  mode: z.enum(["safePush", "dryRun"]).default("safePush"),
  via: z.enum(["click", "key"]).default("click"),
  subject: z.enum(["branch", "wholeStack"]).default("branch"),
  commits: z.array(z.string().regex(/^\([\w./-]+\) .+/)).default(["(auth) Add auth"]),
  target: z.enum(["protected", "unprotected"]).default("protected"),
  remote: z.enum(["none", "upToDate", "behind", "unpushed", "rebuiltOnNewerBaseline"]).default("none"),
  robot: z.strictObject({ state: robotState, on: robotOn }).optional(),
  pullRequest: z
    .strictObject({
      base: z.string().default("master"),
      draft: z.boolean().default(false),
      approvers: z.array(z.string()).default([]),
      reviewers: z.array(z.string()).default([]),
      state: z.enum(["open", "closed", "merged"]).default("open"),
    })
    .optional(),
  readiness: z.enum(["notRequired", "approved", "missing", "unknown", "unavailable", "unlisted", "space"]).default("notRequired"),
  profile: z
    .strictObject({
      chosen: z.string().optional(),
      lastRobot: z.literal("lsc").optional(),
      skipsReview: z.boolean().default(false),
      newApi: z.boolean().default(false),
    })
    .optional(),
  flags: z.array(z.enum(["withoutApprovals", "cancelRunning", "createPullRequest", "allowMissing", "allowDangling"])).default([]),
  rules: z.enum(["keep", "dismiss", "unknown", "loading"]).optional(),
  parked: z.literal(true).optional(),
  stackBranch: z.enum(["built", "conflict", "notBuilt"]).optional(),
  landed: z.literal(true).optional(),
  landingNeeds: z.literal(true).optional(),
})

export type Given = z.infer<typeof given>

const appExpectation = z.strictObject({
  step: z
    .strictObject({
      kind: z.string().optional(),
      safePushBlocker: z.string().nullable().optional(),
      dryRunBlocker: z.string().nullable().optional(),
    })
    .optional(),
  decision: z.strictObject({
    kind: z.enum(["wait", "refuse", "choose", "confirm", "start"]),
    reason: z.string().optional(),
    reasons: z.array(z.string()).optional(),
    what: z.string().optional(),
    offersPush: z.boolean().optional(),
  }),
})

const bdCheck = z.union([
  z.strictObject({ error: z.string() }),
  z.strictObject({
    refusals: z.array(z.string()).optional(),
    push_needed: z.boolean().optional(),
    profile: z.string().nullable().optional(),
    pull_request: z.number().nullable().optional(),
    readiness: z.string().nullable().optional(),
    lands: z.array(z.string()).optional(),
    retargeted: z.array(z.number()).optional(),
  }),
])

const bdStart = z.union([z.strictObject({ started_via: z.enum(["api", "pull_request"]) }), z.strictObject({ error: z.string() })])

export const scenarioCase = z.strictObject({
  id: z.string().regex(/^[a-z0-9]+(-[a-z0-9]+)*$/),
  group: z.string(),
  title: z.string(),
  spec: z.array(z.string()).min(1),
  discrepancy: z
    .string()
    .regex(/^D([1-9]|10)[ab]?$/)
    .nullable(),
  asymmetry: z.strictObject({ kind: z.enum(["byDesign", "unregistered"]), why: z.string() }).optional(),
  given,
  app: appExpectation,
  bd: z.strictObject({ check: bdCheck, start: bdStart.optional() }),
})

export type ScenarioCase = z.infer<typeof scenarioCase>

/** Whether a side starts, refuses or waits: the class the README compares the sides by */
export type OutcomeClass = "start" | "refuse" | "wait"

export function appOutcome(scenario: ScenarioCase): OutcomeClass {
  switch (scenario.app.decision.kind) {
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

export function bdOutcome(scenario: ScenarioCase): OutcomeClass {
  const { start, check } = scenario.bd
  if (start !== undefined) {
    return "error" in start ? "refuse" : "start"
  }
  return "error" in check || (check.refusals?.length ?? 0) > 0 ? "refuse" : "start"
}
