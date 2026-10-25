import { describe, expect, it } from "vite-plus/test"
import type { PatronusProfile, PatronusRobot, PatronusStartOptions, PullRequest } from "@/lib/bindings"
import type { LandedBy, SafePushSlotInput } from "@/lib/patronus"
import {
  allowsPushMode,
  canCancelRobot,
  canRestartRobot,
  canStartRobot,
  canStartStackRobot,
  describeRobot,
  formatLandedReason,
  formatLandedWholeReason,
  formatLandedWith,
  formatLandingReason,
  formatRobotSubject,
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
  needsRobotAttention,
  withLastRobotProfile,
} from "@/lib/patronus"
import type { RemoteStatus } from "@/stores/branch-sync"
import { CREATE, forcePush, PUSH, testBranch, testRemoteStatus, UP_TO_DATE } from "@/stores/test-branch"

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

/** The remote branch is at the commit the robot took ("tip"), the local branch has a commit more */
const unpushedOnRobot: RemoteStatus = testRemoteStatus({ remoteTip: "tip", unpushedCommits: ["fix"], commitsAhead: 1, myCommitsAhead: 1, action: PUSH })
/** The remote branch is at the commit the robot took ("tip"), the local branch has the same commits on a newer baseline */
const rebuiltOnRobot: RemoteStatus = testRemoteStatus({ remoteTip: "tip", fastForward: false })

