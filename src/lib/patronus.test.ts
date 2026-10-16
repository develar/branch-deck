import { describe, expect, it } from "vite-plus/test"
import type { PatronusRobot, PullRequest } from "@/lib/bindings"
import type { SafePushSlotInput } from "@/lib/patronus"
import {
  allowsPushMode,
  canCancelRobot,
  canRestartRobot,
  canStartRobot,
  describeRobot,
  findBranchRobot,
  findLandingAncestor,
  findLandingDescendants,
  findMergedRobots,
  findPushHold,
  formatMergedRobotTitle,
  formatPatronusAvailabilityHint,
  formatPushHoldReason,
  formatReviewerGroup,
  formatReviewerGroupFiles,
  formatRobotCommitNotice,
  getPatronusChipState,
  getPullRequestStart,
  getRobotRestartAction,
  getRowRobotChip,
  getRowRobotStep,
  getSafePushSlot,
  isHoldingRobot,
  isStartBlocked,
  needsRobotAttention,
} from "@/lib/patronus"
import type { RemoteStatus } from "@/stores/branch-sync"
import { testBranch, testRemoteStatus } from "@/stores/test-branch"

function robot(patch: Partial<PatronusRobot>): PatronusRobot {
  return {
    id: "r1",
    name: "IJPL-1 fix the cache",
    status: "running",
    pushMode: "rebaseAutosquash",
    branchName: "IJPL-1",
    sourceRevision: "tip",
    targetBranch: "master",
    startTime: 100,
    finishTime: null,
    cancellationReason: null,
    url: "https://patronus/robot/r1",
    ...patch,
  }
}

function byId(...robots: PatronusRobot[]): Record<string, PatronusRobot> {
  return Object.fromEntries(robots.map(it => [it.id, it]))
}

/** The remote branch is at the commit the robot took ("tip"), the local branch has a commit more */
const unpushedOnRobot: RemoteStatus = testRemoteStatus({ remoteTip: "tip", unpushedCommits: ["fix"], commitsAhead: 1, myCommitsAhead: 1 })
/** The remote branch is at the commit the robot took ("tip"), the local branch has the same commits on a newer baseline */
const rebuiltOnRobot: RemoteStatus = testRemoteStatus({ remoteTip: "tip", fastForward: false })

describe("findBranchRobot", () => {
  it("matches a robot started on the branch, and a Safe Push of a temporary ref by the local or the pushed tip", () => {
    const onBranch = robot({ id: "branch" })
    const cliPush = robot({ id: "cli", branchName: null, sourceRevision: "pushed" })
    const otherBranch = robot({ id: "other", branchName: "IJPL-2", sourceRevision: "tip" })
    expect(findBranchRobot(byId(cliPush, otherBranch), "IJPL-1", "local", "pushed")).toBe(cliPush)
    expect(findBranchRobot(byId(cliPush, otherBranch), "IJPL-1", "pushed", null)).toBe(cliPush)
    // a robot of another virtual branch never belongs to this one, even on the same commit
    expect(findBranchRobot(byId(otherBranch), "IJPL-1", "tip", null)).toBeUndefined()
    expect(findBranchRobot(byId(onBranch, otherBranch), "IJPL-1", "anything", null)).toBe(onBranch)
    expect(findBranchRobot(undefined, "IJPL-1", "tip", null)).toBeUndefined()
  })

  it("prefers a running robot over a canceling one over a finished one, then the newest", () => {
    const failed = robot({ id: "failed", status: "failure", startTime: 300 })
    const canceling = robot({ id: "canceling", status: "canceling", startTime: 200 })
    const restarted = robot({ id: "restarted", status: "running", startTime: 200 })
    const older = robot({ id: "older", status: "success", startTime: 100 })
    expect(findBranchRobot(byId(failed, canceling, older), "IJPL-1", "tip", null)).toBe(canceling)
    expect(findBranchRobot(byId(canceling, restarted), "IJPL-1", "tip", null)).toBe(restarted)
    expect(findBranchRobot(byId(failed, older), "IJPL-1", "tip", null)).toBe(failed)
  })

  it("prefers across the branch and the tips as one list, of equals the first", () => {
    const finishedOnBranch = robot({ id: "branch", status: "success", startTime: 300 })
    const runningOnTip = robot({ id: "cli", branchName: null, sourceRevision: "pushed", status: "running", startTime: 100 })
    const runningOnLocal = robot({ id: "ide", branchName: null, sourceRevision: "local", status: "running", startTime: 100 })
    expect(findBranchRobot(byId(finishedOnBranch, runningOnTip), "IJPL-1", "local", "pushed")).toBe(runningOnTip)
    expect(findBranchRobot(byId(runningOnTip, runningOnLocal), "IJPL-1", "local", "pushed")).toBe(runningOnTip)
    expect(findBranchRobot(byId(runningOnLocal, runningOnTip), "IJPL-1", "local", "pushed")).toBe(runningOnLocal)
  })
})

