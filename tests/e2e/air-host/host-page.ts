/**
 * The fake IDE of the E2E tests of the Air build (docs/design/e2e-testing.md, "The page in the IDE"): one page that
 * plays the host page and `air-ide` of the contract (docs/design/ide-embedding/protocol/README.md). Each view is a frame
 * of the Air build (`dist-air/`) at the origin of the app, served by the routes of the test; the frame link is the
 * `AppBridge` of MCP Apps with the extension `x-air-app`, and the app link is the WebSocket of the test server, where
 * `brd mcp` of the fixture repository answers (`mcp-host`). What a test checks is in `window.__airHost`.
 *
 * Bundled by `fake-host.ts` and written into a page of the test server's origin, so the WebSocket is a loopback one.
 */
import { AppBridge, PostMessageTransport } from "@modelcontextprotocol/ext-apps/app-bridge"
import { z } from "zod"

/** What the test gives the page (`window.__airHostConfig`) */
export interface AirHostConfig {
  /** The WebSocket of the app link on the test server */
  readonly appUrl: string
  /** The origin of the frames of the app (`https://branch-deck.air-genui-app.local`) */
  readonly appOrigin: string
  readonly repositoryPath: string
  /** The Git roots of the project (`roots/list`), the repository first; the repository alone when none */
  readonly roots?: readonly string[]
  readonly theme: HostContextTheme
}

export interface HostContextTheme {
  readonly theme: "light" | "dark"
  readonly styles: { readonly variables: Record<string, string> }
  readonly "com.jetbrains.air/variables"?: Record<string, string>
}

/** A request of `brd mcp` to the IDE, a host tool a page called, or a link a page opened, as the test reads them */
export interface HostCall {
  readonly method: string
  readonly params: Record<string, unknown>
}

export interface HostView {
  readonly instance: string
  readonly view: string
  readonly key: string | null
  readonly title: string | null
  readonly params: Record<string, unknown>
}

const APP = "branch-deck"
const X_AIR_APP = { version: "1.0.0" }
/** The tools of the host itself (`HOST_TOOLS` of src/platform/air/shim.ts): never sent to the app */
const HOST_TOOLS = new Set(["air_copy"])

const config = (window as unknown as { __airHostConfig: AirHostConfig }).__airHostConfig
const calls: HostCall[] = []
const logs: unknown[] = []

// === The app link: JSON-RPC 2.0 over the WebSocket of the test server ===

interface Pending {
  resolve(result: unknown): void
  reject(error: unknown): void
}

class AppLink {
  private nextId = 1
  private readonly pending = new Map<number, Pending>()
  private readonly opened: Promise<void>
  onNotification: (method: string, params: Record<string, unknown>) => void = () => {}
  onRequest: (method: string, params: Record<string, unknown>) => Promise<unknown> = async () => ({})

  constructor(private readonly socket: WebSocket) {
    this.opened = new Promise((resolve, reject) => {
      socket.addEventListener("open", () => resolve())
      socket.addEventListener("error", () => reject(new Error(`Cannot open the app link ${socket.url}`)))
    })
    socket.addEventListener("message", event => this.receive(JSON.parse(String(event.data)) as Record<string, unknown>))
  }

  /** The id the next request takes */
  peekId(): number {
    return this.nextId
  }

  async request(method: string, params: Record<string, unknown>): Promise<unknown> {
    await this.opened
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      this.socket.send(JSON.stringify({ jsonrpc: "2.0", id, method, params }))
    })
  }

  async notify(method: string, params?: Record<string, unknown>): Promise<void> {
    await this.opened
    this.socket.send(JSON.stringify({ jsonrpc: "2.0", method, ...(params !== undefined && { params }) }))
  }

  private receive(message: Record<string, unknown>): void {
    const { id, method } = message
    const params = (message.params ?? {}) as Record<string, unknown>
    if (typeof method === "string") {
      if (id === undefined) {
        this.onNotification(method, params)
        return
      }
      this.onRequest(method, params).then(
        result => this.socket.send(JSON.stringify({ jsonrpc: "2.0", id, result })),
        (error: unknown) => this.socket.send(JSON.stringify({ jsonrpc: "2.0", id, error: { code: -32603, message: String(error) } })),
      )
      return
    }
    const pending = this.pending.get(id as number)
    if (pending === undefined) {
      return
    }
    this.pending.delete(id as number)
    if (message.error !== undefined) {
      pending.reject(message.error)
    } else {
      pending.resolve(message.result)
    }
  }
}

