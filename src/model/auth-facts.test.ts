import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type { AuthStatus } from "@/lib/bindings"
import { retryWindowAuth, tellReadFailures } from "@/model/auth-facts"
import { authStore, refreshAuthStatus } from "@/stores/auth"
import { OCTOCAT_AUTH } from "@/test-support/fake-sessions"

const { backend, session, notifyError } = vi.hoisted(() => ({
  backend: {
    status: null as AuthStatus | null,
    /** The read of the status fails with this message */
    statusFailure: null as string | null,
    /** The reads of `get_auth_status` the window asked */
    reads: 0,
  },
  /** The repository of the session the window holds, and the reads of its facts the window asked */
  session: { path: null as string | null, refreshed: [] as unknown[][] },
  notifyError: vi.fn(),
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    getAuthStatus: async () => {
      backend.reads++
      return backend.statusFailure === null ? { status: "ok", data: backend.status } : { status: "error", error: internalError(backend.statusFailure) }
    },
  },
}))
vi.mock("@/lib/errors", () => ({ notifyError }))
vi.mock("@/model/store", () => ({
  sessionRepositoryPath: () => session.path,
  refreshSessionFacts: async (...args: unknown[]) => {
    session.refreshed.push(args)
    return true
  },
}))

const status = OCTOCAT_AUTH

/** Lets the awaited commands go on */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) {
    await Promise.resolve()
  }
}

beforeEach(() => {
  authStore.setState({ status: null, readError: null, signIns: {} })
  backend.status = null
  backend.statusFailure = null
  backend.reads = 0
  session.path = null
  session.refreshed = []
  notifyError.mockClear()
})

describe("the status of the sign-ins in a window of a repository", () => {
  it("tells a failed read once, as an error with Retry, and keeps the last status until a read answers", async () => {
    const stop = tellReadFailures()
    backend.status = status
    await refreshAuthStatus()
    backend.statusFailure = "the keychain is locked"
    await refreshAuthStatus()
    await refreshAuthStatus()
    // the links keep their YouTrack: a failure is no sign-out
    expect(authStore.getState()).toMatchObject({ status, readError: "the keychain is locked" })
    expect(notifyError).toHaveBeenCalledTimes(1)
    const [title, , action] = notifyError.mock.calls[0]! as unknown as [string, unknown, { children: string; onClick: () => void }]
    expect([title, action.children]).toEqual(["Cannot read the GitHub and YouTrack sign-ins", "Retry"])

    backend.statusFailure = null
    action.onClick()
    await settle()
    expect(authStore.getState()).toMatchObject({ status, readError: null })
    expect(notifyError).toHaveBeenCalledTimes(1)
    stop()
  })

  it("has the session of the window read its fact on Retry, and a window without one read it itself", async () => {
    session.path = "/repo"
    authStore.setState({ readError: "the token file is unreadable" })
    retryWindowAuth()
    await settle()
    expect([session.refreshed, backend.reads]).toEqual([[["/repo", "auth", "manual"]], 0])
    expect(authStore.getState().readError).toBeNull()

    // no repository open: nothing serves the fact
    session.path = null
    backend.status = status
    authStore.setState({ readError: "the token file is unreadable" })
    retryWindowAuth()
    await settle()
    expect([session.refreshed.length, backend.reads]).toEqual([1, 1])
    expect(authStore.getState()).toMatchObject({ status, readError: null })
  })
})