describe("getPatronusChipState", () => {
  it("names the state in its hue; a successful dry run passed, a successful push merged", () => {
    expect(getPatronusChipState(robot({ status: "running" }), "tip", null)).toEqual({ kind: "running", color: "neutral", label: "Running", commit: "current", outdated: false })
    expect(getPatronusChipState(robot({ status: "canceling" }), "tip", null).label).toBe("Canceling")
    expect(getPatronusChipState(robot({ status: "failure" }), "tip", null)).toMatchObject({ kind: "failed", color: "error", label: "Failed" })
    expect(getPatronusChipState(robot({ status: "success" }), "tip", null)).toMatchObject({ kind: "merged", color: "info", label: "Merged" })
    expect(getPatronusChipState(robot({ status: "success", pushMode: "dryRun" }), "tip", null)).toMatchObject({ kind: "passed", color: "success", label: "Dry run passed" })
    expect(getPatronusChipState(robot({ status: "canceled" }), "tip", null)).toMatchObject({ kind: "canceled", color: "neutral" })
  })

  it('names a robot that failed for missing approvals "Needs review"', () => {
    expect(getPatronusChipState(robot({ status: "failure" }), "tip", null, { needsReview: true })).toMatchObject({ kind: "needsReview", color: "warning", label: "Needs review" })
    // only a failure
    expect(getPatronusChipState(robot({ status: "running" }), "tip", null, { needsReview: true }).kind).toBe("running")
  })

  it('names a running robot whose required check failed "Failing", still a running one', () => {
    expect(getPatronusChipState(robot({ status: "running" }), "tip", null, { failing: true })).toEqual({
      kind: "running",
      color: "error",
      label: "Failing",
      commit: "current",
      outdated: false,
    })
    // a cancel was asked for: the user decided already
    expect(getPatronusChipState(robot({ status: "canceling" }), "tip", null, { failing: true })).toMatchObject({ color: "neutral", label: "Canceling" })
    // finished, it is what it ended with
    expect(getPatronusChipState(robot({ status: "failure" }), "tip", null, { failing: true }).label).toBe("Failed")
  })

  it("marks a robot whose branch changed since its commit, not one whose branch was only rebuilt, never a merged one", () => {
    // the remote branch moved on since the robot took its commit
    expect(getPatronusChipState(robot({ status: "failure" }), "newer", testRemoteStatus({ remoteTip: "newer" }))).toMatchObject({ commit: "changedOnRemote", outdated: true })
    // a fix the remote lacks
    expect(getPatronusChipState(robot({ status: "running" }), "fixed", unpushedOnRobot)).toMatchObject({ commit: "changedLocally", outdated: true })
    // the sync rebuilt the branch on a newer baseline: new hashes, the same commits
    expect(getPatronusChipState(robot({ status: "running" }), "rebuilt", rebuiltOnRobot)).toMatchObject({ commit: "rebased", outdated: false })
    // the remote status is not loaded yet
    expect(getPatronusChipState(robot({ status: "running" }), "newer", null).outdated).toBe(false)
    expect(getPatronusChipState(robot({ status: "success" }), "newer", testRemoteStatus({ remoteTip: "newer" }))).toMatchObject({ commit: "current", outdated: false })
  })
})

