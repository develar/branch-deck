// @spec spec/repository/review-requests.spec.md
import { beforeEach, describe, expect, it, vi } from "vitest"
import { announce, resetOsNotification, setWindowFocus } from "@/lib/os-notification"
import { appSettingsStore } from "@/stores/settings/appSettings"

const { os } = vi.hoisted(() => ({ os: { notifications: [] as string[], requests: 0 } }))

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
  appSettingsStore.setState({ notifyReviewRequests: true, notifyPatronusMerges: true })
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
})
