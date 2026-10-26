// @spec spec/branch/branch-row.spec.md
// @spec spec/repository/landing-overview.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { LandingNode as NodeView, NodeLabel } from "@/lib/bindings"
import type { FoundMove } from "@/lib/running-move"
import { presentLandingNode, presentNodeKind } from "@/presenters/landing-node"
import { missingGroup, testContext, testStep } from "@/test-support/next-step"

function nodeView(kind: NodeView["kind"], label: NodeLabel, extra: Partial<NodeView> = {}): NodeView {
  return { kind, label, pending: false, robot: false, shows: null, moveShows: true, ...extra }
}

function present(view: NodeView, move: FoundMove | null = null, statusText = ""): ReturnType<typeof presentLandingNode> {
  return presentLandingNode(view, { step: testContext(), statusText, move, pushing: false })
}

const START: FoundMove = { move: { key: "auth", lands: true, label: "Landing: Safe Push starting" }, with: null }

describe("the words of the node of a row", () => {
  it("words the state of the subject itself: parked, a failure, landed, blocked, its robot running, which a move doesn't replace", () => {
    expect(present(nodeView("parked", { kind: "parked", parked: { note: "", with: "core" }, conflict: false }))).toEqual({ kind: "parked", label: "Parked with core, kept local" })
    const conflict = nodeView("parked", { kind: "parked", parked: { note: "later", with: null }, conflict: true }, { moveShows: false })
    expect(present(conflict, START)).toEqual({ kind: "parked", label: "Parked, kept local: later\nIt doesn't build: fix it before you unpark it" })
    const failed = nodeView("needsFix", { kind: "syncFailed", status: "MergeConflict", problem: null }, { moveShows: false })
    expect(present(failed, START, "merge conflict")).toEqual({ kind: "needsFix", label: "merge conflict" })
    expect(present(nodeView("needsFix", { kind: "syncFailed", status: "Error", problem: null }))).toMatchObject({ label: "The branch failed to sync" })
    const landed = nodeView("inMaster", { kind: "landed", landed: { by: { kind: "baseline", upstream: "origin/master" }, after: [] } }, { moveShows: false })
    expect(present(landed, START)).toEqual({ kind: "inMaster", label: "Landed in origin/master" })
    expect(present(nodeView("waiting", { kind: "blocked", by: "core" }, { shows: "core" }))).toEqual({
      kind: "waiting",
      label: "Blocked by core, which doesn't build",
      shows: "core",
    })
    expect(present(nodeView("waiting", { kind: "blocked", by: null }))).toMatchObject({ label: "Blocked by its parent branch" })
    const running = nodeView("landing", { kind: "robotRunning", dryRun: false, failing: true }, { robot: true, moveShows: false })
    expect(present(running, START)).toEqual({ kind: "landing", label: "Landing: Safe Push running, a required check failed", robot: true })
    expect(present(nodeView("waiting", { kind: "robotRunning", dryRun: true, failing: false }, { robot: true }))).toMatchObject({
      label: "Dry Run running: it checks the branch, it lands nothing",
    })
    expect(present(nodeView("landing", { kind: "landsWith", by: { branch: "air", robotId: "r", wholeStack: true } }))).toMatchObject({
      label: "Lands with the whole air stack: its Safe Push runs",
    })
    expect(present(nodeView("needsFix", { kind: "robotFailed", problems: null }, { robot: true }))).toMatchObject({
      label: "Safe Push failed on auth as it is: fix it, or Restart Safe Push",
    })
  })

  it("is the state after a running move of the row from its start on: Landing for a robot that comes, else Waiting", () => {
    const ready = nodeView("ready", { kind: "ready", lands: 1 })
    expect(present(ready, START)).toEqual({ kind: "landing", label: "Landing: Safe Push starting" })
    expect(present(ready, { move: { key: "auth", lands: false, label: "Requesting a review on #1078" }, with: null })).toEqual({
      kind: "waiting",
      label: "Requesting a review on #1078",
    })
    expect(present(ready, { move: START.move, with: "oauth" })).toEqual({ kind: "landing", label: "Lands with oauth: its Safe Push starts" })
    expect(present(ready)).toEqual({ kind: "ready", label: "Ready to land: Safe Push" })
  })

  it("words where a start lands it: ready with the branches it lands, the user's step while Patronus checks the approvals at the start", () => {
    expect(present(nodeView("ready", { kind: "ready", lands: 3 }))).toMatchObject({ label: "Ready to land: Safe Push lands 3 branches" })
    expect(present(nodeView("yourStep", { kind: "atStart", lands: 1, unlisted: null }))).toMatchObject({ label: "Safe Push: Patronus checks the approvals when the robot starts" })
    expect(present(nodeView("yourStep", { kind: "atStart", lands: 2, unlisted: { status: "signedOut" } }))).toMatchObject({
      label: "Safe Push lands 2 branches · No GitHub token to see pull requests: Patronus checks the approvals when the robot starts",
    })
    const ancestor = { code: "ancestor", landing: { branch: "auth", robotId: "r", wholeStack: false }, mode: "rebaseAutosquash" } as const
    const waits = present(nodeView("waiting", { kind: "startWaits", reason: ancestor }, { shows: "auth" }))
    expect(waits).toMatchObject({ kind: "waiting", shows: "auth", label: "A Safe Push of auth is running: wait until it merges" })
  })

  it("words the step of the node as its slot says it: a pull request, a review request, a wait, In #12, Checking…", () => {
    const step = (view: Parameters<typeof testStep>[0], extra: Parameters<typeof testStep>[1] = {}): NodeLabel => ({ kind: "step", step: testStep(view, extra) })
    expect(present(nodeView("yourStep", step({ kind: "robotPullRequest", number: 7 })))).toMatchObject({
      label: "Approvals count only in a pull request: Patronus checks them when the robot starts",
    })
    expect(present(nodeView("yourStep", step({ kind: "draft", number: 7 }, { safePush: { code: "step" } })))).toMatchObject({
      label: "#7 is a draft: Patronus merges it once it is ready for review",
    })
    const asked = [missingGroup("IJPL Code Platform", ["Ilya Korennoy"])]
    expect(present(nodeView("waiting", step({ kind: "awaiting", number: 1078, groups: asked }, { safePush: { code: "step" } })))).toMatchObject({
      label: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #1078",
    })
    const missing = [missingGroup("IJPL Code Platform")]
    expect(
      present(nodeView("waiting", step({ kind: "createPullRequest", groups: missing, stack: { number: 12, root: "air" }, repository: true }, { safePush: { code: "step" } }))),
    ).toMatchObject({
      label:
        "Code owners must approve in a pull request: IJPL Code Platform\nApprovals in #12, the pull request of the whole air stack, count for its Safe Push only\nOpen Pull Request #12",
    })
    expect(present(nodeView("yourStep", step({ kind: "requestReview", number: 7, groups: missing, noEligible: true }, { safePush: { code: "step" } })))).toMatchObject({
      label: "Nobody of IJPL Code Platform can approve these changes",
    })
    expect(present(nodeView("waiting", step({ kind: "pending", fact: "readiness" }, { safePush: { code: "step" } }), { pending: true }))).toEqual({
      kind: "waiting",
      label: "Waits for Patronus to check the approvals",
      pending: true,
    })
  })

  it("waits for the first answers and says a read that failed", () => {
    expect(present(nodeView("waiting", { kind: "waits", fact: "patronus" }, { pending: true }))).toEqual({
      kind: "waiting",
      label: "Waits for the answer of Patronus",
      pending: true,
    })
    expect(present(nodeView("yourStep", { kind: "unavailable", failure: { kind: "notLoggedIn" } }))).toMatchObject({ label: "Run ./safePush.cmd once to log in to Patronus" })
    expect(present(nodeView("waiting", { kind: "held" }))).toMatchObject({ label: "A running Safe Push holds the branch" })
  })

  it("lands by its pull request in a repository that is not on Space", () => {
    expect(present(nodeView("yourStep", { kind: "pushIt" }))).toMatchObject({ label: "Not on the remote yet: push it" })
    expect(present(nodeView("waiting", { kind: "pushed" }))).toMatchObject({ label: "Pushed: it lands through its pull request" })
    expect(present(nodeView("yourStep", { kind: "noPullRequest" }))).toMatchObject({ label: "Not in a pull request" })
    expect(present(nodeView("yourStep", { kind: "draft", number: 7 }))).toMatchObject({ label: "#7 is a draft" })
    expect(present(nodeView("needsFix", { kind: "changesRequestedBy", number: 7, by: ["alice"] }))).toMatchObject({ label: "alice requested changes on #7" })
    expect(present(nodeView("needsFix", { kind: "changesRequested", number: 7 }))).toMatchObject({ label: "Changes requested on #7" })
    expect(present(nodeView("needsFix", { kind: "checksFail", number: 7 }))).toMatchObject({ label: "Checks fail on #7" })
    expect(present(nodeView("waiting", { kind: "waitsFor", number: 7, reviewers: ["alice", "bob"] }))).toMatchObject({ label: "Waits for alice and bob on #7" })
    expect(present(nodeView("waiting", { kind: "landsThrough", number: 7 }))).toMatchObject({ label: "Lands through #7" })
  })

  it("is the pushed branch from the click of its push where it asks for the push, in a repository that is not on Space", () => {
    const pushing = { step: testContext(), statusText: "", move: null, pushing: true }
    expect(presentLandingNode(nodeView("yourStep", { kind: "pushIt" }), pushing)).toEqual({ kind: "waiting", label: "Pushed: it lands through its pull request" })
    // where the push is no move of it, the node stays
    expect(presentLandingNode(nodeView("yourStep", { kind: "noPullRequest" }), pushing)).toEqual({ kind: "yourStep", label: "Not in a pull request" })
    expect(presentLandingNode(nodeView("ready", { kind: "ready", lands: 1 }), pushing)).toEqual({ kind: "ready", label: "Ready to land: Safe Push" })
  })

  it("counts the kind a row shows without its words: the move that runs and the push from its click laid over the node", () => {
    const ready = nodeView("ready", { kind: "ready", lands: 1 })
    const review = { move: { key: "auth", lands: false, label: "Requesting a review on #1078" }, with: null }
    const conflict = nodeView("parked", { kind: "parked", parked: { note: "", with: null }, conflict: true }, { moveShows: false })
    const pushIt = nodeView("yourStep", { kind: "pushIt" })
    const cases: [NodeView, FoundMove | null, boolean][] = [
      [ready, null, false],
      [ready, START, false],
      [ready, review, false],
      [conflict, START, false],
      [pushIt, null, true],
      [pushIt, null, false],
    ]
    expect(cases.map(([view, move, pushing]) => presentNodeKind(view, move, pushing))).toEqual(["ready", "landing", "waiting", "parked", "waiting", "yourStep"])
    // the kind the words of the node have
    for (const [view, move, pushing] of cases) {
      expect(presentNodeKind(view, move, pushing)).toBe(presentLandingNode(view, { step: testContext(), statusText: "", move, pushing }).kind)
    }
  })
})
