/**
 * The page of Branch Deck as an app of the IDE (openspec/specs/repository/ide-embedding/spec.md, "Page"): the internals
 * of Tauri (`@/platform/tauri-page`) over the frame link of `x-air-app` (`FrameLink`). A command of app-api is the tool
 * `bd_invoke` of `brd mcp`, its `Channel` the call events of its call, `subscribe_view` the mirror of the repository,
 * which the subscriptions of the page share, as Air holds one view for each frame (`FrameView`), `listen` and `emit` the
 * events of `x-air-app`; the plugins of Tauri are what the IDE offers in their place: a link
 * opens by `ui/open-link`, the clipboard is the tool of the host `air_copy` (`HOST_TOOLS`), the log is
 * `notifications/message`; the badge and the progress of the Dock, an OS notification and the close of the window go to
 * `brd mcp` as their plugin calls (`bd_invoke`), which asks the IDE for them, and a window of the IDE has no size, zoom
 * or frame the page sets. No rule of the domain is here: every command goes to `brd mcp` as the page sends it.
 */
import type { Version, ViewPatch, ViewSubscription } from "@/lib/bindings"
import { FrameLinkError } from "@/platform/air/frame-link"
import type { FrameLink } from "@/platform/air/frame-link"
import { applyOps, patchOf, versionOf, viewOf } from "@/platform/air/view-mirror"
import type { MirrorState, ViewOp } from "@/platform/air/view-mirror"
import { channelArgument, commandChannels, commandError, installTauriInternals, PageEvents } from "@/platform/tauri-page"
import type { PageChannel, PageCommand, PluginHandler } from "@/platform/tauri-page"

/** What a view of the IDE shows (`hostContext["x-air-app"]`): the IDE gives it, the page never sets it */
export interface AirViewContext {
  readonly app: string
  readonly view: string
  readonly instance: string
  readonly key?: string
  readonly title?: string
  readonly params: Readonly<Record<string, unknown>>
}

/** The tools of the host the plugins of Tauri map to; the IDE answers them itself, `brd mcp` never sees them */
export const HOST_TOOLS = {
  copy: "air_copy",
} as const

/**
 * The plugin calls of Tauri that `brd mcp` answers as the desktop of the IDE (`bd_invoke`), with their arguments as the
 * page sends them: it asks the IDE for the badge and the progress of the tool window, a notification, the close of the
 * place of the view
 */
const DESKTOP_CALLS: ReadonlySet<string> = new Set([
  "plugin:window|set_badge_count",
  "plugin:window|set_progress_bar",
  "plugin:window|close",
  "plugin:window|destroy",
  "plugin:notification|notify",
])

/** The tools of `brd mcp` (`tools/list`): both only for the app */
export const BD_INVOKE = "bd_invoke"
export const BD_BOOTSTRAP = "bd_bootstrap"

/** The result of `tools/call` as the page reads it (`CallToolResult` of `x-air-app.schema.json`) */
export interface ToolResult {
  readonly content?: readonly unknown[]
  readonly isError?: boolean
  readonly structuredContent?: Readonly<Record<string, unknown>>
}

/** The answer of `x-air-app/subscribe`: the subscription of the frame and the state of its mirror */
interface FrameSubscription {
  readonly subscription: string
  readonly state: MirrorState
  readonly fromSeq: number
}

/**
 * The one view of the frame (`x-air-app/subscribe` of a repository), which Air holds for each page connection: a
 * `subscribe` while another runs gets `-32011 viewOpen` at once, and one while a view is held waits for its end and then
 * gets the same. So every subscription of the page to the repository shares it (`subscribe_view` again, as
 * `resubscribeSession` of `src/model/store.ts` does after `prefix_changed`), and the next repository subscribes only
 * once the view before it answered and was unsubscribed or ended
 */