describe("formatRobotCommitNotice", () => {
  const started = { sourceRevision: "3264f30f0123" }
  const localTip = "88de421d4567"
  const pushedSince = testRemoteStatus({ remoteTip: localTip })
  const unpushed = testRemoteStatus({ remoteTip: started.sourceRevision, unpushedCommits: ["fix"], commitsAhead: 1, myCommitsAhead: 1 })
  const changed = "Started on 3264f30f; the branch has changed since (now 88de421d): the result won't cover the newest commits."

  function notice(subject: PatronusRobot, remote: RemoteStatus): string | null {
    const action = getRobotRestartAction(subject, "IJPL-1", localTip, remote)
    return formatRobotCommitNotice(subject, getPatronusChipState(subject, localTip, remote), localTip, action)
  }

  it("names the change and the restart that tests the branch as it is, pushed first when the remote lacks it", () => {
    const failed = robot({ status: "failure", ...started })
    expect(getRobotRestartAction(failed, "IJPL-1", localTip, pushedSince)).toBe("restart")
    expect(notice(failed, pushedSince)).toBe(`${changed} Restart Safe Push to test the current branch.`)
    // a plain restart would test the commit the remote has
    expect(getRobotRestartAction(failed, "IJPL-1", localTip, unpushed)).toBe("pushAndRestart")
    expect(notice(failed, unpushed)).toBe(`${changed} Push and Restart Safe Push to test the current branch.`)
    // a passed dry run restarts only on a newer pushed commit: the push makes one
    const passed = robot({ status: "success", pushMode: "dryRun", ...started })
    expect(getRobotRestartAction(passed, "IJPL-1", localTip, unpushed)).toBe("pushAndRestart")
    expect(getRobotRestartAction(passed, "IJPL-1", localTip, testRemoteStatus({ remoteTip: started.sourceRevision }))).toBeNull()
  })

  it("pushes and restarts a running robot of a branch changed since, asks to cancel one the remote moved past, and offers nothing for a robot of a temporary ref", () => {
    const running = robot(started)
    // Push and Restart cancels the running robot first: the one push into a branch whose Safe Push runs
    expect(getRobotRestartAction(running, "IJPL-1", localTip, unpushed)).toBe("pushAndRestart")
    expect(notice(running, unpushed)).toBe(`${changed} Push and Restart Safe Push to test the current branch.`)
    expect(getRobotRestartAction(running, "IJPL-1", localTip, unpushed, true)).toBeNull()
    expect(notice(running, pushedSince)).toBe(`${changed} Cancel Safe Push, then Restart Safe Push to test the current branch.`)
    // a restart would test its old commit
    const cliPush = robot({ status: "failure", branchName: null, ...started })
    expect(getRobotRestartAction(cliPush, "IJPL-1", localTip, unpushed)).toBeNull()
    expect(notice(cliPush, unpushed)).toBe(changed)
    expect(notice(robot({ branchName: null, ...started }), unpushed)).toBe(changed)
  })

  it("says quietly that a robot started on an earlier build of the same commits, nothing for the branch as it is", () => {
    expect(notice(robot(started), testRemoteStatus({ remoteTip: started.sourceRevision }))).toBe("Started on an earlier build of the same commits.")
    expect(notice(robot({ sourceRevision: localTip }), unpushed)).toBeNull()
    expect(notice(robot({ sourceRevision: null }), unpushed)).toBeNull()
  })
})

