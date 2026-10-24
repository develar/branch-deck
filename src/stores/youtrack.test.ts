import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
import type { YouTrackAuthStatus } from "@/lib/bindings"
import { cancelYouTrackSignIn, saveYouTrackToken, startYouTrackSignIn, youTrackAuthStore } from "@/stores/youtrack"

const mocks = vi.hoisted(() => {
  // the wait in flight ends with what `finish` is given
  const youtrack = { status: null as YouTrackAuthStatus | null, finish: null as ((answer: unknown) => void) | null, saved: null as unknown }
  return { youtrack, openUrl: vi.fn(async () => {}), notifyError: vi.fn(), showToast: vi.fn() }
})
const { youtrack, openUrl, notifyError, showToast } = mocks

vi.mock("@/lib/bindings", () => ({
  commands: {
    getYoutrackAuthStatus: async () => ({ status: "ok", data: mocks.youtrack.status }),
    startYoutrackSignIn: async (url: string) => ({ status: "ok", data: { authorizeUrl: `${url}/hub/api/rest/oauth2/auth?state=s` } }),
    waitYoutrackSignIn: () =>
      new Promise(resolve => {
        mocks.youtrack.finish = resolve
      }),
    cancelYoutrackSignIn: async () => {
      mocks.youtrack.finish?.({ status: "error", error: { ...internalError("The sign-in was canceled"), code: "canceled" } })
      return { status: "ok", data: null }
    },
    saveYoutrackToken: async () => mocks.youtrack.saved,
  },
}))
vi.mock("@/lib/errors", () => ({ notifyError: mocks.notifyError, logError: () => {} }))
vi.mock("@/lib/toast-message", () => ({ showToast: mocks.showToast }))
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: mocks.openUrl }))

const signedIn: YouTrackAuthStatus = {
  url: "https://youtrack.example.com",
  signedInAs: { login: "jane", fullName: "Jane Doe" },
  source: "signIn",
  problem: null,
  signedInAt: 1_704_117_600,
  manageUrl: null,
}

/** Lets the awaited commands of the store run */
async function settle(): Promise<void> {
  for (let i = 0; i < 5; i++) {
    await Promise.resolve()
  }
}

beforeEach(() => {
  youTrackAuthStore.setState({ status: null, readError: null, signIn: { phase: "idle" }, issues: {} })
  youtrack.status = null
  youtrack.finish = null
  openUrl.mockClear()
  notifyError.mockClear()
  showToast.mockClear()
})

describe("startYouTrackSignIn", () => {
  it("opens the Hub page, waits for the browser, then tells who signed in where", async () => {
    const signingIn = startYouTrackSignIn("https://youtrack.example.com")
    await settle()
    expect(youTrackAuthStore.getState().signIn).toEqual({ phase: "waiting", authorizeUrl: "https://youtrack.example.com/hub/api/rest/oauth2/auth?state=s" })
    expect(openUrl).toHaveBeenCalledWith("https://youtrack.example.com/hub/api/rest/oauth2/auth?state=s")

    youtrack.status = signedIn
    youtrack.finish!({ status: "ok", data: { login: "jane", fullName: "Jane Doe" } })
    await signingIn
    expect(youTrackAuthStore.getState()).toEqual({ status: signedIn, readError: null, signIn: { phase: "idle" }, issues: {} })
    expect(showToast).toHaveBeenCalledWith({ title: "Signed in to youtrack.example.com as Jane Doe" })
  })

  it("says nothing when canceled, and why when the Hub refused, by the code of the error and not its words", async () => {
    const canceled = startYouTrackSignIn("https://youtrack.example.com")
    await settle()
    await cancelYouTrackSignIn()
    await canceled
    expect(youTrackAuthStore.getState().signIn).toEqual({ phase: "idle" })
    expect(notifyError).not.toHaveBeenCalled()

    const denied = startYouTrackSignIn("https://youtrack.example.com")
    await settle()
    youtrack.finish!({ status: "error", error: internalError("The sign-in was denied: access_denied") })
    await denied
    expect(notifyError).toHaveBeenCalledWith("Cannot sign in to YouTrack", internalError("The sign-in was denied: access_denied"))
    notifyError.mockClear()

    // a failure in the words of a cancel is still a failure: only the code tells a cancel
    const failed = startYouTrackSignIn("https://youtrack.example.com")
    await settle()
    youtrack.finish!({ status: "error", error: internalError("canceled") })
    await failed
    expect(notifyError).toHaveBeenCalledWith("Cannot sign in to YouTrack", internalError("canceled"))
    expect(showToast).not.toHaveBeenCalled()
  })
})

describe("saveYouTrackToken", () => {
  it("is false and says why when YouTrack rejects the token", async () => {
    youtrack.saved = { status: "error", error: { message: "YouTrack rejected the token" } }
    expect(await saveYouTrackToken("https://youtrack.example.com", "perm-wrong")).toBe(false)
    expect(notifyError).toHaveBeenCalledWith("Cannot sign in to YouTrack", { message: "YouTrack rejected the token" })
  })
})