class FrameView {
  /** The page subscriptions that hold the view, by their id; null until the page has its answer, so no patch reaches it */
  readonly pages = new Map<number, PageChannel<ViewPatch> | null>()
  /** The answer of its `subscribe`; rejects with its failure */
  answered: Promise<void> = Promise.resolve()
  /** Resolves once Air holds the view no more: its `subscribe` failed, its `unsubscribe` answered, or Air ended it */
  readonly done: Promise<void>
  subscription: string | null = null
  state: MirrorState = {}
  /** No subscription joins the view any more: its `unsubscribe` runs, or it ended */
  closing = false
  private finish: () => void = () => {}

  constructor(
    readonly repositoryPath: string,
    public version: Version,
  ) {
    this.done = new Promise(resolve => {
      this.finish = resolve
    })
  }

  /** Air holds the view no more */
  end(): void {
    this.closing = true
    this.finish()
  }

  /** Tells each page subscription that has its answer `patch` */
  tell(patch: ViewPatch): void {
    for (const patches of this.pages.values()) {
      patches?.onmessage(patch)
    }
  }
}

/** The level of `notifications/message` of each level of the log plugin of Tauri (`LogLevel`: 1 trace … 5 error) */
const LOG_LEVELS: Readonly<Record<number, string>> = { 1: "debug", 2: "debug", 3: "info", 4: "warning", 5: "error" }

type Payload = Record<string, unknown> | undefined

/** The bytes of base64, as `get_blob_syntax` answers them in the app (`tauri::ipc::Response`) */
function decodeBase64(text: string): ArrayBuffer {
  const binary = atob(text)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index++) {
    bytes[index] = binary.charCodeAt(index)
  }
  return bytes.buffer
}

/** A failure of the link (`connectionLost`, `serverExited`, `cancelled`) as the `ApiError` the page reads */
function linkFailure(error: unknown): unknown {
  if (error instanceof FrameLinkError) {
    return commandError({ code: "internal", message: error.message, details: { reason: error.reason }, retry: "no", operation_id: null })
  }
  return error
}

/**
 * The page of a view on the frame link: install it (`installTauriInternals`) before the page's modules load, so they
 * find their globals and their internals
 */
export class AirPage {
  readonly events = new PageEvents()
  private readonly channels: ReadonlyMap<string, { channel: string; open: boolean }>
  /** The channel of each call in flight, by the id of its `tools/call` (`callId`) */
  private readonly calls = new Map<number, PageChannel>()
  /** The view the frame holds or opens, at most one */
  private view: FrameView | null = null
  /** The id of the next subscription of the page (`ViewSubscription.id`) */
  private nextSubscriptionId = 1
  /** The epoch of the versions the page numbers itself, for a mirror whose state has no `version` */
  private epoch = 1

  constructor(
    private readonly link: FrameLink,
    readonly context: AirViewContext,
    commands: readonly PageCommand[],
  ) {
    this.channels = commandChannels(commands)
    link.on("x-air-app/callEvent", params => this.calls.get(params.callId as number)?.onmessage(params.event))
    link.on("x-air-app/event", params => this.events.dispatch(String(params.event), params.payload ?? null))
    link.on("x-air-app/action", params => this.changed(String(params.subscription), params.serverSeq as number, params.action as { type?: string; ops?: ViewOp[] }))
    link.on("x-air-app/resync", params => this.resynced(String(params.subscription), params.state as MirrorState, params.fromSeq as number))
    link.on("x-air-app/subscriptionEnded", params => this.ended(String(params.subscription)))
  }

  /** The label of the window of the page: the view (`main`, `review`, `settings`), as the desktop app names its windows */
  get label(): string {
    return this.context.view
  }

  /** Installs the internals of Tauri for this page on `target` and answers the browser's notifications by the IDE's */
  install(target: Window): void {
    installTauriInternals(target, { label: this.label, invoke: (cmd, payload) => this.invoke(cmd, payload), plugins: this.plugins() })
    this.answerNotifications(target)
    // the events of the page's own window, as the shell tells them
    target.addEventListener("focus", () => this.events.dispatch("tauri://focus", null))
    target.addEventListener("blur", () => this.events.dispatch("tauri://blur", null))
    target.addEventListener("resize", () => this.events.dispatch("tauri://resize", { width: target.innerWidth, height: target.innerHeight }))
  }