describe("needsRobotAttention", () => {
  it("needs the user for a failed robot of the branch as it is, also after a sync rebuilt the branch", () => {
    expect(needsRobotAttention(robot({ status: "failure" }), "tip", null)).toBe(true)
    expect(needsRobotAttention(robot({ status: "failure", pushMode: "dryRun" }), "tip", null)).toBe(true)
    expect(needsRobotAttention(robot({ status: "failure" }), "rebuilt", rebuiltOnRobot)).toBe(true)
    // fixed since: the branch changed
    expect(needsRobotAttention(robot({ status: "failure" }), "fixed", unpushedOnRobot)).toBe(false)
    expect(needsRobotAttention(robot({ status: "failure" }), "fixed", testRemoteStatus({ remoteTip: "fixed" }))).toBe(false)
    // the remote status is not loaded yet
    expect(needsRobotAttention(robot({ status: "failure" }), "fixed", null)).toBe(false)
    expect(needsRobotAttention(robot({ status: "running" }), "tip", null)).toBe(false)
  })
})

describe("formatPatronusAvailabilityHint", () => {
  it("hints at what to do, and says nothing when there is nothing to do", () => {
    expect(formatPatronusAvailabilityHint({ status: "notLoggedIn" })).toBe("Run ./safePush.cmd once to log in to Patronus")
    expect(formatPatronusAvailabilityHint({ status: "error", message: "offline" })).toBe("Patronus: offline")
    expect(formatPatronusAvailabilityHint({ status: "available" })).toBeNull()
    expect(formatPatronusAvailabilityHint({ status: "notPatronus" })).toBeNull()
    expect(formatPatronusAvailabilityHint(null)).toBeNull()
  })
})