describe("the robot of the whole stack", () => {
  const wholeStack = robot({ id: "stack", branchName: null, stackRoot: "air", sourceRevision: "stack-tip" })

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
    const ahead = testRemoteStatus({ remoteTip: "stack-tip", unpushedCommits: ["new"], commitsAhead: 1, action: PUSH })
    expect(getRobotRestartAction(failed, stack, "new-stack-tip", ahead)).toBe("pushAndRestart")
    expect(getRobotRestartAction(failed, stack, "stack-tip", testRemoteStatus({ remoteTip: "stack-tip" }))).toBe("restart")
  })

  it("lands every branch of the stack, and waits for the robot of a branch of it, as that one waits for it", () => {
    const span = { branch: "air", robot: wholeStack, wholeStack: true }
    expect(formatLandingReason("auth", span)).toBe("A Safe Push of the whole air stack is running: it lands auth with it")
    expect(canStartRobot(testBranch("auth", "air"), undefined, null, span)).toEqual({ ok: false, reason: "A Safe Push of the whole air stack is running: it lands auth with it" })

    const landing = { branch: "auth", robot: robot({ id: "auth", branchName: "auth" }) }
    expect(canStartStackRobot(null, undefined, "stack-tip", null, landing)).toEqual({ ok: false, reason: "A Safe Push of auth is running: wait until it merges" })
    // a dry run lands nothing: it waits only for the stack branch
    expect(canStartStackRobot(null, undefined, "stack-tip", null, landing, true)).toEqual({ ok: true })
    expect(canStartStackRobot("The stack branch conflicts", undefined, "stack-tip", null, null)).toEqual({ ok: false, reason: "The stack branch conflicts" })
    expect(canStartStackRobot(null, wholeStack, "stack-tip", null, null)).toEqual({ ok: false, reason: "A Safe Push of the last commit is running" })
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
  const unpushed = testRemoteStatus({ remoteTip: started.sourceRevision, unpushedCommits: ["fix"], commitsAhead: 1, myCommitsAhead: 1, action: PUSH })
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
    // a reword since: the remote has the changes with the old message, which the robot would land
    const reworded = testRemoteStatus({ remoteTip: started.sourceRevision, commitsReworded: 1, fastForward: false, action: forcePush({ replaces: true }) })
    expect(getRobotRestartAction(failed, "IJPL-1", localTip, reworded)).toBe("pushAndRestart")
    // the parent got a new commit since the push of the branch: the remote branch is an older piece
    const olderPiece = testRemoteStatus({ remoteTip: started.sourceRevision, ancestorCommitsAhead: 1, fastForward: false, action: forcePush({ replaces: true }) })
    expect(getRobotRestartAction(failed, "IJPL-1", localTip, olderPiece)).toBe("pushAndRestart")
    expect(needsRobotAttention(failed, localTip, olderPiece)).toBe(false)
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

  it("cancels a running robot only", () => {
    expect(canCancelRobot(robot({ status: "running" }))).toBe(true)
    expect(canCancelRobot(robot({ status: "canceling" }))).toBe(false)
  })

  it("says why a held branch offers no push: its own Safe Push, one that lands it with the commits it counts; a dry run or a cancel holds nothing", () => {
    const running = robot({ status: "running" })
    expect(isHoldingRobot(running)).toBe(true)
    expect(isHoldingRobot(robot({ status: "running", pushMode: "dryRun" }))).toBe(false)
    expect(isHoldingRobot(robot({ status: "canceling" }))).toBe(false)
    expect(isHoldingRobot(robot({ status: "failure" }))).toBe(false)
    expect(formatPushHoldReason("IJPL-1", { kind: "own", branch: "IJPL-1", robot: running })).toBe(
      "A Safe Push of IJPL-1 is running: wait until it merges, or restart it on the current branch",
    )
    const oauth = robot({ id: "oauth", branchName: "oauth", status: "running" })
    expect(formatPushHoldReason("auth", { kind: "span", branch: "oauth", robot: oauth })).toBe("A Safe Push of oauth is running: it lands auth with it")
    // once the number of its commits the robot lands is told, the reason names it, as `bd` does
    expect(formatPushHoldReason("auth", { kind: "span", branch: "oauth", robot: oauth, commits: 2 })).toBe("A Safe Push of oauth is running: it lands 2 commits of auth with it")
    expect(formatLandingReason("auth", { branch: "oauth", robot: oauth, commits: 1 })).toBe("A Safe Push of oauth is running: it lands 1 commit of auth with it")
  })

  it("says why a branch a merged Safe Push landed offers no push: who landed it, its own robot as the merge", () => {
    const oauth = robot({ id: "oauth", branchName: "oauth", status: "success", finishTime: 200 })
    expect(formatPushHoldReason("auth", { kind: "landed", branch: "oauth", robot: oauth })).toBe("Landed in master with oauth: nothing of it is left to push")
    const whole = robot({ id: "whole", branchName: null, stackRoot: "air", status: "success" })
    expect(formatPushHoldReason("bazel", { kind: "landed", branch: "air", robot: whole, wholeStack: true })).toBe(
      "Landed in master with the whole air stack: nothing of it is left to push",
    )
    expect(formatPushHoldReason("oauth", { kind: "landed", branch: "oauth", robot: oauth })).toBe("Merged by Patronus: nothing of it is left to push")
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
    expect(getStartPush(UP_TO_DATE, false)).toEqual({ kind: "none" })
    expect(getStartPush(CREATE, false)).toEqual({ kind: "branch", action: "create" })
    expect(getStartPush(forcePush(), false)).toEqual({ kind: "branch", action: "force" })
    // the ancestors with work for the remote go first, as the push button pushes them
    expect(getStartPush(UP_TO_DATE, true)).toEqual({ kind: "stack" })
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

  it("makes the profile of the last robot the default, with whether it skips the review", () => {
    const profile = (id: string, skipsReview: boolean | null = null): PatronusProfile => ({ id, name: id, description: null, allowedPushModes: [], skipsReview })
    // the profiles of the target as the session read them: Patronus describes only the emergency ones
    const options: PatronusStartOptions = {
      targetBranch: "master",
      supported: true,
      profiles: [profile("lsc"), profile("optional")],
      emergencyProfiles: [profile("emergency", true)],
      defaultProfileId: null,
    }
    expect(withLastRobotProfile(options, null)).toBe(options)
    expect(withLastRobotProfile(options, { id: null, name: "Gone", skipsReview: true })).toBe(options)
    const last = withLastRobotProfile(options, { id: "lsc", name: "Large Scale Change", skipsReview: true })
    expect(last.defaultProfileId).toBe("lsc")
    expect(last.profiles).toEqual([profile("lsc", true), profile("optional")])
    // a profile no longer offered, or an emergency one, is no default
    expect(withLastRobotProfile(options, { id: "retired", name: "Retired", skipsReview: null }).defaultProfileId).toBeNull()
    expect(withLastRobotProfile(options, { id: "emergency", name: "emergency", skipsReview: true }).defaultProfileId).toBeNull()
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

describe("formatMergedRobotTitle", () => {
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

describe("formatLandedWholeReason", () => {
  it("names who landed a branch in its row and why it starts nothing: a branch built on it, the whole stack, its own robot with the work left", () => {
    const piece = robot({ id: "oauth", branchName: "oauth", status: "success", finishTime: 200 })
    const whole = robot({ id: "whole", branchName: null, stackRoot: "auth", status: "success", finishTime: 300 })
    expect(formatLandedWith({ branch: "oauth", robot: piece, after: [] })).toBe("Landed with oauth")
    expect(formatLandedWith({ branch: "oauth", robot: piece, after: ["new"] })).toBe("Landed with oauth · 1 commit after it")
    expect(formatLandedWith({ branch: "auth", robot: whole, wholeStack: true, after: [] })).toBe("Landed with the whole auth stack")
    // its own robot: its row shows the robot ("Merged"), only the work left is said
    expect(formatLandedWith({ branch: "oauth", robot: piece, own: true, after: [] })).toBeNull()
    expect(formatLandedWith({ branch: "oauth", robot: piece, own: true, after: ["new"] })).toBe("Landed · 1 commit after it")
    expect(formatLandedReason({ branch: "oauth", robot: piece })).toBe("Landed in master with oauth")
    expect(formatLandedReason({ branch: "oauth", robot: piece, own: true })).toBe("Merged by Patronus")
  })

  it("says why a group of branches has nothing left once every one of them landed whole: who landed them, in the order they merged", () => {
    const piece = robot({ id: "oauth", branchName: "oauth", status: "success", finishTime: 200 })
    const side = robot({ id: "session", branchName: "session", status: "success", finishTime: 100 })
    const whole = robot({ id: "whole", branchName: null, stackRoot: "auth", status: "success", finishTime: 300 })
    const by = (landing: PatronusRobot, branch: string, patch: Partial<LandedBy> = {}): LandedBy => ({ branch, robot: landing, after: [], ...patch })
    // session merged first, then oauth: each named once, in that order
    expect(formatLandedWholeReason([by(piece, "oauth"), by(piece, "oauth"), by(piece, "oauth", { own: true }), by(side, "session", { own: true })], "auth")).toBe(
      "Landed in master with session and oauth",
    )
    // the robot of the whole stack alone: its own merge
    expect(formatLandedWholeReason([by(whole, "auth", { wholeStack: true }), by(whole, "auth", { wholeStack: true })], "auth")).toBe("Merged by Patronus")
    // a branch left, one with commits after the landed ones, or no branch: nothing to say
    expect(formatLandedWholeReason([by(piece, "oauth"), undefined], "auth")).toBeNull()
    expect(formatLandedWholeReason([by(piece, "oauth"), by(piece, "oauth", { after: ["new"] })], "auth")).toBeNull()
    expect(formatLandedWholeReason([], "auth")).toBeNull()
  })
})
