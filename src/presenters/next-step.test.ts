// @spec spec/branch/next-step.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { LandingRefusal, Reason, StepBlocker } from "@/lib/bindings"
import { FROM_MENU } from "@/lib/menu"
import { presentNextStep, startStepBlocker } from "@/presenters/next-step"
import { missingGroup, testActions, testContext, testStep } from "@/test-support/next-step"

/** The reasons of the catalog Rust writes (`reason_catalog_test.rs`) */
const CATALOG = Object.values(import.meta.glob<Reason[]>("/crates/domain-policy/reasons.json", { eager: true, import: "default" }))[0]!

function reasonOf(code: Reason["code"], index = 0): Reason {
  return CATALOG.filter(reason => reason.code === code)[index]!
}

function rule(reason: Reason): StepBlocker {
  return { code: "rule", reason }
}

const OWN: StepBlocker = { code: "step" }
const MISSING = [missingGroup("IJPL Code Platform")]
const ASKED = [missingGroup("IJPL Code Platform", ["Ilya Korennoy"])]

describe("the words of the next step", () => {
  it("row 0: words the fact a step waits for and a read that failed, with what helps: a login to copy, a refresh, a restart disabled", () => {
    const pending = presentNextStep(testStep({ kind: "pending", fact: "pullRequests" }, { safePush: OWN }), testContext())
    expect(pending).toMatchObject({ kind: "pending", tone: "quiet", waitsFor: "pullRequests", safePushBlocker: "Waits for GitHub to list the pull requests" })
    expect(pending.item).toMatchObject({ label: "Checking…", disabled: true, running: true })
    expect(pending.notice).toEqual({ title: "Safe Push of auth waits for GitHub", description: "Waits for GitHub to list the pull requests" })
    const patronus = presentNextStep(testStep({ kind: "pending", fact: "patronus" }, { safePush: OWN, dryRun: OWN }), testContext())
    expect(patronus.dryRunBlocker).toBe("Waits for the answer of Patronus")
    const needs = presentNextStep(testStep({ kind: "pending", fact: "landingNeeds" }, { safePush: OWN }), testContext())
    expect(needs.safePushBlocker).toBe("Checking what auth needs to land…")

    const calls: string[] = []
    const login = presentNextStep(
      testStep({ kind: "unavailable", failure: { kind: "notLoggedIn" }, help: "copyLoginCommand", robotId: null }, { safePush: OWN, dryRun: OWN }),
      testContext({ actions: testActions(calls) }),
    )
    expect(login.item).toMatchObject({ id: "copy-safe-push-login", label: "Copy Login Command" })
    expect(login.item?.description).toBe("Run ./safePush.cmd once to log in to Patronus: the command goes to the clipboard, for a terminal")
    expect(login.safePushBlocker).toBe("Run ./safePush.cmd once to log in to Patronus")
    expect(login.dryRunBlocker).toBe("Run ./safePush.cmd once to log in to Patronus")
    login.item!.onSelect(FROM_MENU)
    expect(calls).toEqual(["copyLoginCommand"])
    const refresh = presentNextStep(
      testStep({ kind: "unavailable", failure: { kind: "patronusError", message: "502" }, help: "refreshPatronus", robotId: null }, { safePush: OWN }),
      testContext(),
    )
    expect(refresh.item).toMatchObject({ id: "refresh-patronus", label: "Refresh Patronus", description: "Patronus: 502" })
    expect(refresh.notice).toEqual({ title: "Safe Push of auth can't start", description: "Patronus: 502" })
    const restart = { id: "restart-robot", label: "Restart Safe Push", onSelect: () => {} }
    const problems = presentNextStep(
      testStep({ kind: "unavailable", failure: { kind: "problemsFailed", message: "timeout" }, help: null, robotId: "r" }, { safePush: OWN }),
      testContext({ actions: testActions([], { restart }) }),
    )
    expect(problems.item).toMatchObject({ id: "restart-robot", disabled: true })
    expect(problems.safePushBlocker).toBe("Patronus didn't tell why the Safe Push failed (timeout): Check Approvals Again asks again")
  })

  it("row 1: offers nothing, every surface says the reason of the subject: a sync, a failure, no commits, parked, landed", () => {
    const settled = (safePush: StepBlocker) => presentNextStep(testStep({ kind: "settled" }, { safePush, dryRun: safePush }), testContext())
    expect(settled({ code: "syncing" }).safePushBlocker).toBe("Wait for the sync to finish")
    expect(settled({ code: "syncFailed", problem: null }).safePushBlocker).toBe("The branch failed to sync")
    expect(settled({ code: "syncFailed", problem: "Its parent auth conflicts" }).dryRunBlocker).toBe("Its parent auth conflicts")
    expect(settled({ code: "noCommits" }).safePushBlocker).toBe("No commits to push")
    const parked = settled(rule(reasonOf("parked")))
    expect(parked).toMatchObject({
      kind: "none",
      item: null,
      safePushBlocker: "Built on parked auth: Unpark auth to push",
      dryRunBlocker: "Built on parked auth: Unpark auth to push",
    })
    expect(parked.notice).toEqual({ title: "Safe Push of auth can't start", description: "Built on parked auth: Unpark auth to push" })
    const landed = presentNextStep(
      testStep({ kind: "settled" }, { safePush: rule(reasonOf("landedIn")), landed: { by: { kind: "baseline", upstream: "origin/master" }, after: [] } }),
      testContext(),
    )
    expect(landed).toMatchObject({ safePushBlocker: "Landed in origin/master", landed: "Landed in origin/master" })
  })

  it("has no slot off Space and keeps the reasons of the subject, with no notice", () => {
    const hidden = presentNextStep(testStep({ kind: "hidden" }, { safePush: { code: "noCommits" } }), testContext())
    expect(hidden).toMatchObject({ kind: "hidden", item: null, safePushBlocker: "No commits to push", notice: null })
  })

  it("row 1′: names the branch the build needs and runs the fix, or opens the Safe Push", () => {
    const refused = reasonOf("landingRefused") as Extract<Reason, { code: "landingRefused" }>
    const calls: string[] = []
    const step = presentNextStep(testStep({ kind: "needsBranch", refusals: [refused.refusal] }), testContext({ actions: testActions(calls) }))
    expect(step).toMatchObject({ kind: "needsBranch", safePushBlocker: null })
    expect(step.item).toMatchObject({ id: "landing-fix", label: "Needs util" })
    expect(step.landingFix).toMatchObject({ label: "Stack flow on util" })
    step.item!.onSelect(FROM_MENU)
    expect(calls).toEqual(["fixLanding restack"])
    const move = (reasonOf("landingRefused", 1) as Extract<Reason, { code: "landingRefused" }>).refusal satisfies LandingRefusal
    const opens = presentNextStep(testStep({ kind: "needsBranch", refusals: [move] }), testContext({ actions: testActions(calls) }))
    expect(opens.item).toMatchObject({ id: "landing-needs", label: "Needs acp" })
    expect(opens.landingFix).toBeNull()
  })

  it("row 1″: takes in the commits someone else pushed to the remote branch, which a push would drop", () => {
    const calls: string[] = []
    const step = presentNextStep(testStep({ kind: "pickRemoteCommits", count: 1 }), testContext({ actions: testActions(calls) }))
    expect(step).toMatchObject({ kind: "pickRemoteCommits", safePushBlocker: null })
    expect(step.item).toMatchObject({
      id: "next-step-pick-remote",
      label: "Pick Remote Commits",
      description: "The remote has 1 commit of someone else that auth doesn't: a push drops it",
    })
    step.item!.onSelect(FROM_MENU)
    expect(calls).toEqual(["pickRemoteCommits"])
    expect(presentNextStep(testStep({ kind: "pickRemoteCommits", count: 2 }), testContext()).item?.description).toBe(
      "The remote has 2 commits of someone else that auth doesn't: a push drops them",
    )
  })

  it("rows 2 and 3: restarts the robot, or the pull request that collects the approvals, or a Safe Push where it can't restart", () => {
    const restart = { id: "push-and-restart-robot", label: "Restart Safe Push", onSelect: () => {} }
    expect(presentNextStep(testStep({ kind: "restart", robotId: "r" }), testContext({ actions: testActions([], { restart }) })).item).toBe(restart)
    expect(presentNextStep(testStep({ kind: "restart", robotId: "r" }), testContext()).kind).toBe("safePush")
    const open = presentNextStep(testStep({ kind: "robotPullRequest", number: 1078 }), testContext())
    expect(open).toMatchObject({ kind: "pullRequest", tone: "default" })
    expect(open.item).toMatchObject({ id: "open-pull-request", label: "Open Pull Request", description: "Open Pull Request #1078", kbds: ["o"] })
    expect(presentNextStep(testStep({ kind: "robotPullRequest", number: null }), testContext()).item).toMatchObject({ id: "create-pull-request" })
  })

  it("row 5: retargets a pull request into a virtual branch, short in its column; 5′ opens one into another base, the retarget in the menu", () => {
    const retarget = { number: 7, base: "test-user/virtual/core", target: "master" }
    const calls: string[] = []
    const virtual = presentNextStep(testStep({ kind: "retarget", number: 7, base: retarget.base }, { retarget }), testContext({ actions: testActions(calls) }))
    expect(virtual).toMatchObject({ kind: "retarget", safePushBlocker: null })
    expect(virtual.item).toMatchObject({ label: "Retarget #7 to master", compactLabel: "Retarget #7" })
    expect(virtual.item?.description).toBe(
      "Approvals in #7 don't count: its base is test-user/virtual/core, a virtual branch. Changes the base of #7 from test-user/virtual/core to master on GitHub: Patronus merges a pull request into its base",
    )
    virtual.item!.onSelect(FROM_MENU)
    expect(calls).toEqual(["retarget"])
    const other = presentNextStep(testStep({ kind: "otherBase", number: 7, base: "release/253" }, { safePush: OWN, retarget: { ...retarget, base: "release/253" } }), testContext())
    expect(other).toMatchObject({ kind: "otherBase", tone: "quiet", safePushBlocker: "Approvals in #7 don't count: its base is release/253" })
    expect(other.item).toMatchObject({ id: "open-pull-request-other-base", label: "Open Pull Request" })
    expect(other.retarget).toMatchObject({ id: "retarget-pull-request", label: "Retarget #7 to master" })
    expect(other.notice).toEqual({ title: "Approvals in #7 of auth don't count", description: "Its base is release/253, not master" })
  })

  it("row 6: a draft is a quiet step that opens it; its Safe Push says why, a Dry Run runs", () => {
    const draft = presentNextStep(testStep({ kind: "draft", number: 7 }, { safePush: OWN }), testContext())
    expect(draft).toMatchObject({ kind: "draft", tone: "quiet", safePushBlocker: "#7 is a draft: Patronus merges it once it is ready for review", dryRunBlocker: null })
    expect(draft.item).toMatchObject({ id: "open-pull-request-draft", label: "Draft" })
    expect(draft.notice).toEqual({ title: "#7 of auth is a draft", description: "Patronus merges it once it is ready for review" })
  })

  it("row 7: Create Pull Request… naming the code owners, or In #12 of the whole stack, quietly", () => {
    const create = presentNextStep(
      testStep({ kind: "createPullRequest", groups: MISSING, stack: null, repository: true }, { safePush: OWN, withoutReview: { blocker: null } }),
      testContext(),
    )
    expect(create).toMatchObject({
      kind: "createPullRequest",
      tone: "default",
      safePushBlocker: "Code owners must approve in a pull request: IJPL Code Platform",
      withoutReview: { blocker: null },
    })
    expect(create.item).toMatchObject({ id: "create-pull-request", description: "Code owners must approve in a pull request: IJPL Code Platform" })
    expect(create.notice).toEqual({ title: "Safe Push of auth needs a pull request", description: "Code owners must approve in it: IJPL Code Platform" })
    const unknown = presentNextStep(testStep({ kind: "createPullRequest", groups: MISSING, stack: null, repository: false }, { safePush: OWN }), testContext())
    expect(unknown.item).toMatchObject({ disabled: true, description: "The GitHub repository of the branches is unknown" })
    const calls: string[] = []
    const inStack = presentNextStep(
      testStep({ kind: "createPullRequest", groups: MISSING, stack: { number: 12, root: "air" }, repository: true }, { safePush: OWN }),
      testContext({ actions: testActions(calls) }),
    )
    expect(inStack).toMatchObject({ tone: "quiet" })
    expect(inStack.item).toMatchObject({ id: "open-stack-pull-request", label: "In #12" })
    expect(inStack.item?.description).toBe(
      "Code owners must approve in a pull request: IJPL Code Platform\nApprovals in #12, the pull request of the whole air stack, count for its Safe Push only\nOpen Pull Request #12",
    )
    inStack.item!.onSelect(FROM_MENU)
    expect(calls).toEqual(["openStackPullRequest air"])
  })

  it("rows 8 to 10: changes requested, Request Review…, the wait naming the people asked", () => {
    const ilya = { login: "ikorennoy", fullName: "Ilya Korennoy" }
    const changes = presentNextStep(
      testStep({ kind: "changesRequested", number: 1078, groups: [missingGroup("IJPL Code Platform", [], { changesRequestedBy: [ilya] })] }, { safePush: OWN }),
      testContext(),
    )
    expect(changes).toMatchObject({ kind: "changesRequested", tone: "destructive", safePushBlocker: "Ilya Korennoy requested changes on #1078" })
    expect(changes.notice).toEqual({ title: "Safe Push of auth waits for changes", description: "Ilya Korennoy requested changes on #1078" })
    const request = { id: "request-review", label: "Request Review…", description: "Ask for a review of #1078", onSelect: () => {} }
    const review = presentNextStep(
      testStep({ kind: "requestReview", number: 1078, groups: MISSING, noEligible: false }, { safePush: OWN }),
      testContext({ actions: testActions([], { requestReview: request }) }),
    )
    expect(review.safePushBlocker).toBe("Waits for the approval of IJPL Code Platform: nobody of it is requested on #1078")
    expect(review.item?.description).toBe("Waits for the approval of IJPL Code Platform: nobody of it is requested on #1078. Ask for a review of #1078")
    expect(review.notice).toEqual({ title: "Safe Push of auth waits for a review request", description: "IJPL Code Platform · nobody is requested on #1078" })
    const nobody = presentNextStep(
      testStep({ kind: "requestReview", number: 1078, groups: MISSING, noEligible: true }, { safePush: OWN }),
      testContext({ actions: testActions([], { requestReview: request }) }),
    )
    expect(nobody.item).toMatchObject({ disabled: true, description: "Nobody of IJPL Code Platform can approve these changes" })
    const awaiting = presentNextStep(testStep({ kind: "awaiting", number: 1078, groups: ASKED }, { safePush: OWN }), testContext())
    expect(awaiting).toMatchObject({ kind: "awaiting", tone: "quiet", safePushBlocker: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #1078" })
    expect(awaiting.item).toMatchObject({
      label: "Awaiting Ilya Korennoy",
      compactLabel: "Awaiting Review",
      description: "Awaiting Ilya Korennoy · IJPL Code Platform\nOpen Pull Request #1078",
    })
    expect(awaiting.notice).toEqual({ title: "Safe Push of auth waits for Ilya Korennoy", description: "IJPL Code Platform · requested on #1078" })
  })

  it("rows 8′ and 10′: the wait GitHub tells while Patronus can't judge, the Safe Push enabled", () => {
    const waits = presentNextStep(testStep({ kind: "awaitingUnjudged", number: 1078, reviewers: ["ikorennoy"] }), testContext())
    expect(waits).toMatchObject({ kind: "awaiting", tone: "quiet", safePushBlocker: null })
    expect(waits.item?.description).toBe("Waits for ikorennoy on #1078 · Patronus can't tell yet whether the approvals there count: the start checks them\nOpen Pull Request #1078")
    expect(presentNextStep(testStep({ kind: "awaitingUnjudged", number: 1078, reviewers: ["a", "b"] }), testContext()).item?.label).toBe("Awaiting 2 Reviews")
    const changes = presentNextStep(testStep({ kind: "changesRequestedUnjudged", number: 1078, by: ["ikorennoy"] }), testContext())
    expect(changes).toMatchObject({ kind: "changesRequested", tone: "destructive", safePushBlocker: null })
  })

  it("row 11: Safe Push by the branches it lands, and what Patronus checks while GitHub lists no pull request", () => {
    const lone = presentNextStep(testStep({ kind: "safePush" }), testContext())
    expect(lone.item).toMatchObject({ id: "safe-push", label: "Safe Push", kbds: ["s"], disabled: false, description: "Lands auth into master once the checks pass" })
    const stacked = presentNextStep(testStep({ kind: "safePush" }, { lands: 3 }), testContext())
    expect(stacked.item).toMatchObject({ label: "Safe Push · 3", description: "Lands auth with the 2 branches it is built on into master in one Safe Push" })
    const unlisted = presentNextStep(testStep({ kind: "safePush" }, { unlisted: { status: "signedOut" }, withoutReview: { blocker: null } }), testContext())
    expect(unlisted.item?.description).toBe(
      "Lands auth into master once the checks pass · No GitHub token to see pull requests: Patronus checks the approvals when the robot starts",
    )
    expect(unlisted.withoutReview).toEqual({ blocker: null })
    const stack = presentNextStep(
      testStep({ kind: "safePush" }),
      testContext({ subject: "the whole air stack", startDescription: "Lands 2 branches, 3 commits into master in one Safe Push: air and ui" }),
    )
    expect(stack.item?.description).toBe("Lands 2 branches, 3 commits into master in one Safe Push: air and ui")
  })

  it("row push: Push to master by the branches it lands, its start disabled with the refusal of the gate of the route", () => {
    const lone = presentNextStep(testStep({ kind: "pushToBaseline", target: "master" }), testContext())
    expect(lone.item).toMatchObject({ id: "push-to-baseline", label: "Push to master", disabled: false })
    expect(lone.item?.description).toBe("Push the copy of auth into master: it lands without a robot and without the CI of a pull request")
    const stacked = presentNextStep(testStep({ kind: "pushToBaseline", target: "master" }, { lands: 3 }), testContext())
    expect(stacked.item?.label).toBe("Push to master · 3")
    const refused = presentNextStep(testStep({ kind: "pushToBaseline", target: "master" }, { safePush: rule(reasonOf("noWriteAccess")) }), testContext())
    expect(refused.item).toMatchObject({ disabled: true, description: "You have no write access to develar/branch-deck: someone who has lands it into master" })
  })
})

describe("the step's blocker of a start", () => {
  it("is the step's for a Safe Push, of a start that skips the approvals the reasons of the subject on every row, of a Dry Run its own", () => {
    const syncing: StepBlocker = { code: "syncing" }
    const steps = [
      // row 11 listed: no start without review is offered, a default start whose profile skips the review still waits
      // for the sync, which its decision doesn't read
      presentNextStep(testStep({ kind: "safePush" }, { safePush: syncing, dryRun: syncing }), testContext()),
      // row 7: a start without review is offered, held back by the reasons of the subject alone
      presentNextStep(
        testStep({ kind: "createPullRequest", groups: MISSING, stack: null, repository: true }, { safePush: syncing, dryRun: syncing, withoutReview: { blocker: syncing } }),
        testContext(),
      ),
      // row 8: the approvals hold it back, nothing else
      presentNextStep(
        testStep(
          { kind: "changesRequested", number: 1078, groups: [missingGroup("IJPL Code Platform", [], { changesRequestedBy: [{ login: "ikorennoy", fullName: null }] })] },
          { safePush: OWN },
        ),
        testContext(),
      ),
      // row 6: a draft, which a start through the API never reads
      presentNextStep(testStep({ kind: "draft", number: 7 }, { safePush: OWN }), testContext()),
      // row 10 while a start of the row runs: what runs holds every start back
      presentNextStep(testStep({ kind: "awaiting", number: 1078, groups: ASKED }, { safePush: OWN }), testContext({ busy: { id: "", label: "Starting the Safe Push" } })),
    ]
    for (const step of steps) {
      expect(startStepBlocker(step, "rebaseAutosquash", false)).toBe(step.safePushBlocker)
      expect(startStepBlocker(step, "dryRun", true)).toBe(step.dryRunBlocker)
    }
    expect(steps.map(step => startStepBlocker(step, "rebaseAutosquash", true))).toEqual([
      "Wait for the sync to finish",
      "Wait for the sync to finish",
      null,
      null,
      "Starting the Safe Push…",
    ])
    // a start without review is held back as the menu item of it says
    expect(startStepBlocker(steps[1]!, "rebaseAutosquash", true)).toBe(steps[1]!.withoutReview?.blocker)
  })
})

describe("what lies over the next step", () => {
  it("keeps a stale answer, its action waiting for the new one, and a start reads only a fresh one", () => {
    const stale = presentNextStep(testStep({ kind: "safePush" }, { staleReadiness: true, safePush: { code: "staleReadiness" }, needsReadiness: true }), testContext())
    expect(stale).toMatchObject({ kind: "safePush", needsReadiness: true, safePushBlocker: "Waits for Patronus to check the approvals again" })
    expect(stale.item).toMatchObject({ disabled: true, description: "Waits for Patronus to check the approvals again" })
    expect(stale.notice).toEqual({ title: "Safe Push of auth waits for Patronus", description: "Waits for Patronus to check the approvals again" })
    const awaiting = presentNextStep(testStep({ kind: "awaiting", number: 1078, groups: ASKED }, { staleReadiness: true, safePush: OWN }), testContext())
    // a quiet step only opens the pull request: it stays
    expect(awaiting.item?.disabled).toBeUndefined()
    const start = testContext({ forStart: true })
    expect(presentNextStep(testStep({ kind: "awaiting", number: 1078, groups: ASKED }, { staleReadiness: true, safePush: OWN }), start).safePushBlocker).toBeNull()
    expect(presentNextStep(testStep({ kind: "pending", fact: "readiness" }, { safePush: OWN }), start).safePushBlocker).toBeNull()
    expect(presentNextStep(testStep({ kind: "pending", fact: "pullRequests" }, { safePush: OWN }), start).safePushBlocker).toBe("Waits for GitHub to list the pull requests")
    expect(presentNextStep(testStep({ kind: "draft", number: 7 }, { safePush: OWN }), start).safePushBlocker).toBe("#7 is a draft: Patronus merges it once it is ready for review")
  })

  it("lays what runs on the row over the step: its own action runs, the others wait, never a quiet step nor another field of the pull request", () => {
    const retargeting = { id: "retarget-pull-request", label: "Retargeting #7 to master", resources: ["pr:7:base"] }
    const retarget = { number: 7, base: "test-user/virtual/core", target: "master" }
    const own = presentNextStep(testStep({ kind: "retarget", number: 7, base: retarget.base }, { retarget }), testContext({ busy: retargeting }))
    expect(own.item).toMatchObject({ running: true, disabled: true, description: "Retargeting #7 to master…" })
    expect(own.retarget).toMatchObject({ running: true, disabled: true })
    expect(own.safePushBlocker).toBe("Retargeting #7 to master…")
    const start = presentNextStep(testStep({ kind: "safePush" }), testContext({ busy: { id: "", label: "Starting the Safe Push" } }))
    expect(start.item).toMatchObject({ disabled: true, description: "Starting the Safe Push…" })
    expect(start.dryRunBlocker).toBe("Starting the Safe Push…")
    const quiet = presentNextStep(testStep({ kind: "awaiting", number: 1078, groups: ASKED }, { safePush: OWN }), testContext({ busy: retargeting }))
    expect(quiet.item?.disabled).toBeUndefined()
    const request = { id: "request-review", label: "Request Review…", onSelect: () => {} }
    const review = presentNextStep(
      testStep({ kind: "requestReview", number: 7, groups: MISSING, noEligible: false }, { safePush: OWN }),
      testContext({ busy: retargeting, actions: testActions([], { requestReview: request }) }),
    )
    expect(review.item?.disabled).toBeUndefined()
    // a reason of the subject outranks what runs
    expect(presentNextStep(testStep({ kind: "settled" }, { safePush: { code: "syncing" } }), testContext({ busy: retargeting })).safePushBlocker).toBe(
      "Wait for the sync to finish",
    )
  })

  it("words the notice of S as a short title naming the subject, a reason of the subject first", () => {
    const held = presentNextStep(testStep({ kind: "awaiting", number: 1078, groups: ASKED }, { safePush: rule(reasonOf("held", 1)) }), testContext())
    expect(held.notice).toEqual({
      title: "Safe Push of auth can't start",
      description: "A Safe Push of auth is running: wait until it merges, or restart it on the current branch",
    })
    const stack = presentNextStep(testStep({ kind: "awaiting", number: 12, groups: ASKED }, { safePush: OWN }), testContext({ subject: "the whole air stack" }))
    expect(stack.notice?.title).toBe("Safe Push of the whole air stack waits for Ilya Korennoy")
  })
})
