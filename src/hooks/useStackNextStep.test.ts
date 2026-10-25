// @spec spec/branch/next-step.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { getStackNextStepNow, readinessKeysNeedingAnswer } from "@/hooks/useStackNextStep"
import type { BranchPatronus, MissingReviewGroup, PatronusRobot, PatronusView, PullRequest, ReviewReadiness } from "@/lib/bindings"
import { robotsById, testRobot } from "@/lib/test-robot"
import { stackReadinessKey } from "@/lib/whole-stack"
import type { Branch, StackBranch } from "@/stores/branch-sync"
import { branchSyncStore } from "@/stores/branch-sync"
import { patronusStore } from "@/stores/patronus"
import { pullRequestsStore } from "@/stores/pull-requests"
import { reviewReadinessStore } from "@/stores/review-readiness"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"
import { branchFacts, landedWhole, robotOf, testPatronusRepository } from "@/test-support/patronus-repository"

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
  setRobotsAndView(robots, null)
}

/**
 * The robots, and what the backend says they land, landed and hold (`view`); why each failed one failed is answered
 * (not for approvals)
 */
function setRobotsAndView(robots: readonly PatronusRobot[], view: Partial<PatronusView> | null): void {
  patronusStore.setState({
    repositories: {
      [KEY]: {
        ...testPatronusRepository(robotsById(...robots), view === null ? null : { branches: {}, stacks: {}, landedByRobot: {}, ...view }),
        needsReview: Object.fromEntries(robots.filter(robot => robot.status === "failure").map(robot => [robot.id, false])),
      },
    },
  })
}

/** The facts of a branch the merged robot `robotId` of `by` landed whole */
function landedWholeBy(by: string, robotId: string, own = false): BranchPatronus {
  return landedWhole(robotOf(by, robotId), own)
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
        listedAt: 1,
        isRefreshing: false,
      },
    },
  })
}

