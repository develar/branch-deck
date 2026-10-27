// @spec spec/commit/commit-row.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { rewordCommit } from "@/hooks/useRewordCommit"
import type { Outcome, RewordResult, UndoToken } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { CommitUnion } from "@/lib/commit"
import { activityStore, canUndoActivity, clearActivity, selectActivity, undoActivity, UNDO_WINDOW_MS } from "@/stores/activity"
import { fakeActivity } from "@/test-support/fake-activity"

const { toasts, selection, WITNESS } = vi.hoisted(() => ({
  // what each `toast.add` showed, its action included
  toasts: [] as { title?: string; description?: string; timeout?: number; actionProps?: { onClick?: () => void } }[],
  /** The repository the main window shows */
  selection: { path: "/repo" },
  /** The table the window shows, which a reword is named on */
  WITNESS: { head: "h", storeTips: { issueGroups: null, stackParents: null, parked: null } },
}))

/** What the session answers the next reword with */
const reworded: { answer: RewordResult } = { answer: { newCommitIds: {}, previousHead: "", newHead: "", undoToken: null } }

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@tauri-apps/api/core", async () => await import("@/test-support/channel-mock"))
vi.mock("@/components/ui/toast", () => ({
  toast: {
    add: (toast: (typeof toasts)[number]) => {
      toasts.push(toast)
      return `toast-${toasts.length}`
    },
    close: () => {},
  },
}))
vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: selection.path }),
  getSelectedRepositoryPath: () => selection.path,
  isSelectedRepository: (path: string) => path === selection.path,
  useSelectedProject: () => ({ path: selection.path }),
  getEffectiveBranchPrefix: () => "user",
}))
vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ isSyncing: false, operations: {} }) },
  getBranchesOfCommits: () => ["auth"],
}))
vi.mock("@/stores/commit-move", () => ({ requestReveal: () => {} }))
// the rewrites run in the engine: the window holds no session view in this test, the table it shows is at HEAD
vi.mock("@/model/store", () => ({ whenSessionViewAt: async () => {}, getSessionWitness: () => WITNESS }))
vi.mock("@/stores/stack-collapse", () => ({ forgetCollapsedStack: () => {}, renameCollapsedStack: () => {} }))
vi.mock("@/lib/bindings", async importOriginal => {
  const original = await importOriginal<typeof import("@/lib/bindings")>()
  const { fakeActivity } = await import("@/test-support/fake-activity")
  return { ...original, commands: { ...original.commands, ...fakeActivity.commands, execute: vi.fn(), cancelOperation: vi.fn() } }
})

const OLD_ID = "1111111111111111111111111111111111111111"
const NEW_ID = "2222222222222222222222222222222222222222"
const TOKEN = `${OLD_ID}..${NEW_ID}`
const commit = { originalHash: OLD_ID, message: "(auth) Add login" } as unknown as CommitUnion

/** The `execute` of the Undo of the reword in `repositoryPath`: the rewrite `undo` of its regroup token */
function undoOf(repositoryPath: string) {
  return { repositoryPath, intent: { kind: "rewrite", rewrite: "undo", token: { kind: "regroup", token: TOKEN } }, fingerprint: "" }
}

/** The `execute` of the reword of `commitId` to `message` in /repo, as the page sends it: named on the table it shows */
function rewordOf(commitId: string, message: string) {
  return { repositoryPath: "/repo", intent: { kind: "rewrite", rewrite: "reword", rewords: [{ commit: commitId, text: { message } }], witness: WITNESS }, fingerprint: "" }
}

/** The outcome of the operation `op` whose core answered `result`, undone by `undo` */
function outcomeOf(op: string, result: Outcome["result"], undo: UndoToken | null): Outcome {
  return { op, effects: [], newsKey: null, versionAfter: null, result, undo, stats: { gitProcesses: {}, httpRequests: 0, millis: 1 } }
}

/** The session answers the reword with `answer`, its undo by the regroup token when it changed a message */
function answer(result: RewordResult) {
  reworded.answer = result
}

/** The executes of the page: the reword, then the Undo */
function executes() {
  return vi.mocked(commands.execute).mock.calls.map(([params]) => params)
}

function entries() {
  return selectActivity(activityStore.getState(), "/repo")
}

/** The entry of the rewrite once its record answered, with its Undo */
async function recorded() {
  await vi.waitFor(() => expect(entries()[0]?.undo).toBeDefined())
  return entries()[0]!
}

