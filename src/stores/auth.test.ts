import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type { AuthStatus, Fact, FactsView, GitHubAuthStatus, SignInEvent } from "@/lib/bindings"
import { openSignInPageAgain, runSignIn } from "@/lib/sign-in-run"
import { applyAuthFacts, authStore, refreshAuthStatus, retryAuthStatus } from "@/stores/auth"
import { emptyView } from "@/test-support/fake-sessions"

const { backend } = vi.hoisted(() => ({
  backend: {
    /** The channel of the sign-in in flight, and the resolve of its answer */
    channel: null as { onmessage: (event: SignInEvent) => void } | null,
    finish: null as ((answer: unknown) => void) | null,
    status: null as AuthStatus | null,
    /** The read of the status fails with this message */
    statusFailure: null as string | null,
  },
}))
const { notifyError, showToast, openUrl } = vi.hoisted(() => ({ notifyError: vi.fn(), showToast: vi.fn(), openUrl: vi.fn(async (_url: string) => {}) }))

vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: (event: unknown) => void = () => {}
  },
}))
vi.mock("@/lib/bindings", () => ({
  commands: {
    signIn: (_request: unknown, channel: { onmessage: (event: SignInEvent) => void }) => {
      backend.channel = channel
      return new Promise(resolve => {
        backend.finish = resolve
      })
    },
    getAuthStatus: async () => (backend.statusFailure === null ? { status: "ok", data: backend.status } : { status: "error", error: internalError(backend.statusFailure) }),
  },
}))
vi.mock("@/lib/errors", () => ({ notifyError, getErrorCode: (error: { code?: string }) => error.code }))
vi.mock("@/lib/toast-message", () => ({ showToast }))
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl }))

const signedIn: GitHubAuthStatus = { signInAvailable: true, signedInAs: "octocat", tokenSource: "branchDeck", manageUrl: "https://github.com/settings/connections/applications/c" }
const youtrack: AuthStatus["youtrack"] = { url: null, signedInAs: null, source: null, problem: null, signedInAt: null, manageUrl: null }
const status: AuthStatus = { github: signedIn, youtrack }

/** Lets the awaited commands of the run go on */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) {
    await Promise.resolve()
  }
}

function signInPhase() {
  return authStore.getState().signIns.gitHub ?? { phase: "idle" }
}

/** The facts of a session whose fact `auth` is `auth` */
function factsWith(auth: Fact<AuthStatus>): FactsView {
  return { ...emptyView().facts, auth }
}

beforeEach(() => {
  authStore.setState({ status: null, readError: null, signIns: {} })
  backend.channel = null
  backend.finish = null
  backend.status = null
  backend.statusFailure = null
  notifyError.mockClear()
  showToast.mockClear()
  openUrl.mockClear()
})

describe("the sign-in of a window", () => {
  it("shows the code while GitHub waits for it, then tells who signed in", async () => {
    const run = runSignIn({ provider: "gitHub" })
    expect(signInPhase()).toEqual({ phase: "waiting" })
    backend.channel!.onmessage({ kind: "waiting" })
    expect(signInPhase()).toEqual({ phase: "waiting" })
    const before = Date.now()
    backend.channel!.onmessage({ kind: "deviceCode", code: "WDJB-MJHT", url: "https://github.com/login/device", expiresIn: 900 })
    const shown = signInPhase()
    expect(shown).toMatchObject({ phase: "code", code: "WDJB-MJHT", url: "https://github.com/login/device" })
    expect(shown.phase === "code" && shown.expiresAt - before).toBeGreaterThanOrEqual(900_000)
    // a second click while it runs starts nothing of this window: the backend joins calls of other windows
    expect(await runSignIn({ provider: "gitHub" })).toBeNull()

    backend.channel!.onmessage({ kind: "done", done: { provider: "gitHub", login: "octocat" } })
    expect(signInPhase()).toEqual({ phase: "idle" })
    backend.finish!({ status: "ok", data: { provider: "gitHub", login: "octocat" } })
    expect(await run).toEqual({ provider: "gitHub", login: "octocat" })
    expect(showToast).toHaveBeenCalledWith({ title: "Signed in to GitHub as @octocat" })
    expect(notifyError).not.toHaveBeenCalled()
  })

  it("says nothing when canceled, and why when GitHub refused, by the code of the error and not its words", async () => {
    const canceled = runSignIn({ provider: "gitHub" })
    await settle()
    backend.finish!({ status: "error", error: { ...internalError("the sign-in was canceled"), code: "canceled" } })
    expect(await canceled).toBeNull()
    expect(signInPhase()).toEqual({ phase: "idle" })
    expect(notifyError).not.toHaveBeenCalled()

    // the words say canceled, the code does not: an error
    const refused = runSignIn({ provider: "gitHub" })
    await settle()
    backend.finish!({ status: "error", error: internalError("the code expired: the sign-in was canceled") })
    expect(await refused).toBeNull()
    expect(notifyError).toHaveBeenCalledWith("Cannot sign in to GitHub", expect.objectContaining({ code: "internal" }))
    expect(showToast).not.toHaveBeenCalled()
  })
})

