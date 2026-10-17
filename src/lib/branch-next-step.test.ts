// @spec spec/branch/next-step.spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import type { MissingReviewGroup, Person, ReviewReadiness } from "@/lib/bindings"
import type { NextStepFacts, NextStepPullRequest } from "@/lib/branch-next-step"
import { getBranchNextStep } from "@/lib/branch-next-step"
import type { MenuItem } from "@/lib/menu"
import type { PatronusChipKind, PatronusChipState, SafePushSlot } from "@/lib/patronus"

const ilya: Person = { login: "ikorennoy", fullName: "Ilya Korennoy" }
const alice: Person = { login: "alice", fullName: null }
const junie: Person = { login: "junie-jetbrains", fullName: null }

function chip(kind: PatronusChipKind): PatronusChipState {
  return { kind, color: "neutral", label: kind, commit: "current", outdated: false }
}

function finished(kind: PatronusChipKind): SafePushSlot {
  return { kind: "finished", chip: chip(kind), offered: true }
}

function group(patch: Partial<MissingReviewGroup> = {}): MissingReviewGroup {
  return { name: "IJPL Code Platform", needed: 1, approvedBy: [], requested: [ilya], suggested: [ilya], changesRequestedBy: [], ...patch }
}

function missing(groups: MissingReviewGroup[], patch: Partial<Extract<ReviewReadiness, { kind: "missing" }>> = {}): ReviewReadiness {
  return { kind: "missing", groups, notCounted: [junie], reason: "approvalsMissing", ...patch }
}

const openPullRequest: NextStepPullRequest = { number: 1078, state: "open", isDraft: false, base: "master", intoTarget: true, mergedCurrent: false, baseIsFormerParent: false }

const restart: MenuItem = { id: "restart-robot", label: "Restart Safe Push", onSelect: () => {} }
const createPullRequest: MenuItem = { id: "create-pull-request", label: "Create Pull Request…", onSelect: () => {} }
const requestReview: MenuItem = { id: "request-review", label: "Request Review…", description: "Ask Ilya Korennoy to review #1078", onSelect: () => {} }

function facts(patch: Partial<NextStepFacts> = {}): NextStepFacts {
  return {
    branchName: "wsm-skip-wa-2",
    patronusAvailable: true,
    syncing: false,
    hasError: false,
    commitCount: 2,
    processing: false,
    blocked: false,
    startBlocker: null,
    dryRunStartBlocker: null,
    held: false,
    landedIn: null,
    robot: { kind: "start" },
    robotPullRequest: null,
    restart: null,
    pullRequestsAvailable: true,
    pullRequest: openPullRequest,
    pushed: true,
    target: "master",
    stackRoot: null,
    readiness: null,
    actions: {
      openSafePush: vi.fn(),
      openRootSafePush: vi.fn(),
      openPullRequest: vi.fn(),
      retarget: vi.fn(),
      createPullRequest,
      requestReview,
    },
    ...patch,
  }
}

function withReadiness(readiness: ReviewReadiness, patch: Partial<NextStepFacts> = {}): NextStepFacts {
  return facts({ readiness: { readiness, stale: false }, ...patch })
}

