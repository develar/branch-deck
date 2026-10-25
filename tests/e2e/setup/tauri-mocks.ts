/* eslint-disable */

/**
 * Tauri mock script to be injected into the browser context
 * This proxies all Tauri API calls to our test server
 */
export const tauriMockScript = () => {
  // Debug logging function - only logs when debug flag is set via window property
  function debug(...args: any[]) {
    // In browser context, check for a window property instead of process.env
    if ((window as any).__DEBUG_E2E__) {
      console.log(...args)
    }
  }

  debug("[Test Mock] Initializing Tauri mocks...")

  // the mock server serves the page, on the port of this run (E2E_PORT)
  const serverUrl = window.location.origin

  // Get test repository ID from URL - required for test isolation
  const urlParams = new URLSearchParams(window.location.search)
  const repoId = urlParams.get("repoId")

  if (!repoId) {
    throw new Error("[Test Mock] Repository ID is required for test isolation. Pass ?repoId=xxx in URL")
  }

  // the label of the window: `main`, or that of a linked sub-window (`openLinkedSubWindow` sets `__e2eWindowLabel`)
  const windowLabel: string = (window as any).__e2eWindowLabel ?? "main"
  // the version of the snapshot of the settings the page starts from, as the backend remembers it for the window it
  // built (`SubWindowState::built`): the load of a sub-window takes the changes after it (`sub_window_loaded`)
  const startStoreVersion: number = (window as any).__TAURI_STORE_VERSION__ ?? 0
  // the main window's open, its build, as its init script names it (`open_script`): its page settles it on its first
  // rows (`markWindowContentReady`); a sub-window's page gets its open from the test (`openLinkedSubWindow`)
  if (windowLabel === "main") {
    ;(window as any).__SUB_WINDOW_OPEN__ ??= { id: 1, openedAt: Date.now() }
  }

  /**
   * A change of the settings this page heard (`settings_changed`), as the writer of the app holds it: a reload starts
   * from it at the version it heard last, as a window of the app starts from the writer's copy (`common-setup.ts` reads
   * both; null is a deleted key),
   * and so does a sub-window this page opens (`__TAURI_STORE__` and `__TAURI_STORE_VERSION__`, the copy of this page)
   */
  const storeVersions = new Map<string, number>()
  function rememberStoreChange(change: { version: number; key: string; value: unknown }) {
    if ((storeVersions.get(change.key) ?? -1) >= change.version) {
      return
    }
    storeVersions.set(change.key, change.version)
    const written = JSON.parse(sessionStorage.getItem("bd-e2e-store-writes") ?? "{}")
    written[change.key] = change.value ?? null
    sessionStorage.setItem("bd-e2e-store-writes", JSON.stringify(written))
    const copy = ((window as any).__TAURI_STORE__ ??= {})
    if (change.value === null) {
      delete copy[change.key]
    } else {
      copy[change.key] = change.value
    }
    const version = Math.max((window as any).__TAURI_STORE_VERSION__ ?? 0, change.version)
    ;(window as any).__TAURI_STORE_VERSION__ = version
    sessionStorage.setItem("bd-e2e-store-version", String(version))
  }

  /**
   * A request to the mock server on behalf of this page's repository: the server holds the repository while the request
   * runs, so the test's cleanup deletes it only after the commands still running for it
   */
  function serverFetch(path: string, init: RequestInit = {}): Promise<Response> {
    return fetch(`${serverUrl}${path}`, { ...init, headers: { ...(init.headers as Record<string, string> | undefined), "X-Repository-Id": repoId! } })
  }

  /**
   * The IPC of the page's backend commands, for the perf tests (`tests/perf/branch-table.perf.ts`): the commands that
   * reached the mock server or a test's route of `/invoke/<cmd>` (`routeCommand` answers count too), counted at the
   * fetch; the plugin commands the page answers itself (store, event, window, log) are not. `requestBytes` and
   * `responseBytes` are the UTF-8 bytes of the parameters and of the answers, `channelMessages` and `channelBytes` the
   * messages of a command's channel (the SSE events of `get_file_diffs`, `get_file_merges`, the patches of
   * `subscribe_view`, counted under its name for as long as its channel stays open); `pending`
   * the commands whose answer has not been read yet. `reset()` zeroes the counts, not `pending`
   */
  const encoder = new TextEncoder()
  const decoder = new TextDecoder()
  const ipcStats = {
    invokes: {} as Record<string, number>,
    requestBytes: 0,
    responseBytes: 0,
    channelMessages: {} as Record<string, number>,
    channelBytes: 0,
    pending: 0,
    reset() {
      ipcStats.invokes = {}
      ipcStats.requestBytes = 0
      ipcStats.responseBytes = 0
      ipcStats.channelMessages = {}
      ipcStats.channelBytes = 0
    },
  }
  ;(window as any).__ipcStats = ipcStats

  /** A backend command to the mock server, counted in `__ipcStats`: pending until `read` has its answer */
  async function invokeServer<T>(cmd: string, path: string, init: RequestInit, read: (response: Response) => Promise<T>): Promise<T> {
    ipcStats.invokes[cmd] = (ipcStats.invokes[cmd] ?? 0) + 1
    ipcStats.requestBytes += typeof init.body === "string" ? encoder.encode(init.body).byteLength : 0
    ipcStats.pending++
    try {
      return await read(await serverFetch(path, init))
    } finally {
      ipcStats.pending--
    }
  }

  /** The body of a command's answer, its bytes counted in `__ipcStats` */
  async function readAnswer(response: Response): Promise<ArrayBuffer> {
    const body = await response.arrayBuffer()
    ipcStats.responseBytes += body.byteLength
    return body
  }

  /** A command's answer as JSON, its bytes counted in `__ipcStats` */
  async function readJsonAnswer(response: Response): Promise<any> {
    return JSON.parse(decoder.decode(await readAnswer(response)))
  }

  /** A message of a command's channel, counted in `__ipcStats` */
  function countChannelMessage(cmd: string, data: string) {
    ipcStats.channelMessages[cmd] = (ipcStats.channelMessages[cmd] ?? 0) + 1
    ipcStats.channelBytes += encoder.encode(data).byteLength
  }

  // === Helper Functions ===

  /**
   * Parse SSE (Server-Sent Events) stream
   */
  async function parseSSEStream(cmd: string, reader: ReadableStreamDefaultReader<Uint8Array>, onMessage: (event: any) => void) {
    const decoder = new TextDecoder()
    let buffer = ""

    while (true) {
      const { done, value } = await reader.read()

      if (done) {
        debug("[SSE] Stream ended")
        break
      }

      buffer += decoder.decode(value, { stream: true })

      // Parse SSE events from buffer
      const lines = buffer.split("\n")
      buffer = lines.pop() || "" // Keep incomplete line in buffer

      for (const line of lines) {
        debug("[SSE] Raw line:", JSON.stringify(line))
        if (line.startsWith("data: ")) {
          const data = line.slice(6).trim()
          if (data) {
            countChannelMessage(cmd, data)
            const event = JSON.parse(data)
            debug("[SSE] Received event:", JSON.stringify(event))
            onMessage(event)
          }
        }
      }
    }
  }

  // the server keeps the waits of every page by these keys (`cancel_lock_wait`): the pages of tests that run at once must
  // not share one, so each page counts from a random start
  let nextProgressChannel = Math.floor(Math.random() * 2 ** 29)

  /**
   * Keeps a `Channel` the command got (the `progress` of a push or an update, the `wait` of a rewrite), so the server's
   * messages and a test reach its `onmessage`; null without one
   */
  function registerProgressChannel(channel: any): number | null {
    if (channel === null || typeof channel !== "object" || typeof channel.onmessage !== "function") {
      return null
    }
    const channels = ((window as any).__progressChannels ??= {})
    const id = nextProgressChannel++
    channels[id] = channel
    return id
  }

  /** The key of the kept channel whose Tauri id is `tauriId` (`Channel.id`, which Stop Waiting names); null when none */
  function progressChannelKey(tauriId: unknown): number | null {
    const channels: Record<string, any> = (window as any).__progressChannels ?? {}
    const entry = Object.entries(channels).find(([, channel]) => channel.id === tauriId)
    return entry === undefined ? null : Number(entry[0])
  }

  /**
   * Proxy command to test server
   */
  async function proxyToTestServer(cmd: string, payload: any): Promise<any> {
    debug(`[Test Mock] Proxying command to test server: ${cmd}`)

    // Unwrap params if present - the frontend wraps parameters in { params: actualParams }
    // but the test server expects the parameters directly
    const actualPayload = payload?.params || payload
    // a progress channel (`push_branch`, `update_project`, the `wait` of a rewrite, the `patches` of `subscribe_view`)
    // stays in the page: the server sends its messages as SSE events before the answer (`channel_commands.rs`), a test
    // route by its key (`window.__progressChannels`, see `routePush`)
    const channel = payload?.progress ?? payload?.wait ?? payload?.patches
    const channelId = registerProgressChannel(channel)

    const init = {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(channelId === null ? {} : { "X-Progress-Channel": String(channelId), Accept: "text/event-stream, application/json" }),
      },
      body: JSON.stringify(actualPayload),
    }
    const { contentType, body } = await invokeServer(cmd, `/invoke/${cmd}`, init, async response => {
      await checkServerResponse(cmd, response)
      if (response.headers.get("content-type")?.includes("text/event-stream")) {
        return { contentType: "application/json", body: await readChannelAnswer(cmd, response, channel, OPEN_CHANNELS.has(cmd)) }
      }
      return { contentType: response.headers.get("content-type"), body: await readAnswer(response) }
    })

    // raw bytes, as the app answers `get_blob_syntax` (`tauri::ipc::Response`): an ArrayBuffer
    if (contentType?.includes("application/octet-stream")) {
      return body
    }
    const text = decoder.decode(body)
    const data = contentType?.includes("application/json") ? JSON.parse(text) : text

    // a Result of the server ({ status: "ok", data } or { status: "error", error }); a value with a status of its own
    // (a Patronus robot) is not one
    const isResult = data && typeof data === "object" && ((data.status === "ok" && "data" in data) || (data.status === "error" && "error" in data))
    if (!isResult) {
      return data
    }
    if (data.status === "error") {
      throw commandError(data.error)
    }
    return data.data
  }

  /**
   * The rejection of a failed command: Tauri rejects with its `ApiError`, which the bindings return as
   * `{ status: "error", error }` (an Error instance they would throw instead), passed through whole: the code the page
   * branches on, the message, the data of the code, the retry class and the operation that failed (null unless the
   * server names one). `name` tells the mock's own rejection from a failure of the mock; an answer that is no object is
   * an `internal` error with its words.
   */
  function commandError(error: any): { name: "ApiError" } & Record<string, unknown> {
    if (typeof error === "object" && error !== null) {
      return { ...error, name: "ApiError" }
    }
    return { name: "ApiError", code: "internal", message: String(error), details: null, retry: "no", operation_id: null }
  }

  /**
   * A response that is no answer of the command fails as a bug of the mock: no route (405) is a command the mock server
   * does not implement, a 500 a command that panicked, a 410 a page that outlived its test repository
   */
  async function checkServerResponse(cmd: string, response: Response) {
    if (response.status === 405) {
      throw new Error(`Unimplemented Tauri command: ${cmd}`)
    }
    if (!response.ok) {
      throw new Error(`The mock server failed ${cmd}: ${response.status} ${(await response.text()) || response.statusText}`)
    }
  }

  /**
   * The commands whose channel stays open after their answer: the patches of a session come on the `patches` of
   * `subscribe_view` until `unsubscribe_view` or the deletion of the test repository (`session_routes.rs`)
   */
  const OPEN_CHANNELS = new Set(["subscribe_view"])

  /**
   * The answer of a command with a channel, as SSE events (`channel_commands.rs`): each `{ progress }` goes to the
   * channel's `onmessage`, `{ result }` is its answer, as the bytes of the JSON answer. The answer of a command whose
   * channel stays open (`keepsOpen`) comes with its `{ result }`, and the stream is read on for the channel; that of
   * another command once its stream ended
   */
  async function readChannelAnswer(cmd: string, response: Response, channel: any, keepsOpen: boolean): Promise<ArrayBuffer> {
    const reader = response.body?.getReader()
    if (!reader) {
      throw new Error("No response body")
    }
    let result: unknown = undefined
    let answered: (result: unknown) => void = () => {}
    const answer = new Promise<unknown>(resolve => (answered = resolve))
    const read = parseSSEStream(cmd, reader, event => {
      if ("progress" in event) {
        channel.onmessage(event.progress)
      } else if ("result" in event) {
        result = event.result
        answered(result)
      } else if (event.panicked === true) {
        throw new Error(`The mock server failed ${cmd}: the command panicked`)
      }
    })
    if (keepsOpen) {
      // a page that goes away ends the read with its fetch
      read.catch(error => debug(`[SSE] The channel of ${cmd} ended:`, error))
      result = await Promise.race([answer, read.then(() => result)])
    } else {
      await read
    }
    if (result === undefined) {
      throw new Error(`The mock server answered ${cmd} without a result`)
    }
    return encoder.encode(JSON.stringify(result)).buffer as ArrayBuffer
  }

  // === Command Handlers ===

  /**
   * Generic handler for streaming commands
   */
  async function handleStreamingCommand(cmd: string, payload: any): Promise<any> {
    const progress = payload.progress

    debug(`[Test Mock] ${cmd} called with params:`, payload.params)
    debug(`[Test Mock] progress.onmessage type:`, typeof progress.onmessage)

    try {
      // Extract params, excluding the progress channel which can't be serialized
      const requestPayload = payload.params || {} // Empty object if no params

      debug(`[Test Mock] Fetching SSE stream of ${cmd}`)

      // Call the test server's endpoint with SSE; the command is pending until its stream ends
      const init = { method: "POST", headers: { "Content-Type": "application/json", Accept: "text/event-stream" }, body: JSON.stringify(requestPayload) }
      return await invokeServer(cmd, `/invoke/${cmd}`, init, async response => {
        debug(`[Test Mock] SSE response status: ${response.status}`)

        await checkServerResponse(cmd, response)
        // a failed command answers with its error ({ status: "error", error }) instead of the event stream
        if (response.headers.get("content-type")?.includes("application/json")) {
          throw commandError((await readJsonAnswer(response)).error)
        }

        // Read the SSE stream
        const reader = response.body?.getReader()
        if (!reader) {
          throw new Error("No response body")
        }

        await parseSSEStream(cmd, reader, event => {
          if (progress.onmessage) {
            progress.onmessage(event)
          }
        })
        return null
      })
    } catch (error: any) {
      // the error of the command is its answer; anything else is a failure of the mock
      if (error?.name !== "ApiError") {
        console.error(`[Test Mock] Error in ${cmd}:`, error)
      }
      throw error
    }
  }

  /**
   * Handle event plugin commands
   */
  // listeners registered with `listen`; a test fires them with `window.__e2eEmitTauriEvent` (`emitTauriEvent` in
  // sync-helpers). An event the test names a window for (`tauri://destroyed` of a sub-window) reaches the listeners of
  // any target and those of that window (`target: { label }`); one without a window reaches every listener of its name,
  // as the events of the page's own window (`tauri://focus`, `tauri://resize`) do.
  const eventListeners = new Map<number, { event: string; label: string | null; handler: (event: any) => void }>()
  let nextEventId = 1
  ;(window as any).__e2eEmitTauriEvent = (event: string, payload: unknown, label: string | null = null) => {
    if (event === "settings_changed") {
      rememberStoreChange(payload as { version: number; key: string; value: unknown })
    }
    for (const [id, listener] of eventListeners) {
      if (listener.event === event && (label === null || listener.label === null || listener.label === label)) {
        listener.handler({ event, id, payload })
      }
    }
  }

  // what `unlisten` of @tauri-apps/api/event calls before it asks the backend: a component that unmounts (a page of
  // Settings left) stops listening
  ;(window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ ??= { unregisterListener: () => {} }

  // how many listeners an event has: a test waits for the page to listen before it fires the event (`waitForTauriListener`)
  ;(window as any).__e2eTauriListenerCount = (event: string) => [...eventListeners.values()].filter(listener => listener.event === event).length

  // the window of the page, for a toolbar that draws the title bar (`__BD_TITLE_BAR__`): a test sets
  // `__e2eWindowState` and fires `tauri://resize`; the actions are recorded in `__e2eWindowCalls`, the minimum size
  // a zoom sets (`useZoom`) in `__e2eWindowState.minSize`, and what the Dock shows (`useDockStatus`) in
  // `__e2eDockCalls`, each progress bar and badge as it was set
  const windowState = ((window as any).__e2eWindowState ??= { fullscreen: false, maximized: false })
  function handleWindowCommand(cmd: string, payload: any): any {
    const [, action] = cmd.split("|")
    switch (action) {
      case "set_min_size":
        windowState.minSize = payload?.value?.size ?? payload?.value
        break
      case "set_progress_bar":
        ;((window as any).__e2eDockCalls ??= []).push({ progressBar: payload?.value ?? null })
        break
      case "set_badge_count":
        // `undefined` removes the badge
        ;((window as any).__e2eDockCalls ??= []).push({ badgeCount: payload?.value ?? null })
        break
      case "is_fullscreen":
        return windowState.fullscreen
      case "is_maximized":
        return windowState.maximized
      case "toggle_maximize":
        windowState.maximized = !windowState.maximized
        ;(window as any).__e2eEmitTauriEvent("tauri://resize", { width: window.innerWidth, height: window.innerHeight })
        break
    }
    ;((window as any).__e2eWindowCalls ??= []).push(action)
    return null
  }

  function handleEventCommand(cmd: string, payload: any): any {
    const [, action] = cmd.split("|")

    switch (action) {
      case "listen": {
        const id = nextEventId++
        if (typeof payload?.handler === "function") {
          const target = payload.target
          eventListeners.set(id, { event: payload.event, label: target === undefined || target.kind === "Any" ? null : target.label, handler: payload.handler })
        }
        return id
      }

      case "unlisten":
        eventListeners.delete(payload?.eventId)
        return null

      // an event for the other windows (a sub-window asks the main window, the main window tells a sub-window): recorded
      // in `__e2eEmittedEvents`, and delivered to the other pages of a linked sub-window (`openLinkedSubWindow` in
      // sub-window-helpers exposes `__e2eRelay`); a page alone is one window
      case "emit":
      case "emit_to":
        ;((window as any).__e2eEmittedEvents ??= []).push({ event: payload?.event, payload: payload?.payload })
        void (window as any).__e2eRelay?.(payload?.event, payload?.payload)
        return null

      default:
        throw new Error(`Unimplemented event action: ${action}`)
    }
  }

  /**
   * Handle path plugin commands
   */
  function handlePathCommand(cmd: string): any {
    const [, action] = cmd.split("|")

    switch (action) {
      case "resolve_directory":
        // Return a fake home directory for tests
        return "/home/testuser"

      default:
        throw new Error(`Unimplemented path action: ${action}`)
    }
  }

  /**
   * Helper to proxy simple commands to test server with repoId in URL
   */
  async function proxyCommandWithRepoId(command: string): Promise<any> {
    // Empty body since repo_id is in URL
    const init = { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({}) }
    return await invokeServer(command, `/invoke/${command}/${repoId}`, init, async response => {
      if (!response.ok) {
        throw new Error(`${command} failed: ${response.status} ${response.statusText}`)
      }
      return await readJsonAnswer(response)
    })
  }

  // Returned by handleAppCommand for commands it does not handle (handled commands may legitimately return null)
  const NOT_HANDLED = Symbol("not handled")

  /**
   * Handle app-specific commands
   */
  async function handleAppCommand(cmd: string, payload: any): Promise<any> {
    switch (cmd) {
      case "validate_repository_path":
        // Always return empty string for valid path in tests
        return ""

      case "browse_repository":
        // Special-case: when current path is NO_REPO_*, simulate a valid selection by
        // creating a real repo and returning its path. Otherwise, proxy as before.
        try {
          const repoInfoRes = await serverFetch(`/repositories/${repoId}`)
          if (repoInfoRes.ok) {
            const repoInfo = await repoInfoRes.json()
            const currentPath: string | undefined = repoInfo?.path
            if (currentPath && currentPath.startsWith("NO_REPO_")) {
              const createRes = await serverFetch(`/repositories`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ template: "simple_no_prefix", prepopulate_store: false }),
              })
              if (createRes.ok) {
                const created = await createRes.json()
                return { path: created.path, valid: true, error: null }
              }
            }
          }
        } catch (_) {}
        return await proxyCommandWithRepoId("browse_repository")

      default:
        return NOT_HANDLED
    }
  }

  // === Command Registry ===

  const COMMAND_HANDLERS: Record<string, (cmd: string, payload: any) => any> = {
    // Plugin commands
    "plugin:event": handleEventCommand,
    "plugin:window": handleWindowCommand,
    // the page zoom (`useZoom`), recorded in `__e2eWebviewZoom`; a test zooms the page as the webview does with a
    // viewport of the window size divided by the level (`zoomPage` in header-helpers)
    "plugin:webview": (cmd: string, payload: any) => {
      if (cmd === "plugin:webview|set_webview_zoom") {
        ;(window as any).__e2eWebviewZoom = payload?.value
      }
      return null
    },
    "plugin:path": handlePathCommand,
    "plugin:log": () => null, // Ignore log commands
    // the app's name (`getName` of `@tauri-apps/api/app`): the `productName` of tauri.conf.json
    "plugin:app": (cmd: string) => {
      if (cmd !== "plugin:app|name") {
        throw new Error(`[Test Mock] Unmocked app command: ${cmd}`)
      }
      return "BranchDeck"
    },
    // the notification plugin goes through the browser's `Notification`; its permission check never notifies
    "plugin:notification": () => false,
    // a sub-window is a page of its own in the app: record what opens it (`window.__e2eSubWindows`, with the parsed data
    // and the settings it starts from); a test opens the page itself with them (`openSubWindowPage`, `openLinkedSubWindow`
    // in sub-window-helpers)
    open_sub_window: (_cmd: string, payload: any) => {
      const params = payload?.params
      ;((window as any).__e2eSubWindows ??= []).push({
        windowId: params.windowId,
        url: params.url,
        title: params.title,
        data: JSON.parse(params.data),
        // the backend builds a new window from its writer's copy of the settings, which this page's copy follows
        store: structuredClone((window as any).__TAURI_STORE__ ?? {}),
        storeVersion: (window as any).__TAURI_STORE_VERSION__ ?? 0,
      })
      return null
    },
    // the page of a window loaded (`announceWindowLoaded`): no data of a later open, and the changes of the settings
    // after the snapshot the page started from, which the test server's writer has (none for the main window)
    sub_window_loaded: async (cmd: string) => proxyToTestServer(cmd, { since: startStoreVersion, origin: windowLabel }),
    // the page committed its shell (`markWindowShellDrawn`): the backend would count the content budget from now; with
    // the title rows, the loading tables and the branch rows the page had then
    sub_window_drawn: async (cmd: string, payload: any) => {
      ;(window as any).__e2eWindowShell = {
        titleRows: document.querySelectorAll("header[data-title-bar]").length,
        loadingTables: document.querySelectorAll("[data-testid=branch-table-loading]").length,
        branchRows: document.querySelectorAll("[data-testid=branch-row]").length,
      }
      return proxyToTestServer(cmd, payload)
    },
    // the page settled the content of an open (`markWindowContentReady`): the backend would show the window; with the
    // branch rows the page had then, which the main window waits for
    sub_window_ready: (_cmd: string, payload: any) => {
      ;(window as any).__e2eSubWindowReady = payload?.openId ?? null
      ;(window as any).__e2eWindowReadyRows = document.querySelectorAll("[data-testid=branch-row]").length
      return null
    },
    // the window manager's window menu of a right click on the title bar: recorded with the window calls
    show_window_menu: () => {
      ;((window as any).__e2eWindowCalls ??= []).push("show_window_menu")
      return null
    },
    // links open in the browser: record them for the test (`window.__OPENED_URLS__`)
    "plugin:opener": (_cmd: string, payload: any) => {
      ;((window as any).__OPENED_URLS__ ??= []).push(payload?.url)
      return null
    },
    // the app writes text built after the click through the clipboard plugin (`copyTextLater`): into the page's
    // clipboard, so `readClipboard` reads it as any other copy
    "plugin:clipboard-manager": async (_cmd: string, payload: any) => {
      await navigator.clipboard.writeText(payload?.text ?? "")
      return null
    },

    // Stop Waiting names the wait by the Tauri id of its command's channel (`Channel.id`): the server knows the channel by
    // the key the mock gave it
    cancel_lock_wait: async (cmd: string, payload: any) => proxyToTestServer(cmd, { channel: progressChannelKey(payload?.channel) ?? 4294967295 }),

    // the diffs of the files stream through the channel `events` (`FileDiffEvent`); the server answers them as SSE
    get_file_diffs: async (cmd: string, payload: any) => handleStreamingCommand(cmd, { ...payload, progress: payload.events }),
    get_file_merges: async (cmd: string, payload: any) => handleStreamingCommand(cmd, { ...payload, progress: payload.events }),

    // a change of the settings: the test server's writer applies it, and the backend sends the change to every window
    // (`settings_changed`), this page and the other pages of a linked sub-window; the origin is this page's window. The
    // patches are recorded in `__e2eSettingsPatches`, the changes the backend sent in `__e2eEmittedEvents` of this page
    // (`getEmittedEvents`), with their versions
    set_setting: async (cmd: string, payload: any) => {
      ;((window as any).__e2eSettingsPatches ??= []).push(payload?.params?.patch)
      const change = await proxyToTestServer(cmd, { params: { patch: payload?.params?.patch, origin: windowLabel } })
      ;((window as any).__e2eEmittedEvents ??= []).push({ event: "settings_changed", payload: change })
      ;(window as any).__e2eEmitTauriEvent("settings_changed", change)
      void (window as any).__e2eRelay?.("settings_changed", change)
      return change
    },
    // the changes after the snapshot the page started from, which its window gets
    get_settings_changes: async (cmd: string, payload: any) => proxyToTestServer(cmd, { since: payload?.since ?? 0, origin: windowLabel }),

    // a write of Settings → Repository: the backend tells the main window what changed (`repository_config_changed`,
    // `set_repository_config` of src-tauri), here or in the main window's page of a linked sub-window
    set_repository_config: async (cmd: string, payload: any) => {
      const config = await proxyToTestServer(cmd, payload)
      const changed = { repositoryPath: payload?.params?.repositoryPath, key: payload?.params?.change?.key }
      if (windowLabel === "main") {
        ;(window as any).__e2eEmitTauriEvent("repository_config_changed", changed)
      } else {
        void (window as any).__e2eRelay?.("repository_config_changed", changed)
      }
      return config
    },

    // the app's GitHub sign-in: the backend tells every window that it changed (`github-auth-changed`, the
    // `GITHUB_AUTH_CHANGED_EVENT` of src-tauri), this one and the other pages of a linked sub-window
    wait_github_sign_in: async (cmd: string, payload: any) => emitAfter(cmd, payload, "github-auth-changed"),
    sign_out_of_github: async (cmd: string, payload: any) => emitAfter(cmd, payload, "github-auth-changed"),
    // the app's YouTrack sign-in, likewise (`youtrack-auth-changed`, the `YOUTRACK_AUTH_CHANGED_EVENT` of src-tauri)
    wait_youtrack_sign_in: async (cmd: string, payload: any) => emitAfter(cmd, payload, "youtrack-auth-changed"),
    save_youtrack_token: async (cmd: string, payload: any) => emitAfter(cmd, payload, "youtrack-auth-changed"),
    sign_out_of_youtrack: async (cmd: string, payload: any) => emitAfter(cmd, payload, "youtrack-auth-changed"),
  }

  /**
   * Runs `cmd` on the test server, then emits `event` to every window as the backend does after it succeeded: to this
   * page and, through `__e2eRelay`, to the other pages of a linked sub-window
   */
  async function emitAfter(cmd: string, payload: any, event: string): Promise<any> {
    const answer = await proxyToTestServer(cmd, payload)
    ;(window as any).__e2eEmitTauriEvent(event, null)
    void (window as any).__e2eRelay?.(event, null)
    return answer
  }

  // All unhandled commands are proxied to test server for single source of truth

  /**
   * Main command handler
   */
  const handleMockCommand = async (cmd: string, payload: any): Promise<any> => {
    debug(`[Test Mock] Handling command: ${cmd}`, payload)

    // Check app-specific commands first
    const appResult = await handleAppCommand(cmd, payload)
    if (appResult !== NOT_HANDLED) {
      return appResult
    }

    // Check command registry
    const prefix = cmd.split("|")[0]
    const handler = (prefix && COMMAND_HANDLERS[prefix]) || COMMAND_HANDLERS[cmd]

    if (handler) {
      return handler(cmd, payload)
    }

    // Proxy all unhandled commands to test server (single source of truth)
    debug(`[Test Mock] Command not handled locally: ${cmd}. Proxying to test server.`)

    try {
      return await proxyToTestServer(cmd, payload)
    } catch (error: any) {
      // the command ran and failed: the app gets its ApiError; anything else is a bug of the mock
      if (error?.name !== "ApiError") {
        console.error(`[Test Mock] Cannot run ${cmd}:`, error)
      }
      throw error
    }
  }

  // === Initialize Tauri Mocks ===

  // The internals that @tauri-apps/api calls: its invoke is the mock's, nothing replaces it. No poll waits for a
  // `mockIPC` of them: a 10 ms interval for the first 5 s of every window ran on the Playwright clock, one real timer
  // per tick, and under load starved the timers a window opened right then waits for (the focus of a dialog, the end
  // of its exit)
  if (!(window as any).__TAURI_INTERNALS__) {
    debug("[Test Mock] Creating Tauri internals structure")
    ;(window as any).__TAURI_INTERNALS__ = {
      // transformCallback is what's missing in the error
      transformCallback: (callback: any, once: boolean) => {
        debug("[Test Mock] transformCallback called", { callback, once })
        // If this is a channel's onmessage handler, we need to wrap it
        if (callback && typeof callback === "function") {
          return (payload: any) => {
            // For channel callbacks, we get the raw message
            if (payload && typeof payload === "object" && "message" in payload) {
              return callback(payload.message)
            }
            return callback(payload)
          }
        }
        return callback
      },
      invoke: async (cmd: string, args: any) => {
        debug("[Test Mock] Direct invoke called", { cmd, args })
        return handleMockCommand(cmd, args)
      },
      metadata: {
        // `main`, or the label of a linked sub-window (`__e2eWindowLabel`, `openLinkedSubWindow`)
        currentWindow: {
          label: windowLabel,
        },
        currentWebview: {
          windowLabel,
          label: windowLabel,
        },
      },
    }

    // Also override the window.invoke if it exists
    if ((window as any).invoke) {
      ;(window as any).invoke = async (cmd: string, args: any) => {
        debug("[Test Mock] Window invoke intercepted", { cmd, args })
        return handleMockCommand(cmd, args)
      }
    }
  }

  // Mock window API for pages that use getCurrentWindow
  ;(window as any).__TAURI__ = (window as any).__TAURI__ || {}
  ;(window as any).__TAURI__.window = {
    getCurrentWindow: () => {
      debug("[Test Mock] getCurrentWindow called")
      return {
        label: windowLabel,
        close: async () => {
          debug("[Test Mock] window.close called (no-op in tests)")
        },
        onFocusChanged: (callback: (focused: boolean) => void) => {
          debug("[Test Mock] window.onFocusChanged registered (no-op in tests)")
          // Return an unlisten function for cleanup
          return Promise.resolve(() => {
            debug("[Test Mock] window.onFocusChanged unlisten called")
          })
        },
      }
    },
  }

  // Setup HTML formatter for snapshot testing
  ;(window as any).__htmlFormatter = {
    normalizeAndFormat: (element: Element) => {
      // Efficient formatter that normalizes and formats without cloning
      return formatElementWithNormalization(element)
    },
  }

  // Name of a lucide-react icon. lucide-react renders `<svg class="lucide lucide-NAME [lucide-ALIAS...] ...">`,
  // the canonical name is the first `lucide-*` class token (aliases of deprecated names follow it)
  function iconName(element: Element): string {
    for (const token of element.classList) {
      if (token.startsWith("lucide-")) {
        return token.slice("lucide-".length)
      }
    }
    return "unknown"
  }

  // Icons are printed as one stable line: `<svg class="lucide lucide-NAME <own classes>" ... />`.
  // Paths, alias classes and SVG presentation attributes are dropped, own classes (size, colour, animation)
  // and data/aria attributes are kept
  const ICON_SVG_ATTRIBUTES = new Set(["fill", "height", "stroke", "stroke-linecap", "stroke-linejoin", "stroke-width", "viewBox", "width", "xmlns", "xmlns:xlink"])
  function formatIcon(element: Element, spaces: string): string {
    const name = iconName(element)
    const attrs: Array<[string, string]> = []
    for (const attr of element.attributes) {
      if (ICON_SVG_ATTRIBUTES.has(attr.name)) {
        continue
      }
      let value = attr.value
      if (attr.name === "class") {
        const ownClasses = attr.value.split(/\s+/).filter(token => token && token !== "lucide" && !token.startsWith("lucide-"))
        value = ["lucide", `lucide-${name}`, ...ownClasses].join(" ")
      } else if (attr.name !== "style") {
        value = applyNormalizationRules(value, NORMALIZATION_RULES.ids)
      }
      attrs.push([attr.name, value])
    }
    attrs.sort((a, b) => a[0].localeCompare(b[0]))
    return `${spaces}<svg${attrs.map(([n, v]) => ` ${n}="${v}"`).join("")} />`
  }

  // Normalization configuration for consistent snapshot formatting
  const NORMALIZATION_RULES = {
    // Generated ids, applied to every attribute value except `class` and `style`
    // (`id`, `for`, `aria-labelledby`, `aria-describedby`, `aria-controls`, hidden input `id`/`name`, ...)
    ids: [
      // React `useId`: `_r_1f_` (React 19.2+), `«r1»` (React 19.0-19.1), `:r1:` (React 18).
      // Base UI prefixes them: `base-ui-_r_2_`, `base-ui-_r_2_-hidden-input`
      { pattern: /_[rR]_[0-9a-z]+_/g, replacement: "[ID]" },
      { pattern: /«[rR][0-9a-z]+»/g, replacement: "[ID]" },
      { pattern: /:[rR][0-9a-z]+:/g, replacement: "[ID]" },
      // Base UI fallback counter ids
      { pattern: /\bbase-ui-\d+\b/g, replacement: "base-ui-[ID]" },
    ],

    // Per-run values: applied to text nodes and to attribute values
    // (e.g. the hidden input of the repository Select holds the repository path in `value`)
    paths: [
      // The mock server names a repository `repo-<22 digits>` in the order the tests create them; may follow `NO_REPO_`
      { pattern: /(?<![A-Za-z0-9-])repo-\d{22}(?![A-Za-z0-9-])/g, replacement: "[REPO_ID]" },
      { pattern: /\/var\/folders\/\S+\/\.tmp\S+\/\S+/g, replacement: "[TEMP_PATH]" },
    ],

    // Inline styles - filters for consistency
    style: {
      excludeRules: ["pointer-events", "animation-duration", "animation-name"],
    },
  }

  // Helper function to apply normalization rules
  function applyNormalizationRules(value: string, rules: Array<{ pattern: RegExp; replacement: string }>): string {
    return rules.reduce((normalizedValue, rule) => {
      return normalizedValue.replace(rule.pattern, rule.replacement)
    }, value)
  }

  // Helper function to normalize style attributes
  function normalizeStyleAttribute(value: string): string | null {
    const filteredStyle = value
      .split(";")
      .filter(rule => {
        const trimmed = rule.trim()
        return trimmed && !NORMALIZATION_RULES.style.excludeRules.some(excludeRule => trimmed.includes(excludeRule))
      })
      .join("; ")
      .trim()

    return filteredStyle || null // Return null for empty styles
  }

  function isLucideIcon(element: Element): boolean {
    return element.tagName.toLowerCase() === "svg" && element.classList.contains("lucide")
  }

  // Format and normalize HTML element in a single pass
  function formatElementWithNormalization(element: Element, indent = 0): string {
    const spaces = "  ".repeat(indent)
    const tagName = element.tagName.toLowerCase()
    if (isLucideIcon(element)) {
      return formatIcon(element, spaces)
    }
    let result = `${spaces}<${tagName}`

    // Collect and normalize attributes in one pass
    const normalizedAttrs: Array<[string, string]> = []

    for (const attr of element.attributes) {
      let value = attr.value
      const name = attr.name
      // the number of finished syncs depends on timing, not on what the UI shows (`waitForSyncComplete` reads it), and
      // the version of the session's view on a random epoch
      if (name === "data-sync-count" || name === "data-view-version") continue
      // the offsets and the heights of a diff view come from the metrics of the font
      if (name === "style" && element.closest(".dv-document") !== null) continue

      // Apply normalization rules based on attribute type
      if (name === "style") {
        const normalizedStyle = normalizeStyleAttribute(value)
        if (!normalizedStyle) continue // Skip empty styles
        value = normalizedStyle
      } else if (name !== "class") {
        value = applyNormalizationRules(value, NORMALIZATION_RULES.ids)
        value = applyNormalizationRules(value, NORMALIZATION_RULES.paths)
      }

      normalizedAttrs.push([name, value])
    }

    // Sort attributes for consistent output
    normalizedAttrs.sort((a, b) => a[0].localeCompare(b[0]))

    // Add sorted attributes to tag
    for (const [name, value] of normalizedAttrs) {
      result += ` ${name}="${value}"`
    }

    // the rows and the scroll bars of a diff view depend on what is in view, the width and when its tokens arrive: a
    // test reads their text (`toContainText`), a snapshot keeps their element only
    if (element.classList.contains("dv-rows") || element.classList.contains("dv-hscrolls")) {
      return `${result} />`
    }

    // Elements without text are printed self-closed, except that the icons inside them are kept
    // (icon-only buttons and triggers would otherwise lose their identity)
    if (!element.textContent?.trim()) {
      const iconChildren = Array.from(element.children).filter(child => isLucideIcon(child) || child.querySelector("svg.lucide") !== null)
      if (iconChildren.length === 0) {
        result += " />"
        return result
      }
      result += ">\n"
      for (const child of iconChildren) {
        result += formatElementWithNormalization(child, indent + 1) + "\n"
      }
      result += `${spaces}</${tagName}>`
      return result
    }

    result += ">\n"

    // Text content normalization patterns
    const TEXT_NORMALIZATION_RULES = [
      ...NORMALIZATION_RULES.paths,
      { pattern: /\b[a-f0-9]{8}\b/g, replacement: "[SHA]" }, // 8 character short SHAs
    ]

    // Helper function to normalize text content
    function normalizeTextContent(text: string): string {
      return TEXT_NORMALIZATION_RULES.reduce((normalizedText, rule) => {
        return normalizedText.replace(rule.pattern, rule.replacement)
      }, text)
    }

    // Adjacent text nodes are printed as one line: React renders `{count} files` as several text nodes
    let pendingText = ""
    function flushText() {
      const text = pendingText.trim()
      pendingText = ""
      if (text) {
        result += `${"  ".repeat(indent + 1)}${normalizeTextContent(text)}\n`
      }
    }

    for (const child of element.childNodes) {
      if (child.nodeType === Node.TEXT_NODE) {
        pendingText += child.textContent ?? ""
        continue
      }
      flushText()
      if (child.nodeType === Node.COMMENT_NODE) {
        // Skip empty comments (<!---->)
        const commentContent = child.textContent?.trim()
        if (commentContent && commentContent.length > 0) {
          result += `${"  ".repeat(indent + 1)}<!--${child.textContent}-->\n`
        }
      } else if (child.nodeType === Node.ELEMENT_NODE) {
        result += formatElementWithNormalization(child as Element, indent + 1) + "\n"
      }
    }
    flushText()

    result += `${spaces}</${tagName}>`
    return result
  }

  debug("[Test Mock] Tauri mock setup complete")
}