describe("actions", () => {
  it("restarts a failed, canceled or canceling robot of the branch, a passed dry run only with a newer pushed commit", () => {
    expect(canRestartRobot(robot({ status: "failure" }), "IJPL-1", null)).toBe(true)
    expect(canRestartRobot(robot({ status: "canceled" }), "IJPL-1", null)).toBe(true)
    expect(canRestartRobot(robot({ status: "canceling" }), "IJPL-1", null)).toBe(true)
    expect(canRestartRobot(robot({ status: "running" }), "IJPL-1", null)).toBe(false)
    expect(canRestartRobot(robot({ status: "success" }), "IJPL-1", "newer")).toBe(false)
    expect(canRestartRobot(robot({ status: "success", pushMode: "dryRun" }), "IJPL-1", "tip")).toBe(false)
    expect(canRestartRobot(robot({ status: "success", pushMode: "dryRun" }), "IJPL-1", "newer")).toBe(true)
    // a Safe Push of a temporary ref restarts its old commit: not offered
    expect(canRestartRobot(robot({ status: "failure", branchName: null }), "IJPL-1", null)).toBe(false)
    // a restart would check the same merge request without approvals
    expect(canRestartRobot(robot({ status: "failure" }), "IJPL-1", null, true)).toBe(false)
    // a robot of a pull request counts the approvals the pull request has at the restart
    expect(canRestartRobot(robot({ status: "failure", pullRequest: { number: 7, url: "u" } }), "IJPL-1", null, true)).toBe(true)
  })

  it("starts from the open pull request of a branch reviewed on GitHub, a draft only for a dry run", () => {
    const pullRequest = { number: 7, state: "open", isDraft: false } as PullRequest
    expect(getPullRequestStart("acme/monorepo", pullRequest, "rebaseAutosquash", null)).toEqual({ number: 7, blockedByDraft: false, command: "/patronus merge" })
    expect(getPullRequestStart("acme/monorepo", { ...pullRequest, isDraft: true }, "rebaseAutosquash", null)?.blockedByDraft).toBe(true)
    expect(getPullRequestStart("acme/monorepo", { ...pullRequest, isDraft: true }, "dryRun", null)).toEqual({ number: 7, blockedByDraft: false, command: "/patronus dry-run" })
    // on Space, without a pull request, or with a closed one: the Patronus API starts it
    expect(getPullRequestStart(null, pullRequest, "rebaseAutosquash", null)).toBeNull()
    expect(getPullRequestStart("acme/monorepo", undefined, "rebaseAutosquash", null)).toBeNull()
    expect(getPullRequestStart("acme/monorepo", { ...pullRequest, state: "merged" }, "rebaseAutosquash", null)).toBeNull()
  })

  it("never comments on a pull request into another base than the target: Patronus would merge it there", () => {
    const pullRequest = { number: 8, state: "open", isDraft: false, baseRefName: "user/virtual/login" } as PullRequest
    expect(getPullRequestStart("acme/monorepo", pullRequest, "rebaseAutosquash", "master")).toBeNull()
    expect(getPullRequestStart("acme/monorepo", { ...pullRequest, baseRefName: "master" }, "rebaseAutosquash", "master")?.number).toBe(8)
    // the target isn't loaded yet
    expect(getPullRequestStart("acme/monorepo", pullRequest, "rebaseAutosquash", null)?.number).toBe(8)
  })

  it("waits for a robot that lands an ancestor of a stacked branch, not for a dry run of it", () => {
    const robots: Record<string, PatronusRobot> = {
      auth: robot({ status: "running", pushMode: "dryRun" }),
      login: robot({ status: "running" }),
    }
    const landing = findLandingAncestor(["auth", "login"], name => robots[name])
    expect(landing).toEqual({ branch: "login", robot: robots.login })
    expect(findLandingAncestor(["auth"], name => robots[name])).toBeNull()
    expect(findLandingAncestor(["login"], () => ({ ...robots.login!, status: "success" }))).toBeNull()
    expect(canStartRobot(testBranch("oauth", "login"), undefined, landing)).toEqual({ ok: false, reason: "A Safe Push of login is running: wait until it merges" })
  })

  it("waits for a robot of a side branch that lands an ancestor: the branch would land its commits a second time", () => {
    // runtime is built on bazel, side-fix too: the Safe Push of runtime lands air and bazel
    const runtime = robot({ id: "runtime", branchName: "runtime", status: "running" })
    const spans: Record<string, { branch: string; robot: PatronusRobot }> = { air: { branch: "runtime", robot: runtime }, bazel: { branch: "runtime", robot: runtime } }
    const landing = findLandingAncestor(
      ["air", "bazel"],
      () => undefined,
      name => spans[name],
    )
    expect(landing).toEqual({ branch: "runtime", robot: runtime })
    expect(canStartRobot(testBranch("side-fix", "bazel"), undefined, landing)).toEqual({ ok: false, reason: "A Safe Push of runtime is running: wait until it merges" })
  })

  it("knows the ancestors a running robot of a stacked branch lands, and refuses their own Safe Push", () => {
    const ancestors: Record<string, string[]> = { auth: [], login: ["auth"], oauth: ["auth", "login"], sso: ["auth"] }
    const robots: Record<string, PatronusRobot> = {
      oauth: robot({ id: "oauth", branchName: "oauth", status: "running" }),
      // a dry run lands nothing, a finished robot neither
      sso: robot({ id: "sso", branchName: "sso", status: "running", pushMode: "dryRun" }),
      login: robot({ id: "login", branchName: "login", status: "failure" }),
    }
    const landed = findLandingDescendants(
      ["auth", "login", "oauth", "sso"],
      name => ancestors[name] ?? [],
      name => robots[name],
    )
    expect([...landed.keys()]).toEqual(["auth", "login"])
    const span = landed.get("login")!
    expect(span).toEqual({ branch: "oauth", robot: robots.oauth })
    expect(canStartRobot(testBranch("login", "auth"), robots.login, null, span)).toEqual({ ok: false, reason: "A Safe Push of oauth is running: it lands login with it" })
  })

  it("cancels a running robot only", () => {
    expect(canCancelRobot(robot({ status: "running" }))).toBe(true)
    expect(canCancelRobot(robot({ status: "canceling" }))).toBe(false)
  })

  it("holds the push of a branch whose Safe Push runs, or that a running Safe Push of a branch built on it lands; not for a dry run or a cancel", () => {
    const running = robot({ status: "running" })
    expect(isHoldingRobot(running)).toBe(true)
    expect(isHoldingRobot(robot({ status: "running", pushMode: "dryRun" }))).toBe(false)
    expect(isHoldingRobot(robot({ status: "canceling" }))).toBe(false)
    expect(isHoldingRobot(robot({ status: "failure" }))).toBe(false)
    const own = findPushHold("IJPL-1", running, null)
    expect(own).toEqual({ kind: "own", branch: "IJPL-1", robot: running })
    expect(formatPushHoldReason("IJPL-1", own!)).toBe("A Safe Push of IJPL-1 is running: wait until it merges, or push and restart it")
    // a Safe Push of a temporary ref is not on the branch: a push leaves it as it is
    expect(findPushHold("IJPL-1", robot({ status: "running", branchName: null }), null)).toBeNull()
    const oauth = robot({ id: "oauth", branchName: "oauth", status: "running" })
    const span = findPushHold("auth", undefined, { branch: "oauth", robot: oauth })
    expect(span).toEqual({ kind: "span", branch: "oauth", robot: oauth })
    expect(formatPushHoldReason("auth", span!)).toBe("A Safe Push of oauth is running: it lands auth with it")
    expect(findPushHold("auth", undefined, { branch: "oauth", robot: { ...oauth, status: "canceling" } })).toBeNull()
    // its own robot first
    expect(findPushHold("IJPL-1", running, { branch: "oauth", robot: oauth })?.kind).toBe("own")
  })

  it("blocks a start while a robot runs on the commit the branch is at", () => {
    expect(isStartBlocked(robot({ status: "running" }), "tip")).toBe(true)
    expect(isStartBlocked(robot({ status: "running" }), "fixed")).toBe(false)
    expect(isStartBlocked(robot({ status: "failure" }), "tip")).toBe(false)
    expect(isStartBlocked(undefined, "tip")).toBe(false)
  })

  it("starts a robot unless one runs on the commit the branch is at or the branch is blocked, and says why", () => {
    const branch = testBranch("IJPL-1", null, {
      commits: [{ originalHash: "o1", hash: "tip", strippedSubject: "fix", message: "fix", author: "a", authorTime: 1, committerTime: 1 }],
    })
    expect(canStartRobot(branch, undefined)).toEqual({ ok: true })
    expect(canStartRobot(branch, robot({ status: "failure" }))).toEqual({ ok: true })
    // a fix was committed since the robot started: a new one takes it
    expect(canStartRobot(branch, robot({ status: "running", sourceRevision: "older" }))).toEqual({ ok: true })
    expect(canStartRobot(branch, robot({ status: "running" }))).toEqual({ ok: false, reason: "A Safe Push of the last commit is running" })
    expect(canStartRobot(branch, robot({ status: "running", pushMode: "dryRun" }))).toEqual({ ok: false, reason: "A Dry Run of the last commit is running" })

    const blocked = testBranch("login", "auth", { status: "Blocked" })
    expect(canStartRobot(blocked, undefined)).toEqual({ ok: false, reason: "Blocked by auth: fix it first, then sync" })
  })

  it("offers a profile for the modes it allows; none listed allows any", () => {
    expect(allowsPushMode({ id: "a", name: "A", description: null, allowedPushModes: [] }, "dryRun")).toBe(true)
    expect(allowsPushMode({ id: "a", name: "A", description: null, allowedPushModes: ["dryRun"] }, "rebaseAutosquash")).toBe(false)
  })
})

