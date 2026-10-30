import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import type { AuthStatus, YouTrackAuthStatus, YouTrackIssueAnswer } from "@/lib/bindings"
import { authStore } from "@/stores/auth"
import { getYouTrackKeyOfIssue, loadIssue, youTrackIssuesStore } from "@/stores/youtrack-issues"

const { backend } = vi.hoisted(() => ({
  backend: {
    /** The keys asked, in order, and the resolve of the lookup in flight */
    asked: [] as string[],
    finish: null as ((answer: unknown) => void) | null,
    /** The reads of the sign-ins the window asked again */
    authReads: 0,
  },
}))

vi.mock("@/lib/bindings", () => ({
  commands: {
    getYoutrackIssue: (key: string) => {
      backend.asked.push(key)
      return new Promise(resolve => {
        backend.finish = resolve
      })
    },
  },
}))
vi.mock("@/model/auth-facts", () => ({
  refreshWindowAuth: async () => {
    backend.authReads++
  },
}))
vi.mock("@/stores/settings/repositorySettings", () => ({
  getRecentProjects: () => [{ issueNavigationConfig: { links: [{ issueRegexp: "\\b[A-Z]+-\\d+\\b", linkRegexp: "https://youtrack.example.com/issue/$0" }] } }],
}))

const signedOut: YouTrackAuthStatus = { url: null, signedInAs: null, source: null, problem: null, signedInAt: null, manageUrl: null }
const jane: YouTrackAuthStatus = {
  url: "https://youtrack.example.com",
  signedInAs: { login: "jane", fullName: "Jane Doe" },
  source: "signIn",
  problem: null,
  signedInAt: 1_704_117_600,
  manageUrl: null,
}

/** The status of both accounts with `youtrack` */
function statusWith(youtrack: YouTrackAuthStatus): AuthStatus {
  return { github: { signInAvailable: false, signedInAs: null, tokenSource: null, manageUrl: null }, youtrack }
}

const found: YouTrackIssueAnswer = { status: "notFound" }

beforeEach(() => {
  authStore.setState({ status: statusWith(jane), readError: null, signIns: {} })
  youTrackIssuesStore.setState({ issues: {} })
  backend.asked = []
  backend.finish = null
  backend.authReads = 0
})

describe("the issues of the popovers", () => {
  it("asks an issue once while it is asked, and a sign-in problem has the window read the sign-ins again", async () => {
    const first = loadIssue("ABC-1")
    const second = loadIssue("ABC-1")
    expect(backend.asked).toEqual(["ABC-1"])
    backend.finish!({ status: "ok", data: { status: "signInProblem", problem: "expired" } })
    await Promise.all([first, second])
    expect(youTrackIssuesStore.getState().issues["ABC-1"]).toEqual({ status: "signInProblem", problem: "expired" })
    expect(backend.authReads).toBe(1)

    // a failed command is an answer of its own, with the reason
    const failed = loadIssue("ABC-2")
    backend.finish!({ status: "error", error: { code: "internal", message: "YouTrack is down" } })
    await failed
    expect(youTrackIssuesStore.getState().issues["ABC-2"]).toEqual({ status: "error", message: "YouTrack is down" })
    expect(backend.authReads).toBe(1)
  })

  it("forgets the answers once the YouTrack sign-in changed, not for a problem of the same sign-in", async () => {
    const asked = loadIssue("ABC-1")
    backend.finish!({ status: "ok", data: found })
    await asked
    // the sign-in expired: the popover that found it says so with its answer
    authStore.setState({ status: statusWith({ ...jane, problem: "expired" }) })
    expect(youTrackIssuesStore.getState().issues["ABC-1"]).toEqual(found)
    // signed in again, in any window: the answers of the sign-in before go
    authStore.setState({ status: statusWith({ ...jane, signedInAt: jane.signedInAt! + 60 }) })
    expect(youTrackIssuesStore.getState().issues).toEqual({})
  })

  it("names the key of a branch's issue only when its link is an issue of the YouTrack signed in to", () => {
    expect(getYouTrackKeyOfIssue("ABC-1")).toBe("ABC-1")
    expect(getYouTrackKeyOfIssue(null)).toBeNull()
    authStore.setState({ status: statusWith(signedOut) })
    expect(getYouTrackKeyOfIssue("ABC-1")).toBeNull()
  })
})
