// The mocks the unit tests of the pushes of the page share (`push-run.test.ts`, `stack-push.test.ts`): load each in a
// `vi.mock` factory with the stores of the test, `vi.mock("@/model/store", async () => (await
// import("@/test-support/push-mocks")).modelStoreMock(() => stores.pullRequests))`
import { createStore } from "zustand/vanilla"
import type { PullRequest } from "@/lib/bindings"
import type { Branch } from "@/stores/branch-sync"
import { CREATE, PUSH, testBranch, testRemoteStatus } from "@/stores/test-branch"

/**
 * `@/model/store` of a push: the patch of the session is there at once (`whenSessionViewAt`), and Rust offers Create Pull
 * Request… of a branch without a live pull request (`domain_policy::actions`), over the pull requests the test lists
 */
export function modelStoreMock(pullRequests: () => Readonly<Record<string, PullRequest | undefined>>) {
  return {
    whenSessionViewAt: async () => {},
    getRowPart: (key: string, part: string) =>
      part === "actions" && (pullRequests()[key]?.state ?? "closed") === "closed" ? [{ id: "create-pull-request", availability: { kind: "enabled" } }] : undefined,
  }
}

/**
 * `@/stores/pull-requests` over the pull requests GitHub lists, `pullRequests()`: a set of the store tells the toasts
 * that read them; the repository is JetBrains/ultimate
 */
export function pullRequestsMock(pullRequests: () => Readonly<Record<string, PullRequest | undefined>>) {
  return {
    pullRequestsStore: createStore(() => ({})),
    getPullRequest: (name: string) => pullRequests()[name],
    getGitHubRepository: () => ({ owner: "JetBrains", name: "ultimate" }),
  }
}

/** A branch of one commit the remote lacks, built on `parent`; `remote: false` when it isn't on the remote yet */
export function unpushedBranch(name: string, parent: string | null, remote: boolean, extra: Partial<Branch> = {}): Branch {
  const remoteStatus = testRemoteStatus({
    exists: remote,
    unpushedCommits: ["c"],
    commitsAhead: 1,
    myCommitsAhead: 1,
    remoteTip: remote ? "old-tip" : null,
    action: remote ? PUSH : CREATE,
  })
  return testBranch(name, parent, { remoteStatus, ...extra })
}

/** `@tauri-apps/plugin-opener`, each URL it opens recorded in `calls` as "open <url>" */
export function openerMock(calls: string[]) {
  return { openUrl: async (url: string) => void calls.push(`open ${url}`) }
}

/** `@/lib/clipboard`, each text it copies recorded in `calls` as "copy <text>" */
export function clipboardMock(calls: string[]) {
  return { copyText: async (text: string) => void calls.push(`copy ${text}`) }
}