const NO_PATHS = { paths: [] as string[], branchWide: false, pathsSource: "unknown" as const }

describe("formatReviewerGroup", () => {
  it("names the group and the available reviewers Patronus suggests", () => {
    expect(
      formatReviewerGroup({
        ...NO_PATHS,
        name: "Air Plugin Release Team",
        reviewers: [
          { fullName: "Alice", username: "alice", available: true, suggested: true },
          { fullName: "Bob", username: "bob", available: false, suggested: true },
          { fullName: "Carol", username: "carol", available: true, suggested: false },
        ],
      }),
    ).toBe("Air Plugin Release Team (suggested: Alice)")
    expect(formatReviewerGroup({ ...NO_PATHS, name: "Bazel QA", reviewers: [] })).toBe("Bazel QA")
  })
})

describe("formatReviewerGroupFiles", () => {
  it("counts the files of a group, all of them for a branch-wide one, nothing when nobody tells", () => {
    const group = { ...NO_PATHS, name: "Air Plugin", reviewers: [] }
    expect(formatReviewerGroupFiles({ ...group, paths: ["a.kt", "b.kt"], pathsSource: "patronus" })).toBe("2 files")
    expect(formatReviewerGroupFiles({ ...group, paths: ["a.kt"], pathsSource: "ownership" })).toBe("1 file")
    expect(formatReviewerGroupFiles({ ...group, branchWide: true, pathsSource: "patronus" })).toBe("all files")
    expect(formatReviewerGroupFiles(group)).toBeNull()
  })
})

