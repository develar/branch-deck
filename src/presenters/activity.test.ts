// @spec spec/repository/activity.spec.md
import { describe, expect, it, vi } from "vite-plus/test"
import type { ApiError, EffectRecord, Intent, JournalEntry, OperationResult, PageEntry, PatronusRobot, PullRequest, RemoteStatusUpdate } from "@/lib/bindings"
import { presentActivity } from "@/presenters/activity"
import { fakeRemoteStatus } from "@/test-support/fake-sessions"

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async () => {} }))

const GITHUB = { owner: "acme", name: "app" }

const ROBOT: PatronusRobot = {
  id: "r1",
  name: "Add login",
  status: "running",
  pushMode: "rebaseAutosquash",
  branchName: "auth",
  sourceRevision: "tip",
  targetBranch: "master",
  startTime: 1,
  finishTime: null,
  cancellationReason: null,
  url: "https://patronus.example.com/robots/r1",
  pullRequest: null,
  profile: null,
} as PatronusRobot

const PUSH: Intent = { kind: "push", subject: { kind: "branch", name: "auth" }, scope: { kind: "alone" }, expectRemote: null, force: false, restartRunning: false }
const LAND: Intent = { kind: "land", subject: { kind: "branch", name: "auth" }, mode: "rebaseAutosquash", profile: { kind: "default" } }
const DONE = { kind: "done" } as const

function apiError(code: ApiError["code"], message: string): ApiError {
  return { code, message, details: null, retry: "no", operation_id: null }
}

/** The journal entry of the operation `id` of `intent` with its effects, as it ended at `endedAt` */
function operation(id: string, intent: Intent, effects: EffectRecord[], patch: Partial<JournalEntry> = {}): JournalEntry {
  return {
    id,
    tool: "app",
    pid: 1,
    worktree: "/a",
    prefix: "user",
    leaseSlot: false,
    intent,
    subject: "auth",
    startedAt: 100,
    endedAt: 101,
    effects,
    result: { kind: "none" },
    error: null,
    stats: { gitProcesses: {}, httpRequests: 0, millis: 1 },
    transition: null,
    ...patch,
  }
}

function pushed(branches: string[], remote: (RemoteStatusUpdate | null)[], status: EffectRecord["status"] = DONE): EffectRecord {
  return { effect: { kind: "push", branches, force: false }, status, remote }
}

function said(entries: JournalEntry[], page: PageEntry[] = [], repositories = ["/a"]) {
  return presentActivity({ journals: [{ path: "/a/.git/branch-deck/journal.jsonl", repositories, available: true, entries }], page }, GITHUB).map(entry => ({
    repositoryPath: entry.repositoryPath,
    kind: entry.kind,
    title: entry.title,
    description: entry.description,
    ...(entry.warning === true && { warning: true }),
    ...(entry.details !== undefined && { details: entry.details }),
    ...(entry.agent !== undefined && { agent: entry.agent }),
  }))
}

