/**
 * The page of Branch Deck as an app of the IDE (openspec/specs/repository/ide-embedding/spec.md, "Page"): the internals
 * of Tauri (`@/platform/tauri-page`) over the frame link of `x-air-app` (`FrameLink`). A command of app-api is the tool
 * `bd_invoke` of `brd mcp`, its `Channel` the call events of its call, `subscribe_view` the mirror of the repository,
 * `listen` and `emit` the events of `x-air-app`; the plugins of Tauri are what the IDE offers in their place: a link
 * opens by `ui/open-link`, the clipboard, an OS notification, the badge and the progress of the Dock are the tools of the
 * host (`HOST_TOOLS`), the log is `notifications/message`, and a window of the IDE has no size, zoom or frame the page
 * sets. No rule of the domain is here: every command goes to `brd mcp` as the page sends it.
 */
import type { RepositoryView, Version, ViewPatch, ViewSubscription } from "@/lib/bindings"
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
  notify: "air_notify",
  setBadge: "air_set_badge",
  setProgress: "air_set_progress",
} as const

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

/** A mirror the page holds: the state of its subscription and the version of the view it gave the page */
interface HeldMirror {
  readonly id: number
  readonly subscription: string
  readonly repositoryPath: string
  readonly patches: PageChannel<ViewPatch>
  state: MirrorState
  version: Version
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
  private readonly mirrors = new Map<string, HeldMirror>()
  private nextMirrorId = 1
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
        const held = [...this.mirrors.values()].find(mirror => mirror.repositoryPath === repositoryPath)
        if (held !== undefined) {
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

  /** `subscribe_view`: the mirror of the repository, its state the view and each change of it a patch of the channel */
  private async subscribe(payload: Payload): Promise<ViewSubscription> {
    const repositoryPath = String((payload?.params as { repositoryPath?: string } | undefined)?.repositoryPath)
    const patches = channelArgument(payload, "patches") as PageChannel<ViewPatch> | null
    let answer: FrameSubscription
    try {
      answer = await this.link.request<FrameSubscription>("x-air-app/subscribe", { params: { repositoryPath } })
    } catch (error: unknown) {
      throw linkFailure(error)
    }
    const version = versionOf(answer.state, { epoch: this.epoch, seq: answer.fromSeq })
    const mirror: HeldMirror = {
      id: this.nextMirrorId++,
      subscription: answer.subscription,
      repositoryPath,
      patches: patches ?? { onmessage: () => {} },
      state: answer.state,
      version,
    }
    this.mirrors.set(answer.subscription, mirror)
    const view: RepositoryView = viewOf(mirror.state, version)
    return { id: mirror.id, view, catchUp: null }
  }

  private async unsubscribe(payload: Payload): Promise<null> {
    const id = (payload?.params as { id?: number } | undefined)?.id
    const mirror = [...this.mirrors.values()].find(held => held.id === id)
    if (mirror !== undefined) {
      this.mirrors.delete(mirror.subscription)
      await this.link.request("x-air-app/unsubscribe", { subscription: mirror.subscription }).catch(() => null)
    }
    return null
  }

  /** One change of a mirror (`x-air-app/viewChanged`): the patch of the session from the view the page has */
  private changed(subscription: string, serverSeq: number, action: { type?: string; ops?: ViewOp[] }): void {
    const mirror = this.mirrors.get(subscription)
    if (mirror === undefined || action.type !== "x-air-app/viewChanged" || action.ops === undefined) {
      // an echo of an intent the mirror refused: the page sends none (`bd_invoke` carries them)
      return
    }
    const before = mirror.state
    const after = applyOps(before, action.ops)
    const base = mirror.version
    const next = versionOf(after, { epoch: this.epoch, seq: serverSeq })
    mirror.state = after
    mirror.version = next
    mirror.patches.onmessage(patchOf(before, after, action.ops, base, next))
  }

  /**
   * A resync of the mirror: its state is the snapshot of the view. The page asks for a snapshot on a patch whose base
   * it does not hold (`refresh` of src/model/store.ts), and the snapshot of a held mirror is its state (`get_snapshot`)
   */
  private resynced(subscription: string, state: MirrorState, fromSeq: number): void {
    const mirror = this.mirrors.get(subscription)
    if (mirror === undefined) {
      return
    }
    if (state.version === undefined) {
      this.epoch++
    }
    mirror.state = state
    mirror.version = versionOf(state, { epoch: this.epoch, seq: fromSeq })
    mirror.patches.onmessage({ base: { epoch: mirror.version.epoch, seq: -1 }, next: mirror.version, changes: {} })
  }

  /** The end of a mirror: the page asks for the snapshot of the session, which `brd mcp` answers (null: it is gone) */
  private ended(subscription: string): void {
    const mirror = this.mirrors.get(subscription)
    if (mirror === undefined) {
      return
    }
    this.mirrors.delete(subscription)
    this.epoch++
    mirror.patches.onmessage({ base: { epoch: this.epoch, seq: -1 }, next: { epoch: this.epoch, seq: 0 }, changes: {} })
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
        switch (action(cmd)) {
          case "set_badge_count":
            return this.hostTool(HOST_TOOLS.setBadge, { view: "main", count: (payload?.value as number | undefined) ?? 0 })
          case "set_progress_bar": {
            const value = payload?.value as { status?: string; progress?: number } | undefined
            return this.hostTool(HOST_TOOLS.setProgress, { view: "main", status: value?.status ?? "none", ...(value?.progress !== undefined && { progress: value.progress }) })
          }
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
            // the IDE owns the place: its size, its frame, its theme, its title and its close
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
      "plugin:notification": cmd => {
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

  /** The notification plugin of Tauri sends through the browser's `Notification`: in the IDE it is the host's */
  private answerNotifications(target: Window): void {
    const notify = (title: string, body: string | undefined) => this.hostTool(HOST_TOOLS.notify, { title, ...(body !== undefined && { body }) }).catch(() => null)
    class IdeNotification {
      static readonly permission: NotificationPermission = "granted"
      static requestPermission(): Promise<NotificationPermission> {
        return Promise.resolve("granted")
      }
      onclick: (() => void) | null = null
      constructor(title: string, options: NotificationOptions = {}) {
        void notify(title, options.body)
      }
      close(): void {}
      addEventListener(): void {}
      removeEventListener(): void {}
    }
    ;(target as unknown as { Notification: unknown }).Notification = IdeNotification
  }
}
