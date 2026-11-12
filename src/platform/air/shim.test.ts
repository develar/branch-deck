// The shim of the page in the IDE (openspec/specs/repository/ide-embedding/spec.md, "Page: The shim maps the Tauri calls
// onto x-air-app"): the calls of the page as they reach the host on the frame link, and what the host tells the page
import { describe, expect, it } from "vite-plus/test"
import type { RepositoryView, ViewPatch, ViewSubscription } from "@/lib/bindings"
import { FrameLink } from "@/platform/air/frame-link"
import type { FrameEndpoint } from "@/platform/air/frame-link"
import { AirPage, HOST_TOOLS } from "@/platform/air/shim"
import type { PageCommand } from "@/platform/tauri-page"
import { emptyView } from "@/test-support/fake-sessions"

const COMMANDS: PageCommand[] = [
  { name: "get_file_diffs", class: "read", transport: { channel: "events", open: false } },
  { name: "subscribe_view", class: "read", transport: { channel: "patches", open: true } },
  { name: "get_blob_syntax", class: "read", transport: "raw" },
  { name: "get_settings_changes", class: "read", transport: "json" },
]

interface Message {
  readonly id?: number
  readonly method?: string
  readonly params?: Record<string, unknown>
}

/** The host page of a test: what the frame posted, and its answers and notifications to the frame */
class FakeHost implements FrameEndpoint {
  readonly posted: Message[] = []
  private listener: (message: unknown) => void = () => {}

  post(message: unknown): void {
    this.posted.push(message as Message)
  }

  listen(listener: (message: unknown) => void): () => void {
    this.listener = listener
    return () => {}
  }

  /** The last request of the frame */
  last(): Message {
    return this.posted.at(-1)!
  }

  answer(id: number, result: unknown): void {
    this.listener({ jsonrpc: "2.0", id, result })
  }

  fail(id: number, code: number, reason: string): void {
    this.listener({ jsonrpc: "2.0", id, error: { code, message: reason, data: { reason } } })
  }

  tell(method: string, params: Record<string, unknown>): void {
    this.listener({ jsonrpc: "2.0", method, params })
  }
}

function pageOn(host: FakeHost): { page: AirPage; tauri: (cmd: string, payload?: Record<string, unknown>) => Promise<unknown> } {
  const page = new AirPage(new FrameLink(host), { app: "branch-deck", view: "main", instance: "main:1", params: {} }, COMMANDS)
  const target = { addEventListener: () => {} } as unknown as Window
  page.install(target)
  const internals = (target as unknown as { __TAURI_INTERNALS__: { invoke: (cmd: string, payload?: Record<string, unknown>) => Promise<unknown> } }).__TAURI_INTERNALS__
  return { page, tauri: (cmd, payload) => internals.invoke(cmd, payload) }
}

/** Lets the promises of the page run */
const settle = () => new Promise(resolve => setTimeout(resolve, 0))

