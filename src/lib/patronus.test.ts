import { describe, expect, it } from "vite-plus/test"
import type { PatronusProfile, PatronusRobot, PatronusStartOptions, PullRequest } from "@/lib/bindings"
import type { SafePushSlotInput } from "@/lib/patronus"
import {
  allowsPushMode,
  canCancelRobot,
  canRestartRobot,
  canStartRobot,
  canStartStackRobot,
  describeRobot,
  findBranchRobot,
  findLandedBy,
  findLandingAncestor,
  findLandingDescendants,
  findLandingMember,
  findStackRobot,
  formatLandedReason,
  formatLandedWholeReason,
  formatLandedWith,
  formatLandingReason,
  formatRobotSubject,
  findMergedRobots,
  findPushHold,
  formatMergedRobotTitle,
  formatPatronusAvailabilityHint,
  formatSafePushLoginCommand,
  formatPushHoldReason,
  formatReviewerGroup,
  formatReviewerGroupFiles,
  formatRobotCommitNotice,
  getBypassProfiles,
  getPatronusChipState,
  getPullRequestStart,
  getRobotRestartAction,
  getRowRobotChip,
  getSafePushSlot,
  getStartPush,
  isHoldingRobot,
  isStartBlocked,
  landingKey,
  landingRequestOf,
  needsRobotAttention,
} from "@/lib/patronus"
import { testLanding } from "@/lib/test-robot"
import type { Branch, RemoteStatus, StackBranch } from "@/stores/branch-sync"
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

