// The push of a branch alone as the page runs it: the intent `Push` of the engine (`prepare`, `execute`, mocked here as
// the session answers them), and what the page says of its plan and outcome: the toast of the pushes of the branch, the
// question of its push button, the refusal, the remote that moved
import { createElement, Fragment } from "react"
import type { ReactNode } from "react"
import { renderToStaticMarkup } from "react-dom/server"
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { EffectRecord, EffectStatus, ErrorCode, Intent, Outcome, PatronusRobot, Plan, PlannedEffect, PullRequest } from "@/lib/bindings"
import { resetEventToasts } from "@/lib/event-toast"
import { pushBranch, pushWithChildren } from "@/lib/push-run"
import type { Branch } from "@/stores/branch-sync"
import { createPullRequestDialogStore } from "@/stores/create-pull-request-dialog"
import { pullRequestsStore } from "@/stores/pull-requests"
import { pushConfirmStore } from "@/stores/push-confirm"
import { forcePush, PUSH, testBranch, testRemoteStatus } from "@/stores/test-branch"
import { unpushedBranch } from "@/test-support/push-mocks"
import { openToasts, resetToasts, toasts } from "@/test-support/toast-mock"

const { engine, stores } = vi.hoisted(() => ({
  engine: {
    /** What reached the backend and the clipboard, in order: "prepare auth", "execute auth", "copy …", "open …" */
    calls: [] as string[],
    /** What Activity recorded */
    records: [] as string[],
    /** The plan `prepare` answers of the intent */
    plan: (_intent: Intent): Partial<Plan> => ({}),
    /** The effects of the run and their statuses, of its plan */
    run: (plan: Plan): EffectRecord[] => plan.effects.map(effect => ({ effect, status: { kind: "done" } })),
    /** What the result of the run names besides its effects */
    restarted: null as PatronusRobot | null,
    /** The intents `prepare` was asked */
    intents: [] as Intent[],
  },
  stores: {
    branches: {} as Record<string, Branch>,
    pullRequests: {} as Record<string, PullRequest>,
    selected: "/repo",
  },
}))

// what the toasts and the links of a push reach: the toasts, the browser, the clipboard, the log
vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)
vi.mock("@tauri-apps/plugin-opener", async () => (await import("@/test-support/push-mocks")).openerMock(engine.calls))
vi.mock("@/lib/clipboard", async () => (await import("@/test-support/push-mocks")).clipboardMock(engine.calls))
vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@tauri-apps/api/core", () => import("@/test-support/channel-mock"))

