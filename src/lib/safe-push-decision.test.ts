import { describe, expect, it } from "vite-plus/test"
import type { PatronusProfile, PatronusStartOptions } from "@/lib/bindings"
import type { SafePushFacts } from "@/lib/safe-push-decision"
import { getSafePushDecision } from "@/lib/safe-push-decision"

const OPTIONS: PatronusStartOptions = { targetBranch: "master", supported: true, profiles: [], emergencyProfiles: [], defaultProfileId: null }
const LSC: PatronusProfile = { id: "lsc", name: "Large Scale Change", description: null, allowedPushModes: [], skipsReview: null }

/** A lone branch on GitHub, approved, nothing to push: a click starts at once */
function facts(patch: Partial<SafePushFacts> = {}): SafePushFacts {
  return {
    branchName: "auth",
    subject: "auth",
    mode: "rebaseAutosquash",
    via: "click",
    step: { kind: "safePush", blocker: null },
    target: "master",
    options: { status: "loaded", options: OPTIONS },
    profile: null,
    bypassReview: false,
    fromPullRequest: { number: 7, blockedByDraft: false, command: "/patronus merge" },
    startPush: { kind: "none" },
    hasTip: true,
    replaces: null,
    pushHeld: null,
    review: { kind: "loaded", readiness: { kind: "approved", groups: [] } },
    dismiss: null,
    ancestorCount: 0,
    wholeStackPullRequest: null,
    ...patch,
  }
}