  /** A tool of `brd mcp` or of the host */
  async callTool(name: string, args: Record<string, unknown>, channel: PageChannel | null = null): Promise<ToolResult> {
    const callId = this.link.peekId()
    if (channel !== null) {
      this.calls.set(callId, channel)
    }
    try {
      return await this.link.request<ToolResult>("tools/call", { name, arguments: args })
    } finally {
      this.calls.delete(callId)
    }
  }

  /** A command of the page: the view's own (`subscribe_view`, `unsubscribe_view`, the snapshot of a mirror) or `bd_invoke` */
  async invoke(cmd: string, payload: Payload): Promise<unknown> {
    switch (cmd) {
      case "subscribe_view":
        return this.subscribe(payload)
      case "unsubscribe_view":
        return this.unsubscribe(payload)
      case "get_snapshot": {
        const repositoryPath = (payload?.params as { repositoryPath?: string } | undefined)?.repositoryPath
        const held = this.view
        if (held !== null && !held.closing && held.subscription !== null && held.repositoryPath === repositoryPath) {
          return viewOf(held.state, held.version)
        }
        return this.runCommand(cmd, payload)
      }
      default:
        return this.runCommand(cmd, payload)
    }
  }

  /** `bd_invoke {command, args}`: the arguments as the page sent them, its `Channel` the call events of the call */
  private async runCommand(cmd: string, payload: Payload): Promise<unknown> {
    const transport = this.channels.get(cmd)
    const channel = transport === undefined ? null : channelArgument(payload, transport.channel)
    // the channel stays in the page: its messages are the call events of the call
    const { [transport?.channel ?? ""]: _channel, ...args } = payload ?? {}
    let result: ToolResult
    try {
      result = await this.callTool(BD_INVOKE, { command: cmd, args }, channel)
    } catch (error: unknown) {
      throw linkFailure(error)
    }
    const content = result.structuredContent ?? {}
    if (result.isError === true) {
      throw commandError(content.error)
    }
    if (typeof content.bytes === "string") {
      return decodeBase64(content.bytes)
    }
    return content.value ?? null
  }

  /**
   * `subscribe_view`: the mirror of the repository, its state the view and each change of it a patch of the channel. The
   * frame holds one view (`FrameView`): a subscription of the repository it holds joins it and asks `brd mcp` to
   * subscribe again (`bd_invoke` of `subscribe_view`), which moves the mirror to the session of the prefix the git
   * config names now before it answers; a subscription of another repository waits until the view before it ended
   */
  private async subscribe(payload: Payload): Promise<ViewSubscription> {
    const params = (payload?.params ?? {}) as { repositoryPath?: string; have?: Version }
    const repositoryPath = String(params.repositoryPath)
    const patches = channelArgument(payload, "patches") as PageChannel<ViewPatch> | null
    const id = this.nextSubscriptionId++
    for (;;) {
      const held = this.view
      if (held === null) {
        const view = this.open(repositoryPath)
        view.pages.set(id, null)
        return this.joined(view, id, patches, params, () => view.answered)
      }
      if (held.repositoryPath === repositoryPath && !held.closing) {
        held.pages.set(id, null)
        // the answer of the view first: brd subscribes the mirror it holds again
        return this.joined(held, id, patches, params, async () => {
          await held.answered
          await this.runCommand("subscribe_view", { params })
        })
      }
      await held.done
    }
  }

  /** The view of `repositoryPath`: its `subscribe` sent */
  private open(repositoryPath: string): FrameView {
    const view = new FrameView(repositoryPath, { epoch: this.epoch, seq: 0 })
    this.view = view
    view.answered = this.link.request<FrameSubscription>("x-air-app/subscribe", { params: { repositoryPath } }).then(
      answer => {
        view.subscription = answer.subscription
        view.state = answer.state
        view.version = versionOf(answer.state, { epoch: this.epoch, seq: answer.fromSeq })
      },
      (error: unknown) => {
        this.end(view)
        throw linkFailure(error)
      },
    )
    return view
  }

