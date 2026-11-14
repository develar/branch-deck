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

  /** The views the frame opened: its `x-air-app/subscribe` requests */
  subscribes(): Message[] {
    return this.posted.filter(message => message.method === "x-air-app/subscribe")
  }

  /** The last request of the frame */
  last(): Message {
    return this.posted.at(-1)!
  }

  answer(id: number, result: unknown): void {
    this.listener({ jsonrpc: "2.0", id, result })
  }

  fail(id: number, code: number, reason: string, data: Record<string, unknown> = {}): void {
    this.listener({ jsonrpc: "2.0", id, error: { code, message: reason, data: { reason, ...data } } })
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

/**
 * The page subscribes to `/r` and again while the view of the frame opens, which the second joins: one `subscribe` of
 * the frame, `opening`, for both
 */
async function subscribeTwiceWhileOpening(host: FakeHost, tauri: (cmd: string, payload?: Record<string, unknown>) => Promise<unknown>) {
  const first = tauri("subscribe_view", { params: { repositoryPath: "/r" }, patches: { onmessage: () => {} } })
  await settle()
  const opening = host.last()
  const second = tauri("subscribe_view", { params: { repositoryPath: "/r" }, patches: { onmessage: () => {} } }) as Promise<ViewSubscription>
  await settle()
  expect(host.subscribes()).toHaveLength(1)
  return { first, second, opening }
}

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

  it("The end of a mirror, for each reason alike (sessionClosed, serverExited, closed, a reason the page does not know): the snapshot of its repository is none, so the page subscribes again, and a late change of the mirror reaches nothing", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    for (const [reason, subscription] of [
      ["sessionClosed", "x-air-ide:/appView/1"],
      ["serverExited", "x-air-ide:/appView/2"],
      ["closed", "x-air-ide:/appView/3"],
      ["somethingNew", "x-air-ide:/appView/4"],
    ] as const) {
      const patches: ViewPatch[] = []
      const subscribed = tauri("subscribe_view", {
        params: { repositoryPath: "/r" },
        patches: { onmessage: (patch: ViewPatch) => patches.push(patch) },
      }) as Promise<ViewSubscription>
      await settle()
      expect(host.last()).toMatchObject({ method: "x-air-app/subscribe", params: { params: { repositoryPath: "/r" } } })
      host.answer(host.last().id!, { subscription, state: emptyView({ epoch: 4, seq: 10 }), fromSeq: 0 })
      const { id } = await subscribed

      host.tell("x-air-app/subscriptionEnded", { subscription, reason })
      // a patch from no base the page holds: the page asks for the snapshot, which is none, whatever session brd holds
      expect(patches).toEqual([{ base: { epoch: expect.any(Number), seq: -1 }, next: { epoch: expect.any(Number), seq: 0 }, changes: {} }])
      const posted = host.posted.length
      expect(await tauri("get_snapshot", { params: { repositoryPath: "/r" } })).toBeNull()
      expect(host.posted).toHaveLength(posted)

      // a late change of the ended subscription, and the unsubscribe of the page, which Air already ended
      host.tell("x-air-app/action", { subscription, serverSeq: 9, action: { type: "x-air-app/viewChanged", ops: [{ op: "part", part: "order", value: ["late"] }] } })
      expect(patches).toHaveLength(1)
      await tauri("unsubscribe_view", { params: { id } })
      expect(host.posted).toHaveLength(posted)
    }

    // the page subscribes again: the frame opens the view anew, whose state is the snapshot from then on
    const again = tauri("subscribe_view", { params: { repositoryPath: "/r" }, patches: { onmessage: () => {} } }) as Promise<ViewSubscription>
    await settle()
    expect(host.last()).toMatchObject({ method: "x-air-app/subscribe", params: { params: { repositoryPath: "/r" } } })
    const state: RepositoryView = { ...emptyView({ epoch: 5, seq: 0 }), order: ["b"] }
    host.answer(host.last().id!, { subscription: "x-air-ide:/appView/5", state, fromSeq: 9 })
    expect((await again).view).toEqual(state)
    expect(await tauri("get_snapshot", { params: { repositoryPath: "/r" } })).toEqual(state)
  })

  it("A subscription that brd refused fails with the ApiError brd answered, which Air passes on in the data of its error", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    const subscribed = tauri("subscribe_view", { params: { repositoryPath: "/r" }, patches: { onmessage: () => {} } })
    await settle()
    const refusal = { code: "no_branch_prefix", message: "No branch prefix", details: { prefixSuggestion: "test" }, retry: "no", operation_id: null }
    host.fail(host.last().id!, -32603, "badResponse", { error: refusal })
    await expect(subscribed).rejects.toEqual({ ...refusal, name: "ApiError" })

    // the view of the frame went with it: the next subscription subscribes the frame anew
    void tauri("subscribe_view", { params: { repositoryPath: "/r" }, patches: { onmessage: () => {} } })
    await settle()
    expect(host.subscribes()).toHaveLength(2)
  })

  it("A subscription asked while the view of the frame opens gets an answer of brd asked after it: the refusal of that view, asked before a prefix was saved, makes it subscribe anew", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    // the focus of the window asks the subscription again while the repository has no prefix; Save of the welcome card
    // wrote the prefix: the page subscribes again, which joins the view that opens
    const { first, second, opening } = await subscribeTwiceWhileOpening(host, tauri)

    // brd read the config before the prefix: the first is refused, the second subscribes the frame anew
    const refusal = { code: "no_branch_prefix", message: "No branch prefix", details: { prefixSuggestion: "test" }, retry: "no", operation_id: null }
    host.fail(opening.id!, -32603, "badResponse", { error: refusal })
    await expect(first).rejects.toMatchObject({ name: "ApiError", code: "no_branch_prefix" })
    await settle()
    expect(host.subscribes()).toHaveLength(2)
    expect(host.last()).toMatchObject({ method: "x-air-app/subscribe", params: { params: { repositoryPath: "/r" } } })
    const state = emptyView({ epoch: 1, seq: 0 })
    host.answer(host.last().id!, { subscription: "x-air-ide:/appView/1", state, fromSeq: 0 })
    expect((await second).view).toEqual(state)
    // the frame holds the one view it opened: no unsubscribe, no bd_invoke of a subscribe again
    expect(host.posted.map(message => message.method)).toEqual(["x-air-app/subscribe", "x-air-app/subscribe"])
  })

  it("A subscription that joined a view whose link broke fails with it: only a refusal of brd is an answer of the config as it was", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    const { first, second, opening } = await subscribeTwiceWhileOpening(host, tauri)
    host.fail(opening.id!, -32003, "connectionLost")
    await expect(first).rejects.toMatchObject({ name: "ApiError", code: "internal", details: { reason: "connectionLost" } })
    await expect(second).rejects.toMatchObject({ name: "ApiError", code: "internal", details: { reason: "connectionLost" } })
    expect(host.subscribes()).toHaveLength(1)
  })

  it("A switch of the repository before the first answer: the next repository subscribes once the view of the one before answered and was unsubscribed, so the frame holds one view", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    const first = tauri("subscribe_view", { params: { repositoryPath: "/a" }, patches: { onmessage: () => {} } }) as Promise<ViewSubscription>
    await settle()
    const subscribeA = host.last()
    // `openSession` of another repository: the session it left has no answer yet, so the page unsubscribes it later
    const patches: ViewPatch[] = []
    const second = tauri("subscribe_view", { params: { repositoryPath: "/b" }, patches: { onmessage: (patch: ViewPatch) => patches.push(patch) } }) as Promise<ViewSubscription>
    await settle()
    expect(host.subscribes()).toEqual([expect.objectContaining({ params: { params: { repositoryPath: "/a" } } })])

    host.answer(subscribeA.id!, { subscription: "x-air-ide:/appView/1", state: emptyView({ epoch: 1, seq: 3 }), fromSeq: 0 })
    const { id } = await first
    void tauri("unsubscribe_view", { params: { id } })
    await settle()
    expect(host.last()).toMatchObject({ method: "x-air-app/unsubscribe", params: { subscription: "x-air-ide:/appView/1" } })
    expect(host.subscribes()).toHaveLength(1)
    host.answer(host.last().id!, {})
    await settle()
    expect(host.last()).toMatchObject({ method: "x-air-app/subscribe", params: { params: { repositoryPath: "/b" } } })
    const state = emptyView({ epoch: 2, seq: 0 })
    host.answer(host.last().id!, { subscription: "x-air-ide:/appView/2", state, fromSeq: 4 })
    expect((await second).view).toEqual(state)

    // a late change of the view it left reaches nothing, one of the view of /b its subscription
    const change = (subscription: string, seq: number) => ({
      subscription,
      serverSeq: seq,
      action: { type: "x-air-app/viewChanged", ops: [{ op: "part", part: "version", value: { epoch: 2, seq } }] },
    })
    host.tell("x-air-app/action", change("x-air-ide:/appView/1", 1))
    host.tell("x-air-app/action", change("x-air-ide:/appView/2", 1))
    expect(patches).toEqual([{ base: { epoch: 2, seq: 0 }, next: { epoch: 2, seq: 1 }, changes: {} }])
  })

  it("A subscription again of the repository the frame holds shares its view: brd subscribes again, moving the mirror to the session of another prefix before it answers, and the subscription it replaces leaves without an unsubscribe of the frame", async () => {
    const host = new FakeHost()
    const { tauri } = pageOn(host)
    const subscription = "x-air-ide:/appView/1"
    const replaced: ViewPatch[] = []
    const first = tauri("subscribe_view", { params: { repositoryPath: "/r" }, patches: { onmessage: (patch: ViewPatch) => replaced.push(patch) } }) as Promise<ViewSubscription>
    await settle()
    host.answer(host.last().id!, { subscription, state: emptyView({ epoch: 4, seq: 10 }), fromSeq: 0 })
    const held = await first

    // `resubscribeSession` after `prefix_changed`: the subscription it replaces holds until the new one answers
    const patches: ViewPatch[] = []
    const again = tauri("subscribe_view", {
      params: { repositoryPath: "/r", have: { epoch: 4, seq: 10 } },
      patches: { onmessage: (patch: ViewPatch) => patches.push(patch) },
    }) as Promise<ViewSubscription>
    await settle()
    expect(host.last()).toMatchObject({
      method: "tools/call",
      params: { name: "bd_invoke", arguments: { command: "subscribe_view", args: { params: { repositoryPath: "/r", have: { epoch: 4, seq: 10 } } } } },
    })
    // brd moves the mirror to the session of the new prefix: every part, a version of another epoch
    const moved: RepositoryView = { ...emptyView({ epoch: 9, seq: 0 }), order: ["b"] }
    const parts = Object.entries(moved)
      .filter(([part]) => part !== "version")
      .map(([part, value]) => ({ op: "part", part, value }))
    host.tell("x-air-app/action", { subscription, serverSeq: 1, action: { type: "x-air-app/viewChanged", ops: [...parts, { op: "part", part: "version", value: moved.version }] } })
    // no patch of the view the page holds: a snapshot, which is the state of the mirror
    expect(replaced).toEqual([{ base: { epoch: 9, seq: -1 }, next: { epoch: 9, seq: 0 }, changes: {} }])
    host.answer(host.last().id!, { content: [], structuredContent: { value: null } })
    expect((await again).view).toEqual(moved)
    expect(await tauri("get_snapshot", { params: { repositoryPath: "/r" } })).toEqual(moved)

    // the replaced subscription leaves: the frame keeps its view for the new one, which gets the next change
    await tauri("unsubscribe_view", { params: { id: held.id } })
    host.tell("x-air-app/action", { subscription, serverSeq: 2, action: { type: "x-air-app/viewChanged", ops: [{ op: "part", part: "version", value: { epoch: 9, seq: 1 } }] } })
    expect(patches).toEqual([{ base: { epoch: 9, seq: 0 }, next: { epoch: 9, seq: 1 }, changes: {} }])
    expect(replaced).toHaveLength(1)

    // again under the same prefix: brd moves nothing, and the page holds the view at that version, so none comes
    const same = tauri("subscribe_view", { params: { repositoryPath: "/r", have: { epoch: 9, seq: 1 } }, patches: { onmessage: () => {} } }) as Promise<ViewSubscription>
    await settle()
    host.answer(host.last().id!, { content: [], structuredContent: { value: null } })
    expect(await same).toMatchObject({ view: null, catchUp: null })
    // a session brd cannot open now fails the subscription with its error, and the view stays for those that hold it
    const refused = tauri("subscribe_view", { params: { repositoryPath: "/r" }, patches: { onmessage: () => {} } })
    await settle()
    host.answer(host.last().id!, { content: [], isError: true, structuredContent: { error: { code: "no_branch_prefix", message: "no prefix" } } })
    await expect(refused).rejects.toMatchObject({ name: "ApiError", code: "no_branch_prefix" })
    expect(host.posted.filter(message => message.method === "x-air-app/subscribe" || message.method === "x-air-app/unsubscribe")).toHaveLength(1)
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
