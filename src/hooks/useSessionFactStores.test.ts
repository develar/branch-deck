// @spec openspec/specs/review/review-window/spec.md
import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { applySessionFactStores } from "@/hooks/useSessionFactStores"
import { tellReviewRequestNews, reviewRequestsStore } from "@/stores/review-requests"
import { emptyView } from "@/test-support/fake-sessions"
import { reviewRequest as request } from "@/test-support/review-request"

const { told } = vi.hoisted(() => ({ told: { toasts: [] as string[], notifications: [] as string[], activity: [] as string[] } }))

vi.mock("@/lib/bindings", async importOriginal => ({ ...(await importOriginal<typeof import("@/lib/bindings")>()), commands: {} }))
vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {}, info: async () => {} }))
vi.mock("@/lib/event-toast", async importOriginal => ({
  ...(await importOriginal<typeof import("@/lib/event-toast")>()),
  showEventToast: ({ key }: { key: string }) => void told.toasts.push(key),
}))
vi.mock("@/lib/os-notification", async importOriginal => ({
  ...(await importOriginal<typeof import("@/lib/os-notification")>()),
  announce: (_kind: string, notification: { title: string }, show: () => void) => {
    told.notifications.push(notification.title)
    show()
  },
}))
vi.mock("@/stores/activity", () => ({ recordReviewRequest: (_path: string, it: { number: number }) => void told.activity.push(`#${it.number}`) }))

beforeEach(() => {
  told.toasts = []
  told.notifications = []
  told.activity = []
})

describe("the stores of the facts of a window's session", () => {
  it("fill in the review window from the patches of its session and tell no news there: the main window tells them", () => {
    const view = emptyView()
    const reviewRequests = {
      head: { value: { status: "available" as const, owner: "acme", name: "monorepo" }, state: { kind: "fresh" as const }, seq: 1, observedAt: 1_000_000 },
      requests: [request(1), request(2)],
      totalCount: 2,
      checkedAt: 1_000_000,
    }
    // what every window with a session applies (`useSessionFactStores`), of a patch whose news name #2
    applySessionFactStores("/repo", { ...view, facts: { ...view.facts, reviewRequests } }, { facts: { reviewRequests } })
    expect(reviewRequestsStore.getState().repositories["/repo"]?.requests.map(it => it.number)).toEqual([1, 2])
    expect(told).toEqual({ toasts: [], notifications: [], activity: [] })

    // the main window tells them (`useReviewStatusRefresh`)
    tellReviewRequestNews("/repo", [{ id: 1, event: { kind: "reviewRequested", numbers: [2] } }])
    expect(told.activity).toEqual(["#2"])
    expect(told.notifications).toHaveLength(1)
    expect(told.toasts).toHaveLength(1)
  })
})
