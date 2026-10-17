import { beforeEach, describe, expect, it, vi } from "vite-plus/test"

const emitted: { event: string; payload: unknown }[] = []
/** The listeners of `tauri://destroyed`, by the label they listen to */
const closeListeners: { label: string; handler: () => void }[] = []
let openFails = false

vi.mock("@tauri-apps/api/event", () => ({
  emit: async (event: string, payload: unknown) => {
    emitted.push({ event, payload })
  },
  listen: async (_event: string, handler: () => void, options: { target: { label: string } }) => {
    const listener = { label: options.target.label, handler }
    closeListeners.push(listener)
    return () => {
      closeListeners.splice(closeListeners.indexOf(listener), 1)
    }
  },
}))
vi.mock("@/lib/window-management", () => ({
  openSubWindow: async () => {
    if (openFails) {
      throw new Error("no window")
    }
  },
}))
vi.mock("@tauri-apps/plugin-log", () => ({ error: async () => {} }))

const { createWindowLink, windowLinkEvents } = await import("@/lib/sub-window-link")

function destroy(label: string) {
  for (const listener of closeListeners.filter(it => it.label === label)) {
    listener.handler()
  }
}

const WINDOW = { url: "/thing", title: "Thing", data: {} }

describe("createWindowLink", () => {
  beforeEach(() => {
    emitted.length = 0
    closeListeners.length = 0
    openFails = false
  })

  it("knows what the window shows until it is destroyed, and only by its own label", async () => {
    const closed: string[] = []
    const link = createWindowLink<{ repositoryPath: string }>({ label: "thing-review", onClose: shown => closed.push(shown.repositoryPath) })
    await link.open({ repositoryPath: "/repo" }, WINDOW)
    // shown again in the open window: one listener of the close
    await link.open({ repositoryPath: "/repo" }, WINDOW)
    await vi.waitFor(() => expect(closeListeners).toHaveLength(1))
    expect(link.shown()).toEqual({ repositoryPath: "/repo" })

    destroy("other-review")
    expect(link.shown()).toEqual({ repositoryPath: "/repo" })
    destroy("thing-review")
    expect(link.shown()).toBeNull()
    expect(closed).toEqual(["/repo"])
    expect(closeListeners).toHaveLength(0)

    // the next open listens again
    await link.open({ repositoryPath: "/repo" }, WINDOW)
    await vi.waitFor(() => expect(closeListeners).toHaveLength(1))
  })

  it("sends the state only when it changed", async () => {
    let count = 1
    const link = createWindowLink<{ repositoryPath: string }, { repositoryPath: string; count: number }>({
      label: "thing-review",
      buildState: shown => ({ repositoryPath: shown.repositoryPath, count }),
    })
    link.broadcast()
    expect(emitted).toEqual([])
    link.setShown({ repositoryPath: "/repo" })
    // the data of the open carries it
    expect(link.takeState()).toEqual({ repositoryPath: "/repo", count: 1 })
    link.broadcast()
    expect(emitted).toEqual([])
    count = 2
    link.broadcast()
    link.broadcast()
    expect(emitted).toEqual([{ event: windowLinkEvents("thing-review").state, payload: { repositoryPath: "/repo", count: 2 } }])
  })

  it("answers each request by its id, with the feedback in between", async () => {
    const link = createWindowLink<{ repositoryPath: string }>({ label: "thing-review" })
    const serve = link.serve<{ id: string; repositoryPath: string; fail: boolean }, string>((request, feedback) => {
      if (request.fail) {
        feedback("Cannot do it")
        return false
      }
      return Promise.resolve(true)
    })
    serve({ id: "a", repositoryPath: "/repo", fail: true })
    serve({ id: "b", repositoryPath: "/repo", fail: false })
    const events = windowLinkEvents("thing-review")
    await vi.waitFor(() => expect(emitted).toHaveLength(3))
    expect(emitted).toEqual([
      { event: events.feedback, payload: "Cannot do it" },
      { event: events.reply, payload: { id: "a", ok: false } },
      { event: events.reply, payload: { id: "b", ok: true } },
    ])
  })

  it("forgets a window that failed to open", async () => {
    const link = createWindowLink<{ repositoryPath: string }>({ label: "thing-review" })
    openFails = true
    await expect(link.open({ repositoryPath: "/repo" }, WINDOW)).rejects.toThrow("no window")
    expect(link.shown()).toBeNull()
    expect(closeListeners).toHaveLength(0)
  })
})