const app = new AppLink(new WebSocket(config.appUrl))

/** An error of the app link answered to the frame as it came: its code, its words and its reason */
class LinkError extends Error {
  constructor(readonly error: { code: number; message: string; data?: unknown }) {
    super(error.message)
  }
}

// === The views: a frame each, its instance numbered by its view ===

interface OpenView {
  readonly info: HostView
  readonly frame: HTMLIFrameElement
  readonly bridge: AppBridge
  title: string | null
}

const views = new Map<string, OpenView>()
const counts = new Map<string, number>()
let theme = config.theme

/** The mirrors by their key (`repositoryPath`), each with its serverSeq and the subscriptions of the frames */
interface Mirror {
  serverSeq: number
  readonly subscriptions: Map<string, { readonly instance: string; readonly bridge: AppBridge }>
}
const mirrors = new Map<string, Mirror>()
let nextSubscription = 1

/** The calls of the frames in flight on the app link, by the id of the app link */
const appCalls = new Map<number, { readonly bridge: AppBridge; readonly frameId: string | number; readonly progressToken: string | number | undefined }>()

function hostContext(info: HostView): Record<string, unknown> {
  return { ...theme, locale: "en-US", "x-air-app": { app: APP, view: info.view, instance: info.instance, ...(info.key !== null && { key: info.key }), params: info.params } }
}

/** The answer of a call of the app link as the frame takes it: its result, or its error thrown with its code */
async function forward(method: string, params: Record<string, unknown>): Promise<Record<string, unknown>> {
  try {
    return (await app.request(method, params)) as Record<string, unknown>
  } catch (error: unknown) {
    throw new LinkError(error as { code: number; message: string; data?: unknown })
  }
}

function bridgeOf(info: HostView): AppBridge {
  const bridge = new AppBridge(
    null,
    { name: "air", version: "0.0.0" },
    { openLinks: {}, serverTools: {}, logging: {}, experimental: { "x-air-app": X_AIR_APP } },
    { hostContext: hostContext(info) },
  )
  bridge.oncalltool = async (params, extra) => {
    if (HOST_TOOLS.has(params.name)) {
      calls.push({ method: params.name, params: { ...params.arguments, instance: info.instance } })
      return { content: [] }
    }
    const appId = app.peekId()
    appCalls.set(appId, { bridge, frameId: extra.mcpReq.id, progressToken: params._meta?.progressToken as string | number | undefined })
    extra.mcpReq.signal.addEventListener("abort", () => void app.notify("notifications/cancelled", { requestId: appId }))
    try {
      return (await forward("tools/call", {
        name: params.name,
        arguments: params.arguments ?? {},
        _meta: { progressToken: appId, "x-air-app": { origin: "click", app: APP, view: info.view, instance: info.instance } },
      })) as never
    } finally {
      appCalls.delete(appId)
    }
  }
  bridge.onopenlink = async params => {
    calls.push({ method: "ui/open-link", params: { url: params.url, instance: info.instance } })
    return {}
  }
  bridge.onloggingmessage = params => void logs.push(params)
  bridge.setRequestHandler("x-air-app/subscribe", { params: z.object({ params: z.record(z.string(), z.unknown()).optional() }) }, async request => {
    const answer = await forward("x-air-app/subscribe", { instance: info.instance, params: request.params ?? {} })
    const key = String(answer.mirror)
    const mirror = mirrors.get(key) ?? { serverSeq: 0, subscriptions: new Map() }
    mirrors.set(key, mirror)
    const subscription = `x-air-ide:/appView/${nextSubscription++}`
    mirror.subscriptions.set(subscription, { instance: info.instance, bridge })
    return { subscription, state: answer.state as Record<string, unknown>, fromSeq: mirror.serverSeq }
  })
  bridge.setRequestHandler("x-air-app/unsubscribe", { params: z.object({ subscription: z.string() }) }, async request => {
    for (const mirror of mirrors.values()) {
      mirror.subscriptions.delete(request.subscription)
    }
    const held = [...mirrors.values()].some(mirror => [...mirror.subscriptions.values()].some(it => it.instance === info.instance))
    if (!held) {
      await forward("x-air-app/unsubscribe", { instance: info.instance })
    }
    return {}
  })
  return bridge
}

