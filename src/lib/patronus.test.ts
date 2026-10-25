import { describe, expect, it } from "vite-plus/test"
import type { PatronusRobot } from "@/lib/bindings"
import type { LandedBy, RobotRestartAction, SafePushSlotInput } from "@/lib/patronus"
import {
  canCancelRobot,
  describeRobot,
  formatLandedReason,
  formatLandedWholeReason,
  formatLandedWith,
  formatRobotSubject,
  formatMergedRobotTitle,
  formatPatronusAvailabilityHint,
  formatSafePushLoginCommand,
  formatReviewerGroup,
  formatReviewerGroupFiles,
  formatRobotCommitNotice,
  getPatronusChipState,
  getRowRobotChip,
  getSafePushSlot,
  isHoldingRobot,
  needsRobotAttention,
} from "@/lib/patronus"
import type { RemoteStatus } from "@/stores/branch-sync"
import { forcePush, PUSH, testRemoteStatus } from "@/stores/test-branch"

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

  /** The notice of `subject` with the restart Rust decides for it (`RobotRestart.action` of the view) */
  function notice(subject: PatronusRobot, remote: RemoteStatus, action: RobotRestartAction): string | null {
    return formatRobotCommitNotice(subject, getPatronusChipState(subject, localTip, remote), localTip, action)
  }

  it("names the change and the restart that tests the branch as it is", () => {
    const failed = robot({ status: "failure", ...started })
    expect(notice(failed, pushedSince, "restart")).toBe(`${changed} Restart Safe Push to test the current branch.`)
    // the restart pushes first: it has the same name
    expect(notice(failed, unpushed, "pushAndRestart")).toBe(`${changed} Restart Safe Push to test the current branch.`)
    // the parent got a new commit since the push of the branch: the remote branch is an older piece
    const olderPiece = testRemoteStatus({ remoteTip: started.sourceRevision, ancestorCommitsAhead: 1, fastForward: false, action: forcePush({ replaces: true }) })
    expect(needsRobotAttention(failed, localTip, olderPiece)).toBe(false)
  })

  it("asks to cancel a running robot the remote moved past, and offers nothing for a robot of a temporary ref", () => {
    const running = robot(started)
    expect(notice(running, unpushed, "pushAndRestart")).toBe(`${changed} Restart Safe Push to test the current branch.`)
    expect(notice(running, pushedSince, null)).toBe(`${changed} Cancel Safe Push, then Restart Safe Push to test the current branch.`)
    // a restart would test its old commit
    expect(notice(robot({ status: "failure", branchName: null, ...started }), unpushed, null)).toBe(changed)
    expect(notice(robot({ branchName: null, ...started }), unpushed, null)).toBe(changed)
  })

  it("says quietly that a robot started on an earlier build of the same commits, nothing for the branch as it is", () => {
    expect(notice(robot(started), testRemoteStatus({ remoteTip: started.sourceRevision }), null)).toBe("Started on an earlier build of the same commits.")
    expect(notice(robot({ sourceRevision: localTip }), unpushed, null)).toBeNull()
    expect(notice(robot({ sourceRevision: null }), unpushed, null)).toBeNull()
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
  it("cancels a running robot only", () => {
    expect(canCancelRobot(robot({ status: "running" }))).toBe(true)
    expect(canCancelRobot(robot({ status: "canceling" }))).toBe(false)
  })

  it("holds pushes with a running Safe Push only: a dry run or a cancel holds nothing", () => {
    expect(isHoldingRobot(robot({ status: "running" }))).toBe(true)
    expect(isHoldingRobot(robot({ status: "running", pushMode: "dryRun" }))).toBe(false)
    expect(isHoldingRobot(robot({ status: "canceling" }))).toBe(false)
    expect(isHoldingRobot(robot({ status: "failure" }))).toBe(false)
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