describe("rewordCommit", () => {
  beforeEach(async () => {
    vi.useFakeTimers({ now: 1_000_000 })
    toasts.length = 0
    selection.path = "/repo"
    fakeActivity.reset()
    await clearActivity("/repo")
    // each rewrite ends (`done` on its channel) before it answers, once its sync ended
    vi.mocked(commands.execute).mockImplementation(async (params, events) => {
      const intent = params.intent
      const outcome =
        intent.kind === "rewrite" && intent.rewrite === "reword"
          ? outcomeOf(
              "op-reword",
              { kind: "rewrite", rewrite: "reworded", answer: reworded.answer },
              reworded.answer.undoToken == null ? null : { kind: "regroup", token: reworded.answer.undoToken },
            )
          : outcomeOf("op-undo", { kind: "rewrite", rewrite: "undone", answer: { alreadyUndone: false } }, null)
      events.onmessage({ kind: "done", outcome })
      return { status: "ok", data: outcome }
    })
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
  })

  it("records a reword with its Undo for the window of the toast, once", async () => {
    answer({ newCommitIds: { [OLD_ID]: NEW_ID }, previousHead: OLD_ID, newHead: NEW_ID, undoToken: TOKEN })
    const follow = vi.fn()

    expect(await rewordCommit(commit, "(auth) Add the login form\n\nWith a body", follow)).toBe(true)

    expect(executes()).toEqual([rewordOf(OLD_ID, "(auth) Add the login form\n\nWith a body")])
    expect(follow.mock.calls).toEqual([[OLD_ID, NEW_ID]])
    expect(toasts.map(toast => [toast.title, toast.description, toast.timeout])).toEqual([["Reworded 11111111", "(auth) Add the login form", UNDO_WINDOW_MS]])
    const entry = await recorded()
    expect([entry.kind, entry.title, entry.undo, fakeActivity.page.map(it => it.activity.repositoryPath)]).toEqual([
      "rewrite",
      "Reworded 11111111",
      { until: 1_000_000 + UNDO_WINDOW_MS },
      ["/repo"],
    ])
    expect(canUndoActivity(entry)).toBe(true)

    undoActivity(entry)
    await vi.waitFor(() => expect(follow.mock.calls).toHaveLength(2))
    expect(executes().slice(1)).toEqual([undoOf("/repo")])
    // the preview on the new commit follows the Undo back, once it is written, before its sync
    expect(follow.mock.calls[1]).toEqual([NEW_ID, OLD_ID])
    await vi.waitFor(() => expect(entries()[0]!.undo?.undone).toBe(true))

    // the toast's Undo after the one in Activity undoes nothing more
    toasts[0]!.actionProps!.onClick!()
    await vi.advanceTimersByTimeAsync(0)
    expect(executes()).toHaveLength(2)
  })

  it("undoes the reword in its repository when the toast's Undo comes after a switch of the repository", async () => {
    answer({ newCommitIds: { [OLD_ID]: NEW_ID }, previousHead: OLD_ID, newHead: NEW_ID, undoToken: TOKEN })
    await rewordCommit(commit, "(auth) Add the login form", () => {})
    selection.path = "/other"

    toasts[0]!.actionProps!.onClick!()
    await vi.waitFor(() => expect(executes().slice(1)).toEqual([undoOf("/repo")]))
  })

  it("rewords the commit as the rewrite before it in the queue left it, its preview and its Undo on that commit", async () => {
    // a move before the reword gave the commit another id; the session rewords it once its turn comes, through the id
    // the move answered
    const MOVED_ID = "3333333333333333333333333333333333333333"
    answer({ newCommitIds: { [MOVED_ID]: NEW_ID }, previousHead: MOVED_ID, newHead: NEW_ID, undoToken: TOKEN })
    const follow = vi.fn()

    expect(await rewordCommit(commit, "(auth) Add the login form", follow)).toBe(true)

    // the page names the commit on the table it shows
    expect(executes()).toEqual([rewordOf(OLD_ID, "(auth) Add the login form")])
    expect(follow.mock.calls).toEqual([[MOVED_ID, NEW_ID]])
    expect(toasts.map(toast => toast.title)).toEqual(["Reworded 33333333"])
  })

  it("offers no Undo past the window of the toast", async () => {
    answer({ newCommitIds: { [OLD_ID]: NEW_ID }, previousHead: OLD_ID, newHead: NEW_ID, undoToken: TOKEN })
    await rewordCommit(commit, "(auth) Add the login form", () => {})
    await recorded()

    vi.advanceTimersByTime(UNDO_WINDOW_MS)
    const entry = entries()[0]!
    expect(canUndoActivity(entry)).toBe(false)
    undoActivity(entry)
    expect(executes()).toHaveLength(1)
  })

  it("records nothing for an unchanged message", async () => {
    answer({ newCommitIds: { [OLD_ID]: OLD_ID }, previousHead: OLD_ID, newHead: OLD_ID, undoToken: null })
    const follow = vi.fn()

    expect(await rewordCommit(commit, "(auth) Add login\n", follow)).toBe(true)

    expect(executes()).toHaveLength(1)
    expect([toasts, entries(), follow.mock.calls]).toEqual([[], [], []])
  })
})