function setReadiness(readiness: ReviewReadiness, key = stackReadinessKey("air")): void {
  reviewReadinessStore.setState(state => ({
    repositories: {
      [KEY]: {
        ...state.repositories[KEY],
        [key]: { key: "k", readiness, requirement: null, stale: false, answeredAt: 0, tip: "stack-tip", pullRequest: 12, waitingFor: [], approvedBy: [] },
      },
    },
  }))
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
    // before the first answer of the readiness of the whole stack its step waits for it
    expect(getStackNextStepNow("air")?.step).toMatchObject({ kind: "pending", safePushBlocker: "Waits for Patronus to check the approvals", needsReadiness: true })
    expect(readinessKeysNeedingAnswer([stackReadinessKey("air")])).toEqual([stackReadinessKey("air")])
    setReadiness({ kind: "notRequired" })
    const next = getStackNextStepNow("air")
    expect(next?.leaf).toBeNull()
    expect(next?.step).toMatchObject({ kind: "safePush", item: { id: "safe-push", disabled: false }, safePushBlocker: null, dryRunBlocker: null, needsReadiness: true })
    expect(readinessKeysNeedingAnswer([stackReadinessKey("air")])).toEqual([stackReadinessKey("air")])
    // the same object while nothing it reads changed
    expect(getStackNextStepNow("air")).toBe(next)
  })

  it("is the step of the last branch of a straight chain, which has no stack branch: its Safe Push lands the whole stack", () => {
    setStack(null, branches.slice(0, 3))
    setReadiness({ kind: "notRequired" }, "deploy")
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
    const landing = { branch: "bazel", robotId: "r1", wholeStack: false }
    setRobotsAndView([testRobot("r1", "bazel")], {
      stacks: { air: { robotId: null, landingMember: landing, holdRobotId: null, startBlocked: false, start: { kind: "ancestor", landing }, dryRun: null } },
    })
    expect(getStackNextStepNow("air")?.step).toMatchObject({
      safePushBlocker: "A Safe Push of bazel is running: wait until it merges",
      dryRunBlocker: null,
    })
  })

  it("waits for the robot of a branch of a straight chain, not the failure of the earlier piece; a new parent commit starts anew", () => {
    const chain = branches.slice(0, 3)
    setStack(null, chain)
    const failed = testRobot("d1", "deploy", { status: "failure", finishTime: 100 })
    setRobots(failed)
    // the robot of the last branch failed on the chain as it is: the user restarts it
    expect(getStackNextStepNow("air")?.step.kind).toBe("restart")
    // then a Safe Push of air started: it lands a part of the chain, the failure waits for it
    const landing = { branch: "air", robotId: "a1", wholeStack: false }
    setRobotsAndView([failed, testRobot("a1", "air", { startTime: 1704189600 })], {
      branches: { deploy: branchFacts({ landingAncestor: landing, start: { kind: "ancestor", landing } }) },
    })
    expect(getStackNextStepNow("air")?.step).toMatchObject({
      kind: "safePush",
      item: { disabled: true },
      safePushBlocker: "A Safe Push of air is running: wait until it merges",
    })
    // air got a new commit, deploy was rebuilt on it, and its remote branch lacks it: the failed robot tested an older piece
    const deploy = branch("deploy", "bazel")
    const rebuilt: Branch = {
      ...deploy,
      commits: deploy.commits.map(commit => ({ ...commit, hash: "deploy-rebuilt" })),
      remoteStatus: testRemoteStatus({ remoteTip: "deploy-tip", ancestorCommitsAhead: 1, fastForward: false }),
    }
    setStack(null, [...branches.slice(0, 2), rebuilt])
    setRobots(failed)
    setReadiness({ kind: "notRequired" }, "deploy")
    expect(getStackNextStepNow("air")?.step).toMatchObject({ kind: "safePush", item: { disabled: false } })
  })

  it("lands only the rest after a piece landed its main line, and offers nothing once every branch of it landed", () => {
    // the Safe Push of deploy landed air, bazel and deploy: side is left, with its 2 commits
    const byDeploy = { air: landedWholeBy("deploy", "d1"), bazel: landedWholeBy("deploy", "d1"), deploy: landedWholeBy("deploy", "d1", true) }
    setRobotsAndView([testRobot("d1", "deploy", { status: "success", sourceRevision: "deploy-tip", finishTime: 200 })], { branches: byDeploy })
    setReadiness({ kind: "notRequired" })
    const rest = "Lands 1 branch · 2 commits into master in one Safe Push: side; air, bazel and deploy landed with deploy"
    expect(getStackNextStepNow("air")?.step).toMatchObject({
      kind: "safePush",
      item: { label: "Safe Push", disabled: false, description: rest },
      safePushBlocker: null,
    })
    // side landed too, with its own Safe Push: nothing of the whole stack is left
    setRobotsAndView(
      [
        testRobot("d1", "deploy", { status: "success", sourceRevision: "deploy-tip", finishTime: 200 }),
        testRobot("s1", "side", { status: "success", sourceRevision: "side-tip", finishTime: 300 }),
      ],
      { branches: { ...byDeploy, side: landedWholeBy("side", "s1", true) } },
    )
    expect(getStackNextStepNow("air")?.step).toMatchObject({
      kind: "none",
      item: null,
      safePushBlocker: "Landed in master with deploy and side",
      dryRunBlocker: "Landed in master with deploy and side",
    })
  })

  it("offers nothing once its own robot landed every branch of it", () => {
    setRobots(testRobot("w1", null, { stackRoot: "air", sourceRevision: "stack-tip", status: "success", finishTime: 200 }))
    expect(getStackNextStepNow("air")?.step).toMatchObject({ kind: "none", safePushBlocker: "Merged by Patronus" })
  })

  it("offers nothing while the robot of the whole stack runs: it holds every branch of the stack", () => {
    setRobots(testRobot("w1", null, { stackRoot: "air", sourceRevision: "stack-tip" }))
    expect(getStackNextStepNow("air")?.step).toMatchObject({ kind: "none", item: null })
  })

  it("asks for the pull request of the whole stack where the approvals are missing, then waits in it", () => {
    setReadiness(missing([{ name: "IJPL Code Platform", needed: 1, approvedBy: [], requested: [], suggested: [ilya], changesRequestedBy: [] }]))
    expect(getStackNextStepNow("air")?.step).toMatchObject({
      kind: "createPullRequest",
      // in the header of the whole stack's island no "of the Stack": the notice and the menus keep it
      item: {
        id: "create-stack-pull-request",
        label: "Create Pull Request of the Stack…",
        compactLabel: "Create Pull Request…",
        description: "Code owners must approve in a pull request: IJPL Code Platform",
      },
      notice: { title: "Safe Push of the whole air stack needs a pull request" },
    })

    setPullRequests(stackPullRequest())
    setReadiness(missing([{ name: "IJPL Code Platform", needed: 1, approvedBy: [], requested: [ilya], suggested: [ilya], changesRequestedBy: [] }]))
    expect(getStackNextStepNow("air")?.step).toMatchObject({
      kind: "awaiting",
      item: { label: "Awaiting Ilya Korennoy", compactLabel: "Awaiting Review" },
      safePushBlocker: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #12",
      notice: { title: "Safe Push of the whole air stack waits for Ilya Korennoy", description: "IJPL Code Platform · requested on #12" },
    })
  })

  it("is settled once the pull request of the whole stack merged with the stack branch as it is", () => {
    setPullRequests(stackPullRequest({ state: "merged" }))
    expect(getStackNextStepNow("air")?.step).toMatchObject({ kind: "none", safePushBlocker: "Pull request #12 is merged" })
  })
})
