// @spec spec/branch/patronus.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ApiError, EffectRecord, LandResult, Outcome, PatronusRobot, Plan } from "@/lib/bindings"
import { presentLandOutcome, presentLandRefusal } from "@/presenters/outcome"
import { MISSING_STILLS } from "@/test-support/landing-refusal"

/** A Safe Push of auth started now */
const ROBOT: PatronusRobot = {
  id: "started-1",
  name: "Safe Push of auth",
  status: "running",
  pushMode: "rebaseAutosquash",
  branchName: "auth",
  stackRoot: null,
  sourceRevision: "a1",
  targetBranch: "master",
  startTime: 1704189600,
  finishTime: null,
  cancellationReason: null,
  url: "https://patronus/robot/started-1",
  pullRequest: null,
  profile: null,
}

/** The plan of a Safe Push of auth that cancels `older`, pushes auth and starts with the profile Large Scale Change */
const PLAN: Plan = {
  intent: { kind: "land", subject: { kind: "branch", name: "auth" }, mode: "rebaseAutosquash", profile: { kind: "default" } },
  fingerprint: "f1",
  lands: ["auth"],
  effects: [
    { kind: "cancelRobot", robotId: "older" },
    { kind: "push", branches: ["auth"], force: false },
    {
      kind: "startRobot",
      route: { kind: "api", profile: { id: "lsc", name: "Large Scale Change", description: null, allowedPushModes: [], skipsReview: null } },
    },
  ],
  confirmations: [],
  blockers: [],
  waits: [],
  alternatives: null,
  pullRequest: null,
  approvalsSkippedBy: null,
}

function apiError(code: ApiError["code"], message: string, details: ApiError["details"] = null): ApiError {
  return { code, message, details, retry: "no", operation_id: null }
}

/** The outcome of `PLAN` with the statuses of its three effects and what they answered */
function outcome(statuses: EffectRecord["status"][], result: Partial<LandResult> = {}): Outcome {
  return {
    op: "op-1",
    effects: PLAN.effects.map((effect, index) => ({ effect, status: statuses[index] ?? { kind: "skipped", reason: null } })),
    newsKey: { kind: "robotSubject", subject: "auth", mode: "rebaseAutosquash" },
    versionAfter: null,
    result: { kind: "land", robot: null, canceled: null, comment: null, notes: [], lost: null, ...result },
    stats: { gitProcesses: {}, httpRequests: 0, millis: 1 },
  }
}

const DONE = { kind: "done" } as const

