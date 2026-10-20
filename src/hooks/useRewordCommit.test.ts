// @spec spec/commit/commit-row.spec.md
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { rewordCommit } from "@/hooks/useRewordCommit"
import type { RewordResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { CommitUnion } from "@/lib/commit"
import { activityStore, canUndoActivity, clearActivity, selectActivity, undoActivity, UNDO_WINDOW_MS } from "@/stores/activity"

const { toasts } = vi.hoisted(() => ({
  // what each `toast.add` showed, its action included
  toasts: [] as { title?: string; description?: string; timeout?: number; actionProps?: { onClick?: () => void } }[],
}))

vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))
vi.mock("@/components/ui/toast", () => ({
  toast: {
    add: (toast: (typeof toasts)[number]) => {
      toasts.push(toast)
      return `toast-${toasts.length}`
    },
    close: () => {},
  },
}))
vi.mock("@/stores/settings/app-store", () => ({ isMainWindow: true, getRawStoreValue: () => undefined, registerStore: () => {} }))
vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: "/repo" }),
  useSelectedProject: () => ({ path: "/repo" }),
  getEffectiveBranchPrefix: () => "user",
}))
vi.mock("@/stores/branch-sync", () => ({
  branchSyncStore: { getState: () => ({ isSyncing: false }) },
  getBranchesOfCommits: () => ["auth"],
  syncBranches: async () => {},
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

    expect(vi.mocked(commands.reword)).toHaveBeenCalledWith({
      repositoryPath: "/repo",
      rewords: [{ commit: OLD_ID, text: { message: "(auth) Add the login form\n\nWith a body" } }],
    })
    expect(follow.mock.calls).toEqual([[OLD_ID, NEW_ID]])
    expect(toasts.map(toast => [toast.title, toast.description, toast.timeout])).toEqual([["Reworded 11111111", "(auth) Add the login form", UNDO_WINDOW_MS]])
    const entry = entries()[0]!
    expect([entry.kind, entry.title, entry.undo]).toEqual(["rewrite", "Reworded 11111111", { until: 1_000_000 + UNDO_WINDOW_MS }])
    expect(canUndoActivity(entry)).toBe(true)

    undoActivity(entry)
    await vi.waitFor(() => expect(follow.mock.calls).toHaveLength(2))
    expect(vi.mocked(commands.undoRegroup)).toHaveBeenCalledExactlyOnceWith({ repositoryPath: "/repo", token: TOKEN })
    // the preview on the new commit follows the Undo back
    expect(follow.mock.calls[1]).toEqual([NEW_ID, OLD_ID])
    expect(entries()[0]!.undo?.undone).toBe(true)

    // the toast's Undo after the one in Activity undoes nothing more
    toasts[0]!.actionProps!.onClick!()
    await vi.advanceTimersByTimeAsync(0)
    expect(vi.mocked(commands.undoRegroup)).toHaveBeenCalledOnce()
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