describe("the sign-in to YouTrack", () => {
  const url = "https://youtrack.example.com"
  const hubPage = `${url}/hub/api/rest/oauth2/auth?state=s`

  it("opens the Hub page once the sign-in waits for the browser, again on request, then tells who signed in where", async () => {
    const run = runSignIn({ provider: "youTrack", url })
    backend.channel!.onmessage({ kind: "waiting" })
    expect(openUrl).not.toHaveBeenCalled()
    backend.channel!.onmessage({ kind: "browser", url: hubPage })
    expect(authStore.getState().signIns.youTrack).toEqual({ phase: "browser", url: hubPage })
    expect(openUrl).toHaveBeenCalledWith(hubPage)
    // the tab was closed: the same page again
    await openSignInPageAgain("youTrack")
    expect(openUrl.mock.calls).toEqual([[hubPage], [hubPage]])

    const done = { provider: "youTrack" as const, url, user: { login: "jane", fullName: "Jane Doe" } }
    backend.channel!.onmessage({ kind: "done", done })
    backend.finish!({ status: "ok", data: done })
    expect(await run).toEqual(done)
    expect(authStore.getState().signIns.youTrack).toEqual({ phase: "idle" })
    expect(showToast).toHaveBeenCalledWith({ title: "Signed in to youtrack.example.com as Jane Doe" })
  })

  it("says why YouTrack refused a pasted token, and answers null, so the field keeps it", async () => {
    const run = runSignIn({ provider: "youTrack", url, token: "perm-wrong" })
    await settle()
    backend.finish!({ status: "error", error: internalError("YouTrack rejected the token") })
    expect(await run).toBeNull()
    expect(notifyError).toHaveBeenCalledWith("Cannot sign in to YouTrack", expect.objectContaining({ message: "YouTrack rejected the token" }))
    expect([openUrl.mock.calls, showToast.mock.calls]).toEqual([[], []])
  })
})

describe("the status of the sign-ins", () => {
  it("comes from the fact of the session, which keeps the last status until it answered and says why a read failed", () => {
    applyAuthFacts(factsWith({ value: status, state: { kind: "fresh" }, seq: 3, observedAt: 1 }), null)
    expect(authStore.getState()).toMatchObject({ status, readError: null })

    // another session that has not read it yet: the status is the app's, it stays
    applyAuthFacts(factsWith({ value: null, state: { kind: "loading" }, seq: 0, observedAt: 0 }), null)
    expect(authStore.getState().status).toBe(status)

    // a patch without the fact leaves it alone; a failed read says why above the last status
    const failed = factsWith({ value: status, state: { kind: "stale", reason: internalError("the token file is unreadable") }, seq: 4, observedAt: 2 })
    applyAuthFacts(failed, {})
    expect(authStore.getState().readError).toBeNull()
    applyAuthFacts(failed, { auth: failed.auth })
    expect(authStore.getState()).toMatchObject({ status, readError: "the token file is unreadable" })
  })

  it("says why a read failed and keeps the last status, until a read answers; Retry reads again, the failure gone meanwhile", async () => {
    backend.status = status
    await refreshAuthStatus()
    expect(authStore.getState()).toMatchObject({ status, readError: null })

    backend.statusFailure = "the backend is gone"
    await refreshAuthStatus()
    expect(authStore.getState()).toMatchObject({ status, readError: "the backend is gone" })

    backend.statusFailure = null
    retryAuthStatus()
    expect(authStore.getState().readError).toBeNull()
    await settle()
    expect(authStore.getState()).toMatchObject({ status, readError: null })
  })
})
