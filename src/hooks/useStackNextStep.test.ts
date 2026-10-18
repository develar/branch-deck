// @spec spec/branch/next-step.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { getStackNextStepNow, readinessKeysNeedingAnswer } from "@/hooks/useStackNextStep"
import type { MissingReviewGroup, PatronusRobot, PullRequest, ReviewReadiness } from "@/lib/bindings"
import { robotsById, testRobot } from "@/lib/test-robot"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { Branch, StackBranch } from "@/stores/branch-sync"
import { branchSyncStore } from "@/stores/branch-sync"
import { patronusStore } from "@/stores/patronus"
import { pullRequestsStore } from "@/stores/pull-requests"
import { reviewReadinessStore } from "@/stores/review-readiness"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

vi.mock("@/stores/repository", () => import("@/test-support/selected-repository-mock"))

/** The key of `/repo` with the prefix `user` in the stores of Patronus and GitHub answers */
const KEY = "/repo\nuser"

function branch(name: string, parent: string | null): Branch {
  return testBranch(name, parent, {
    commits: [{ originalHash: `${name}-original`, hash: `${name}-tip`, strippedSubject: name, message: name, author: "Ann", authorTime: 0, committerTime: 0 }],
    commitCount: 2,
    remoteStatus: testRemoteStatus({ remoteTip: `${name}-tip` }),
  })
}

/** The stack of the island: `air` → `bazel` → `deploy`, `side` off `bazel` */
const branches = [branch("air", null), branch("bazel", "air"), branch("deploy", "bazel"), branch("side", "bazel")]

function stackBranch(patch: Partial<StackBranch> = {}): StackBranch {
  return {
    root: "air",
    branchName: "user/stack/air",
    members: ["air", "bazel", "deploy", "side"],
    commitCount: 8,
    state: "Built",
    tip: "stack-tip",
    blockedBy: null,
    conflict: null,
    error: null,
    remoteStatus: testRemoteStatus({ remoteTip: "stack-tip" }),
    ...patch,
  }
}

function setStack(stack: StackBranch | null, list: readonly Branch[] = branches): void {
  branchSyncStore.setState({
    branches: Object.fromEntries(list.map(it => [it.name, it])),
    branchOrder: list.map(it => it.name),
    stacksEnabled: true,
    baselineBranch: "origin/master",
    stackBranches: stack === null ? {} : { air: stack },
  })
}

function setRobots(...robots: PatronusRobot[]): void {
  patronusStore.setState({
    repositories: {
      [KEY]: {
        availability: { status: "available" },
        robots: robotsById(...robots),
        listedFrom: 0,
        fetchedAt: 0,
        isRefreshing: false,
        needsReview: {},
        githubRepository: "acme/air",
        watched: {},
        failing: {},
      },
    },
  })
}

function stackPullRequest(patch: Partial<PullRequest> = {}): PullRequest {
  return {
    number: 12,
    title: "The whole air stack",
    url: "https://github.com/acme/air/pull/12",
    state: "open",
    isDraft: false,
    reviewDecision: null,
    headRefName: "user/stack/air",
    baseRefName: "master",
    headRefOid: "stack-tip",
    checks: null,
    updatedAt: 0,
    reviewers: [{ login: "ikorennoy", state: "requested", requested: true }],
    author: "me",
    ...patch,
  }
}

function setPullRequests(stack: PullRequest | null): void {
  pullRequestsStore.setState({
    repositories: {
      [KEY]: {
        availability: { status: "available", owner: "acme", name: "air" },
        pullRequests: {},
        stackPullRequests: stack === null ? {} : { air: stack },
        fetchedAt: 1,
        isRefreshing: false,
      },
    },
  })
}

function setReadiness(readiness: ReviewReadiness): void {
  reviewReadinessStore.setState({
    repositories: {
      [KEY]: { [stackReadinessKey("air")]: { key: "k", readiness, requirement: null, stale: false, answeredAt: 0, tip: "stack-tip", pullRequest: 12 } },
    },
  })
}

const ilya = { login: "ikorennoy", fullName: "Ilya Korennoy" }

function missing(groups: MissingReviewGroup[]): ReviewReadiness {
  return { kind: "missing", groups, notCounted: [], reason: "approvalsMissing" }
}

beforeEach(() => {
  setStack(stackBranch())
  setRobots()
  setPullRequests(null)
})