describe("getSafePushDecision", () => {
  it("starts a lone branch at once when there is nothing to say: default checks, approved or not needed", () => {
    expect(getSafePushDecision(facts())).toEqual({ kind: "start" })
    expect(getSafePushDecision(facts({ review: { kind: "loaded", readiness: { kind: "notRequired" } } }))).toEqual({ kind: "start" })
    // a dry run is never checked
    expect(getSafePushDecision(facts({ mode: "dryRun", review: { kind: "none" } }))).toEqual({ kind: "start" })
  })

  it("opens the popover for a key, a stack, a profile, a running Safe Push it cancels, a review Patronus judges only at the start, a piece of a stack whose pull request is open", () => {
    expect(getSafePushDecision(facts({ via: "key" }))).toEqual({ kind: "confirm", reasons: ["key"] })
    expect(getSafePushDecision(facts({ ancestorCount: 2 }))).toEqual({ kind: "confirm", reasons: ["stack"] })
    expect(getSafePushDecision(facts({ profile: LSC }))).toEqual({ kind: "confirm", reasons: ["profile"] })
    expect(getSafePushDecision(facts({ replaces: "1a2b3c4" }))).toEqual({ kind: "confirm", reasons: ["replaces"] })
    expect(getSafePushDecision(facts({ review: { kind: "loaded", readiness: { kind: "unknown", reason: "approvalsNotEvaluated" } } }))).toEqual({
      kind: "confirm",
      reasons: ["review"],
    })
    expect(getSafePushDecision(facts({ review: { kind: "unlisted" } }))).toEqual({ kind: "confirm", reasons: ["review"] })
    expect(getSafePushDecision(facts({ review: { kind: "space", required: true } }))).toEqual({ kind: "confirm", reasons: ["review"] })
    expect(getSafePushDecision(facts({ review: { kind: "space", required: false } }))).toEqual({ kind: "start" })
    expect(getSafePushDecision(facts({ via: "key", ancestorCount: 1, replaces: "1a2b3c4" }))).toEqual({ kind: "confirm", reasons: ["key", "stack", "replaces"] })
    // a piece of a stack whose pull request is open: its approvals count for the whole stack only, said first
    expect(getSafePushDecision(facts({ wholeStackPullRequest: 12 }))).toEqual({ kind: "confirm", reasons: ["wholeStack"] })
  })

  it("refuses with the reason of the next step first, the reason every surface says", () => {
    expect(getSafePushDecision(facts({ step: { kind: "none", blocker: "A Safe Push of the last commit is running" } }))).toEqual({
      kind: "refuse",
      reason: "A Safe Push of the last commit is running",
      offersPush: false,
    })
  })

  it("waits for the profiles, the remote status, the review and the rules, each saying what it waits for", () => {
    expect(getSafePushDecision(facts({ options: { status: "loading" } }))).toEqual({ kind: "wait", what: "Asking Patronus for the merge profiles…" })
    expect(getSafePushDecision(facts({ startPush: { kind: "unknown" } }))).toEqual({ kind: "wait", what: "Checking auth…" })
    expect(getSafePushDecision(facts({ review: { kind: "loading" } }))).toEqual({ kind: "wait", what: "Checking the code-owner review…" })
    expect(getSafePushDecision(facts({ startPush: { kind: "branch", action: "push" }, dismiss: { pullRequest: 7, dismisses: null } }))).toEqual({
      kind: "wait",
      what: "Checking whether the push keeps the approvals of #7…",
    })
  })

  it("refuses what Patronus would refuse or fail: an unprotected target, a draft, missing approvals, a push that dismisses the counted ones", () => {
    expect(getSafePushDecision(facts({ options: { status: "loaded", options: { ...OPTIONS, supported: false } } }))).toMatchObject({
      kind: "refuse",
      reason: "Patronus does not push into master",
    })
    expect(getSafePushDecision(facts({ fromPullRequest: { number: 7, blockedByDraft: true, command: "/patronus merge" } }))).toMatchObject({
      kind: "refuse",
      reason: "#7 is a draft: Patronus merges it once it is ready for review",
    })
    expect(getSafePushDecision(facts({ review: { kind: "loaded", readiness: { kind: "missing", groups: [], notCounted: [], reason: "approvalsMissing" } } }))).toMatchObject({
      kind: "refuse",
      offersPush: false,
    })
    expect(getSafePushDecision(facts({ startPush: { kind: "branch", action: "push" }, dismiss: { pullRequest: 7, dismisses: true } }))).toEqual({
      kind: "refuse",
      reason: "The push dismisses the approvals of #7: push first, then ask for the review again",
      offersPush: true,
    })
  })

  it("tells a push that dismisses approvals Patronus doesn't count, and a dry run that pushes into a held branch can't", () => {
    const notCounted = { review: { kind: "loaded", readiness: { kind: "unknown", reason: "approvalsNotEvaluated" } } } as const
    expect(getSafePushDecision(facts({ ...notCounted, startPush: { kind: "branch", action: "push" }, dismiss: { pullRequest: 7, dismisses: true } }))).toEqual({
      kind: "confirm",
      reasons: ["review", "dismisses"],
    })
    expect(getSafePushDecision(facts({ mode: "dryRun", review: { kind: "none" }, pushHeld: "A Safe Push of auth is running" }))).toMatchObject({ kind: "refuse" })
  })

  it("lets a profile that skips the review start without the approvals, and a start without review wait for its profile", () => {
    const missing: Partial<SafePushFacts> = { review: { kind: "loaded", readiness: { kind: "missing", groups: [], notCounted: [], reason: "noPullRequestContext" } } }
    const awaiting: Partial<SafePushFacts> = { step: { kind: "createPullRequest", blocker: "Code owners must approve in a pull request: IJPL" } }
    expect(getSafePushDecision(facts({ ...missing, ...awaiting, fromPullRequest: null, profile: { ...LSC, skipsReview: true } }))).toEqual({
      kind: "confirm",
      reasons: ["profile"],
    })
    expect(getSafePushDecision(facts({ ...missing, ...awaiting, fromPullRequest: null, bypassReview: true }))).toEqual({
      kind: "choose",
      reason: "Choose the profile that skips the code-owner review",
    })
    expect(getSafePushDecision(facts({ ...missing, ...awaiting, fromPullRequest: null, bypassReview: true, profile: LSC }))).toEqual({
      kind: "confirm",
      reasons: ["profile", "bypass"],
    })
    // a reason that isn't the approvals still refuses
    expect(getSafePushDecision(facts({ ...missing, step: { kind: "none", blocker: "Wait for the sync to finish" }, bypassReview: true, profile: LSC }))).toMatchObject({
      kind: "refuse",
    })
  })
})
