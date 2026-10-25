import { beforeEach, describe, expect, it, vi } from "vite-plus/test"
import { createStore } from "zustand/vanilla"
import type { EventToastState } from "@/lib/event-toast"
import { closeEventToasts, isEventToastOpen, replaceEventToast, resetEventToasts, showEventToast } from "@/lib/event-toast"
import { describeToast, openToasts, resetToasts, toastModule, toasts } from "@/test-support/toast-mock"

vi.mock("@/components/ui/toast", async () => (await import("@/test-support/toast-mock")).toastModule)

function said(title: string, extra: Partial<EventToastState> = {}): () => EventToastState {
  return () => ({ type: "success", title, ...extra })
}

beforeEach(() => {
  resetEventToasts()
  resetToasts()
})

describe("one toast per event", () => {
  it("takes a later state of its key in place, its timer started again, and shows a new toast once that one closed", () => {
    const first = showEventToast({ key: "a", render: said("auth merged into master") })
    showEventToast({ key: "a", render: said("auth landed in origin/master") })
    expect(openToasts().map(it => [it.id, it.title])).toEqual([[first, "auth landed in origin/master"]])
    expect(toasts).toHaveLength(1)

    toastModule.toast.close(first!)
    expect(isEventToastOpen("a")).toBe(false)
    const second = showEventToast({ key: "a", render: said("auth landed again") })
    expect(second).not.toBe(first)
    expect(openToasts().map(it => it.title)).toEqual(["auth landed again"])
  })

  it("replaces the toasts of the keys it supersedes: the newest in place, the others close; an alias finds a toast too", () => {
    showEventToast({ key: "robot 1", aliases: ["merged"], render: said("auth merged into master") })
    const newest = showEventToast({ key: "robot 2", aliases: ["merged"], render: said("login merged into master") })
    showEventToast({ key: "push x", render: said("Pushed x") })
    showEventToast({ key: "update", supersedes: ["merged"], render: said("2 branches landed in origin/master") })
    expect(openToasts().map(it => [it.id, it.title])).toEqual([
      [newest, "2 branches landed in origin/master"],
      [expect.any(String), "Pushed x"],
    ])
    // the update's toast is of its own key now: the merges are told
    expect(isEventToastOpen("merged")).toBe(false)
    expect(isEventToastOpen("update")).toBe(true)
  })

  it("replaces only an open toast with a state that is no news of its own", () => {
    expect(replaceEventToast({ key: "a", render: said("auth landed in origin/master") })).toBeNull()
    expect(toasts).toEqual([])
    showEventToast({ key: "a", render: said("auth merged into master") })
    replaceEventToast({ key: "a", render: said("auth landed in origin/master") })
    expect(openToasts().map(it => it.title)).toEqual(["auth landed in origin/master"])
  })

  it("closes the toasts of a key, and only those", () => {
    showEventToast({ key: "a", render: said("A") })
    showEventToast({ key: "b", render: said("B") })
    closeEventToasts("a")
    expect(openToasts().map(it => it.title)).toEqual(["B"])
  })
})

describe("the state of a toast", () => {
  it("is rendered again on a change of what it reads: its actions are those of the state now, a gone object closes it", () => {
    const store = createStore(() => ({ cancelable: true, exists: true }))
    const cancel = vi.fn()
    showEventToast({
      key: "robot",
      watch: [store],
      render: () => {
        const { cancelable, exists } = store.getState()
        return exists ? { type: "success", title: "Safe Push started", ...(cancelable && { secondaryAction: { label: "Cancel Safe Push", onClick: cancel } }) } : null
      },
    })
    expect(openToasts().map(describeToast)).toEqual([["Safe Push started", undefined, undefined, "Cancel Safe Push"]])
    const shown = openToasts()[0]!
    // a render that says the same leaves the toast as it is
    const updates = vi.spyOn(toastModule.toast, "update")
    store.setState({})
    expect(updates).not.toHaveBeenCalled()

    store.setState({ cancelable: false })
    expect(openToasts()).toEqual([shown])
    expect(openToasts().map(describeToast)).toEqual([["Safe Push started", undefined, undefined, undefined]])
    store.setState({ exists: false })
    expect(openToasts()).toEqual([])
    updates.mockRestore()
  })

  it("stays while its action runs, disabled, and runs the action of the latest render", () => {
    const store = createStore(() => ({ running: false, target: "first" }))
    const clicks: string[] = []
    showEventToast({
      key: "merge",
      watch: [store],
      render: () => {
        const { running, target } = store.getState()
        return {
          type: "success",
          title: "auth merged into master",
          action: running ? { label: "Updating…", running: true } : { label: "Update Project", onClick: () => clicks.push(target) },
        }
      },
    })
    expect(openToasts()[0]).toMatchObject({ timeout: 10_000, actionProps: { children: "Update Project" }, data: { running: false } })
    // the state changed in what the toast doesn't show: the click is of the state now
    store.setState({ target: "second" })
    openToasts()[0]!.actionProps!.onClick!()
    expect(clicks).toEqual(["second"])

    store.setState({ running: true })
    expect(openToasts()[0]).toMatchObject({ timeout: 0, actionProps: { children: "Updating…" }, data: { running: true } })
    store.setState({ running: false })
    // its whole time again
    expect(openToasts()[0]).toMatchObject({ timeout: 10_000, actionProps: { children: "Update Project" } })
  })

  it("closes after its run when asked to: the outcome of the run says it", () => {
    const store = createStore(() => ({ running: false }))
    showEventToast({
      key: "merge",
      watch: [store],
      closesAfterRun: true,
      render: () => ({
        type: "success",
        title: "auth merged into master",
        action: store.getState().running ? { label: "Updating…", running: true } : { label: "Update Project", onClick: () => {} },
      }),
    })
    store.setState({ running: true })
    expect(openToasts()).toHaveLength(1)
    store.setState({ running: false })
    expect(openToasts()).toEqual([])
  })

  it("stops reading the stores once it closed or another state took its place", () => {
    const store = createStore(() => ({ n: 0 }))
    const render = vi.fn(said("A"))
    const id = showEventToast({ key: "a", watch: [store], render })
    showEventToast({ key: "a", render: said("B") })
    store.setState({ n: 1 })
    expect(render).toHaveBeenCalledTimes(1)
    toastModule.toast.close(id!)
    store.setState({ n: 2 })
    expect(render).toHaveBeenCalledTimes(1)
  })
})
