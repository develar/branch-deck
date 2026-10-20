// @spec spec/branch/branch-row.spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ReviewReadiness } from "@/lib/bindings"
import type { NextStepFacts } from "@/lib/branch-next-step"
import type { LandingNodeFacts } from "@/lib/landing-node"
import { getLandingNode } from "@/lib/landing-node"
import type { FoundMove } from "@/lib/running-move"
import { chip, facts, finished, group, missing, openPullRequest, restart, withReadiness } from "@/lib/test-next-step-facts"

const approved: ReviewReadiness = { kind: "approved", groups: [{ name: "IJPL Code Platform", approvedBy: [] }] }

function landing(next: NextStepFacts, patch: Partial<LandingNodeFacts> = {}): ReturnType<typeof getLandingNode> {
  return getLandingNode({
    next,
    status: "Unchanged",
    statusText: "",
    blockedBy: null,
    landingWith: null,
    robotDryRun: false,
    patronusKnown: true,
    move: null,
    pushPending: false,
    ...patch,
  })
}

describe("getLandingNode", () => {
  it("is parked, never a move, also with a conflict, until something lands it already", () => {
    const parked = { note: "waits for IJPL-1", with: null }
    expect(landing(withReadiness(approved, { parked }))).toEqual({ kind: "parked", label: "Parked, kept local: waits for IJPL-1" })
    expect(landing(facts({ parked, hasError: true }), { status: "MergeConflict" })).toEqual({
      kind: "parked",
      label: "Parked, kept local: waits for IJPL-1\nIt doesn't build: fix it before you unpark it",
    })
    // a Safe Push of a branch built on it lands it whatever the park says
    expect(landing(facts({ parked }), { landingWith: "oauth" }).kind).toBe("landing")
  })

  it("is ready to land once Patronus answered that a Safe Push can land it, with the branches it lands", () => {
    expect(landing(withReadiness(approved))).toEqual({ kind: "ready", label: "Ready to land: Safe Push" })
    expect(landing(withReadiness(approved, { landsBranches: 3 }))).toEqual({ kind: "ready", label: "Ready to land: Safe Push lands 3 branches" })
    // a canceled robot, a robot whose approvals arrived in its pull request: the restart lands it
    expect(landing(withReadiness(approved, { robot: finished("canceled"), restart })).kind).toBe("ready")
    expect(landing(withReadiness(approved, { robot: finished("needsReview"), robotPullRequest: 1078, restart })).kind).toBe("ready")
  })

  it("is never ready while Patronus can't judge the approvals: a bot's approval is no judge, GitHub's wait is told", () => {
    const unjudged: ReviewReadiness[] = [
      { kind: "unknown", reason: "approvalsNotEvaluated" },
      { kind: "unknown", reason: "skipped" },
      { kind: "unavailable", reason: "error", message: "Patronus is restarting" },
    ]
    for (const readiness of unjudged) {
      // nobody asked, only a bot approved: the start is the user's step, Patronus checks the approvals then
      expect(landing(withReadiness(readiness))).toEqual({ kind: "yourStep", label: "Safe Push: Patronus checks the approvals when the robot starts" })
      // the pull request waits for a requested reviewer, also one asked again after a review
      const waiting = { ...openPullRequest, waitingFor: ["ikorennoy"] }
      expect(landing(withReadiness(readiness, { pullRequest: waiting }))).toEqual({
        kind: "waiting",
        label: "Waits for ikorennoy on #1078 · Patronus can't tell yet whether the approvals there count: the start checks them\nOpen Pull Request #1078",
      })
      expect(landing(withReadiness(readiness, { pullRequest: { ...openPullRequest, changesRequestedBy: ["alice"] } })).kind).toBe("needsFix")
    }
    // no pull request GitHub lists: nobody judges either
    expect(landing(facts({ pullRequestsAvailable: false })).kind).toBe("yourStep")
  })

  it("waits before the first answer of Patronus, never a green that turns blue", () => {
    expect(landing(facts())).toEqual({ kind: "waiting", label: "Waits for the answer of Patronus about the approvals", pending: true })
    expect(landing(facts({ patronusAvailable: false, robot: { kind: "none" } }), { patronusKnown: false })).toEqual({
      kind: "waiting",
      label: "Waits for the answer of Patronus",
      pending: true,
    })
  })

  it("is the user's step before it lands: a pull request, a review request, a retarget, a draft", () => {
    expect(landing(withReadiness(missing([group()]), { pullRequest: null }))).toEqual({ kind: "yourStep", label: "Code owners must approve in a pull request: IJPL Code Platform" })
    expect(landing(withReadiness(missing([group({ requested: [] })])))).toMatchObject({ kind: "yourStep" })
    const virtualBase = { ...openPullRequest, base: "user/virtual/auth", intoTarget: false, baseIsVirtualBranch: true }
    expect(landing(withReadiness(approved, { pullRequest: virtualBase }))).toEqual({ kind: "yourStep", label: "Approvals in #1078 don't count: its base is user/virtual/auth" })
    expect(landing(withReadiness(approved, { pullRequest: { ...openPullRequest, isDraft: true } }))).toMatchObject({ kind: "yourStep" })
  })

  it("needs a fix: a sync error, a conflict, a robot that failed on the branch as it is, changes requested", () => {
    expect(landing(facts({ hasError: true }), { status: "Error", statusText: "cannot read" })).toEqual({ kind: "needsFix", label: "cannot read" })
    expect(landing(facts(), { status: "MergeConflict", statusText: "merge conflict" }).kind).toBe("needsFix")
    // the hover shows the robot, whose restart and prompts are buttons; the advice names the restart as its button does
    expect(landing(withReadiness(approved, { robot: finished("failed"), restart }))).toEqual({
      kind: "needsFix",
      label: "Safe Push failed on wsm-skip-wa-2 as it is: fix it, or Restart Safe Push",
      robot: true,
    })
    const changes = missing([group({ changesRequestedBy: [{ login: "ikorennoy", fullName: "Ilya Korennoy" }] })])
    expect(landing(withReadiness(changes))).toEqual({ kind: "needsFix", label: "Ilya Korennoy requested changes on #1078" })
  })

  it("is landing while its own Safe Push runs or one of a branch built on it lands it; a dry run lands nothing", () => {
    const running = facts({ robot: { kind: "running", chip: chip("running") } })
    expect(landing(running)).toEqual({ kind: "landing", label: "Landing: Safe Push running", robot: true })
    expect(landing(running, { robotDryRun: true })).toMatchObject({ kind: "waiting", robot: true })
    expect(landing(facts({ held: true }), { landingWith: "oauth" })).toEqual({ kind: "landing", label: "Lands with oauth: its Safe Push runs" })
  })

  it("is the state after a running move of the row from its start on: Landing for a robot that comes, else Waiting", () => {
    const own = (lands: boolean, label: string): FoundMove => ({ move: { key: "wsm-skip-wa-2", lands, label }, with: null })
    const ready = withReadiness(approved)
    expect(landing(ready, { move: own(true, "Landing: Safe Push starting") })).toEqual({ kind: "landing", label: "Landing: Safe Push starting" })
    // a restart of a failed robot is no fix to make any more, nor a step before the answer of Patronus
    expect(landing(withReadiness(approved, { robot: finished("failed"), restart }), { move: own(true, "Landing: Safe Push restarting") }).kind).toBe("landing")
    expect(landing(facts(), { move: own(true, "Landing: Safe Push starting") }).kind).toBe("landing")
    expect(landing(withReadiness(missing([group({ requested: [] })])), { move: own(false, "Requesting a review on #1078") })).toEqual({
      kind: "waiting",
      label: "Requesting a review on #1078",
    })
    expect(landing(ready, { move: own(false, "Dry Run starting: it checks the branch, it lands nothing") }).kind).toBe("waiting")
    const stack: FoundMove = { move: { key: "stack:air", lands: true, label: "Landing: Safe Push starting" }, with: "the whole air stack" }
    expect(landing(ready, { move: stack })).toEqual({ kind: "landing", label: "Lands with the whole air stack: its Safe Push starts" })
    // a conflict is still the user's to fix
    expect(landing(ready, { status: "MergeConflict", statusText: "merge conflict", move: own(true, "Landing: Safe Push starting") }).kind).toBe("needsFix")
  })

  it("waits for others: the reviewers, its parent, a robot that lands an ancestor, a base that may be meant", () => {
    expect(landing(withReadiness(missing([group()])))).toEqual({ kind: "waiting", label: "Waits for the approval of IJPL Code Platform: Ilya Korennoy is requested on #1078" })
    expect(landing(facts({ blocked: true }), { status: "Blocked", blockedBy: "auth" })).toEqual({ kind: "waiting", label: "Blocked by auth, which doesn't build", shows: "auth" })
    const ancestor = "A Safe Push of login is running: wait until it merges"
    expect(landing(withReadiness(approved, { startBlocker: ancestor }))).toEqual({ kind: "waiting", label: ancestor })
    // the approvals still wait in the pull request whatever runs below
    expect(landing(withReadiness(missing([group()]), { startBlocker: ancestor })).label).toMatch(/^Waits for the approval/)
    const releaseBase = { ...openPullRequest, base: "release-252", intoTarget: false }
    expect(landing(withReadiness(approved, { pullRequest: releaseBase }))).toEqual({ kind: "waiting", label: "Approvals in #1078 don't count: its base is release-252" })
    // the pull request of the whole stack collects its approvals: its step is in the island header
    const stackPullRequest = { number: 12, subject: "the whole air stack", open: () => {} }
    expect(landing(withReadiness(missing([group()]), { pullRequest: null, stackPullRequest })).kind).toBe("waiting")
  })

  it("is in master once the baseline has it or a merge says so, also over a robot that failed before", () => {
    expect(landing(facts({ landedIn: "origin/master" }))).toEqual({ kind: "inMaster", label: "Landed in origin/master" })
    expect(landing(facts({ landedWith: "Landed in master with oauth", held: true }))).toEqual({ kind: "inMaster", label: "Landed in master with oauth" })
    expect(landing(facts({ robot: finished("merged") }))).toEqual({ kind: "inMaster", label: "Merged by Patronus" })
    expect(landing(facts({ landedIn: "origin/master", robot: finished("failed"), restart })).kind).toBe("inMaster")
  })

  it("keeps its state while a sync or an operation on the row runs", () => {
    const ready = withReadiness(approved)
    expect(landing({ ...ready, syncing: true }, { status: "Syncing" })).toEqual(landing(ready))
    expect(landing({ ...ready, busy: { id: "rename", label: "Renaming auth to login" } })).toEqual(landing(ready))
  })

  it("lands by its pull request in a repository that is not on Space", () => {
    const notOnSpace = { patronusAvailable: false, robot: { kind: "none" } } as const
    expect(landing(facts({ ...notOnSpace, pullRequest: null }))).toEqual({ kind: "yourStep", label: "Not in a pull request" })
    expect(landing(facts(notOnSpace))).toEqual({ kind: "waiting", label: "Lands through #1078" })
    expect(landing(facts({ ...notOnSpace, pullRequest: { ...openPullRequest, state: "merged", mergedCurrent: true } }))).toEqual({
      kind: "inMaster",
      label: "Pull request #1078 is merged",
    })
    // it lands through the pull request: its failing checks and changes requested are the author's to fix
    expect(landing(facts({ ...notOnSpace, pullRequest: { ...openPullRequest, checksFailing: true } }))).toEqual({ kind: "needsFix", label: "Checks fail on #1078" })
    expect(landing(facts({ ...notOnSpace, pullRequest: { ...openPullRequest, waitingFor: ["ikorennoy"] } }))).toEqual({ kind: "waiting", label: "Waits for ikorennoy on #1078" })
    // GitHub doesn't list the pull requests: the push is the step it shows
    const unlisted = facts({ ...notOnSpace, pullRequestsAvailable: false })
    expect(landing(unlisted, { pushPending: true })).toEqual({ kind: "yourStep", label: "Not on the remote yet: push it" })
    expect(landing(unlisted)).toEqual({ kind: "waiting", label: "Pushed: it lands through its pull request" })
  })
})
