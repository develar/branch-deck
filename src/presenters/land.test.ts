// @spec spec/branch/patronus.spec.md
import { describe, expect, it } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type { LandDecision, LandingRefusal, Reason } from "@/lib/bindings"
import type { LandAsk } from "@/presenters/land"
import { presentLand } from "@/presenters/land"

/** A Safe Push of auth through the API with the default checks: nothing in its way */
function decision(patch: Partial<LandDecision> = {}): LandDecision {
  return {
    blockers: [],
    confirmations: [],
    waits: [],
    effects: [
      { kind: "push", branches: ["auth"], force: false },
      { kind: "startRobot", route: { kind: "api", profile: null } },
    ],
    lands: ["auth"],
    pullRequest: null,
    approvalsSkippedBy: null,
    ...patch,
  }
}

const CLICK: LandAsk = { subject: "auth", via: "click", stepBlocker: null, withoutReview: false }

const MISSING: LandingRefusal = {
  kind: "missingDeclaration",
  branch: "lane-run",
  identifier: "Stills",
  file: "lane/src/BuildLog.kt",
  line: 3,
  usedBy: { id: "c1", subject: "Show each scenario", branch: "lane-run" },
  declaredBy: { id: "c2", subject: "Name lossy stills", branch: "stills-codec" },
  fix: { kind: "restack", branch: "lane-run", onto: "stills-codec" },
  landingIn: null,
}

describe("presentLand", () => {
  it("starts at once when Rust decided there is nothing to say, and waits for its decision before it", () => {
    expect(presentLand(decision(), CLICK)).toEqual({ kind: "start" })
    expect(presentLand(null, CLICK)).toEqual({ kind: "wait", what: "Reading auth…" })
  })

  it("opens the popover for a key and for each confirmation of the decision: a stack, a profile, a running Safe Push it cancels, a review Patronus judges only at the start, a piece of a stack whose pull request is open", () => {
    expect(presentLand(decision(), { ...CLICK, via: "key" })).toEqual({ kind: "confirm", reasons: ["key"] })
    const confirmed = decision({
      confirmations: [
        { kind: "landsMoreThanRow", count: 2 },
        { kind: "nonDefaultProfile", id: "lsc" },
        { kind: "cancelsRobot", robotId: "older" },
        { kind: "approvalsJudgedAtStart" },
        { kind: "reviewUnknown", message: "Patronus answered 500" },
        { kind: "dismissesApprovals", number: 7 },
        { kind: "rulesUnknown", number: 7 },
        { kind: "stackPullRequestApprovalsDontCount", number: 12 },
      ],
    })
    expect(presentLand(confirmed, CLICK)).toEqual({ kind: "confirm", reasons: ["stack", "profile", "replaces", "review", "dismisses", "rulesUnknown", "wholeStack"] })
  })

  it("opens the popover when the build of what it lands needs a branch outside it: the reason, the fix and Start Anyway before the start", () => {
    const needs = decision({ blockers: [{ code: "landingRefused", refusal: MISSING }] })
    expect(presentLand(needs, CLICK)).toEqual({ kind: "confirm", reasons: ["landing"] })
  })

  it("refuses with the reason of the next step first, the reason every surface says", () => {
    expect(presentLand(decision(), { ...CLICK, stepBlocker: "Wait for the sync to finish" })).toEqual({ kind: "refuse", reason: "Wait for the sync to finish", offersPush: false })
  })

  it("waits for the facts Rust waits for, each saying what it waits for, and refuses with why a read failed", () => {
    const waiting = (fact: LandDecision["waits"][number]["fact"]) => presentLand(decision({ waits: [{ fact, failed: null }] }), CLICK)
    expect(waiting({ kind: "remote" })).toEqual({ kind: "wait", what: "Checking auth…" })
    expect(waiting({ kind: "profiles" })).toEqual({ kind: "wait", what: "Asking Patronus for the merge profiles…" })
    expect(waiting({ kind: "pullRequests" })).toEqual({ kind: "wait", what: "Checking the code-owner review…" })
    expect(waiting({ kind: "readiness" })).toEqual({ kind: "wait", what: "Checking the code-owner review…" })
    expect(waiting({ kind: "rules", number: 7 })).toEqual({ kind: "wait", what: "Checking whether the push keeps the approvals of #7…" })
    expect(waiting({ kind: "landingNeeds" })).toEqual({ kind: "wait", what: "Checking what auth needs to land…" })
    // the subject first, then the profiles
    expect(
      presentLand(
        decision({
          waits: [
            { fact: { kind: "profiles" }, failed: null },
            { fact: { kind: "remote" }, failed: null },
          ],
        }),
        CLICK,
      ),
    ).toEqual({
      kind: "wait",
      what: "Checking auth…",
    })
    const failed = decision({ waits: [{ fact: { kind: "profiles" }, failed: { ...internalError("Patronus answered 502"), code: "patronus_error" } }] })
    expect(presentLand(failed, CLICK)).toEqual({ kind: "refuse", reason: "Patronus answered 502", offersPush: false })
  })

  it("refuses with the words of the first blocker, offering the push or the object it names: an unprotected target, a draft, missing approvals, a push that dismisses the counted ones", () => {
    const refused = (reason: Reason) => presentLand(decision({ blockers: [reason], waits: [{ fact: { kind: "readiness" }, failed: null }] }), CLICK)
    expect(refused({ code: "unprotected", target: "master" })).toEqual({ kind: "refuse", reason: "Patronus does not push into master", offersPush: false })
    expect(refused({ code: "draft", number: 7 })).toEqual({
      kind: "refuse",
      reason: "#7 is a draft: Patronus merges it once it is ready for review",
      offersPush: false,
      opens: "pullRequest",
    })
    expect(refused({ code: "approvalsMissing", groups: ["Air"] })).toEqual({
      kind: "refuse",
      reason: "Patronus checks the approvals when the robot starts and fails without them",
      offersPush: false,
      opens: "review",
    })
    expect(refused({ code: "dismissesApprovals", number: 7, target: "master" })).toEqual({
      kind: "refuse",
      reason: "The push dismisses the approvals of #7: push first, then ask for the review again",
      offersPush: true,
    })
    // a refusal of the landing gate beside another blocker: the other refuses
    expect(
      presentLand(
        decision({
          blockers: [
            { code: "landingRefused", refusal: MISSING },
            { code: "unprotected", target: "master" },
          ],
        }),
        CLICK,
      ),
    ).toMatchObject({ kind: "refuse" })
  })

  it("lets a start without review wait for its profile, and confirm the one that skips the review", () => {
    const without = { ...CLICK, withoutReview: true }
    const route = (profile: Extract<LandDecision["effects"][number], { kind: "startRobot" }>["route"]) =>
      decision({ confirmations: [{ kind: "withoutApprovals" }], effects: [{ kind: "startRobot", route: profile }], approvalsSkippedBy: "withoutApprovals" })
    expect(presentLand(route({ kind: "api", profile: null }), without)).toEqual({ kind: "choose", reason: "Choose the profile that skips the code-owner review" })
    const chosen = route({ kind: "api", profile: { id: "lsc", name: "LSC", description: null, allowedPushModes: [], skipsReview: true } })
    expect(presentLand(chosen, without)).toEqual({ kind: "confirm", reasons: ["bypass"] })
    // the approvals not known yet (GitHub doesn't list the pull requests): it is still the start without review
    expect(presentLand(decision({ confirmations: [{ kind: "approvalsJudgedAtStart" }] }), without)).toEqual({
      kind: "choose",
      reason: "Choose the profile that skips the code-owner review",
    })
  })
})