/** The views of one instance: the tool window of the app file (`main`, `place: toolWindow`) */
const SINGLE_VIEWS = new Set(["main"])

/** The instances whose place the IDE hides (another tab, a hidden tool window) */
const hidden = new Set<string>()

async function openView(view: string, key: string | null, params: Record<string, unknown>, title: string | null): Promise<string> {
  const existing = [...views.values()].find(open => open.info.view === view && (key === null ? SINGLE_VIEWS.has(view) : open.info.key === key))
  if (existing !== undefined) {
    existing.frame.dataset.focused = "true"
    // the place of an instance the IDE hid shows again
    if (hidden.delete(existing.info.instance)) {
      await app.notify("x-air-app/viewShown", {
        instance: existing.info.instance,
        view,
        ...(existing.info.key !== null && { key: existing.info.key }),
        params: existing.info.params,
      })
    }
    return existing.info.instance
  }
  const count = (counts.get(view) ?? 0) + 1
  counts.set(view, count)
  const info: HostView = { instance: `${view}:${count}`, view, key, title, params }
  // the place shows before its page loads, as a tool window shows its frame
  await app.notify("x-air-app/viewShown", { instance: info.instance, view, ...(key !== null && { key }), params })
  const frame = document.createElement("iframe")
  frame.dataset.instance = info.instance
  frame.title = title ?? view
  frame.setAttribute("sandbox", "allow-scripts allow-same-origin allow-forms allow-popups")
  frame.setAttribute("allow", "clipboard-read; clipboard-write")
  frame.src = `${config.appOrigin}/${view}`
  frame.style.cssText = "display: block; width: 1280px; height: 800px; border: 0"
  document.body.append(frame)
  const bridge = bridgeOf(info)
  views.set(info.instance, { info, frame, bridge, title })
  await bridge.connect(new PostMessageTransport(frame.contentWindow!, frame.contentWindow!))
  return info.instance
}

// === What the app link tells and asks ===

app.onNotification = (method, params) => {
  switch (method) {
    case "x-air-app/viewChanged": {
      const mirror = mirrors.get(String(params.mirror))
      if (mirror === undefined) {
        return
      }
      mirror.serverSeq++
      for (const [subscription, { bridge }] of mirror.subscriptions) {
        void bridge.notification({ method: "x-air-app/action", params: { subscription, serverSeq: mirror.serverSeq, action: { type: "x-air-app/viewChanged", ops: params.ops } } })
      }
      return
    }
    case "x-air-app/viewEnded": {
      const mirror = mirrors.get(String(params.mirror))
      mirrors.delete(String(params.mirror))
      for (const [subscription, { bridge }] of mirror?.subscriptions ?? []) {
        void bridge.notification({ method: "x-air-app/subscriptionEnded", params: { subscription, reason: params.reason } })
      }
      return
    }
    case "x-air-app/event": {
      const target = params.instance === undefined ? [...views.values()] : [views.get(params.instance as string)].filter(open => open !== undefined)
      for (const open of target) {
        void open.bridge.notification({ method: "x-air-app/event", params: { event: params.event, payload: params.payload ?? null } })
      }
      return
    }
    case "x-air-app/callEvent": {
      const call = appCalls.get(params.callId as number)
      void call?.bridge.notification({ method: "x-air-app/callEvent", params: { callId: call.frameId, event: params.event } })
      return
    }
    case "notifications/progress": {
      const call = appCalls.get(params.progressToken as number)
      if (call?.progressToken !== undefined) {
        void call.bridge.notification({ method: "notifications/progress", params: { ...params, progressToken: call.progressToken } })
      }
      return
    }
    case "notifications/message":
      logs.push(params)
      return
  }
}

