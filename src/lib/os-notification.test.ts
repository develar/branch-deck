// @spec openspec/specs/repository/review-requests/spec.md
// @spec openspec/specs/repository/backend-protocol/spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { announce, resetOsNotification, setWindowFocus } from "@/lib/os-notification"
import { appSettingsStore } from "@/stores/settings/appSettings"

const { os, backend } = vi.hoisted(() => ({
  os: { notifications: [] as string[], requests: 0 },
  backend: { claims: [] as unknown[], first: new Set<string>() },
}))

// the claims of the backend: the first claim of a news is first, of this window or of another client
vi.mock(import("@/lib/bindings"), async importOriginal => {
  const original = await importOriginal()
  return {
    ...original,
    commands: {
      ...original.commands,
      claimNews: async (params: Parameters<typeof original.commands.claimNews>[0]) => {
        backend.claims.push(params)
        const key = `${params.repositoryPath}\n${JSON.stringify(params.news)}`
        const first = !backend.first.has(key)
        backend.first.add(key)
        return { status: "ok" as const, data: first }
      },
    },
  }
})

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: async () => false,
  requestPermission: async () => {
    os.requests++
    return "granted"
  },
  sendNotification: ({ title }: { title: string }) => os.notifications.push(title),
}))

beforeEach(() => {
  resetOsNotification()
  os.notifications = []
  os.requests = 0
  backend.claims = []
  backend.first = new Set()
  appSettingsStore.setState({ notifyReviewRequests: true, notifyPatronusMerges: true, notifyPatronusFailures: true })
})

describe("announce", () => {
  it("shows the toast at once while focused, else notifies and shows it on focus", async () => {
    const toasts: string[] = []
    announce("reviewRequests", { title: "a", body: "" }, () => toasts.push("a"))
    expect(toasts).toEqual(["a"])

    setWindowFocus(false)
    announce("reviewRequests", { title: "b", body: "" }, () => toasts.push("b"))
    await vi.waitFor(() => expect(os.notifications).toEqual(["b"]))
    expect(toasts).toEqual(["a"])
    setWindowFocus(true)
    expect(toasts).toEqual(["a", "b"])
    // asked once, on the first notification
    expect(os.requests).toBe(1)
  })

  it("sends no OS notification of a kind Settings turned off; the toast waits for the focus all the same", async () => {
    appSettingsStore.setState({ notifyPatronusMerges: false })
    const toasts: string[] = []
    setWindowFocus(false)
    announce("patronusMerges", { title: "merged", body: "" }, () => toasts.push("merged"))
    announce("reviewRequests", { title: "review", body: "" }, () => toasts.push("review"))
    await vi.waitFor(() => expect(os.notifications).toEqual(["review"]))
    setWindowFocus(true)
    expect(toasts).toEqual(["merged", "review"])
  })

  it("tells a news by the OS only when its claim is the first of the backend, so the app and the IDE tell it once", async () => {
    const claim = { kind: "robotFailed" as const, robotId: "r1" }
    // the IDE told it first
    backend.first.add(`/repo\n${JSON.stringify(claim)}`)
    const toasts: string[] = []
    setWindowFocus(false)
    announce("patronusFailures", { title: "other", body: "", news: { repositoryPath: "/repo", id: 4, claim: { kind: "robotFailed", robotId: "r2" } } }, () => toasts.push("other"))
    await vi.waitFor(() => expect(os.notifications).toEqual(["other"]))
    announce("patronusFailures", { title: "failed", body: "", news: { repositoryPath: "/repo", id: 3, claim } }, () => toasts.push("failed"))
    await vi.waitFor(() => expect(backend.claims).toHaveLength(2))
    expect(os.notifications).toEqual(["other"])
    // the toasts of both wait for the focus all the same
    setWindowFocus(true)
    expect(toasts).toEqual(["other", "failed"])
  })
})
