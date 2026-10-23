import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type { GitHubAuthStatus } from "@/lib/bindings"
import { cancelGitHubSignIn, gitHubAuthStore, loadGitHubAuthStatus, startGitHubSignIn } from "@/stores/github-auth"

const { github } = vi.hoisted(() => ({
  github: {
    status: null as GitHubAuthStatus | null,
    start: { status: "ok", data: { userCode: "WDJB-MJHT", verificationUri: "https://github.com/login/device", expiresIn: 900 } } as unknown,
    /** Resolves the wait in flight with its answer */
    finish: null as ((answer: unknown) => void) | null,
    canceled: 0,
    statusAsked: 0,
  },
}))
const { notifyError, showToast } = vi.hoisted(() => ({ notifyError: vi.fn(), showToast: vi.fn() }))

vi.mock("@/lib/bindings", () => ({
  commands: {
    getGithubAuthStatus: async () => {
      github.statusAsked++
      return { status: "ok", data: github.status }
    },
    startGithubSignIn: async () => github.start,
    waitGithubSignIn: () =>
      new Promise(resolve => {
        github.finish = resolve
      }),
    cancelGithubSignIn: async () => {
      github.canceled++
      github.finish?.({ status: "error", error: { ...internalError("the sign-in was canceled"), code: "canceled" } })
      return { status: "ok", data: null }
    },
  },
}))
vi.mock("@/lib/errors", () => ({ notifyError }))
vi.mock("@/lib/toast-message", () => ({ showToast }))
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }))

const signedIn: GitHubAuthStatus = { signInAvailable: true, signedInAs: "octocat", tokenSource: "branchDeck", manageUrl: "https://github.com/settings/connections/applications/c" }

/** Lets the awaited commands of the store run */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) {
    await Promise.resolve()
  }
}

beforeEach(() => {
  gitHubAuthStore.setState({ status: null, signIn: { phase: "idle" } })
  github.status = null
  github.finish = null
  github.canceled = 0
  github.statusAsked = 0
  notifyError.mockClear()
  showToast.mockClear()
})

describe("startGitHubSignIn", () => {
  it("shows the code while GitHub waits for it, then tells who signed in", async () => {
    const signingIn = startGitHubSignIn()
    await settle()
    expect(gitHubAuthStore.getState().signIn).toMatchObject({ phase: "waiting", userCode: "WDJB-MJHT", verificationUri: "https://github.com/login/device" })

    github.status = signedIn
    github.finish!({ status: "ok", data: "octocat" })
    await signingIn
    expect(gitHubAuthStore.getState()).toEqual({ status: signedIn, signIn: { phase: "idle" } })
    expect(showToast).toHaveBeenCalledWith({ title: "Signed in to GitHub as @octocat" })
    expect(notifyError).not.toHaveBeenCalled()
  })

  it("says nothing when canceled, and why when GitHub refused, by the code of the error and not its words", async () => {
    const canceled = startGitHubSignIn()
    await settle()
    await cancelGitHubSignIn()
    await canceled
    expect(github.canceled).toBe(1)
    expect(gitHubAuthStore.getState().signIn).toEqual({ phase: "idle" })
    expect(notifyError).not.toHaveBeenCalled()

    const denied = startGitHubSignIn()
    await settle()
    github.finish!({ status: "error", error: internalError("the access was denied on GitHub") })
    await denied
    expect(notifyError).toHaveBeenCalledWith("Cannot sign in to GitHub", internalError("the access was denied on GitHub"))
    notifyError.mockClear()

    // a failure in the words of a cancel is still a failure: only the code tells a cancel
    const failed = startGitHubSignIn()
    await settle()
    github.finish!({ status: "error", error: internalError("canceled") })
    await failed
    expect(notifyError).toHaveBeenCalledWith("Cannot sign in to GitHub", internalError("canceled"))
    expect(showToast).not.toHaveBeenCalled()
  })
})

describe("loadGitHubAuthStatus", () => {
  it("asks for the status once, however many hints need it", async () => {
    github.status = { ...signedIn, signInAvailable: false, signedInAs: null }
    await Promise.all([loadGitHubAuthStatus(), loadGitHubAuthStatus()])
    await loadGitHubAuthStatus()
    expect(github.statusAsked).toBe(1)
    expect(gitHubAuthStore.getState().status?.signInAvailable).toBe(false)
  })
})