  /** Air holds `view` no more: the next subscription subscribes anew */
  private end(view: FrameView): void {
    view.end()
    if (this.view === view) {
      this.view = null
    }
  }

  /**
   * The answer of the page subscription `id` of `view` once `ready` resolved: the view of the mirror, or nothing when the
   * page holds it at that version (`have`); from then its channel gets each change. A failure leaves the view, which
   * ends with its last subscription, and a view Air ended meanwhile is subscribed anew
   */
  private async joined(view: FrameView, id: number, patches: PageChannel<ViewPatch> | null, params: { have?: Version }, ready: () => Promise<void>): Promise<ViewSubscription> {
    try {
      await ready()
    } catch (error: unknown) {
      await this.leave(view, id)
      throw linkFailure(error)
    }
    if (!view.pages.has(id)) {
      return this.subscribe({ params, ...(patches !== null && { patches }) })
    }
    view.pages.set(id, patches ?? { onmessage: () => {} })
    const have = params.have
    const holds = have !== undefined && have.epoch === view.version.epoch && have.seq === view.version.seq
    return { id, view: holds ? null : viewOf(view.state, view.version), catchUp: null }
  }

  private async unsubscribe(payload: Payload): Promise<null> {
    const id = (payload?.params as { id?: number } | undefined)?.id
    if (this.view !== null && typeof id === "number") {
      await this.leave(this.view, id)
    }
    return null
  }

  /** The page subscription `id` leaves `view`: the last one ends it (`x-air-app/unsubscribe`) */
  private async leave(view: FrameView, id: number): Promise<void> {
    if (!view.pages.delete(id) || view.pages.size > 0 || view.closing) {
      return
    }
    view.closing = true
    await view.answered.catch(() => null)
    if (view.subscription !== null) {
      await this.link.request("x-air-app/unsubscribe", { subscription: view.subscription }).catch(() => null)
    }
    this.end(view)
  }

  /** The view of the subscription `subscription` of the frame, while Air holds it */
  private held(subscription: string): FrameView | null {
    const view = this.view
    return view !== null && view.subscription === subscription ? view : null
  }

  /**
   * One change of a mirror (`x-air-app/viewChanged`): the patch of the session from the view the page has. A change to a
   * version of another epoch moves the mirror to another session (another prefix): no patch of the view the page holds,
   * so the page asks for the snapshot, which is the state of the mirror
   */
  private changed(subscription: string, serverSeq: number, action: { type?: string; ops?: ViewOp[] }): void {
    const view = this.held(subscription)
    if (view === null || action.type !== "x-air-app/viewChanged" || action.ops === undefined) {
      // an echo of an intent the mirror refused: the page sends none (`bd_invoke` carries them)
      return
    }
    const before = view.state
    const after = applyOps(before, action.ops)
    const base = view.version
    const next = versionOf(after, { epoch: this.epoch, seq: serverSeq })
    view.state = after
    view.version = next
    view.tell(next.epoch === base.epoch ? patchOf(before, after, action.ops, base, next) : { base: { epoch: next.epoch, seq: -1 }, next, changes: {} })
  }

  /**
   * A resync of the mirror: its state is the snapshot of the view. The page asks for a snapshot on a patch whose base
   * it does not hold (`refresh` of src/model/store.ts), and the snapshot of a held mirror is its state (`get_snapshot`)
   */
  private resynced(subscription: string, state: MirrorState, fromSeq: number): void {
    const view = this.held(subscription)
    if (view === null) {
      return
    }
    if (state.version === undefined) {
      this.epoch++
    }
    view.state = state
    view.version = versionOf(state, { epoch: this.epoch, seq: fromSeq })
    view.tell({ base: { epoch: view.version.epoch, seq: -1 }, next: view.version, changes: {} })
  }