app.onRequest = async (method, params) => {
  switch (method) {
    case "roots/list":
      return { roots: (config.roots ?? [config.repositoryPath]).map(root => ({ uri: `file://${root}`, name: root.split("/").at(-1) })) }
    case "x-air-app/openView": {
      calls.push({ method, params })
      const instance = await openView(
        String(params.view),
        (params.key as string | undefined) ?? null,
        (params.params as Record<string, unknown> | undefined) ?? {},
        (params.title as string | undefined) ?? null,
      )
      return { instance }
    }
    case "x-air-app/setViewTitle": {
      calls.push({ method, params })
      const open = views.get(String(params.instance))
      if (open !== undefined) {
        open.title = String(params.title)
        open.frame.title = open.title
      }
      return {}
    }
    case "x-air-app/closeView":
      // the IDE closes the place of the view, as a click on the cross of its tab
      calls.push({ method, params })
      await closeView(String(params.instance))
      return {}
    default:
      // notify, setBadge, setProgress: the IDE shows them, the test reads them
      calls.push({ method, params })
      return {}
  }
}

/** The IDE closes the place of a view: its frame goes, `brd mcp` forgets its window */
async function closeView(instance: string): Promise<void> {
  const open = views.get(instance)
  views.delete(instance)
  open?.frame.remove()
  await app.notify("x-air-app/viewClosed", { instance })
}

/**
 * An action of the app file in the title of the tool window (`toolWindow.titleActions`): the IDE calls its tool with
 * the origin action and the instance of the tool window; the answer of the call
 */
async function runAction(action: string, tool: string): Promise<unknown> {
  const main = [...views.values()].find(open => open.info.view === "main")
  calls.push({ method: "action", params: { action, tool } })
  return app.request("tools/call", {
    name: tool,
    arguments: {},
    _meta: { "x-air-app": { origin: "action", app: APP, action, ...(main !== undefined && { view: "main", instance: main.info.instance }) } },
  })
}

/** The IDE hides the place of `instance`: another tab shows, its frame stays */
async function hideView(instance: string): Promise<void> {
  hidden.add(instance)
  await app.notify("x-air-app/viewHidden", { instance })
}

/**
 * A click on the action `index` of the last notification `brd mcp` asked for: the IDE runs its `run`, a call of its tool
 * with the origin action, or the open of its view; the answer of the call, or the instance of the view
 */
async function runNotificationAction(index: number): Promise<unknown> {
  const notification = [...calls].reverse().find(call => call.method === "x-air-app/notify")
  const run = (notification?.params.actions as { run: Record<string, unknown> }[] | undefined)?.[index]?.run
  if (run === undefined) {
    throw new Error(`No notification of brd has the action ${index}`)
  }
  if (typeof run.view === "string") {
    return openView(run.view, (run.key as string | undefined) ?? null, (run.params as Record<string, unknown> | undefined) ?? {}, null)
  }
  return app.request("tools/call", { name: run.tool, arguments: run.arguments ?? {}, _meta: { "x-air-app": { origin: "action", app: APP } } })
}

/** What a test reads and does (`fake-host.ts`) */
const host = {
  calls: () => calls,
  logs: () => logs,
  views: (): HostView[] => [...views.values()].map(open => ({ ...open.info, title: open.title })),
  open: (view: string, params: Record<string, unknown> = {}) => openView(view, null, params, null),
  /** The IDE switches its theme: every frame gets `host-context-changed` */
  setTheme: (next: HostContextTheme) => {
    theme = next
    for (const open of views.values()) {
      open.bridge.setHostContext(hostContext(open.info))
    }
  },
  close: closeView,
  hide: hideView,
  runAction,
  runNotificationAction,
  ready: (async () => {
    await app.request("initialize", {
      protocolVersion: "2025-06-18",
      capabilities: { roots: { listChanged: true }, experimental: { "x-air-app": { ...X_AIR_APP, ahpVersion: "1.0.0" } } },
      clientInfo: { name: "air-ide", version: "1.0.0" },
    })
    await app.notify("notifications/initialized")
  })(),
}
;(window as unknown as { __airHost: typeof host }).__airHost = host