describe("getBranchNextStep", () => {
  it("has no slot in a repository that is not on Space, and keeps the reasons of the branch", () => {
    expect(getBranchNextStep(facts({ patronusAvailable: false, robot: { kind: "none" } }))).toMatchObject({ kind: "hidden", item: null, safePushBlocker: null })
    expect(getBranchNextStep(facts({ patronusAvailable: false, robot: { kind: "none" }, hasError: true })).safePushBlocker).toBe("The branch failed to sync")
  })

  it("row 1: offers nothing while the Status cell says why, and disables Safe Push and Dry Run with the reason", () => {
    const cases: [Partial<NextStepFacts>, string | null][] = [
      [{ syncing: true }, "Wait for the sync to finish"],
      [{ hasError: true, robot: { kind: "none" } }, "The branch failed to sync"],
      [{ commitCount: 0, robot: { kind: "none" } }, "No commits to push"],
      [{ blocked: true, startBlocker: "Blocked by auth: fix it first, then sync" }, "Blocked by auth: fix it first, then sync"],
      [{ held: true, startBlocker: "A Safe Push of oauth is running: it lands login with it" }, "A Safe Push of oauth is running: it lands login with it"],
      [{ landedIn: "origin/master" }, "Landed in origin/master"],
      [{ robot: finished("merged") }, "Merged by Patronus"],
      [{ pullRequest: { ...openPullRequest, state: "merged", mergedCurrent: true } }, "Pull request #1078 is merged"],
      // a running robot of an older commit: a start replaces it
      [{ robot: { kind: "running", chip: chip("running") } }, null],
    ]
    for (const [patch, reason] of cases) {
      const step = getBranchNextStep(facts(patch))
      expect(step, JSON.stringify(patch)).toMatchObject({ kind: "none", item: null, safePushBlocker: reason })
    }
    expect(getBranchNextStep(facts({ landedIn: "origin/master" })).dryRunBlocker).toBe("Landed in origin/master")
    expect(getBranchNextStep(facts({ robot: finished("merged") })).dryRunBlocker).toBe("Merged by Patronus")
  })

  it("row 1: a merged pull request of other commits (new work under the same name) blocks nothing", () => {
    const step = getBranchNextStep(facts({ pullRequest: { ...openPullRequest, state: "merged", mergedCurrent: false } }))
    expect(step).toMatchObject({ kind: "safePush", safePushBlocker: null })
  })

  it("row 2: restarts a failed or canceled robot, or offers Safe Push… where Patronus can't restart it", () => {
    expect(getBranchNextStep(facts({ robot: finished("failed"), restart }))).toMatchObject({ kind: "restart", item: restart, safePushBlocker: null })
    expect(getBranchNextStep(facts({ robot: finished("canceled"), restart }))).toMatchObject({ kind: "restart", item: restart })
    expect(getBranchNextStep(facts({ robot: finished("failed") }))).toMatchObject({ kind: "safePush", item: { id: "safe-push", disabled: false } })
  })

  it("row 3: restarts a robot that failed for approvals only from the pull request they now count in", () => {
    const approved: ReviewReadiness = { kind: "approved", groups: [{ name: "IJPL Code Platform", approvedBy: [ilya] }] }
    expect(getBranchNextStep(withReadiness(approved, { robot: finished("needsReview"), robotPullRequest: 1078, restart }))).toMatchObject({ kind: "restart", item: restart })
    // started through the API: its restart counts no pull request, a new Safe Push from it does
    expect(getBranchNextStep(withReadiness(approved, { robot: finished("needsReview"), robotPullRequest: null, restart }))).toMatchObject({ kind: "safePush" })
    // still missing: the wait, never a restart that fails again
    expect(getBranchNextStep(withReadiness(missing([group()]), { robot: finished("needsReview"), robotPullRequest: 1078, restart })).kind).toBe("awaiting")
  })

  it("row 3: keeps the pull request step while nobody can tell whether the approvals count", () => {
    const unknown: ReviewReadiness = { kind: "unknown", reason: "approvalsNotEvaluated" }
    for (const readiness of [null, { readiness: unknown, stale: false }]) {
      const step = getBranchNextStep(facts({ robot: finished("needsReview"), robotPullRequest: 1078, restart, readiness }))
      expect(step).toMatchObject({ kind: "pullRequest", item: { label: "Open Pull Request", kbds: ["o"] }, safePushBlocker: null })
    }
    expect(getBranchNextStep(facts({ robot: finished("needsReview"), pullRequest: null }))).toMatchObject({ kind: "pullRequest", item: createPullRequest })
  })

  it("row 4: a stacked branch lands root first, Safe Push… as the root", () => {
    const step = getBranchNextStep(facts({ stackRoot: { name: "auth", blocker: null } }))
    expect(step).toMatchObject({ kind: "stackRoot", item: { label: "Safe Push auth…", disabled: false }, safePushBlocker: null, needsReadiness: false })
    const waiting = getBranchNextStep(facts({ stackRoot: { name: "auth", blocker: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #7" } }))
    expect(waiting).toMatchObject({
      kind: "stackRoot",
      item: { disabled: true, description: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #7" },
      safePushBlocker: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #7",
    })
  })

  it("row 5: retargets an open pull request into the branch of a former parent, whose approvals don't count", () => {
    const step = getBranchNextStep(facts({ pullRequest: { ...openPullRequest, base: "user/virtual/auth", intoTarget: false, baseIsFormerParent: true } }))
    expect(step).toMatchObject({
      kind: "retarget",
      item: { label: "Retarget #1078 to master" },
      safePushBlocker: "Approvals in #1078 don't count: its base is user/virtual/auth",
      dryRunBlocker: null,
      needsReadiness: false,
      notice: { title: "Approvals in #1078 of wsm-skip-wa-2 don't count", description: "Its base is user/virtual/auth, not master" },
      retarget: { id: "retarget-pull-request", label: "Retarget #1078 to master" },
    })
  })

  it("row 5: opens a pull request into another base that may be meant, the retarget only in the menu", () => {
    const step = getBranchNextStep(facts({ pullRequest: { ...openPullRequest, base: "release-252", intoTarget: false } }))
    expect(step).toMatchObject({
      kind: "otherBase",
      tone: "quiet",
      item: { label: "Open Pull Request", kbds: ["o"], description: "Approvals in #1078 don't count: its base is release-252. Open Pull Request #1078" },
      safePushBlocker: "Approvals in #1078 don't count: its base is release-252",
      retarget: { label: "Retarget #1078 to master" },
    })
  })

  it("row 6: a draft is a quiet step that opens it; Patronus merges it once it is ready, a dry run runs", () => {
    const actions = facts().actions
    const step = getBranchNextStep(facts({ pullRequest: { ...openPullRequest, isDraft: true }, actions }))
    expect(step).toMatchObject({
      kind: "draft",
      tone: "quiet",
      item: { label: "Draft", kbds: ["o"] },
      safePushBlocker: "#1078 is a draft: Patronus merges it once it is ready for review",
      dryRunBlocker: null,
    })
    step.item!.onSelect()
    expect(actions.openPullRequest).toHaveBeenCalled()
  })

  it("row 7: missing approvals without an open pull request: Create Pull Request…, the code owners named", () => {
    for (const pullRequest of [null, { ...openPullRequest, state: "closed" as const }]) {
      const step = getBranchNextStep(withReadiness(missing([group({ requested: [] })], { reason: "noPullRequestContext", notCounted: [] }), { pullRequest }))
      expect(step).toMatchObject({
        kind: "createPullRequest",
        item: { id: "create-pull-request", description: "Code owners must approve in a pull request: IJPL Code Platform" },
        safePushBlocker: "Code owners must approve in a pull request: IJPL Code Platform",
        needsReadiness: true,
      })
    }
    // not pushed yet: the dialog needs the remote branch
    const unpushed = getBranchNextStep(
      withReadiness(missing([group()], { reason: "noPullRequestContext" }), { pullRequest: null, pushed: false, actions: { ...facts().actions, createPullRequest: null } }),
    )
    expect(unpushed.item).toMatchObject({ disabled: true, description: "Push first: a pull request needs the remote branch" })
  })

  it("row 8: changes requested by a member of a missing group: a destructive quiet step that opens the pull request", () => {
    const step = getBranchNextStep(withReadiness(missing([group({ changesRequestedBy: [ilya] })])))
    expect(step).toMatchObject({ kind: "changesRequested", tone: "destructive", item: { label: "Changes Requested" }, safePushBlocker: "Ilya Korennoy requested changes on #1078" })
  })

  it("row 8: changes requested in an approved group don't block, as Patronus doesn't", () => {
    // the readiness lists only the groups still short of approvals: an approved group's request is not there
    expect(getBranchNextStep(withReadiness({ kind: "approved", groups: [{ name: "IJPL Code Platform", approvedBy: [ilya] }] })).kind).toBe("safePush")
  })

  it("row 9: a missing group with nobody asked: Request Review…, disabled until the commits are pushed", () => {
    const readiness = missing([group(), group({ name: "Air Platform", requested: [] })])
    const step = getBranchNextStep(withReadiness(readiness))
    expect(step).toMatchObject({
      kind: "requestReview",
      item: {
        id: "request-review",
        label: "Request Review…",
        description: "Waits for the approval of Air Platform: nobody of it is requested on #1078. Ask Ilya Korennoy to review #1078",
      },
      safePushBlocker: "Waits for the approval of Air Platform: nobody of it is requested on #1078",
    })
    // `getRequestReviewItem` says why it waits for a push
    const pushFirst: MenuItem = { ...requestReview, disabled: true, description: "Push first, so the reviewers see the commits" }
    expect(getBranchNextStep(withReadiness(readiness, { actions: { ...facts().actions, requestReview: pushFirst } })).item).toEqual(pushFirst)
    const noEligible = getBranchNextStep(withReadiness(missing([group({ requested: [] })], { reason: "noEligibleReviewer" })))
    expect(noEligible.item).toMatchObject({ disabled: true, description: "Nobody of IJPL Code Platform can approve these changes" })
  })

  it("row 10: every missing group has someone asked: the quiet wait names them and opens the pull request", () => {
    const step = getBranchNextStep(withReadiness(missing([group()])))
    expect(step.notice).toEqual({ title: "Safe Push of wsm-skip-wa-2 waits for Ilya Korennoy", description: "IJPL Code Platform · requested on #1078" })
    expect(step).toMatchObject({
      kind: "awaiting",
      tone: "quiet",
      item: { label: "Awaiting Ilya Korennoy", kbds: ["o"], description: "Awaiting Ilya Korennoy · IJPL Code Platform\nOpen Pull Request #1078" },
      safePushBlocker: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #1078",
      dryRunBlocker: null,
    })
    const two = getBranchNextStep(withReadiness(missing([group(), group({ name: "Air Platform", requested: [alice] })])))
    expect(two.item).toMatchObject({ label: "Awaiting 2 Reviews" })
    expect(two.safePushBlocker).toBe("Waits for the approval of IJPL Code Platform and Air Platform: Ilya Korennoy and alice are requested on #1078")
    // a group that needs two approvals and has one
    const more = getBranchNextStep(withReadiness(missing([group({ needed: 1, approvedBy: [alice] })])))
    expect(more.item?.description).toBe("1 more approval · IJPL Code Platform\nOpen Pull Request #1078")
  })

  it("row 9: a group that needs two approvals and asks one person still needs a request", () => {
    const step = getBranchNextStep(withReadiness(missing([group({ needed: 2 })])))
    expect(step).toMatchObject({ kind: "requestReview", safePushBlocker: "Waits for the approval of IJPL Code Platform: fewer people are requested on #1078 than it needs" })
  })

  it("rows 8 to 10: the wait shows with work to push; the push slot pushes it", () => {
    const pushFirst: MenuItem = { ...requestReview, disabled: true, description: "Push first, so the reviewers see the commits" }
    expect(getBranchNextStep(withReadiness(missing([group()]), { actions: { ...facts().actions, requestReview: pushFirst } })).kind).toBe("awaiting")
  })

  it("row 11: Safe Push… for every other readiness, and before the first answer", () => {
    const readinesses: (ReviewReadiness | null)[] = [
      null,
      { kind: "notRequired" },
      { kind: "approved", groups: [] },
      { kind: "unknown", reason: "approvalsNotEvaluated" },
      { kind: "unavailable", reason: "error", message: "503" },
    ]
    for (const readiness of readinesses) {
      const step = getBranchNextStep(facts({ readiness: readiness === null ? null : { readiness, stale: false } }))
      expect(step).toMatchObject({ kind: "safePush", item: { label: "Safe Push…", kbds: ["s"], disabled: false }, safePushBlocker: null, needsReadiness: true })
    }
  })

  it("keeps a stale answer while the new one loads, so the row doesn't flicker", () => {
    expect(getBranchNextStep(facts({ readiness: { readiness: missing([group()]), stale: true } })).kind).toBe("awaiting")
  })

  it("treats the readiness as unavailable without the pull requests: never a duplicate Create Pull Request…", () => {
    const step = getBranchNextStep(withReadiness(missing([group()], { reason: "noPullRequestContext" }), { pullRequestsAvailable: false, pullRequest: null }))
    expect(step).toMatchObject({ kind: "safePush", needsReadiness: false })
  })

  it("asks for the readiness only of a pushed branch on GitHub", () => {
    expect(getBranchNextStep(facts({ pushed: false })).needsReadiness).toBe(false)
    expect(getBranchNextStep(facts({ pullRequestsAvailable: false })).needsReadiness).toBe(false)
  })

  it("disables the action of the slot while an inline action of the row runs, never the quiet steps", () => {
    expect(getBranchNextStep(facts({ processing: true }))).toMatchObject({ item: { disabled: true }, safePushBlocker: "Wait for the action on the branch to finish" })
    expect(getBranchNextStep(withReadiness(missing([group()]), { processing: true })).item?.disabled).toBeUndefined()
  })

  it("words the notice of S as a short title naming the branch, the reason below; a reason of the branch first", () => {
    expect(getBranchNextStep(facts()).notice).toBeNull()
    expect(getBranchNextStep(withReadiness(missing([group(), group({ name: "Air Platform", requested: [alice] })]))).notice?.title).toBe(
      "Safe Push of wsm-skip-wa-2 waits for 2 reviews",
    )
    expect(getBranchNextStep(facts({ landedIn: "origin/master" })).notice).toEqual({ title: "Safe Push of wsm-skip-wa-2 can't start", description: "Landed in origin/master" })
    // the robot of an ancestor holds the start before the wait
    const ancestor = "A Safe Push of login is running: wait until it merges"
    expect(getBranchNextStep(withReadiness(missing([group()]), { startBlocker: ancestor })).notice).toEqual({
      title: "Safe Push of wsm-skip-wa-2 can't start",
      description: ancestor,
    })
    expect(getBranchNextStep(facts({ pullRequest: { ...openPullRequest, isDraft: true } })).notice).toEqual({
      title: "#1078 of wsm-skip-wa-2 is a draft",
      description: "Patronus merges it once it is ready for review",
    })
  })

  it("disables the Safe Push of a branch a robot of its ancestor lands, its slot says why", () => {
    const step = getBranchNextStep(facts({ startBlocker: "A Safe Push of login is running: wait until it merges" }))
    expect(step).toMatchObject({ kind: "safePush", item: { disabled: true, description: "A Safe Push of login is running: wait until it merges" } })
    // a dry run may run meanwhile: it lands nothing
    expect(step.dryRunBlocker).toBeNull()
  })
})