afterEach(() => {
  branchSyncStore.setState({ branches: {}, branchOrder: [], stacksEnabled: false, stackBranches: {} })
  patronusStore.setState({ repositories: {} })
  pullRequestsStore.setState({ repositories: {} })
  reviewReadinessStore.setState({ repositories: {} })
})

describe("the next step of the whole stack", () => {
  it("is the Safe Push of the stack branch of a stack that forks, which needs the readiness of the whole stack", () => {
    const next = getStackNextStepNow("air")
    expect(next?.leaf).toBeNull()
    expect(next?.step).toMatchObject({ kind: "safePush", item: { id: "safe-push", disabled: false }, safePushBlocker: null, dryRunBlocker: null, needsReadiness: true })
    expect(readinessKeysNeedingAnswer([stackReadinessKey("air")])).toEqual([stackReadinessKey("air")])
    // the same object while nothing it reads changed
    expect(getStackNextStepNow("air")).toBe(next)
  })

  it("is the step of the last branch of a straight chain, which has no stack branch: its Safe Push lands the whole stack", () => {
    setStack(null, branches.slice(0, 3))
    const next = getStackNextStepNow("air")
    expect(next?.leaf).toBe("deploy")
    expect(next?.step.kind).toBe("safePush")
    // its readiness is the branch's own, asked by its row
    expect(readinessKeysNeedingAnswer([stackReadinessKey("air")])).toEqual([])
    // a lone branch heads no stack
    setStack(null, [branch("docs", null)])
    expect(getStackNextStepNow("docs")).toBeNull()
  })

  it("is disabled with the reason of a stack branch the sync didn't build, for a Dry Run too: the stack lands in pieces", () => {
    setStack(stackBranch({ state: "Conflict", conflict: { commit: "0123456789abcdef", files: ["config.js"] } }))
    const reason = "The stack branch conflicts at 01234567 in config.js: the stack lands in pieces"
    expect(getStackNextStepNow("air")?.step).toMatchObject({
      kind: "safePush",
      item: { id: "safe-push", disabled: true, description: reason },
      safePushBlocker: reason,
      dryRunBlocker: reason,
      notice: { title: "Safe Push of the whole air stack can't start", description: reason },
    })
    setStack(stackBranch({ state: "Blocked", blockedBy: { member: "side", reason: "Conflict" } }))
    expect(getStackNextStepNow("air")?.step.safePushBlocker).toBe("Blocked by side: fix it first, then sync")
  })

  it("waits for the robot of a branch of the stack, which lands a part of it; a Dry Run doesn't", () => {
    setRobots(testRobot("r1", "bazel"))
    expect(getStackNextStepNow("air")?.step).toMatchObject({
      safePushBlocker: "A Safe Push of bazel is running: wait until it merges",
      dryRunBlocker: null,
    })
  })

  it("offers nothing while the robot of the whole stack runs: it holds every branch of the stack", () => {
    setRobots(testRobot("w1", null, { stackRoot: "air", sourceRevision: "stack-tip" }))
    expect(getStackNextStepNow("air")?.step).toMatchObject({ kind: "none", item: null })
  })

  it("asks for the pull request of the whole stack where the approvals are missing, then waits in it", () => {
    setReadiness(missing([{ name: "IJPL Code Platform", needed: 1, approvedBy: [], requested: [], suggested: [ilya], changesRequestedBy: [] }]))
    expect(getStackNextStepNow("air")?.step).toMatchObject({
      kind: "createPullRequest",
      item: { id: "create-stack-pull-request", label: "Create Pull Request of the Stack…", description: "Code owners must approve in a pull request: IJPL Code Platform" },
      notice: { title: "Safe Push of the whole air stack needs a pull request" },
    })

    setPullRequests(stackPullRequest())
    setReadiness(missing([{ name: "IJPL Code Platform", needed: 1, approvedBy: [], requested: [ilya], suggested: [ilya], changesRequestedBy: [] }]))
    expect(getStackNextStepNow("air")?.step).toMatchObject({
      kind: "awaiting",
      item: { label: "Awaiting Ilya Korennoy" },
      safePushBlocker: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #12",
      notice: { title: "Safe Push of the whole air stack waits for Ilya Korennoy", description: "IJPL Code Platform · requested on #12" },
    })
  })

  it("is settled once the pull request of the whole stack merged with the stack branch as it is", () => {
    setPullRequests(stackPullRequest({ state: "merged" }))
    expect(getStackNextStepNow("air")?.step).toMatchObject({ kind: "none", safePushBlocker: "Pull request #12 is merged" })
  })
})