describe("findMergedRobots", () => {
  it("finds a running or canceling robot that succeeded, not a dry run and not a robot first seen finished", () => {
    const previous = byId(robot({ id: "running" }), robot({ id: "canceling", status: "canceling" }), robot({ id: "dry", pushMode: "dryRun" }))
    const next = byId(
      robot({ id: "running", status: "success", finishTime: 200 }),
      robot({ id: "canceling", status: "success", finishTime: 200 }),
      robot({ id: "dry", pushMode: "dryRun", status: "success" }),
      robot({ id: "new", status: "success" }),
    )
    expect(findMergedRobots(previous, next).map(it => it.id)).toEqual(["running", "canceling"])
    // the next answer is no news again
    expect(findMergedRobots(next, next)).toEqual([])
    expect(findMergedRobots(previous, byId(robot({ id: "running", status: "failure" })))).toEqual([])
  })

  it("names the branch, else the subject of a robot of a temporary ref", () => {
    expect(formatMergedRobotTitle(robot({}))).toBe("IJPL-1 merged into master")
    expect(formatMergedRobotTitle(robot({ branchName: null }))).toBe("IJPL-1 fix the cache merged into master")
  })

  it("describes a robot by its branch and target, with the profile it was started with", () => {
    expect(describeRobot(robot({}))).toBe("IJPL-1 → master")
    expect(describeRobot(robot({}), "Large Scale Change")).toBe("IJPL-1 → master · profile Large Scale Change")
    expect(describeRobot(robot({ branchName: null }))).toBe("IJPL-1 fix the cache → master")
  })
})

