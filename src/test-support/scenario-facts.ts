// The app's side of a scenario case (`crates/domain-policy/cases/README.md`): `given` as the facts the stores would
// hold (`NextStepFacts`, then `SafePushFacts` as `readSafePush` builds them), the profile chosen as `readSafePush`
// chooses it. Only what the README marks as mapped; anything else fails with "not mapped yet"
import type { PatronusProfile, PatronusPushMode, PatronusStartOptions, PullRequest, ReviewReadiness } from "@/lib/bindings"
import type { BranchNextStep, NextStepFacts, NextStepPullRequest } from "@/lib/branch-next-step"
import { getBranchNextStep } from "@/lib/branch-next-step"
import { allowsPushMode, getBypassProfiles, getPullRequestStart, getStartPush, isPullRequestIntoTarget } from "@/lib/patronus"
import { changesRequestedBy, waitingReviewers } from "@/lib/pull-request"
import type { ReviewFact, SafePushFacts } from "@/lib/safe-push-decision"
import { facts, finished, group, ilya, missing, restart } from "@/lib/test-next-step-facts"
import type { Given } from "@/test-support/scenario-case"

const BRANCH = "auth"
const TARGET = "master"
const VIRTUAL = "test-user/virtual/"
const GITHUB_REPOSITORY = "acme/monorepo"

function notMapped(what: string): never {
  throw new Error(`given.${what} is not mapped yet`)
}

/** The profiles the fake Patronus answers (`robot-start-supported`), `lsc` as the last robot's options describe it */
function startOptions(given: Given): PatronusStartOptions {
  const profile = given.profile
  const lastRobot = profile?.lastRobot ?? null
  const lsc: PatronusProfile = {
    id: "lsc",
    name: "Large Scale Change",
    description: "All the tests",
    allowedPushModes: ["rebaseAutosquash", "dryRun"],
    skipsReview: lastRobot === "lsc" && profile?.skipsReview === true ? true : null,
  }
  return {
    targetBranch: TARGET,
    supported: given.target === "protected",
    profiles: [{ id: "default", name: "Default", description: null, allowedPushModes: [], skipsReview: null }, lsc],
    emergencyProfiles: [{ id: "emergency", name: "Emergency", description: null, allowedPushModes: [], skipsReview: true }],
    defaultProfileId: lastRobot,
  }
}

/** The pull request #7 of auth as GitHub lists it */
function pullRequestOf(given: Given): PullRequest | undefined {
  const pullRequest = given.pullRequest
  if (pullRequest === undefined || given.readiness === "unlisted" || given.readiness === "space") {
    return undefined
  }
  return {
    number: 7,
    title: "Add auth",
    url: "https://github.com/acme/monorepo/pull/7",
    state: pullRequest.state,
    isDraft: pullRequest.draft,
    reviewDecision: null,
    headRefName: `${VIRTUAL}${BRANCH}`,
    baseRefName: pullRequest.base,
    headRefOid: "1111",
    checks: null,
    updatedAt: 0,
    reviewers: [
      ...pullRequest.approvers.map(login => ({ login, state: "approved" as const, requested: false })),
      ...pullRequest.reviewers.map(login => ({ login, state: "requested" as const, requested: true })),
    ],
    author: "me",
  }
}

function nextStepPullRequest(pullRequest: PullRequest): NextStepPullRequest {
  const base = pullRequest.baseRefName ?? ""
  return {
    number: pullRequest.number,
    state: pullRequest.state,
    isDraft: pullRequest.isDraft,
    base,
    intoTarget: isPullRequestIntoTarget(pullRequest, TARGET),
    mergedCurrent: false,
    baseIsVirtualBranch: base.length > VIRTUAL.length && base.startsWith(VIRTUAL),
    waitingFor: waitingReviewers(pullRequest),
    changesRequestedBy: changesRequestedBy(pullRequest),
    checksFailing: false,
    changesRequested: false,
  }
}

/** What Rust answers for the branch (`to_branch_readiness`), as the review-readiness store holds it */
function readinessOf(given: Given, pullRequest: PullRequest | undefined): ReviewReadiness {
  switch (given.readiness) {
    case "notRequired":
    case "unlisted":
      return { kind: "notRequired" }
    case "approved":
      return { kind: "approved", groups: [{ name: "IJPL Code Platform", approvedBy: [ilya] }] }
    case "missing": {
      const requested = pullRequest?.reviewers.some(reviewer => reviewer.requested && reviewer.login === ilya.login) === true ? [ilya] : []
      return missing([group({ requested })], { notCounted: [], reason: pullRequest === undefined ? "noPullRequestContext" : "approvalsMissing" })
    }
    case "space":
      return missing([group({ requested: [] })], { notCounted: [], reason: "approvalsMissing" })
    case "unknown":
      return { kind: "unknown", reason: "approvalsNotEvaluated" }
    case "unavailable":
      return { kind: "unavailable", reason: "error", message: "Patronus answered 500" }
  }
}