  /**
   * The end of a mirror: the page asks for the snapshot of the session, which `brd mcp` answers (null: it is gone); its
   * subscriptions hold no view of the frame any more, and the next one subscribes anew
   */
  private ended(subscription: string): void {
    const view = this.held(subscription)
    if (view === null) {
      return
    }
    this.end(view)
    this.epoch++
    view.tell({ base: { epoch: this.epoch, seq: -1 }, next: { epoch: this.epoch, seq: 0 }, changes: {} })
    view.pages.clear()
  }

  private hostTool(name: string, args: Record<string, unknown>): Promise<null> {
    return this.callTool(name, args).then(() => null)
  }

  /** The plugins of Tauri, by the prefix of their command */
  private plugins(): Record<`plugin:${string}`, PluginHandler> {
    const action = (cmd: string) => cmd.split("|")[1]
    return {
      // an event for the windows goes through `brd mcp`, which tells it to the views of the app (`x-air-app/event`)
      "plugin:event": this.events.plugin((event, payload, target) => this.runCommand("plugin:event|emit", { event, payload, target })),
      "plugin:window": (cmd, payload) => {
        if (DESKTOP_CALLS.has(cmd)) {
          return this.runCommand(cmd, payload)
        }
        switch (action(cmd)) {
          case "is_focused":
            return document.hasFocus()
          case "is_fullscreen":
          case "is_maximized":
          case "is_minimized":
            return false
          case "is_visible":
            return document.visibilityState === "visible"
          case "get_all_windows":
            return [this.label]
          case "scale_factor":
            return window.devicePixelRatio
          case "inner_size":
          case "outer_size":
            return { width: window.innerWidth * window.devicePixelRatio, height: window.innerHeight * window.devicePixelRatio }
          default:
            // the IDE owns the place: its size, its frame, its theme and its title
            return null
        }
      },
      "plugin:webview": () => null,
      "plugin:path": () => {
        throw new Error("The IDE names no home directory")
      },
      "plugin:log": (_cmd, payload) => {
        this.link.notify("notifications/message", { level: LOG_LEVELS[payload?.level as number] ?? "info", logger: this.context.app, data: payload?.message ?? "" })
        return null
      },
      "plugin:app": cmd => (action(cmd) === "name" ? "BranchDeck" : ""),
      "plugin:notification": (cmd, payload) => {
        if (DESKTOP_CALLS.has(cmd)) {
          return this.runCommand(cmd, payload)
        }
        switch (action(cmd)) {
          case "is_permission_granted":
            return true
          case "request_permission":
            return "granted"
          default:
            return null
        }
      },
      "plugin:opener": (cmd, payload) => {
        if (action(cmd) === "open_url") {
          return this.link.request("ui/open-link", { url: payload?.url }).then(() => null)
        }
        return null
      },
      "plugin:clipboard-manager": (cmd, payload) => {
        if (action(cmd) === "write_text") {
          return this.hostTool(HOST_TOOLS.copy, { text: payload?.text ?? "" })
        }
        return navigator.clipboard.readText()
      },
    }
  }

  /**
   * The notification plugin of Tauri sends through the browser's `Notification`: in the IDE it is the plugin call of
   * Tauri's own `Notification`, which `brd mcp` asks the IDE to show, with the action of the news its `extra` names
   */
  private answerNotifications(target: Window): void {
    // `extra` of the options of Tauri names the news a notification tells, for brd to give it its action
    const notify = (title: string, body: string | undefined, extra: unknown) =>
      this.runCommand("plugin:notification|notify", { options: { title, ...(body !== undefined && { body }), ...(extra !== undefined && { extra }) } }).catch(() => null)
    class IdeNotification {
      static readonly permission: NotificationPermission = "granted"
      static requestPermission(): Promise<NotificationPermission> {
        return Promise.resolve("granted")
      }
      onclick: (() => void) | null = null
      constructor(title: string, options: NotificationOptions & { readonly extra?: unknown } = {}) {
        void notify(title, options.body, options.extra)
      }
      close(): void {}
      addEventListener(): void {}
      removeEventListener(): void {}
    }
    ;(target as unknown as { Notification: unknown }).Notification = IdeNotification
  }
}
