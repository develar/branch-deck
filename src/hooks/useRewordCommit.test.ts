// @spec spec/commit/commit-row.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { rewordCommit } from "@/hooks/useRewordCommit"
import type { RewordResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { CommitUnion } from "@/lib/commit"
import { activityStore, canUndoActivity, clearActivity, selectActivity, undoActivity, UNDO_WINDOW_MS } from "@/stores/activity"

const { toasts, selection, syncs, renamed } = vi.hoisted(() => ({
  // what each `toast.add` showed, its action included
  toasts: [] as { title?: string; description?: string; timeout?: number; actionProps?: { onClick?: () => void } }[],
  /** The repository the main window shows */
  selection: { path: "/repo" },
  /** The syncs the rewrites asked for */
  syncs: { count: 0 },
  /** The ids the rewrites before a reword gave its commits, as the last sync shows them */
  renamed: new Map<string, string>(),
}))

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
  branchSyncStore: { getState: () => ({ isSyncing: false }) },
  getBranchesOfCommits: () => ["auth"],
  resolveCommitsOf: (_repositoryPath: string, commits: readonly { originalHash: string }[]) =>
    commits.map(commit => ({ ...commit, originalHash: renamed.get(commit.originalHash) ?? commit.originalHash })),
  syncBranches: async () => {
    syncs.count++
  },
}))
vi.mock("@/stores/commit-move", () => ({ requestReveal: () => {} }))
vi.mock("@/stores/stack-collapse", () => ({ forgetCollapsedStack: () => {}, renameCollapsedStack: () => {} }))
vi.mock("@/lib/bindings", async importOriginal => {
  const original = await importOriginal<typeof import("@/lib/bindings")>()
  return { ...original, commands: { ...original.commands, reword: vi.fn(), undoRegroup: vi.fn() } }
})

const OLD_ID = "1111111111111111111111111111111111111111"
const NEW_ID = "2222222222222222222222222222222222222222"
const TOKEN = `${OLD_ID}..${NEW_ID}`
const commit = { originalHash: OLD_ID, message: "(auth) Add login" } as unknown as CommitUnion

function answer(result: RewordResult) {
  vi.mocked(commands.reword).mockResolvedValue({ status: "ok", data: result })
}

function entries() {
  return selectActivity(activityStore.getState(), "/repo")
}

describe("rewordCommit", () => {
  beforeEach(() => {
    vi.useFakeTimers({ now: 1_000_000 })
    toasts.length = 0
    selection.path = "/repo"
    syncs.count = 0
    renamed.clear()
    clearActivity("/repo")
    vi.mocked(commands.undoRegroup).mockResolvedValue({ status: "ok", data: null })
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
  })

  it("records a reword with its Undo for the window of the toast, once", async () => {
    answer({ newCommitIds: { [OLD_ID]: NEW_ID }, previousHead: OLD_ID, newHead: NEW_ID, undoToken: TOKEN })
    const follow = vi.fn()

    expect(await rewordCommit(commit, "(auth) Add the login form\n\nWith a body", follow)).toBe(true)

    expect(vi.mocked(commands.reword)).toHaveBeenCalledWith(
      {
        repositoryPath: "/repo",
        rewords: [{ commit: OLD_ID, text: { message: "(auth) Add the login form\n\nWith a body" } }],
      },
      expect.anything(),
    )
    expect(follow.mock.calls).toEqual([[OLD_ID, NEW_ID]])
    expect(toasts.map(toast => [toast.title, toast.description, toast.timeout])).toEqual([["Reworded 11111111", "(auth) Add the login form", UNDO_WINDOW_MS]])
    const entry = entries()[0]!
    expect([entry.kind, entry.title, entry.undo]).toEqual(["rewrite", "Reworded 11111111", { until: 1_000_000 + UNDO_WINDOW_MS }])
    expect(canUndoActivity(entry)).toBe(true)

    undoActivity(entry)
    await vi.waitFor(() => expect(follow.mock.calls).toHaveLength(2))
    expect(vi.mocked(commands.undoRegroup)).toHaveBeenCalledExactlyOnceWith({ repositoryPath: "/repo", token: TOKEN }, expect.anything())
    // the preview on the new commit follows the Undo back
    expect(follow.mock.calls[1]).toEqual([NEW_ID, OLD_ID])
    expect(entries()[0]!.undo?.undone).toBe(true)

    // the toast's Undo after the one in Activity undoes nothing more
    toasts[0]!.actionProps!.onClick!()
    await vi.advanceTimersByTimeAsync(0)
    expect(vi.mocked(commands.undoRegroup)).toHaveBeenCalledOnce()
  })

  it("undoes the reword in its repository when the toast's Undo comes after a switch of the repository", async () => {
    answer({ newCommitIds: { [OLD_ID]: NEW_ID }, previousHead: OLD_ID, newHead: NEW_ID, undoToken: TOKEN })
    await rewordCommit(commit, "(auth) Add the login form", () => {})
    expect(syncs.count).toBe(1)
    selection.path = "/other"

    toasts[0]!.actionProps!.onClick!()
    await vi.waitFor(() => expect(vi.mocked(commands.undoRegroup)).toHaveBeenCalledExactlyOnceWith({ repositoryPath: "/repo", token: TOKEN }, expect.anything()))
    // the table shows /other: nothing of it is synced for the undo of /repo
    await vi.advanceTimersByTimeAsync(0)
    expect(syncs.count).toBe(1)
  })

  it("rewords the commit as the rewrite before it in the queue left it, its preview and its Undo on that commit", async () => {
    // a move before the reword gave the commit another id; the reword runs once its turn comes
    const MOVED_ID = "3333333333333333333333333333333333333333"
    renamed.set(OLD_ID, MOVED_ID)
    answer({ newCommitIds: { [MOVED_ID]: NEW_ID }, previousHead: MOVED_ID, newHead: NEW_ID, undoToken: TOKEN })
    const follow = vi.fn()

    expect(await rewordCommit(commit, "(auth) Add the login form", follow)).toBe(true)

    expect(vi.mocked(commands.reword)).toHaveBeenCalledExactlyOnceWith(
      { repositoryPath: "/repo", rewords: [{ commit: MOVED_ID, text: { message: "(auth) Add the login form" } }] },
      expect.anything(),
    )
    expect(follow.mock.calls).toEqual([[MOVED_ID, NEW_ID]])
    expect(toasts.map(toast => toast.title)).toEqual(["Reworded 33333333"])
  })

  it("offers no Undo past the window of the toast", async () => {
    answer({ newCommitIds: { [OLD_ID]: NEW_ID }, previousHead: OLD_ID, newHead: NEW_ID, undoToken: TOKEN })
    await rewordCommit(commit, "(auth) Add the login form", () => {})

    vi.advanceTimersByTime(UNDO_WINDOW_MS)
    const entry = entries()[0]!
    expect(canUndoActivity(entry)).toBe(false)
    undoActivity(entry)
    expect(vi.mocked(commands.undoRegroup)).not.toHaveBeenCalled()
  })

  it("records nothing for an unchanged message", async () => {
    answer({ newCommitIds: { [OLD_ID]: OLD_ID }, previousHead: OLD_ID, newHead: OLD_ID, undoToken: null })
    const follow = vi.fn()

    expect(await rewordCommit(commit, "(auth) Add login\n", follow)).toBe(true)

    expect(vi.mocked(commands.reword)).toHaveBeenCalledOnce()
    expect([toasts, entries(), follow.mock.calls]).toEqual([[], [], []])
  })
})