describe("the robot of the whole stack", () => {
  const wholeStack = robot({ id: "stack", branchName: null, stackRoot: "air", sourceRevision: "stack-tip" })

  it("belongs to the root of its stack by its stack branch, or as a robot of a temporary ref by the tips of the stack branch", () => {
    expect(findStackRobot(byId(wholeStack), "air", "other", null)).toBe(wholeStack)
    expect(findStackRobot(byId(wholeStack), "auth", "stack-tip", null)).toBeUndefined()
    const cli = robot({ id: "cli", branchName: null, sourceRevision: "pushed-stack" })
    expect(findStackRobot(byId(cli), "air", "stack-tip", "pushed-stack")).toBe(cli)
    // a robot of a branch is never the whole stack's, not the root's either
    expect(findStackRobot(byId(robot({ branchName: "air", sourceRevision: "stack-tip" })), "air", "local", null)).toBeUndefined()
  })

  it("is named the whole stack, at the start of a sentence too", () => {
    expect(formatRobotSubject(wholeStack)).toBe("the whole air stack")
    expect(formatMergedRobotTitle(wholeStack)).toBe("Whole air stack merged into master")
    expect(describeRobot(wholeStack, "Large Scale Change")).toBe("Whole air stack → master · profile Large Scale Change")
    expect(formatRobotSubject(robot({ branchName: null, sourceRevision: "x" }))).toBe("IJPL-1 fix the cache")
  })

  it("restarts on its stack branch, pushed first while the stack branch has work the remote lacks", () => {
    const failed = { ...wholeStack, status: "failure" as const }
    const stack = { kind: "stack", root: "air" } as const
    expect(canRestartRobot(failed, stack, "stack-tip")).toBe(true)
    expect(canRestartRobot(failed, { kind: "stack", root: "auth" }, "stack-tip")).toBe(false)
    expect(canRestartRobot(failed, "air", "stack-tip")).toBe(false)
    const ahead = testRemoteStatus({ remoteTip: "stack-tip", unpushedCommits: ["new"], commitsAhead: 1 })
    expect(getRobotRestartAction(failed, stack, "new-stack-tip", ahead)).toBe("pushAndRestart")
    expect(getRobotRestartAction(failed, stack, "stack-tip", testRemoteStatus({ remoteTip: "stack-tip" }))).toBe("restart")
  })

  it("lands every branch of the stack, and waits for the robot of a branch of it, as that one waits for it", () => {
    const span = { branch: "air", robot: wholeStack, wholeStack: true }
    expect(formatLandingReason("auth", span)).toBe("A Safe Push of the whole air stack is running: it lands auth with it")
    expect(canStartRobot(testBranch("auth", "air"), undefined, null, span)).toEqual({ ok: false, reason: "A Safe Push of the whole air stack is running: it lands auth with it" })
    expect(findPushHold("auth", undefined, span)).toEqual({ kind: "span", branch: "air", robot: wholeStack, wholeStack: true })

    const auth = robot({ id: "auth", branchName: "auth" })
    const landing = findLandingMember(["air", "auth", "side"], name => (name === "auth" ? auth : undefined))
    expect(landing).toEqual({ branch: "auth", robot: auth })
    expect(findLandingMember(["air", "auth"], () => robot({ pushMode: "dryRun" }))).toBeNull()
    expect(canStartStackRobot(null, undefined, "stack-tip", null, landing)).toEqual({ ok: false, reason: "A Safe Push of auth is running: wait until it merges" })
    // a dry run lands nothing: it waits only for the stack branch
    expect(canStartStackRobot(null, undefined, "stack-tip", null, landing, true)).toEqual({ ok: true })
    expect(canStartStackRobot("The stack branch conflicts", undefined, "stack-tip", null, null)).toEqual({ ok: false, reason: "The stack branch conflicts" })
    expect(canStartStackRobot(null, wholeStack, "stack-tip", null, null)).toEqual({ ok: false, reason: "A Safe Push of the last commit is running" })
  })
})

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

  it("never takes the robot of the whole stack, which runs on its stack branch, for a branch's", () => {
    const wholeStack = robot({ id: "stack", branchName: null, stackRoot: "IJPL-1", sourceRevision: "tip" })
    expect(findBranchRobot(byId(wholeStack), "IJPL-1", "tip", "tip")).toBeUndefined()
    const onBranch = robot({ id: "branch", status: "failure" })
    expect(findBranchRobot(byId(wholeStack, onBranch), "IJPL-1", "tip", null)).toBe(onBranch)
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
    expect(notice(failed, unpushed)).toBe(`${changed} Restart Safe Push to test the current branch.`)
    // a passed dry run restarts only on a newer pushed commit: the push makes one
    const passed = robot({ status: "success", pushMode: "dryRun", ...started })
    expect(getRobotRestartAction(passed, "IJPL-1", localTip, unpushed)).toBe("pushAndRestart")
    expect(getRobotRestartAction(passed, "IJPL-1", localTip, testRemoteStatus({ remoteTip: started.sourceRevision }))).toBeNull()
  })

  it("pushes and restarts a running robot of a branch changed since, asks to cancel one the remote moved past, and offers nothing for a robot of a temporary ref", () => {
    const running = robot(started)
    // the restart cancels the running robot first: the one push into a branch whose Safe Push runs
    expect(getRobotRestartAction(running, "IJPL-1", localTip, unpushed)).toBe("pushAndRestart")
    expect(notice(running, unpushed)).toBe(`${changed} Restart Safe Push to test the current branch.`)
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

describe("formatSafePushLoginCommand", () => {
  it("names the command of the login hint, into the repository, quoted for the shell of the platform", () => {
    expect(formatSafePushLoginCommand("/Users/me/idea", false)).toBe("cd /Users/me/idea && ./safePush.cmd")
    expect(formatSafePushLoginCommand("/Users/me/my idea", false)).toBe("cd '/Users/me/my idea' && ./safePush.cmd")
    expect(formatSafePushLoginCommand("C:\\work\\idea", true)).toBe("cd 'C:\\work\\idea'; .\\safePush.cmd")
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
    // a restart has no pull request to count the approvals in
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
    expect(formatPushHoldReason("IJPL-1", own!)).toBe("A Safe Push of IJPL-1 is running: wait until it merges, or restart it on the current branch")
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

  it("holds the push of a branch a merged Safe Push landed, before a robot of its own", () => {
    const oauth = robot({ id: "oauth", branchName: "oauth", status: "success", finishTime: 200 })
    const landed = findPushHold(
      "auth",
      robot({ status: "running", branchName: "auth" }),
      { branch: "login", robot: robot({ id: "login", branchName: "login" }) },
      { branch: "oauth", robot: oauth, after: [] },
    )
    expect(landed).toEqual({ kind: "landed", branch: "oauth", robot: oauth })
    expect(formatPushHoldReason("auth", landed!)).toBe("Landed in master with oauth: nothing of it is left to push")
    const whole = robot({ id: "whole", branchName: null, stackRoot: "air", status: "success" })
    const member = findPushHold("bazel", undefined, null, { branch: "air", robot: whole, wholeStack: true, after: [] })
    expect(member).toEqual({ kind: "landed", branch: "air", robot: whole, wholeStack: true })
    expect(formatPushHoldReason("bazel", member!)).toBe("Landed in master with the whole air stack: nothing of it is left to push")
    // its own merged robot landed all of it: its push is held too
    const own = findPushHold("oauth", oauth, null, { branch: "oauth", robot: oauth, own: true, after: [] })
    expect(own).toEqual({ kind: "landed", branch: "oauth", robot: oauth })
    expect(formatPushHoldReason("oauth", own!)).toBe("Merged by Patronus: nothing of it is left to push")
    // a commit added after the landed ones is work for the remote: no hold
    expect(findPushHold("auth", undefined, null, { branch: "oauth", robot: oauth, after: ["auth-2"] })).toBeNull()
  })

  it("blocks a start while a robot runs on the commit the branch is at", () => {
    expect(isStartBlocked(robot({ status: "running" }), "tip", null)).toBe(true)
    expect(isStartBlocked(robot({ status: "running" }), "fixed", null)).toBe(false)
    expect(isStartBlocked(robot({ status: "failure" }), "tip", null)).toBe(false)
    expect(isStartBlocked(undefined, "tip", null)).toBe(false)
  })

  it("blocks a start while a robot runs on the remote branch that has the same commits, rebuilt on a newer baseline", () => {
    // the robot took the remote branch; the sync rebuilt the same commits on a newer baseline since
    expect(isStartBlocked(robot({ status: "running" }), "rebuilt", rebuiltOnRobot)).toBe(true)
    // a fix the remote lacks: a new robot takes it
    expect(isStartBlocked(robot({ status: "running" }), "fixed", unpushedOnRobot)).toBe(false)
    // pushed since the robot started
    expect(isStartBlocked(robot({ status: "running" }), "fixed", testRemoteStatus({ remoteTip: "fixed" }))).toBe(false)
    expect(isStartBlocked(robot({ status: "failure" }), "rebuilt", testRemoteStatus({ remoteTip: "tip" }))).toBe(false)
  })

  it("pushes before a start only what the remote lacks by content, and waits for the remote status", () => {
    expect(getStartPush(null, false)).toEqual({ kind: "unknown" })
    expect(getStartPush(null, true)).toEqual({ kind: "unknown" })
    // the remote branch has the commits, also under other hashes: Patronus rebases it, nothing is pushed
    expect(getStartPush("upToDate", false)).toEqual({ kind: "none" })
    expect(getStartPush("create", false)).toEqual({ kind: "branch", action: "create" })
    expect(getStartPush("force", false)).toEqual({ kind: "branch", action: "force" })
    // the ancestors with work for the remote go first, as the push button pushes them
    expect(getStartPush("upToDate", true)).toEqual({ kind: "stack" })
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
    // on the remote branch, which has the same commits: the sync rebuilt them on a newer baseline since
    const rebuilt = { ...branch, remoteStatus: testRemoteStatus({ remoteTip: "pushed", fastForward: false }) }
    expect(canStartRobot(rebuilt, robot({ status: "running", sourceRevision: "pushed" }))).toEqual({ ok: false, reason: "A Safe Push of the last commit is running" })

    const blocked = testBranch("login", "auth", { status: "Blocked" })
    expect(canStartRobot(blocked, undefined)).toEqual({ ok: false, reason: "Blocked by auth: fix it first, then sync" })
  })

  it("offers a profile for the modes it allows; none listed allows any", () => {
    expect(allowsPushMode({ id: "a", name: "A", description: null, allowedPushModes: [], skipsReview: null }, "dryRun")).toBe(true)
    expect(allowsPushMode({ id: "a", name: "A", description: null, allowedPushModes: ["dryRun"], skipsReview: null }, "rebaseAutosquash")).toBe(false)
  })

  it("offers a Safe Push without review the profiles known to skip it, the unknown ones and the emergency ones apart", () => {
    const profile = (id: string, skipsReview: boolean | null, allowedPushModes: PatronusProfile["allowedPushModes"] = []): PatronusProfile => ({
      id,
      name: id,
      description: null,
      allowedPushModes,
      skipsReview,
    })
    const options: PatronusStartOptions = {
      targetBranch: "master",
      supported: true,
      // the last robot ran with lsc, whose options said it skips the review; optional checks it
      profiles: [profile("lsc", true), profile("optional", false), profile("external", null), profile("dry-only", null, ["dryRun"])],
      emergencyProfiles: [profile("emergency", true)],
      defaultProfileId: "lsc",
    }
    const ids = (profiles: PatronusProfile[]) => profiles.map(it => it.id)
    const offered = getBypassProfiles(options, "rebaseAutosquash")
    expect([ids(offered.skipping), ids(offered.other), ids(offered.emergency), offered.defaultProfileId]).toEqual([["lsc"], ["external"], ["emergency"], "lsc"])
    // the last robot's profile checks the review: none is chosen for the user
    expect(getBypassProfiles({ ...options, defaultProfileId: "optional" }, "rebaseAutosquash").defaultProfileId).toBeNull()
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
          { fullName: "Alice", username: "alice", githubLogin: null, available: true, suggested: true },
          { fullName: "Bob", username: "bob", githubLogin: null, available: false, suggested: true },
          { fullName: "Carol", username: "carol", githubLogin: null, available: true, suggested: false },
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
    // the merged robot landed all of the branch as it is now, by content: a rebuild since gave it new hashes, not new work
    expect(getSafePushSlot({ ...input, robot: robot({ status: "success" }), localTip: "rebuilt", remote: unpushedOnRobot, landed: true })).toMatchObject({
      kind: "finished",
      chip: { label: "Merged" },
    })
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
})

describe("findLandedBy", () => {
  /** A branch of the stack whose newest commit is `<name>-tip`, older ones `<name>-1`, … */
  function branch(name: string, parent: string | null, patch: Partial<Branch> = {}): Branch {
    return testBranch(name, parent, { commits: [{ hash: `${name}-tip` } as Branch["commits"][number]], ...patch })
  }
  const auth = branch("auth", null)
  const login = branch("login", "auth")
  const oauth = branch("oauth", "login")
  const session = branch("session", "auth")
  const stack = { root: "auth", members: ["auth", "login", "oauth", "session"], tip: "stack-tip", remoteStatus: null } as unknown as StackBranch

  function landedBy(robots: PatronusRobot[], branches: Branch[] = [auth, login, oauth, session], stacks: Record<string, StackBranch> = { auth: stack }) {
    const byName = Object.fromEntries(branches.map(it => [it.name, it]))
    const landed = findLandedBy(byId(...robots), { branchOf: name => byName[name], stackOf: root => stacks[root] })
    return Object.fromEntries([...landed].map(([name, by]) => [name, formatLandedWith(by)]))
  }

  it("knows the ancestors a merged piece carried and every branch a merged whole stack landed, and names who landed them", () => {
    const piece = robot({ id: "oauth", branchName: "oauth", sourceRevision: "oauth-tip", status: "success", finishTime: 200 })
    // its own branch landed too, its row shows its own robot ("Merged"): no label
    expect(landedBy([piece])).toEqual({ auth: "Landed with oauth", login: "Landed with oauth", oauth: null })
    const whole = robot({ id: "whole", branchName: null, stackRoot: "auth", sourceRevision: "stack-tip", status: "success", finishTime: 300 })
    expect(landedBy([whole])).toEqual({
      auth: "Landed with the whole auth stack",
      login: "Landed with the whole auth stack",
      oauth: "Landed with the whole auth stack",
      session: "Landed with the whole auth stack",
    })
    // the first merge landed it
    expect(landedBy([whole, piece])).toMatchObject({ auth: "Landed with oauth", oauth: null, session: "Landed with the whole auth stack" })
    const [first] = findLandedBy(byId(piece), { branchOf: name => ({ auth, login, oauth })[name], stackOf: () => undefined }).values()
    expect(formatLandedReason(first!)).toBe("Landed in master with oauth")
  })

  it("keeps the ancestors landed while the branch has the commit the robot took: rebuilt on a newer baseline, or with new commits on it", () => {
    const piece = robot({ id: "oauth", branchName: "oauth", sourceRevision: "oauth-pushed", status: "success", finishTime: 200 })
    // the remote branch is at the robot's commit and has what the branch has: a sync rebuilt the same commits since
    const rebuilt = branch("oauth", "login", { remoteStatus: testRemoteStatus({ remoteTip: "oauth-pushed" }) })
    expect(landedBy([piece], [auth, login, rebuilt])).toEqual({ auth: "Landed with oauth", login: "Landed with oauth", oauth: null })
    // a commit on top of the one the robot took: the copies below it are the ones it landed, the new one is left
    const grown = branch("oauth", "login", {
      commits: [
        { hash: "oauth-new", originalHash: "new" },
        { hash: "oauth-pushed", originalHash: "pushed" },
      ] as unknown as Branch["commits"],
    })
    expect(landedBy([piece], [auth, login, grown])).toEqual({ auth: "Landed with oauth", login: "Landed with oauth", oauth: "Landed · 1 commit after it" })
  })

  it("decides by content once a change below the robot's commit gave the branches new copies: a commit added after the landed ones is the only work left", () => {
    const piece = robot({ id: "oauth", branchName: "oauth", sourceRevision: "oauth-old", status: "success", finishTime: 200 })
    const branches = [auth, login, oauth]
    const byName = Object.fromEntries(branches.map(it => [it.name, it]))
    const source = { branchOf: (name: string) => byName[name], stackOf: () => undefined }
    // the hashes don't tell: the content is asked, for the branches as the table has them
    const request = landingRequestOf(piece, source)
    expect(request).toEqual({ robotId: "oauth", branchName: "oauth", stackRoot: null, sourceRevision: "oauth-old", branches: ["auth", "login", "oauth"], tip: null })
    expect(findLandedBy(byId(piece), source).size).toBe(0)
    // auth got a commit after the landed one, which rebuilt login and oauth with the same changes
    const landing = testLanding("oauth", { auth: ["auth-2"], login: [], oauth: [] })
    const answered = { ...source, landings: { [landingKey(request!, source)]: landing } }
    const landed = findLandedBy(byId(piece), answered)
    expect(Object.fromEntries([...landed].map(([name, by]) => [name, formatLandedWith(by)]))).toEqual({
      auth: "Landed with oauth · 1 commit after it",
      login: "Landed with oauth",
      oauth: null,
    })
    expect(landed.get("oauth")).toMatchObject({ own: true, after: [] })
    // an answer for another state of the branches is not this one's
    const amended: Record<string, Branch> = { ...byName, auth: branch("auth", null, { commits: [{ hash: "auth-amended" } as Branch["commits"][number]] }) }
    expect(findLandedBy(byId(piece), { ...answered, branchOf: name => amended[name] }).size).toBe(0)
    // the hashes tell: nothing is asked
    expect(landingRequestOf({ ...piece, sourceRevision: "oauth-tip" }, source)).toBeNull()
  })

  it("takes the landing that left the fewest commits after a branch, of equals the first merge", () => {
    const piece = robot({ id: "oauth", branchName: "oauth", sourceRevision: "oauth-old", status: "success", finishTime: 100 })
    const own = robot({ id: "auth", branchName: "auth", sourceRevision: "auth-tip", status: "success", finishTime: 300 })
    const byName: Record<string, Branch> = { auth, login, oauth }
    const source = { branchOf: (name: string) => byName[name], stackOf: () => undefined }
    const request = landingRequestOf(piece, source)!
    const landing = {
      robotId: "oauth",
      branches: [
        { branch: "auth", landed: ["auth-1"], after: ["auth-2"] },
        { branch: "login", landed: ["login-1"], after: [] },
      ],
    }
    const landed = findLandedBy(byId(piece, own), { ...source, landings: { [landingKey(request, source)]: landing } })
    // the later Safe Push of auth landed all of it, the earlier one of oauth only its older commit
    expect(landed.get("auth")).toMatchObject({ branch: "auth", own: true, after: [] })
    expect(landed.get("login")).toMatchObject({ branch: "oauth", after: [] })
  })

  it("says why a group of branches has nothing left once every one of them landed whole: who landed them, in the order they merged", () => {
    const piece = robot({ id: "oauth", branchName: "oauth", sourceRevision: "oauth-tip", status: "success", finishTime: 200 })
    const side = robot({ id: "session", branchName: "session", sourceRevision: "session-tip", status: "success", finishTime: 100 })
    const whole = robot({ id: "whole", branchName: null, stackRoot: "auth", sourceRevision: "stack-tip", status: "success", finishTime: 300 })
    const source = { branchOf: (name: string) => ({ auth, login, oauth, session })[name], stackOf: () => stack }
    const reasonOf = (robots: PatronusRobot[], names = ["auth", "login", "oauth", "session"]) => {
      const landed = findLandedBy(byId(...robots), source)
      return formatLandedWholeReason(
        names.map(name => landed.get(name)),
        "auth",
      )
    }
    // session merged first, then oauth: each named once, in that order
    expect(reasonOf([piece, side])).toBe("Landed in master with session and oauth")
    // the robot of the whole stack alone: its own merge
    expect(reasonOf([whole])).toBe("Merged by Patronus")
    expect(reasonOf([whole], ["login", "oauth"])).toBe("Merged by Patronus")
    // a branch left, one with commits after the landed ones, or no branch: nothing to say
    expect(reasonOf([piece])).toBeNull()
    const grown = branch("oauth", "login", {
      commits: [
        { hash: "oauth-new", originalHash: "new" },
        { hash: "oauth-tip", originalHash: "tip" },
      ] as unknown as Branch["commits"],
    })
    const landed = findLandedBy(byId(piece, side), { ...source, branchOf: name => ({ auth, login, oauth: grown, session })[name] })
    expect(formatLandedWholeReason([landed.get("auth"), landed.get("oauth")], "auth")).toBeNull()
    expect(formatLandedWholeReason([], "auth")).toBeNull()
  })

  it("lands nothing for a dry run, a robot that didn't merge, one of another state of the branch, or of a temporary ref", () => {
    const merged = { status: "success" as const, finishTime: 200 }
    expect(landedBy([robot({ id: "dry", branchName: "oauth", sourceRevision: "oauth-tip", pushMode: "dryRun", ...merged })])).toEqual({})
    expect(landedBy([robot({ id: "running", branchName: "oauth", sourceRevision: "oauth-tip" })])).toEqual({})
    expect(landedBy([robot({ id: "failed", branchName: "oauth", sourceRevision: "oauth-tip", status: "failure" })])).toEqual({})
    // an amend since: the robot landed other commits than the table has
    expect(landedBy([robot({ id: "older", branchName: "oauth", sourceRevision: "oauth-older", ...merged })])).toEqual({})
    expect(landedBy([robot({ id: "stale", branchName: null, stackRoot: "auth", sourceRevision: "stack-older", ...merged })])).toEqual({})
    expect(landedBy([robot({ id: "ref", branchName: null, sourceRevision: "oauth-tip", ...merged })])).toEqual({})
  })
})