vi.mock("@/lib/bindings", () => ({
  commands: {
    prepare: async ({ intent }: { intent: Intent }) => {
      engine.intents.push(intent)
      const name = intent.kind === "push" && intent.subject.kind === "branch" ? intent.subject.name : ""
      engine.calls.push(`prepare ${name}`)
      const plan: Plan = {
        intent,
        fingerprint: "f",
        lands: [],
        effects: [{ kind: "push", branches: [name], force: false }],
        confirmations: [],
        blockers: [],
        waits: [],
        alternatives: null,
        pullRequest: null,
        approvalsSkippedBy: null,
        ...engine.plan(intent),
      }
      return { status: "ok", data: plan }
    },
    execute: async ({ intent }: { intent: Intent }) => {
      const name = intent.kind === "push" && intent.subject.kind === "branch" ? intent.subject.name : ""
      engine.calls.push(`execute ${name}`)
      const plan = (await (await import("@/lib/bindings")).commands.prepare({ repositoryPath: "", intent })) as { data: Plan }
      engine.calls.pop()
      engine.intents.pop()
      const outcome: Outcome = {
        op: "op",
        effects: engine.run(plan.data),
        newsKey: { kind: "push", branch: name },
        versionAfter: null,
        result: { kind: "push", pushedOver: [], restarted: engine.restarted },
        stats: { gitProcesses: {}, httpRequests: 0, millis: 1 },
      }
      return { status: "ok", data: outcome }
    },
    cancelOperation: async () => true,
  },
}))
vi.mock("@/model/store", async () => (await import("@/test-support/push-mocks")).modelStoreMock(() => stores.pullRequests))
vi.mock("@/stores/branch-sync", async () => {
  const { openPullRequestOf } = await import("@/test-support/open-pull-request")
  return {
    branchSyncStore: { getState: () => ({ baselineBranch: "origin/master", branches: stores.branches }), subscribe: () => () => {} },
    getBranch: (name: string) => stores.branches[name],
    getOpenPullRequest: (name: string) => openPullRequestOf(stores.pullRequests[name]),
    setOperationPushProgress: () => {},
    toRemoteStatus: () => null,
  }
})
vi.mock("@/stores/operations", () => ({ whenRewritesDone: async () => {} }))
vi.mock("@/stores/patronus", () => ({
  getPushHold: (name: string) => (name === "auth" ? { kind: "own", branch: "auth", robot: ROBOT } : null),
  getPatronusGitHubRepository: () => null,
}))
vi.mock("@/stores/pull-requests", async () => (await import("@/test-support/push-mocks")).pullRequestsMock(() => stores.pullRequests))
vi.mock("@/stores/repository", async () => ({
  repositoryStore: (await import("@/test-support/quiet-store")).quietStore,
  getSelectedRepositoryPath: () => stores.selected,
  isSelectedRepository: (path: string) => path === "/repo",
  getRepositorySwitchReason: (path: string) => (path === "/repo" ? null : `${path} is no longer the open repository`),
  getEffectiveBranchPrefix: () => "user",
}))
vi.mock("@/stores/activity", async () => ({
  ...(await import("@/stores/test-store-mocks")).activityMock,
  recordPush: (_repositoryPath: string, branchName: string) => void engine.records.push(`push ${branchName}`),
  recordRobot: (title: string) => void engine.records.push(title),
  recordRetarget: (title: string, description: string) => void engine.records.push(`${title}: ${description}`),
}))
vi.mock("@/stores/unapplied-branches", () => ({ getUnappliedBranch: () => undefined }))
vi.mock("@/lib/robot-toast", () => ({ showRobotToast: (title: string, robot: PatronusRobot) => void engine.calls.push(`robot toast ${title}: ${robot.id}`) }))

/** The Safe Push of auth, which holds its push */
const ROBOT = { id: "r1", status: "running", pushMode: "rebaseAutosquash", branchName: "auth", targetBranch: "master", url: "https://patronus/r1" } as PatronusRobot

/** A branch of one commit the remote lacks; `remote: false` when it isn't on the remote yet */
function addBranch(name: string, parent: string | null, remote = true): void {
  stores.branches[name] = unpushedBranch(name, parent, remote)
}

/** The text of a toast description, which names a branch in medium weight */
function text(description: unknown): string {
  return renderToStaticMarkup(createElement(Fragment, null, description as ReactNode)).replace(/<[^>]+>/g, "")
}

/** The failure of an effect with `code` */
function failed(code: ErrorCode, message: string): EffectStatus {
  return { kind: "failed", error: { code, message, details: null, retry: "no", operation_id: null } }
}

/** The retarget of the pull request #8 of login out of auth, before its push */
const RETARGET: PlannedEffect = { kind: "retarget", branch: "login", number: 8, from: "user/virtual/auth", to: "master" }

beforeEach(() => {
  engine.calls.length = 0
  engine.records = []
  engine.intents = []
  engine.plan = () => ({})
  engine.run = plan => plan.effects.map(effect => ({ effect, status: { kind: "done" } }))
  engine.restarted = null
  stores.branches = {}
  stores.pullRequests = {}
  stores.selected = "/repo"
  pushConfirmStore.setState({ request: null })
  resetEventToasts()
  resetToasts()
  addBranch("auth", null, false)
})

