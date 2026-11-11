/**
 * The internals of Tauri a page runs on when no Tauri shell hosts it (docs/design/ide-embedding.md, "Implementation
 * notes"): what `@tauri-apps/api` calls (`window.__TAURI_INTERNALS__`), the plugins of Tauri the page answers itself by
 * the prefix of their command (`plugin:window|set_min_size`), the listeners of the event plugin, and the table of the
 * commands of app-api that says which argument of a command gets its events. Two hosts install it: the page of the IDE
 * (`src/platform/air/`) and the page mock of the E2E tests (`tests/e2e/setup/tauri-mocks.ts`).
 */

/**
 * A command of app-api as a page sends it, an entry of the table of its registry (`crates/app-api/commands.json`,
 * written from `app_api::commands::COMMANDS` by `commands_json_test.rs`): its class, and whether it answers JSON, raw
 * bytes, or tells the page its events on the channel argument `channel` first, which stays open after the answer while
 * `open` (the patches of `subscribe_view`)
 */
export interface PageCommand {
  readonly name: string
  readonly class: "read" | "engine" | "app"
  readonly transport: "json" | "raw" | CommandChannel
}

/** The argument of a command that gets its events, and whether it stays open after the answer */
export interface CommandChannel {
  readonly channel: string
  readonly open: boolean
}

/** The `Channel` of `@tauri-apps/api/core` as a host sees it: the page reads each message in `onmessage` */
export interface PageChannel<T = unknown> {
  onmessage: (message: T) => void
}

/** A plugin of Tauri the page answers itself: the command (`plugin:window|set_min_size`) and its payload */
export type PluginHandler = (cmd: string, payload: Record<string, unknown> | undefined) => unknown

/** What a host gives the internals: the label of the window, the commands of the app and the plugins it answers */
export interface TauriPageHost {
  readonly label: string
  /** A command of the app, anything that is no plugin of `plugins` */
  readonly invoke: (cmd: string, payload: Record<string, unknown> | undefined) => Promise<unknown>
  readonly plugins: Readonly<Record<`plugin:${string}`, PluginHandler>>
}

/** The channel argument of each command that has one, by the name of the command */
export function commandChannels(commands: readonly PageCommand[]): ReadonlyMap<string, CommandChannel> {
  const channels = new Map<string, CommandChannel>()
  for (const command of commands) {
    if (typeof command.transport === "object") {
      channels.set(command.name, command.transport)
    }
  }
  return channels
}

/** The `Channel` a command got in its argument `name`, null without one */
export function channelArgument(payload: Record<string, unknown> | undefined, name: string): PageChannel | null {
  const channel = payload?.[name]
  if (channel === null || typeof channel !== "object" || typeof (channel as Partial<PageChannel>).onmessage !== "function") {
    return null
  }
  return channel as PageChannel
}

/**
 * The rejection of a failed command: Tauri rejects with its `ApiError`, which the bindings return as
 * `{ status: "error", error }`, passed through whole: the code the page branches on, the message, the data of the code,
 * the retry class and the operation that failed. `name` tells the rejection of the host from its own failure; an answer
 * that is no object is an `internal` error with its words
 */
export function commandError(error: unknown): { name: "ApiError" } & Record<string, unknown> {
  if (typeof error === "object" && error !== null) {
    return { ...(error as Record<string, unknown>), name: "ApiError" }
  }
  return { name: "ApiError", code: "internal", message: String(error), details: null, retry: "no", operation_id: null }
}

interface Listener {
  readonly event: string
  /** The window it listens for (`target: { label }`), null for any */
  readonly label: string | null
  readonly handler: (event: { event: string; id: number; payload: unknown }) => void
}

/**
 * The listeners of the event plugin of the page (`listen` of `@tauri-apps/api/event`). An event for a window reaches the
 * listeners of any target and those of that window; one without a window reaches every listener of its name, as the
 * events of the page's own window (`tauri://focus`, `tauri://resize`) do
 */
export class PageEvents {
  private readonly listeners = new Map<number, Listener>()
  private nextId = 1

  /** `plugin:event|listen`: the id that `unlisten` takes */
  listen(payload: Record<string, unknown> | undefined): number {
    const id = this.nextId++
    const handler = payload?.handler
    if (typeof handler === "function") {
      const target = payload?.target as { kind?: string; label?: string } | undefined
      this.listeners.set(id, {
        event: String(payload?.event),
        label: target === undefined || target.kind === "Any" ? null : (target.label ?? null),
        handler: handler as Listener["handler"],
      })
    }
    return id
  }

  unlisten(payload: Record<string, unknown> | undefined): void {
    this.listeners.delete(payload?.eventId as number)
  }

  /** Tells the listeners of `event` for the window `label` (null for every window) */
  dispatch(event: string, payload: unknown, label: string | null = null): void {
    for (const [id, listener] of this.listeners) {
      if (listener.event === event && (label === null || listener.label === null || listener.label === label)) {
        listener.handler({ event, id, payload })
      }
    }
  }

  /** How many listeners `event` has */
  count(event: string): number {
    return [...this.listeners.values()].filter(listener => listener.event === event).length
  }

  /**
   * The plugin `plugin:event`: `listen` and `unlisten` here, `emit` and `emit_to` (an event for the windows of the app)
   * through `emit` of the host, with the label of the window it goes to (null for all)
   */
  plugin(emit: (event: string, payload: unknown, target: string | null) => unknown): PluginHandler {
    return (cmd, payload) => {
      const action = cmd.split("|")[1]
      switch (action) {
        case "listen":
          return this.listen(payload)
        case "unlisten":
          this.unlisten(payload)
          return null
        case "emit":
        case "emit_to": {
          const target = action === "emit_to" ? ((payload?.target as { label?: string } | undefined)?.label ?? null) : null
          return emit(String(payload?.event), payload?.payload ?? null, target)
        }
        default:
          throw new Error(`Unimplemented event action: ${action}`)
      }
    }
  }
}

/**
 * Installs the internals of Tauri on `target` (the window of the page) for `host`: its `invoke` is the host's, a plugin
 * by the prefix of its command and every other command of the app; `transformCallback` hands the callback itself back,
 * so a host calls `onmessage` of a `Channel` it got in an argument. Nothing replaces internals a shell set already
 */
export function installTauriInternals(target: Window, host: TauriPageHost): void {
  const page = target as unknown as Record<string, unknown>
  if (page.__TAURI_INTERNALS__ !== undefined) {
    return
  }
  page.__TAURI_INTERNALS__ = {
    transformCallback: (callback: unknown) => {
      if (typeof callback !== "function") {
        return callback
      }
      // a channel's callback gets `{ message }` from a shell; a host here calls it with the message
      return (payload: unknown) => (payload !== null && typeof payload === "object" && "message" in payload ? callback(payload.message) : callback(payload))
    },
    unregisterCallback: () => {},
    invoke: async (cmd: string, payload: Record<string, unknown> | undefined) => {
      const plugin = cmd.startsWith("plugin:") ? host.plugins[cmd.split("|")[0] as `plugin:${string}`] : undefined
      return plugin === undefined ? host.invoke(cmd, payload) : plugin(cmd, payload)
    },
    metadata: {
      currentWindow: { label: host.label },
      currentWebview: { windowLabel: host.label, label: host.label },
    },
  }
  // what `unlisten` of @tauri-apps/api/event calls before it asks the backend: a component that unmounts stops listening
  page.__TAURI_EVENT_PLUGIN_INTERNALS__ ??= { unregisterListener: () => {} }
}