function checkMapped(given: Given): void {
  if (given.subject !== "branch") {
    notMapped("subject")
  }
  if (given.commits.length !== 1) {
    notMapped("commits (more than one branch)")
  }
  if (given.remote !== "none" && given.remote !== "upToDate") {
    notMapped(`remote ${given.remote}`)
  }
  if (given.robot !== undefined && (given.robot.state !== "failure" || given.robot.on !== "older")) {
    notMapped(`robot ${given.robot.state} on ${given.robot.on}`)
  }
  if (given.rules !== undefined) {
    notMapped("rules")
  }
  if (given.parked !== undefined) {
    notMapped("parked")
  }
  if (given.stackBranch !== undefined) {
    notMapped("stackBranch")
  }
  if (given.landed !== undefined) {
    notMapped("landed")
  }
  if (given.landingNeeds !== undefined) {
    notMapped("landingNeeds")
  }
  if (given.profile?.lastRobot !== undefined && given.robot === undefined) {
    throw new Error("given.profile.lastRobot needs given.robot")
  }
  if (given.profile?.skipsReview === true && given.profile.lastRobot === undefined) {
    throw new Error("given.profile.skipsReview needs given.profile.lastRobot: Patronus says it only of the last robot's profile")
  }
}

/** The facts of the next step of auth */
export function nextStepFactsOf(given: Given): NextStepFacts {
  checkMapped(given)
  const pullRequest = pullRequestOf(given)
  const listed = given.readiness !== "unlisted" && given.readiness !== "space"
  return facts({
    branchName: BRANCH,
    subject: BRANCH,
    commitCount: 1,
    robot: given.robot === undefined ? { kind: "start" } : finished("failed"),
    restart: given.robot === undefined ? null : restart,
    pullRequestsAvailable: listed,
    pullRequest: pullRequest === undefined ? null : nextStepPullRequest(pullRequest),
    target: TARGET,
    landingNeeds: [],
    readiness: { readiness: readinessOf(given, pullRequest), stale: false },
  })
}

/** The review as `reviewFactOf` of `useSafePush` gives it */
function reviewFactOf(given: Given, mode: PatronusPushMode, options: PatronusStartOptions, readiness: ReviewReadiness): ReviewFact {
  if (mode !== "rebaseAutosquash" || !options.supported) {
    return { kind: "none" }
  }
  if (given.readiness === "space") {
    return { kind: "space", required: true }
  }
  if (given.readiness === "unlisted") {
    return { kind: "unlisted" }
  }
  return { kind: "loaded", readiness }
}

/** The facts of the start of auth, with the step of `getBranchNextStep`, the profile chosen as `readSafePush` chooses it */
export function safePushFactsOf(given: Given): { step: BranchNextStep; facts: SafePushFacts } {
  const next = nextStepFactsOf(given)
  const step = getBranchNextStep(next)
  const mode: PatronusPushMode = given.mode === "dryRun" ? "dryRun" : "rebaseAutosquash"
  const options = startOptions(given)
  const pullRequest = pullRequestOf(given)
  const fromPullRequest = getPullRequestStart(given.readiness === "space" ? null : GITHUB_REPOSITORY, pullRequest, mode, TARGET)
  const review = reviewFactOf(given, mode, options, next.readiness!.readiness)
  const profiles = options.profiles.filter(profile => allowsPushMode(profile, mode))
  const branchProfileId = profiles.some(profile => profile.id === options.defaultProfileId) ? options.defaultProfileId : null
  const bypassReview =
    given.flags.includes("withoutApprovals") && mode === "rebaseAutosquash" && fromPullRequest === null && !(review.kind === "loaded" && review.readiness.kind !== "missing")
  const bypass = bypassReview ? getBypassProfiles(options, mode) : null
  const offered = bypass === null ? profiles : [...bypass.skipping, ...bypass.other, ...bypass.emergency]
  const chosen = given.profile?.chosen
  const profileId = chosen !== undefined ? (chosen === "none" ? null : chosen) : bypass === null ? branchProfileId : bypass.defaultProfileId
  const profile = fromPullRequest === null ? (offered.find(it => it.id === profileId) ?? null) : null
  return {
    step,
    facts: {
      branchName: BRANCH,
      subject: BRANCH,
      mode,
      via: given.via,
      step: { kind: step.kind, blocker: mode === "dryRun" ? step.dryRunBlocker : step.safePushBlocker },
      target: TARGET,
      options: { status: "loaded", options },
      profile,
      bypassReview,
      fromPullRequest,
      startPush: getStartPush(given.remote === "upToDate" ? "upToDate" : "create", false),
      hasTip: true,
      replaces: null,
      pushHeld: null,
      review,
      dismiss: null,
      ancestorCount: 0,
      wholeStackPullRequest: null,
      landingRefusals: [],
    },
  }
}