describe("the toast of a push", () => {
  it("offers to create the pull request of a branch without one, into the baseline", async () => {
    expect(await pushBranch("auth")).toBe(true)
    expect(engine.calls).toEqual(["prepare auth", "execute auth"])
    // the remote head the row showed: none, the push creates the branch
    expect(engine.intents).toEqual([{ kind: "push", subject: { kind: "branch", name: "auth" }, scope: { kind: "alone" }, expectRemote: null, force: false, restartRunning: false }])
    expect(toasts).toHaveLength(1)
    const toast = toasts[0]!
    expect(toast.title).toBe("Pushed auth")
    expect(text(toast.description)).toBe("1 commit · created the remote branch")
    expect(toast.actionProps?.children).toBe("Create Pull Request…")
    toast.actionProps!.onClick()
    expect(createPullRequestDialogStore.getState().branchName).toBe("auth")
  })

  it("is one toast per branch: a later push of it takes its place, its pull request action as the branch has it now", async () => {
    expect(await pushBranch("auth")).toBe(true)
    const pushed = openToasts()[0]!
    expect(pushed.actionProps?.children).toBe("Create Pull Request…")
    // created meanwhile on another surface (the row, bd): the toast opens it
    stores.pullRequests.auth = { url: "https://github.com/JetBrains/ultimate/pull/7", state: "open" } as PullRequest
    pullRequestsStore.setState({})
    expect(openToasts()).toEqual([pushed])
    expect(pushed.actionProps?.children).toBe("Open Pull Request")

    addBranch("auth", null)
    expect(await pushBranch("auth")).toBe(true)
    expect(openToasts()).toEqual([pushed])
    expect(text(pushed.description)).toBe("1 commit")
  })

  it("opens the pull request the branch has", async () => {
    stores.pullRequests.auth = { url: "https://github.com/JetBrains/ultimate/pull/7", state: "open" } as PullRequest
    expect(await pushBranch("auth")).toBe(true)
    const toast = toasts.at(-1)!
    expect(toast.actionProps?.children).toBe("Open Pull Request")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(engine.calls).toContain("open https://github.com/JetBrains/ultimate/pull/7"))
  })

  it("offers a new pull request when the branch has only a closed one", async () => {
    stores.pullRequests.auth = { url: "https://github.com/JetBrains/ultimate/pull/7", state: "closed" } as PullRequest
    expect(await pushBranch("auth")).toBe(true)
    expect(toasts.at(-1)!.actionProps?.children).toBe("Create Pull Request…")
  })

  it("retargets a pull request into a virtual branch to the baseline before the push, and Activity records it", async () => {
    addBranch("login", "auth")
    stores.pullRequests.login = { number: 8, state: "open", baseRefName: "user/virtual/auth", url: "https://github.com/JetBrains/ultimate/pull/8" } as PullRequest
    engine.plan = () => ({ effects: [RETARGET, { kind: "push", branches: ["login"], force: false }] })
    expect(await pushBranch("login")).toBe(true)
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit · retargeted pull request #8 to master")
    expect(engine.records).toEqual(["Retargeted #8 to master: login · it went into auth", "push login"])
  })

  it("pushes and copies the command that retargets a pull request when the retarget fails", async () => {
    addBranch("login", "auth")
    stores.pullRequests.login = { number: 8, state: "open", baseRefName: "user/virtual/auth" } as PullRequest
    engine.plan = () => ({ effects: [RETARGET, { kind: "push", branches: ["login"], force: false }] })
    engine.run = plan => plan.effects.map(effect => (effect.kind === "retarget" ? { effect, status: failed("network_error", "HTTP 403") } : { effect, status: { kind: "done" } }))
    expect(await pushBranch("login")).toBe(true)
    expect(toasts.some(toast => toast.title === "Cannot retarget pull request #8")).toBe(true)
    const toast = toasts.at(-1)!
    expect(text(toast.description)).toBe("1 commit · the pull request goes into auth: retarget it to master")
    expect(toast.actionProps?.children).toBe("Copy gh pr edit")
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(engine.calls).toContain("copy gh pr edit 8 --base master"))
  })

  it("says nothing for a push that is a step of another action, and Activity records it", async () => {
    expect(await pushBranch("auth", { announce: false })).toBe(true)
    expect(toasts).toEqual([])
    expect(engine.records).toEqual(["push auth"])
  })

  it("says only the restart a push that is a step of another action made, in the toast of its robot and Activity", async () => {
    engine.restarted = { ...ROBOT, id: "r2" }
    expect(await pushBranch("auth", { announce: false, restartRunning: true })).toBe(true)
    expect(engine.records).toEqual(["push auth", "Safe Push restarted"])
    expect(engine.calls).toEqual(["prepare auth", "execute auth", "robot toast Safe Push restarted: r2"])
    expect(toasts).toEqual([])
  })
})

