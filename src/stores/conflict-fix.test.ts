// The search of the conflict fix on the page's side of a session (`src/test-support/fake-sessions.ts`): a fix is looked
// for again only when the commits of the table changed, never for a push or a deleted archived branch
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test"
import type { TableChanges } from "@/lib/bindings"
import { closeSession, openSession, requestSessionSync, whenSessionReady } from "@/model/store"
import { branchSyncStore, whenSynced } from "@/stores/branch-sync"
import { conflictFixStore, requestConflictFix } from "@/stores/conflict-fix"
import { fakeRemoteStatus, fakeSessions, grouping } from "@/test-support/fake-sessions"

const { searches } = vi.hoisted(() => ({
  /** The repository of each search of the fix that reached the backend (`find_conflict_fix`) */
  searches: [] as string[],
}))

vi.mock("@tauri-apps/api/core", () => import("@/test-support/channel-mock"))
vi.mock("@/lib/bindings", async () => ({
  commands: {
    ...(await import("@/test-support/fake-sessions")).sessionCommands(),
    findConflictFix: async ({ repositoryPath }: { repositoryPath: string }) => {
      searches.push(repositoryPath)
      return { status: "ok", data: { head: "h", fingerprint: "f", components: [], unresolved: [], search: "complete", trials: 1 } }
    },
    cancelConflictFix: async () => ({ status: "ok", data: null }),
  },
}))
vi.mock("@/lib/toast-message", () => ({ showToast: () => {} }))
vi.mock("@/hooks/useStackActions", () => ({ undoRegroup: async () => true }))
vi.mock("@/lib/rewrite-run", () => ({ runRewriteIntent: async () => null, witnessed: () => ({}) }))
vi.mock("@/stores/commit-move", () => ({ requestReveal: () => {} }))
vi.mock("@/stores/repository", () => ({
  getSelectedProject: () => ({ path: "/repo" }),
  getSelectedRepositoryPath: () => "/repo",
  setSessionAnswer: () => {},
}))
vi.mock("@/stores/settings/repositorySettings", () => ({ updateProject: () => {} }))
vi.mock("@/stores/settings/repositoryState", () => ({ updateRepositoryState: () => {} }))
vi.mock("@/stores/landed", () => ({ hideAfterSync: () => {}, setLandedStatus: () => {} }))
vi.mock("@/stores/unapplied-branches", () => ({ setUnappliedBranches: () => {}, resetUnappliedBranches: () => {} }))
vi.mock("@/stores/archived-branches", () => ({
  updateFromArchivedNames: () => {},
  updateFromIntegrationInfos: () => {},
  settleArchivedPlaceholders: () => {},
  resetArchivedBranches: () => {},
}))

/** Sends `changes` as a patch of the session of /repo and waits until the store holds its version */
async function send(changes: TableChanges): Promise<void> {
  const { next } = fakeSessions.send("/repo", changes)
  await vi.waitFor(() => expect(branchSyncStore.getState().viewVersion).toEqual(next))
}

beforeEach(() => {
  fakeSessions.reset()
  branchSyncStore.setState(branchSyncStore.getInitialState(), true)
  conflictFixStore.setState(conflictFixStore.getInitialState(), true)
  searches.length = 0
})

afterEach(async () => {
  for (const sync of fakeSessions.syncs) {
    sync.answer()
  }
  closeSession()
  await whenSynced()
})

it("looks for the fix once per commits of the table: a push or a deleted archived branch looks for nothing again", async () => {
  openSession("/repo")
  await whenSessionReady()
  const synced = requestSessionSync("manual")
  await vi.waitFor(() => expect(fakeSessions.syncs).toHaveLength(1))
  await send({ ...grouping("a"), archived: { names: ["user/archived/old"], newlyArchived: [] } })
  fakeSessions.syncs[0]!.answer()
  await synced
  await requestConflictFix()
  expect(searches).toEqual(["/repo"])
  expect(conflictFixStore.getState().status).toBe("done")

  // a push of the branch answers its remote status; the deletion of an archived branch its section
  await send({ branches: { a: { remote: { status: fakeRemoteStatus("a", 0) } } } })
  await send({ archived: { names: [] } })
  await requestConflictFix()
  expect(searches).toEqual(["/repo"])
  expect(conflictFixStore.getState().status).toBe("done")

  // a new copy of a commit: the fix of other commits
  await send({ branches: { a: { copies: [{ hash: "a1-copy", status: "Created" }] } } })
  await requestConflictFix()
  expect(searches).toEqual(["/repo", "/repo"])
})