describe("getSafePushSlot", () => {
  const input: SafePushSlotInput = { robot: undefined, localTip: "tip", remote: null, needsReview: false, failing: false, canOffer: true }

  it("offers \"Safe Push…\" without a robot, nothing where Patronus doesn't answer or the branch can't be pushed", () => {
    expect(getSafePushSlot(input)).toEqual({ kind: "start" })
    expect(getSafePushSlot({ ...input, canOffer: false })).toEqual({ kind: "none" })
  })

  it("is the live robot while it runs, also while it cancels", () => {
    expect(getSafePushSlot({ ...input, robot: robot({ status: "running" }) })).toMatchObject({ kind: "running", chip: { label: "Running" } })
    expect(getSafePushSlot({ ...input, robot: robot({ status: "canceling" }) })).toMatchObject({ kind: "running", chip: { label: "Canceling" } })
    expect(getSafePushSlot({ ...input, robot: robot({ status: "running" }), failing: true })).toMatchObject({ kind: "running", chip: { label: "Failing", color: "error" } })
    // a fix committed since keeps the robot in the slot, with the warning dot
    expect(getSafePushSlot({ ...input, robot: robot({ status: "running" }), localTip: "fixed", remote: unpushedOnRobot })).toMatchObject({
      kind: "running",
      chip: { outdated: true },
    })
  })

  it("shows a finished robot of the branch as it is, and whether a next step can be offered", () => {
    expect(getSafePushSlot({ ...input, robot: robot({ status: "failure" }) })).toMatchObject({ kind: "finished", chip: { label: "Failed" }, offered: true })
    expect(getSafePushSlot({ ...input, robot: robot({ status: "success" }) })).toMatchObject({ kind: "finished", chip: { label: "Merged" }, offered: true })
    // only rebuilt on a newer baseline: the same commits
    expect(getSafePushSlot({ ...input, robot: robot({ status: "failure" }), localTip: "rebuilt", remote: rebuiltOnRobot })).toMatchObject({ kind: "finished" })
  })

  it('offers "Safe Push…" for a branch changed since its robot, a merged one too', () => {
    expect(getSafePushSlot({ ...input, robot: robot({ status: "failure" }), localTip: "fixed", remote: unpushedOnRobot })).toEqual({ kind: "start" })
    expect(getSafePushSlot({ ...input, robot: robot({ status: "success" }), localTip: "next", remote: unpushedOnRobot })).toEqual({ kind: "start" })
    // nothing to start from where Patronus doesn't answer: the robot stays, with its dot
    expect(getSafePushSlot({ ...input, robot: robot({ status: "failure" }), localTip: "fixed", remote: unpushedOnRobot, canOffer: false })).toMatchObject({
      kind: "finished",
      chip: { outdated: true },
      offered: false,
    })
  })
})

describe("getRowRobot", () => {
  const input: SafePushSlotInput = { robot: undefined, localTip: "tip", remote: null, needsReview: false, failing: false, canOffer: true }
  const slotOf = (patch: Partial<SafePushSlotInput>) => getSafePushSlot({ ...input, ...patch })

  it("shows a running robot and a finished one of the branch as it is in the Status cell, the git status otherwise", () => {
    expect(getRowRobotChip(slotOf({ robot: robot({ status: "running" }) }))).toMatchObject({ label: "Running" })
    expect(getRowRobotChip(slotOf({ robot: robot({ status: "failure" }) }))).toMatchObject({ label: "Failed" })
    expect(getRowRobotChip(slotOf({}))).toBeNull()
    // changed since: the robot gives way to the status of the branch as it is
    expect(getRowRobotChip(slotOf({ robot: robot({ status: "failure" }), localTip: "fixed", remote: unpushedOnRobot }))).toBeNull()
  })

  it("offers the next step: Safe Push without a robot and after a dry run, Restart after a failure, the pull request for the review", () => {
    expect(getRowRobotStep(slotOf({}))).toBe("safePush")
    expect(getRowRobotStep(slotOf({ robot: robot({ status: "success", pushMode: "dryRun" }) }))).toBe("safePush")
    expect(getRowRobotStep(slotOf({ robot: robot({ status: "failure" }) }))).toBe("restart")
    expect(getRowRobotStep(slotOf({ robot: robot({ status: "canceled" }) }))).toBe("restart")
    // a restart before the approvals fails again: they count in the pull request
    expect(getRowRobotStep(slotOf({ robot: robot({ status: "failure" }), needsReview: true }))).toBe("pullRequest")
  })

  it("offers nothing while a robot runs, after a merge, and where nothing can be pushed", () => {
    expect(getRowRobotStep(slotOf({ robot: robot({ status: "running" }) }))).toBe("none")
    expect(getRowRobotStep(slotOf({ robot: robot({ status: "success" }) }))).toBe("none")
    expect(getRowRobotStep(slotOf({ robot: robot({ status: "failure" }), canOffer: false }))).toBe("none")
    expect(getRowRobotStep(slotOf({ canOffer: false }))).toBe("none")
  })
})
