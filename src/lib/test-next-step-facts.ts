// The facts of the next step of a branch for tests (`getBranchNextStep`, `getLandingNode`): an open pull request into
// master, a start offered, every patch on top
import { vi } from "vite-plus/test"
import type { Landed, MissingReviewGroup, Person, ReviewReadiness } from "@/lib/bindings"
import type { NextStepFacts, NextStepPullRequest } from "@/lib/branch-next-step"
import type { MenuItem } from "@/lib/menu"
import type { PatronusChipKind, PatronusChipState, SafePushSlot } from "@/lib/patronus"

export const ilya: Person = { login: "ikorennoy", fullName: "Ilya Korennoy" }
export const alice: Person = { login: "alice", fullName: null }
export const junie: Person = { login: "junie-jetbrains", fullName: null }

export function chip(kind: PatronusChipKind): PatronusChipState {
  return { kind, color: "neutral", label: kind, commit: "current", outdated: false }
}

export function finished(kind: PatronusChipKind): SafePushSlot {
  return { kind: "finished", chip: chip(kind), offered: true }
}

export function group(patch: Partial<MissingReviewGroup> = {}): MissingReviewGroup {
  return { name: "IJPL Code Platform", needed: 1, approvedBy: [], requested: [ilya], suggested: [ilya], changesRequestedBy: [], ...patch }
}

export function missing(groups: MissingReviewGroup[], patch: Partial<Extract<ReviewReadiness, { kind: "missing" }>> = {}): ReviewReadiness {
  return { kind: "missing", groups, notCounted: [junie], reason: "approvalsMissing", ...patch }
}

export const openPullRequest: NextStepPullRequest = {
  number: 1078,
  state: "open",
  isDraft: false,
  base: "master",
  intoTarget: true,
  baseIsVirtualBranch: false,
  waitingFor: [],
  changesRequestedBy: [],
  checksFailing: false,
  changesRequested: false,
}

/** What the view says landed a branch (`landedStates`): the fetched baseline */
export const inBaseline: Landed = { by: { kind: "baseline", upstream: "origin/master" }, after: [] }

/** A merged Safe Push into master of `branch` (of the whole stack of the root `branch`) landed all of the branch */
export function landedWith(branch: string, wholeStack = false): Landed {
  return { by: { kind: "robots", by: [{ branch, robotId: "r", wholeStack }], own: false, target: "master" }, after: [] }
}

/** GitHub merged the pull request #1078 with the commits of the branch */
export const mergedPullRequest: Landed = { by: { kind: "pullRequest", number: 1078 }, after: [] }

export const restart: MenuItem = { id: "restart-robot", label: "Restart Safe Push", onSelect: () => {} }
export const createPullRequest: MenuItem = { id: "create-pull-request", label: "Create Pull Request…", onSelect: () => {} }
export const requestReview: MenuItem = { id: "request-review", label: "Request Review…", description: "Ask Ilya Korennoy to review #1078", onSelect: () => {} }

export function facts(patch: Partial<NextStepFacts> = {}): NextStepFacts {
  return {
    branchName: "wsm-skip-wa-2",
    subject: "wsm-skip-wa-2",
    patronusAvailable: true,
    syncing: false,
    hasError: false,
    errorReason: null,
    commitCount: 2,
    busy: null,
    blocked: false,
    startBlocker: null,
    dryRunStartBlocker: null,
    landingPart: null,
    held: false,
    parked: null,
    landed: null,
    robot: { kind: "start" },
    robotPullRequest: null,
    restart: null,
    pullRequestsAvailable: true,
    pullRequestsUnlisted: null,
    pullRequest: openPullRequest,
    target: "master",
    landsBranches: 1,
    landingNeeds: null,
    readiness: null,
    pending: null,
    unavailable: null,
    forStart: false,
    startDescription: null,
    stackPullRequest: null,
    actions: {
      openSafePush: vi.fn(),
      openPullRequest: vi.fn(),
      retarget: vi.fn(),
      createPullRequest,
      requestReview,
      fixLanding: vi.fn(),
    },
    ...patch,
  }
}

/** An answer of the readiness for the branch as it is that holds nothing back */
export const answered: NonNullable<NextStepFacts["readiness"]> = { readiness: { kind: "notRequired" }, stale: false }

export function withReadiness(readiness: ReviewReadiness, patch: Partial<NextStepFacts> = {}): NextStepFacts {
  return facts({ readiness: { readiness, stale: false }, ...patch })
}