describe("The shim of the page in the IDE", () => {
  it("A command with a channel: the channel gets each call event of the call, in order, before the promise resolves", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    const events: unknown[] = []
    let resolved = false
    const answer = tauri("get_file_diffs", { params: { commit: "c" }, events: { onmessage: (event: unknown) => events.push(event) } }).then(value => {
      resolved = true
      expect(events).toEqual([{ file: "a.txt" }, { file: "b.txt" }])
      return value
    })
    await settle()
    const call = host.last()
    expect(call).toMatchObject({ method: "tools/call", params: { name: "bd_invoke", arguments: { command: "get_file_diffs", args: { params: { commit: "c" } } } } })
    host.tell("x-air-app/callEvent", { callId: call.id, event: { file: "a.txt" } })
    host.tell("x-air-app/callEvent", { callId: 999, event: { file: "of another call" } })
    host.tell("x-air-app/callEvent", { callId: call.id, event: { file: "b.txt" } })
    expect(resolved).toBe(false)
    host.answer(call.id!, { content: [], structuredContent: { value: null } })
    expect(await answer).toBeNull()
  })

  it("A JSON command answers its value, a raw one its bytes, a failed one its ApiError", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    const json = tauri("get_settings_changes", { since: 0 })
    await settle()
    host.answer(host.last().id!, { content: [], structuredContent: { value: { changes: [] } } })
    expect(await json).toEqual({ changes: [] })

    const raw = tauri("get_blob_syntax", { params: { id: "b" } })
    await settle()
    host.answer(host.last().id!, { content: [], structuredContent: { bytes: btoa("\u0001\u0002") } })
    expect([...new Uint8Array((await raw) as ArrayBuffer)]).toEqual([1, 2])

    const failed = tauri("nothing", {})
    await settle()
    host.answer(host.last().id!, { content: [{ type: "text", text: "no" }], isError: true, structuredContent: { error: { code: "invalid_input", message: "no" } } })
    await expect(failed).rejects.toMatchObject({ name: "ApiError", code: "invalid_input" })

    const lost = tauri("get_settings_changes", { since: 0 })
    await settle()
    host.fail(host.last().id!, -32003, "connectionLost")
    await expect(lost).rejects.toMatchObject({ name: "ApiError", code: "internal", details: { reason: "connectionLost" } })
  })

  it("subscribe_view is the mirror of the repository: its state the view, each change a patch, a resync a snapshot without a reload", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    const patches: ViewPatch[] = []
    const subscribed = tauri("subscribe_view", { params: { repositoryPath: "/r" }, patches: { onmessage: (patch: ViewPatch) => patches.push(patch) } }) as Promise<ViewSubscription>
    await settle()
    expect(host.last()).toMatchObject({ method: "x-air-app/subscribe", params: { params: { repositoryPath: "/r" } } })
    const state = emptyView({ epoch: 4, seq: 10 })
    host.answer(host.last().id!, { subscription: "x-air-ide:/appView/1", state, fromSeq: 0 })
    const subscription = await subscribed
    expect(subscription.view).toEqual(state)
    expect(subscription.catchUp).toBeNull()

    host.tell("x-air-app/action", {
      subscription: "x-air-ide:/appView/1",
      serverSeq: 1,
      action: {
        type: "x-air-app/viewChanged",
        ops: [
          { op: "part", part: "version", value: { epoch: 4, seq: 11 } },
          { op: "part", part: "order", value: ["a"] },
        ],
      },
    })
    expect(patches).toEqual([{ base: { epoch: 4, seq: 10 }, next: { epoch: 4, seq: 11 }, changes: { order: ["a"] } }])

    // a resync: the page asks for the snapshot (a patch from no base it holds), which is the state of the mirror
    const resynced: RepositoryView = { ...emptyView({ epoch: 4, seq: 20 }), order: ["b"] }
    host.tell("x-air-app/resync", { subscription: "x-air-ide:/appView/1", state: resynced, fromSeq: 7 })
    expect(patches.at(-1)).toEqual({ base: { epoch: 4, seq: -1 }, next: { epoch: 4, seq: 20 }, changes: {} })
    const posted = host.posted.length
    expect(await tauri("get_snapshot", { params: { repositoryPath: "/r" } })).toEqual(resynced)
    expect(host.posted).toHaveLength(posted)

    // unsubscribe_view ends the subscription of the frame
    void tauri("unsubscribe_view", { params: { id: subscription.id } })
    await settle()
    expect(host.last()).toMatchObject({ method: "x-air-app/unsubscribe", params: { subscription: "x-air-ide:/appView/1" } })
  })

  it("listen hears the events of x-air-app, and emit goes to the views of the app through brd mcp", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    const heard: unknown[] = []
    await tauri("plugin:event|listen", { event: "settings_changed", target: { kind: "Any" }, handler: (event: { payload: unknown }) => heard.push(event.payload) })
    host.tell("x-air-app/event", { event: "settings_changed", payload: { key: "theme" } })
    host.tell("x-air-app/event", { event: "other", payload: {} })
    expect(heard).toEqual([{ key: "theme" }])

    void tauri("plugin:event|emit_to", { event: "review/run", payload: { a: 1 }, target: { kind: "WebviewWindow", label: "main" } })
    await settle()
    expect(host.last()).toMatchObject({
      method: "tools/call",
      params: { name: "bd_invoke", arguments: { command: "plugin:event|emit", args: { event: "review/run", payload: { a: 1 }, target: "main" } } },
    })
  })

  it("The plugins: a link, the clipboard and the log go to the host, the badge, the progress, a notification and the close of the window are bd_invoke of their plugin calls, the size does nothing", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    void tauri("plugin:opener|open_url", { url: "https://example.com" })
    expect(host.last()).toMatchObject({ method: "ui/open-link", params: { url: "https://example.com" } })
    void tauri("plugin:clipboard-manager|write_text", { text: "copied" })
    expect(host.last()).toMatchObject({ method: "tools/call", params: { name: HOST_TOOLS.copy, arguments: { text: "copied" } } })
    await tauri("plugin:log|log", { level: 5, message: "boom" })
    expect(host.last()).toEqual({ jsonrpc: "2.0", method: "notifications/message", params: { level: "error", logger: "branch-deck", data: "boom" } })

    // brd mcp is the one path of the desktop of the IDE: it asks the IDE for the badge, the progress, the notification
    const desktop = (command: string, args: Record<string, unknown>) => ({ method: "tools/call", params: { name: "bd_invoke", arguments: { command, args } } })
    void tauri("plugin:window|set_badge_count", { label: "main", value: 2 })
    await settle()
    expect(host.last()).toMatchObject(desktop("plugin:window|set_badge_count", { value: 2 }))
    void tauri("plugin:window|set_progress_bar", { label: "main", value: { status: "normal", progress: 40 } })
    await settle()
    expect(host.last()).toMatchObject(desktop("plugin:window|set_progress_bar", { value: { status: "normal", progress: 40 } }))
    const target = { addEventListener: () => {} } as unknown as Window & { Notification: new (title: string, options?: NotificationOptions) => unknown }
    new AirPage(new FrameLink(host), { app: "branch-deck", view: "main", instance: "main:1", params: {} }, COMMANDS).install(target)
    new target.Notification("Safe Push failed", { body: "feature" })
    await settle()
    expect(host.last()).toMatchObject(desktop("plugin:notification|notify", { options: { title: "Safe Push failed", body: "feature" } }))
    // the news a notification tells goes to brd with it, which gives it the action of the news
    const news = { repositoryPath: "/repo", news: 4 }
    new target.Notification("Safe Push of feature failed", { body: "feature → master", extra: news } as NotificationOptions)
    await settle()
    expect(host.last()).toMatchObject(desktop("plugin:notification|notify", { options: { title: "Safe Push of feature failed", body: "feature → master", extra: news } }))
    // Escape in a review closes its window: brd asks the IDE to close the place
    void tauri("plugin:window|close", { label: "review" })
    await settle()
    expect(host.last()).toMatchObject(desktop("plugin:window|close", { label: "review" }))
    expect(host.posted.filter(message => message.method === "tools/call" && String((message.params as { name?: string }).name).startsWith("air_set"))).toEqual([])

    // the IDE owns the place: its size and its frame are no calls of the page
    const posted = host.posted.length
    expect(await tauri("plugin:window|set_min_size", { value: { width: 1, height: 1 } })).toBeNull()
    expect(host.posted).toHaveLength(posted)
  })
})