describe("presentActivity", () => {
  it("says each branch a push sent from the status it went over, a force push with the remote commits it dropped", () => {
    const created = { ...fakeRemoteStatus("auth", 4), remoteExists: false, remoteTip: null, action: { kind: "create" as const } }
    const dropped = [{ id: "c0ffee", subject: "Fix login", author: "Ann", authorEmail: "ann@example.com", authorTime: 1 }]
    const forced = {
      ...fakeRemoteStatus("auth", 1),
      commitsBehind: 3,
      fastForward: false,
      droppedCommits: dropped,
      action: { kind: "force" as const, drops: true, replaces: false },
    }
    const first = operation("op1", PUSH, [pushed(["auth"], [created])], { endedAt: 101 })
    const second = operation("op2", PUSH, [pushed(["auth"], [forced])], { endedAt: 101 })
    expect(said([first, second])).toEqual([
      {
        repositoryPath: "/a",
        kind: "push",
        title: "Force pushed auth",
        description: "1 commit · dropped 3 remote commits",
        warning: true,
        details: { type: "dropped", commits: dropped, more: 2 },
      },
      { repositoryPath: "/a", kind: "push", title: "Pushed auth", description: "4 commits · created the remote branch" },
    ])
  })

  it("says a remote that moved under the push of a branch, nothing pushed", () => {
    const moved = { ...fakeRemoteStatus("auth", 1), commitsBehind: 1, droppedCommits: [{ id: "c1", subject: "Fix", author: "Ann", authorEmail: "a@x", authorTime: 1 }] }
    const refused = operation("op1", PUSH, [pushed(["auth"], [moved], { kind: "failed", error: apiError("lease_rejected", "The remote branch of auth moved") })])
    expect(said([refused])).toEqual([
      { repositoryPath: "/a", kind: "push", title: "auth changed on the remote", description: "1 commit you don't have (by Ann). Nothing was pushed.", warning: true },
    ])
  })

  it("says the retarget of a pull request out of a virtual branch, with the pull request on GitHub", () => {
    const retarget: EffectRecord = { effect: { kind: "retarget", branch: "login", number: 8, from: "user/virtual/auth", to: "master" }, status: DONE }
    const entry = operation("op1", { ...PUSH, subject: { kind: "branch", name: "login" } }, [retarget, pushed(["login"], [null])], { subject: "login" })
    // newest first: the push came after the retarget
    expect(said([entry])).toEqual([
      { repositoryPath: "/a", kind: "push", title: "Pushed login", description: undefined },
      {
        repositoryPath: "/a",
        kind: "review",
        title: "Retargeted #8 to master",
        description: "login · it went into auth",
        details: { type: "pullRequest", url: "https://github.com/acme/app/pull/8" },
      },
    ])
  })

  it("says the robot a start started in the words of its toast, and the comment Patronus accepted", () => {
    const start: EffectRecord = { effect: { kind: "startRobot", route: { kind: "api", profile: null } }, status: DONE }
    const result = (patch: object): OperationResult => ({ kind: "land", robot: null, canceled: null, comment: null, notes: [], lost: null, ...patch })
    const started = operation("op1", LAND, [start], { result: result({ robot: ROBOT }) })
    const comment = { number: 7, url: "https://github.com/acme/app/pull/7#issuecomment-1", accepted: true }
    const commented = operation("op2", LAND, [{ effect: { kind: "startRobot", route: { kind: "comment", number: 7 } }, status: DONE }], {
      result: result({ comment }),
      endedAt: 100,
    })
    expect(said([commented, started])).toEqual([
      { repositoryPath: "/a", kind: "patronus", title: "Safe Push started", description: "auth → master", details: { type: "robot", url: ROBOT.url } },
      {
        repositoryPath: "/a",
        kind: "patronus",
        title: "Safe Push started",
        description: "Patronus accepted the comment on #7: the robot shows once Patronus lists it",
        details: { type: "pullRequest", url: comment.url },
      },
    ])
  })

  it("says a restart and a cancel of a robot, and the pull request a creation opened", () => {
    const restarted = operation("op1", { kind: "restartRobot", robotId: "r0" }, [], {
      result: { kind: "robot", robot: ROBOT, canceled: null, alreadyFinished: false },
      endedAt: 103,
    })
    const canceled = operation("op2", { kind: "cancelRobot", robotId: "r1" }, [], { result: { kind: "robot", robot: ROBOT, canceled: null, alreadyFinished: false }, endedAt: 102 })
    const pullRequest = { number: 12, url: "https://github.com/acme/app/pull/12", baseRefName: "master", reviewers: [] } as unknown as PullRequest
    const creation = {
      kind: "createPullRequest" as const,
      subject: { kind: "branch" as const, name: "auth" },
      title: "Add auth",
      body: "",
      assignSelf: false,
      reviewers: [],
      onGitHub: false,
    }
    const created = operation("op3", creation, [], { result: { kind: "pullRequest", pullRequest, created: true, assignees: [], warnings: [] }, endedAt: 101 })
    expect(said([created, canceled, restarted]).map(it => [it.kind, it.title, it.description])).toEqual([
      ["patronus", "Safe Push restarted", "auth → master"],
      ["patronus", "Safe Push is canceling", "auth → master"],
      ["review", "Created pull request #12", "auth → master"],
    ])
  })

  it("says the close the user asked and a review, not the closes of a merged robot", () => {
    const close: EffectRecord = { effect: { kind: "closePullRequest", branch: "air", number: 12, comment: "Closed in Branch Deck: auth landed." }, status: DONE }
    const closed = operation("op1", { kind: "closePullRequest", number: 12, comment: "Closed in Branch Deck: auth landed." }, [close], { endedAt: 102 })
    const merged = operation("op2", { kind: "closeLandedPullRequests", landing: { by: "robot", robotId: "r1" } }, [close], { endedAt: 103 })
    const review: EffectRecord = { effect: { kind: "submitReview", number: 7, verdict: "approve", commit: "h" }, status: DONE }
    const reviewed = operation("op3", { kind: "submitReview", number: 7, verdict: "approve", commit: "h", body: null, thenLand: false }, [review], { endedAt: 101 })
    expect(said([reviewed, closed, merged]).map(it => [it.title, it.description, it.details])).toEqual([
      ["Closed pull request #12", "auth landed", { type: "pullRequest", url: "https://github.com/acme/app/pull/12" }],
      ["Approved #7", undefined, { type: "pullRequest", url: "https://github.com/acme/app/pull/7" }],
    ])
  })

  it("says each close of a landing without a robot by its comment, which no toast says", () => {
    const comment = "Landed in master as 0123456 with the push of user/virtual/login, which is built on this branch."
    const close: EffectRecord = { effect: { kind: "closePullRequest", branch: "auth", number: 5, comment }, status: DONE }
    const landed = operation("op1", { kind: "closeLandedPullRequests", landing: { by: "told", id: "landing-1" } }, [close], { endedAt: 102 })
    expect(said([landed]).map(it => [it.title, it.description, it.details])).toEqual([
      ["Closed pull request #5", comment.slice(0, -1), { type: "pullRequest", url: "https://github.com/acme/app/pull/5" }],
    ])
  })

  it("says an operation that failed unless the page recorded the error of its toast, and leaves out the canceled", () => {
    const failed = operation("op1", PUSH, [pushed(["auth"], [null], { kind: "failed", error: apiError("push_refused", "The remote refused the push: hook") })])
    const refused = operation("op2", LAND, [], { error: apiError("busy", "bd runs the Safe Push of auth"), endedAt: 102 })
    const canceled = operation("op4", PUSH, [], { error: apiError("canceled", "Canceled"), endedAt: 104 })
    expect(said([failed, refused, canceled])).toEqual([
      { repositoryPath: "/a", kind: "error", title: "Cannot start the Safe Push of auth", description: "bd runs the Safe Push of auth" },
      { repositoryPath: "/a", kind: "error", title: "Cannot push auth", description: "The remote refused the push: hook" },
    ])
    const toast: PageEntry = {
      id: "p1",
      endedAt: 101,
      activity: { repositoryPath: "/a", kind: "error", title: "Cannot push auth", description: "hook", warning: false, details: null, operationId: "op1" },
    }
    expect(said([failed], [toast])).toEqual([{ repositoryPath: "/a", kind: "error", title: "Cannot push auth", description: "hook" }])
  })

  it("puts the error the page told of an operation where the operation ran, whichever repository the window showed then", () => {
    const failed = operation("op1", PUSH, [pushed(["auth"], [null], { kind: "failed", error: apiError("push_refused", "The remote refused the push: hook") })])
    const toast = (id: string, operationId: string): PageEntry => ({
      id,
      endedAt: 101,
      activity: { repositoryPath: "/b", kind: "error", title: "Cannot push auth", description: "hook", warning: false, details: null, operationId },
    })
    const shown = said([failed], [toast("p1", "op1"), toast("p2", "sync-of-b")], ["/a", "/a-feature"])
    expect(shown.map(it => [it.repositoryPath, it.title, it.description])).toEqual([
      ["/b", "Cannot push auth", "hook"],
      ["/a-feature", "Cannot push auth", "hook"],
      ["/a", "Cannot push auth", "hook"],
    ])
    const ids = presentActivity(
      { journals: [{ path: "/a/.git/branch-deck/journal.jsonl", repositories: ["/a", "/a-feature"], available: true, entries: [failed] }], page: [toast("p1", "op1")] },
      GITHUB,
    ).map(entry => entry.id)
    expect(ids).toEqual(["p1/1", "p1"])
  })

  it("says an operation of an agent as any other, with the worktree of its slot of the lease pool", () => {
    const agents = operation("op3", PUSH, [pushed(["auth"], [null])], { leaseSlot: true, worktree: "/pool/branch-deck-1/1/a", endedAt: 103 })
    const users = operation("op4", PUSH, [pushed(["docs"], [null])], { endedAt: 104 })
    expect(said([agents, users]).map(it => [it.title, it.agent])).toEqual([
      ["Pushed docs", undefined],
      ["Pushed auth", { worktree: "/pool/branch-deck-1/1/a" }],
    ])
  })

  it("says a rewrite and a pick of bd, which no toast told, an agent's marked, and not the rewrites of the app nor an update of bd", () => {
    const witness = { head: "", storeTips: { issueGroups: null, stackParents: null, parked: null } }
    const unstack: Intent = { kind: "rewrite", rewrite: "unstack", branchName: "beta", witness }
    const bd = { tool: "bd" } as const
    const unstacked = operation("op1", unstack, [], { ...bd, subject: "beta", leaseSlot: true, worktree: "/pool/branch-deck-1/1/a", endedAt: 101 })
    const picked = operation("op2", { kind: "pick", source: "/pool/branch-deck-1/1/a" }, [], { ...bd, subject: null, endedAt: 102 })
    const refused = operation("op3", unstack, [], { ...bd, subject: "beta", error: apiError("invalid_plan", "beta is not stacked"), endedAt: 103 })
    const apps = operation("op4", unstack, [], { subject: "beta", endedAt: 104 })
    const updated = operation("op5", { kind: "updateProject", fetch: true }, [], { ...bd, subject: null, endedAt: 105 })
    expect(said([unstacked, picked, refused, apps, updated]).map(it => [it.kind, it.title, it.description, it.agent])).toEqual([
      ["error", "Cannot unstack beta", "beta is not stacked", undefined],
      ["rewrite", "Picked commits", undefined, undefined],
      ["rewrite", "Unstacked beta", undefined, { worktree: "/pool/branch-deck-1/1/a" }],
    ])
  })

  it("puts the operations of a journal in every project of its repository, and the page's entries in theirs, newest first", () => {
    const entry = operation("op1", PUSH, [pushed(["auth"], [null])], { endedAt: 100 })
    const page: PageEntry = {
      id: "p1",
      endedAt: 100,
      activity: { repositoryPath: "/b", kind: "update", title: "Updated master", description: null, warning: false, details: null, operationId: null },
    }
    expect(said([entry], [page], ["/a", "/a-feature"]).map(it => [it.repositoryPath, it.title])).toEqual([
      ["/b", "Updated master"],
      ["/a-feature", "Pushed auth"],
      ["/a", "Pushed auth"],
    ])
  })
})
