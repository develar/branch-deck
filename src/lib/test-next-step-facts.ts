// The facts of the next step of a branch for tests (`getBranchNextStep`, `getLandingNode`): an open pull request into
// master, a start offered, every patch on top
import { vi } from "vite-plus/test"
import type { MissingReviewGroup, Person, ReviewReadiness } from "@/lib/bindings"
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
  mergedCurrent: false,
  baseIsVirtualBranch: false,
  waitingFor: [],
  changesRequestedBy: [],
  checksFailing: false,
  changesRequested: false,
}

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
    landedIn: null,
    landedWith: null,
    robot: { kind: "start" },
    robotPullRequest: null,
    restart: null,
    pullRequestsAvailable: true,
    pullRequest: openPullRequest,
    target: "master",
    landsBranches: 1,
    readiness: null,
    startDescription: null,
    stackPullRequest: null,
    actions: {
      openSafePush: vi.fn(),
      openPullRequest: vi.fn(),
      retarget: vi.fn(),
      createPullRequest,
      requestReview,
    },
    ...patch,
  }
}

export function withReadiness(readiness: ReviewReadiness, patch: Partial<NextStepFacts> = {}): NextStepFacts {
  return facts({ readiness: { readiness, stale: false }, ...patch })
}