describe("the plan of a push", () => {
  it("opens the description of its push button when it asks first: the push drops remote commits the branch lacks", async () => {
    stores.branches.auth = testBranch("auth", null, {
      remoteStatus: testRemoteStatus({ commitsAhead: 1, commitsBehind: 1, remoteTip: "theirs", action: forcePush({ drops: true }) }),
    })
    engine.plan = () => ({ confirmations: [{ kind: "forcePushDrops", branch: "auth", commits: 1 }] })
    expect(await pushBranch("auth", { confirmAt: "preview" })).toBe(false)
    expect(engine.calls).toEqual(["prepare auth"])
    expect(pushConfirmStore.getState().request).toMatchObject({ branchName: "auth", target: "preview", options: { expectRemote: "theirs" } })
  })

  it("of a stack asks first in the push button of the branch that drops remote commits, whose Force Push runs it again forced", async () => {
    addBranch("login", "auth")
    const drops = { kind: "forcePushDrops", branch: "auth", commits: 1 } as const
    engine.plan = intent => ({
      effects: [{ kind: "push", branches: ["auth", "login"], force: true }],
      confirmations: intent.kind === "push" && intent.force ? [] : [drops],
    })
    expect(await pushWithChildren("auth")).toBe(false)
    // nothing ran: the user decides on the drop
    expect(engine.calls).toEqual(["prepare auth"])
    const request = pushConfirmStore.getState().request
    expect(request).toMatchObject({ branchName: "auth", target: "row" })
    request!.forced!()
    await vi.waitFor(() => expect(engine.calls).toEqual(["prepare auth", "prepare auth", "execute auth"]))
    expect(engine.intents.at(-1)).toMatchObject({ scope: { kind: "withChildren" }, force: true })
  })

  it("of a stack that drops remote commits of more than one branch is refused, each drop named", async () => {
    addBranch("login", "auth")
    engine.plan = () => ({
      effects: [{ kind: "push", branches: ["auth", "login"], force: true }],
      confirmations: [
        { kind: "forcePushDrops", branch: "auth", commits: 1 },
        { kind: "forcePushDrops", branch: "login", commits: 2 },
      ],
    })
    expect(await pushWithChildren("auth")).toBe(false)
    expect(engine.calls).toEqual(["prepare auth"])
    expect(pushConfirmStore.getState().request).toBeNull()
    const toast = toasts.at(-1)!
    expect(toast.title).toBe("Cannot push auth")
    expect(text(toast.description)).toBe("auth drops a commit of its remote branch it lacks; login drops 2 commits of its remote branch it lacks: nothing was pushed")
  })

  it("is refused in the words of its blocker, and the toast opens the Safe Push that holds the branch", async () => {
    const hold = { kind: "own", by: { branch: "auth", robotId: "r1", wholeStack: false }, carried: null } as const
    engine.plan = () => ({ blockers: [{ code: "held", branch: "auth", hold, mode: "rebaseAutosquash", target: "master" }] })
    expect(await pushBranch("auth")).toBe(false)
    expect(engine.calls).toEqual(["prepare auth"])
    const toast = toasts.at(-1)!
    expect(toast.title).toBe("Cannot push auth")
    expect(toast.actionProps?.children).toBe("Open in Patronus")
  })

  it("pushes nothing of the branch of the same name in another repository the main window shows now, and says why", async () => {
    stores.selected = "/other"
    expect(await pushBranch("auth")).toBe(false)
    expect(engine.calls).toEqual([])
  })

  it("opens the toast of the remote that moved: its push goes over what the remote has, leased on it", async () => {
    addBranch("auth", null)
    engine.run = plan => {
      // the session has the remote branch as the fetch after the refusal found it, which the toast pushes over
      stores.branches.auth = testBranch("auth", null, { remoteStatus: testRemoteStatus({ commitsAhead: 1, remoteTip: "theirs", action: PUSH }) })
      return plan.effects.map(effect => ({ effect, status: failed("lease_rejected", "moved") }))
    }
    expect(await pushBranch("auth")).toBe(false)
    const toast = toasts.at(-1)!
    expect(toast.type).toBe("warning")
    expect(engine.records).toEqual(["push auth"])
    engine.run = plan => plan.effects.map(effect => ({ effect, status: { kind: "done" } }))
    toast.actionProps!.onClick()
    await vi.waitFor(() => expect(engine.calls.filter(call => call === "execute auth")).toHaveLength(2))
    expect(engine.intents.at(-1)).toMatchObject({ expectRemote: "theirs" })
  })
})