describe("presentLandOutcome", () => {
  it("says the robot it started with the profile of its plan, and the checks that ran on the last fetched target", () => {
    const notes: LandResult["notes"] = [{ kind: "targetFetchFailed", target: "origin/master", error: "Could not resolve host" }]
    expect(presentLandOutcome(outcome([DONE, DONE, DONE], { robot: ROBOT, notes }), PLAN.intent, "auth")).toEqual({
      kind: "started",
      title: "Safe Push started",
      description: "auth → master · profile Large Scale Change. The fetch of origin/master failed, so the check used the last fetched origin/master: Could not resolve host",
      robot: ROBOT,
    })
  })

  it("says a comment Patronus accepted whose robot it lists not yet, and a comment it never answered as a start not known yet that opens it", () => {
    const comment = { number: 7, url: "https://github.com/acme/monorepo/pull/7#issuecomment-1", accepted: true }
    expect(presentLandOutcome(outcome([DONE, DONE, DONE], { comment }), PLAN.intent, "auth")).toEqual({
      kind: "commented",
      title: "Safe Push started",
      description: "Patronus accepted the comment on #7: the robot shows once Patronus lists it",
      number: 7,
      url: comment.url,
    })
    const silent = presentLandOutcome(outcome([DONE, DONE, { kind: "unknown" }], { comment: { ...comment, accepted: false } }), PLAN.intent, "auth")
    expect(silent).toEqual({
      kind: "unknown",
      title: "Cannot tell whether the Safe Push of auth started",
      description: "Patronus did not answer the comment on #7 within a minute. It holds auth until Patronus lists its robot, or until Forget This Start on the row",
      url: comment.url,
    })
  })

  it("says a start whose answer was lost as not known yet, with why", () => {
    const lost = apiError("patronus_error", "The connection to Patronus broke off before its answer")
    expect(presentLandOutcome(outcome([DONE, DONE, { kind: "unknown" }], { lost }), PLAN.intent, "auth")).toEqual({
      kind: "unknown",
      title: "Cannot tell whether the Safe Push of auth started",
      description: "The connection to Patronus broke off before its answer. It holds auth until Patronus lists its robot, or until Forget This Start on the row",
      url: null,
    })
  })

  it("offers the start again when the facts changed after the push, naming the robot it canceled; a failed effect says its error", () => {
    const changed = apiError("plan_changed", "auth changed after the push: no robot started")
    const older = { ...ROBOT, id: "older", status: "canceling" as const }
    expect(presentLandOutcome(outcome([DONE, DONE, { kind: "skipped", reason: changed }], { canceled: older }), PLAN.intent, "auth")).toEqual({
      kind: "failed",
      title: "Cannot start the Safe Push of auth",
      description: "auth changed after the push: no robot started; the Safe Push it replaced was canceled and the push stays",
      error: changed,
      again: true,
      canceled: older,
    })
    const refused = apiError("push_refused", "The remote refused the push: pre-receive hook declined")
    expect(presentLandOutcome(outcome([DONE, { kind: "failed", error: refused }]), PLAN.intent, "auth")).toMatchObject({
      kind: "failed",
      description: "The remote refused the push: pre-receive hook declined",
      error: refused,
      again: false,
    })
    const stopped = presentLandOutcome(outcome([DONE, DONE, { kind: "skipped", reason: apiError("canceled", "stopped") }]), PLAN.intent, "auth")
    expect(stopped).toEqual({ kind: "stopped", title: "Canceled the Safe Push of auth before its robot started", description: "The push stays" })
  })
})

describe("presentLandRefusal", () => {
  const MISSING = MISSING_STILLS

  it("offers the fix and Start Anyway of the landing gate, Update Project of a conflict with the target, the start again of a changed plan", () => {
    const landing = apiError("missing_declaration", "Nothing was pushed or started: …", { type: "landing", refusals: [MISSING] })
    expect(presentLandRefusal(landing, "rebaseAutosquash", "the whole lane-run stack")).toEqual({
      kind: "landing",
      title: "Cannot start the Safe Push of the whole lane-run stack",
      reason:
        "“Show each scenario” of lane-run uses Stills (lane/src/BuildLog.kt:3), which “Name lossy stills” of stills-codec adds: git merges it, but the build of the whole lane-run stack fails. Stack lane-run on stills-codec to land it too.",
      fix: MISSING.fix,
      anyway: { allowDangling: false, allowMissing: true },
    })
    const conflict = apiError("target_conflict", "Nothing was pushed or started: auth conflicts with the newest target in a.txt")
    expect(presentLandRefusal(conflict, "dryRun", "auth")).toEqual({ kind: "target", title: "Cannot start the Dry Run of auth", error: conflict })
    const changed = apiError("plan_changed", "a.txt moved since the plan")
    expect(presentLandRefusal(changed, "rebaseAutosquash", "auth")).toMatchObject({ kind: "changed", error: changed })
    expect(presentLandRefusal(apiError("canceled", "stopped"), "rebaseAutosquash", "auth")).toEqual({
      kind: "stopped",
      title: "Canceled the Safe Push of auth before it started",
      description: "Nothing was pushed or started",
    })
    const parked = apiError("parked", "Nothing was pushed or started: auth is parked")
    expect(presentLandRefusal(parked, "rebaseAutosquash", "auth")).toEqual({ kind: "error", title: "Cannot start the Safe Push of auth", error: parked })
  })
})
