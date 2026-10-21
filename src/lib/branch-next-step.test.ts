// @spec spec/branch/next-step.spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import type { ReviewReadiness } from "@/lib/bindings"
import type { NextStepFacts } from "@/lib/branch-next-step"
import { getBranchNextStep } from "@/lib/branch-next-step"
import type { MenuItem } from "@/lib/menu"
import { FROM_MENU } from "@/lib/menu"
import { alice, chip, createPullRequest, facts, finished, group, ilya, missing, openPullRequest, requestReview, restart, withReadiness } from "@/lib/test-next-step-facts"
import { testRobot } from "@/lib/test-robot"

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

  it("row 1: a branch a merged Safe Push of a branch built on it or of the whole stack landed offers nothing, its Safe Push and Dry Run say who landed it", () => {
    for (const landedWith of ["Landed in master with oauth", "Landed in master with the whole air stack"]) {
      // its own approvals are missing, its pull request open: all of it landed already
      const step = getBranchNextStep(withReadiness(missing([group()]), { landedWith }))
      expect(step).toMatchObject({ kind: "none", item: null, safePushBlocker: landedWith, dryRunBlocker: landedWith, needsReadiness: false })
      expect(step.notice).toEqual({ title: "Safe Push of wsm-skip-wa-2 can't start", description: landedWith })
    }
    // the fetched baseline says it once it has the commits
    expect(getBranchNextStep(facts({ landedIn: "origin/master", landedWith: "Landed in master with oauth" })).safePushBlocker).toBe("Landed in origin/master")
  })

  it("row 1: a merged pull request of other commits (new work under the same name) blocks nothing", () => {
    const step = getBranchNextStep(facts({ pullRequest: { ...openPullRequest, state: "merged", mergedCurrent: false } }))
    expect(step).toMatchObject({ kind: "safePush", safePushBlocker: null, landed: null })
  })

  it("says why nothing of a branch is left to land, what disables its Create Pull Request…, also behind another reason of its start", () => {
    const cases: [Partial<NextStepFacts>, string | null][] = [
      [{ landedIn: "origin/master", landedWith: "Landed in master with oauth" }, "Landed in origin/master"],
      [{ landedWith: "Landed in master with oauth" }, "Landed in master with oauth"],
      [{ robot: finished("merged") }, "Merged by Patronus"],
      [{ pullRequest: { ...openPullRequest, state: "merged", mergedCurrent: true } }, "Pull request #1078 is merged"],
      // a sync or a block says why the start waits; the branch landed all the same
      [{ syncing: true, landedWith: "Landed in master with oauth" }, "Landed in master with oauth"],
      [{ patronusAvailable: false, robot: { kind: "none" }, landedIn: "origin/master" }, "Landed in origin/master"],
      // work is left: a failed robot, commits after the landed ones
      [{ robot: finished("failed") }, null],
      [{}, null],
    ]
    for (const [patch, landed] of cases) {
      expect(getBranchNextStep(facts(patch)).landed, JSON.stringify(patch)).toBe(landed)
    }
  })

  it("row 2: restarts a failed or canceled robot, or offers Safe Push… where Patronus can't restart it", () => {
    expect(getBranchNextStep(facts({ robot: finished("failed"), restart }))).toMatchObject({ kind: "restart", item: restart, safePushBlocker: null })
    expect(getBranchNextStep(facts({ robot: finished("canceled"), restart }))).toMatchObject({ kind: "restart", item: restart })
    expect(getBranchNextStep(facts({ robot: finished("failed") }))).toMatchObject({ kind: "safePush", item: { id: "safe-push", disabled: false } })
  })

  it("row 2: no restart while a robot of a branch below lands a part of it, the steps of its pull request stay", () => {
    const landingPart = { branch: "runtime-vcs-split", robot: testRobot("root", "runtime-vcs-split") }
    const wait = "A Safe Push of runtime-vcs-split is running: wait until it merges"
    const failed = { robot: finished("failed"), restart, startBlocker: wait, landingPart }
    expect(getBranchNextStep(withReadiness({ kind: "approved", groups: [] }, failed))).toMatchObject({
      kind: "safePush",
      item: { id: "safe-push", disabled: true, description: wait },
    })
    expect(getBranchNextStep(facts({ ...failed, robot: finished("canceled") })).kind).not.toBe("restart")
    // the user's step in the pull request is still the user's
    const noPullRequest = missing([group({ requested: [] })], { reason: "noPullRequestContext", notCounted: [] })
    expect(getBranchNextStep(withReadiness(noPullRequest, { ...failed, pullRequest: null }))).toMatchObject({ kind: "createPullRequest" })
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

  it("a stacked branch takes the steps of its own pull request into master", () => {
    // oauth is built on auth and login: its Safe Push lands them with it, its approvals count in its own pull request
    const oauth: Partial<NextStepFacts> = { branchName: "oauth", subject: "oauth", pullRequest: { ...openPullRequest, number: 8 } }
    expect(getBranchNextStep(withReadiness(missing([group()]), oauth))).toMatchObject({
      kind: "awaiting",
      needsReadiness: true,
      safePushBlocker: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #8",
      notice: { title: "Safe Push of oauth waits for Ilya Korennoy", description: "IJPL Code Platform · requested on #8" },
    })
    const approved = withReadiness({ kind: "approved", groups: [] }, oauth)
    const step = getBranchNextStep(approved)
    expect(step).toMatchObject({ kind: "safePush", item: { id: "safe-push", label: "Safe Push", disabled: false }, safePushBlocker: null })
    // its own form, below its row
    step.item?.onSelect(FROM_MENU)
    expect(approved.actions.openSafePush).toHaveBeenCalledOnce()
  })

  it("a stacked branch waits for the reasons of its ancestors", () => {
    // a parent that failed to build blocks it: row 1, the Status cell says it
    const blocked = "Blocked by login: fix it first, then sync"
    expect(getBranchNextStep(facts({ blocked: true, startBlocker: blocked }))).toMatchObject({ kind: "none", item: null, safePushBlocker: blocked })
    // a Safe Push of an ancestor lands its commits: the start waits for its merge, whatever the approvals
    const landing = "A Safe Push of login is running: wait until it merges"
    expect(getBranchNextStep(facts({ startBlocker: landing }))).toMatchObject({ kind: "safePush", item: { disabled: true, description: landing }, safePushBlocker: landing })
    expect(getBranchNextStep(withReadiness(missing([group()]), { startBlocker: landing }))).toMatchObject({
      kind: "awaiting",
      safePushBlocker: landing,
      notice: { title: "Safe Push of wsm-skip-wa-2 can't start", description: landing },
    })
  })

  it("row 5: retargets an open pull request into a virtual branch, of its parent or of one that landed, whose approvals don't count", () => {
    const step = getBranchNextStep(facts({ pullRequest: { ...openPullRequest, base: "user/virtual/auth", intoTarget: false, baseIsVirtualBranch: true } }))
    expect(step).toMatchObject({
      kind: "retarget",
      // short in its column; the menus and the notices name the target
      item: {
        label: "Retarget #1078 to master",
        compactLabel: "Retarget #1078",
        description:
          "Approvals in #1078 don't count: its base is user/virtual/auth, a virtual branch. Changes the base of #1078 from user/virtual/auth to master on GitHub: Patronus merges a pull request into its base",
      },
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
    step.item!.onSelect(FROM_MENU)
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
        // the other way: a Safe Push with a profile that skips the review
        withoutReview: { blocker: null },
      })
    }
    // a branch not pushed yet gets the menu's item too: the dialog pushes it first; only an unknown repository disables it
    const unknownRepository = getBranchNextStep(
      withReadiness(missing([group()], { reason: "noPullRequestContext" }), { pullRequest: null, actions: { ...facts().actions, createPullRequest: null } }),
    )
    expect(unknownRepository.item).toMatchObject({ disabled: true, description: "The GitHub repository of the branches is unknown" })
  })

  it("row 7: a branch of a whole stack whose pull request is open points to it, quietly: its approvals count for the Safe Push of the whole stack", () => {
    const open = vi.fn()
    const step = getBranchNextStep(
      withReadiness(missing([group({ requested: [] })], { reason: "noPullRequestContext", notCounted: [] }), {
        pullRequest: null,
        stackPullRequest: { number: 12, subject: "the whole air stack", open },
      }),
    )
    expect(step).toMatchObject({
      kind: "createPullRequest",
      tone: "quiet",
      item: {
        id: "open-stack-pull-request",
        label: "In #12",
        description:
          "Code owners must approve in a pull request: IJPL Code Platform\nApprovals in #12, the pull request of the whole air stack, count for its Safe Push only\nOpen Pull Request #12",
      },
      // the Safe Push of the branch itself still needs approvals in a pull request of its own
      safePushBlocker: "Code owners must approve in a pull request: IJPL Code Platform",
      withoutReview: { blocker: null },
      notice: {
        title: "Safe Push of wsm-skip-wa-2 needs a pull request",
        description: "Code owners must approve in it: IJPL Code Platform · Approvals in #12, the pull request of the whole air stack, count for its Safe Push only",
      },
    })
    step.item?.onSelect(FROM_MENU)
    expect(open).toHaveBeenCalledOnce()
    // a branch with a pull request of its own waits in it, as any branch
    const own = getBranchNextStep(withReadiness(missing([group()]), { stackPullRequest: { number: 12, subject: "the whole air stack", open } }))
    expect(own.kind).toBe("awaiting")
  })

  it("row 8: changes requested by a member of a missing group: a destructive quiet step that opens the pull request", () => {
    const step = getBranchNextStep(withReadiness(missing([group({ changesRequestedBy: [ilya] })])))
    // an open pull request: a comment starts the robot and can't choose a profile, so no Safe Push without review
    expect(step).toMatchObject({
      kind: "changesRequested",
      tone: "destructive",
      item: { label: "Changes Requested" },
      safePushBlocker: "Ilya Korennoy requested changes on #1078",
      withoutReview: null,
    })
  })

  it("row 8: changes requested in an approved group don't block, as Patronus doesn't", () => {
    // the readiness lists only the groups still short of approvals: an approved group's request is not there
    expect(getBranchNextStep(withReadiness({ kind: "approved", groups: [{ name: "IJPL Code Platform", approvedBy: [ilya] }] })).kind).toBe("safePush")
  })

  it("row 9: a missing group with nobody asked: Request Review…, disabled only with the item's own reason", () => {
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
    // a reason of `getRequestReviewItem` is the step's as it is
    const unavailable: MenuItem = { ...requestReview, disabled: true, description: "#1078 is a draft: mark it ready for review on GitHub first" }
    expect(getBranchNextStep(withReadiness(readiness, { actions: { ...facts().actions, requestReview: unavailable } })).item).toEqual(unavailable)
    const noEligible = getBranchNextStep(withReadiness(missing([group({ requested: [] })], { reason: "noEligibleReviewer" })))
    expect(noEligible.item).toMatchObject({ disabled: true, description: "Nobody of IJPL Code Platform can approve these changes" })
  })

  it("row 10: every missing group has someone asked: the quiet wait names them and opens the pull request", () => {
    const step = getBranchNextStep(withReadiness(missing([group()])))
    expect(step.notice).toEqual({ title: "Safe Push of wsm-skip-wa-2 waits for Ilya Korennoy", description: "IJPL Code Platform · requested on #1078" })
    expect(step).toMatchObject({
      kind: "awaiting",
      tone: "quiet",
      // short in its column, whom it waits for in its hover
      item: { label: "Awaiting Ilya Korennoy", compactLabel: "Awaiting Review", kbds: ["o"], description: "Awaiting Ilya Korennoy · IJPL Code Platform\nOpen Pull Request #1078" },
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

  it("row 10 right after Request Review…: a suggested member the pull request waits for is requested, while Patronus answers again", () => {
    // the kept answer of Patronus asks nobody; GitHub waits for Ilya Korennoy since the request
    const asked = { pullRequest: { ...openPullRequest, waitingFor: ["IKorennoy"] } }
    const step = getBranchNextStep(withReadiness(missing([group({ requested: [] })]), asked))
    expect(step).toMatchObject({ kind: "awaiting", item: { label: "Awaiting Ilya Korennoy" } })
    // someone GitHub waits for who is no suggested member of the group, or one who approved, asks nobody of it
    expect(getBranchNextStep(withReadiness(missing([group({ requested: [], suggested: [alice] })]), asked)).kind).toBe("requestReview")
    expect(getBranchNextStep(withReadiness(missing([group({ requested: [], approvedBy: [ilya], needed: 1 })]), asked)).kind).toBe("requestReview")
    // two needed: the one asked is still too few
    expect(getBranchNextStep(withReadiness(missing([group({ requested: [], needed: 2 })]), asked)).kind).toBe("requestReview")
  })

  it("row 9: a group that needs two approvals and asks one person still needs a request", () => {
    const step = getBranchNextStep(withReadiness(missing([group({ needed: 2 })])))
    expect(step).toMatchObject({ kind: "requestReview", safePushBlocker: "Waits for the approval of IJPL Code Platform: fewer people are requested on #1078 than it needs" })
  })

  it("rows 8′ and 10′: while Patronus can't judge the approvals, the wait GitHub tells, the Safe Push still enabled", () => {
    const unknown: ReviewReadiness = { kind: "unknown", reason: "approvalsNotEvaluated" }
    const waits = getBranchNextStep(withReadiness(unknown, { pullRequest: { ...openPullRequest, waitingFor: ["ikorennoy"] } }))
    expect(waits).toMatchObject({ kind: "awaiting", tone: "quiet", safePushBlocker: null, notice: null, needsReadiness: true })
    expect(waits.item).toMatchObject({ id: "open-pull-request", label: "Awaiting ikorennoy", compactLabel: "Awaiting Review" })
    expect(waits.item?.description).toBe("Waits for ikorennoy on #1078 · Patronus can't tell yet whether the approvals there count: the start checks them\nOpen Pull Request #1078")
    const several = getBranchNextStep(withReadiness(unknown, { pullRequest: { ...openPullRequest, waitingFor: ["ikorennoy", "alice"] } }))
    expect(several.item?.label).toBe("Awaiting 2 Reviews")
    const changes = getBranchNextStep(withReadiness(unknown, { pullRequest: { ...openPullRequest, changesRequestedBy: ["alice"] } }))
    expect(changes).toMatchObject({ kind: "changesRequested", tone: "destructive", safePushBlocker: null })
    // a reason of the branch still disables the Safe Push
    const ancestor = "A Safe Push of login is running: wait until it merges"
    expect(getBranchNextStep(withReadiness(unknown, { pullRequest: { ...openPullRequest, waitingFor: ["ikorennoy"] }, startBlocker: ancestor })).safePushBlocker).toBe(ancestor)
    // nothing GitHub tells: Safe Push, the start says that Patronus checks the approvals
    expect(getBranchNextStep(withReadiness(unknown)).kind).toBe("safePush")
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
      expect(step).toMatchObject({ kind: "safePush", item: { label: "Safe Push", kbds: ["s"], disabled: false }, safePushBlocker: null, needsReadiness: true })
    }
  })

  it("names the Safe Push of a stacked branch by the branches it lands, and says what it lands on hover", () => {
    expect(getBranchNextStep(facts()).item).toMatchObject({ label: "Safe Push", description: "Lands wsm-skip-wa-2 into master once the checks pass" })
    expect(getBranchNextStep(facts({ landsBranches: 3 })).item).toMatchObject({
      label: "Safe Push · 3",
      description: "Lands wsm-skip-wa-2 with the 2 branches it is built on into master in one Safe Push",
    })
  })

  it("keeps a stale answer while the new one loads, so the row doesn't flicker", () => {
    expect(getBranchNextStep(facts({ readiness: { readiness: missing([group()]), stale: true } })).kind).toBe("awaiting")
  })

  it("treats the readiness as unavailable without the pull requests: never a duplicate Create Pull Request…", () => {
    const step = getBranchNextStep(withReadiness(missing([group()], { reason: "noPullRequestContext" }), { pullRequestsAvailable: false, pullRequest: null }))
    expect(step).toMatchObject({ kind: "safePush", needsReadiness: false })
  })

  it("asks for the readiness of a branch on GitHub, pushed or not: Patronus judges the local commits", () => {
    expect(getBranchNextStep(facts()).needsReadiness).toBe(true)
    expect(getBranchNextStep(facts({ pullRequestsAvailable: false })).needsReadiness).toBe(false)
  })

  it("disables the action of the slot while an operation on the row runs, never the quiet steps; its own runs", () => {
    const renaming = { id: "rename", label: "Renaming auth to login" }
    expect(getBranchNextStep(facts({ busy: renaming }))).toMatchObject({
      item: { disabled: true, description: "Renaming auth to login…" },
      safePushBlocker: "Renaming auth to login…",
    })
    expect(getBranchNextStep(withReadiness(missing([group()]), { busy: renaming })).item?.disabled).toBeUndefined()
    const safePushing = { id: "safe-push", label: "Starting the Safe Push" }
    expect(getBranchNextStep(facts({ busy: safePushing })).item).toMatchObject({ id: "safe-push", running: true, disabled: true, description: "Starting the Safe Push…" })
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
