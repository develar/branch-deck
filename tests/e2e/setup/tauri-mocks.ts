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
  
  // Use a constant store resource ID for consistency across reloads
  const storeResourceId = "test-store-resource"

  // === Helper Functions ===

  /**
   * Parse SSE (Server-Sent Events) stream
   */
  async function parseSSEStream(reader: ReadableStreamDefaultReader<Uint8Array>, onMessage: (event: any) => void) {
    const decoder = new TextDecoder()
    let buffer = ""
    
    while (true) {
      const { done, value } = await reader.read()
      
      if (done) {
        debug('[SSE] Stream ended')
        break
      }
      
      buffer += decoder.decode(value, { stream: true })
      
      // Parse SSE events from buffer
      const lines = buffer.split("\n")
      buffer = lines.pop() || "" // Keep incomplete line in buffer
      
      for (const line of lines) {
        debug('[SSE] Raw line:', JSON.stringify(line))
        if (line.startsWith("data: ")) {
          const data = line.slice(6).trim()
          if (data) {
            const event = JSON.parse(data)
            debug('[SSE] Received event:', JSON.stringify(event))
            onMessage(event)
          }
        }
      }
    }
  }

  let nextProgressChannel = 0

  /** Keeps a `Channel` the command got, so a test can call its `onmessage`; null without one */
  function registerProgressChannel(channel: any): number | null {
    if (channel === null || typeof channel !== "object" || typeof channel.onmessage !== "function") {
      return null
    }
    const channels = ((window as any).__progressChannels ??= {})
    const id = nextProgressChannel++
    channels[id] = channel
    return id
  }

  /**
   * Proxy command to test server
   */
  async function proxyToTestServer(cmd: string, payload: any): Promise<any> {
    debug(`[Test Mock] Proxying command to test server: ${cmd}`)
    
    // Unwrap params if present - the frontend wraps parameters in { params: actualParams }
    // but the test server expects the parameters directly
    const actualPayload = payload?.params || payload
    // a progress channel (`push_branch`) stays in the page: a test route sends messages to it by its key
    // (`window.__progressChannels`, see `routePush`)
    const channelId = registerProgressChannel(payload?.progress)
    
    try {
      const response = await fetch(`${serverUrl}/invoke/${cmd}`, {
        method: "POST",
        headers: { "Content-Type": "application/json", ...(channelId === null ? {} : { "X-Progress-Channel": String(channelId) }) },
        body: JSON.stringify(actualPayload),
      })

      if (!response.ok) {
        // a failed command rejects with its CommandError ({ message }), as Tauri does; the bindings return it as
        // `{ status: "error", error }` (a resolved value would read as success)
        const message = (await response.text()) || response.statusText
        throw { name: "CommandError", message }
      }

      const contentType = response.headers.get("content-type")
      let data
      if (contentType && contentType.includes("application/json")) {
        data = await response.json()
      } else {
        data = await response.text()
      }

      // Check if the test server returned a Result-like object; a value with a status of its own (a Patronus robot)
      // is not one
      const isResult = data && typeof data === "object" && ((data.status === "ok" && "data" in data) || (data.status === "error" && "error" in data))
      if (isResult) {
        if (data.status === "error") {
          // Throw a proper Error object so the bindings layer catches it and wraps it properly
          // Commands return CommandError ({ message }) as the error payload
          const error = new Error(typeof data.error === "object" && data.error !== null ? data.error.message : String(data.error))
          error.name = "TauriError" // Mark it as a Tauri error, not a network error
          throw error
        }
        // For success responses, return just the data (bindings will wrap it)
        return data.data
      }

      // For backwards compatibility, return raw data (bindings will wrap it)
      return data
    } catch (networkError: any) {
      // Re-throw TauriErrors and CommandErrors so they reach the bindings layer
      if (networkError.name === "TauriError" || networkError.name === "CommandError") {
        throw networkError
      }
      // Network errors should also be wrapped in Result type
      return {
        status: "error",
        error: `Network error: ${networkError.message}`,
      }
    }
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
      const requestPayload = payload.params || {}  // Empty object if no params
      
      const url = `${serverUrl}/invoke/${cmd}`
      debug(`[Test Mock] Fetching SSE stream from: ${url}`)
      
      // Call the test server's endpoint with SSE
      const response = await fetch(url, {
        method: "POST",
        headers: { 
          "Content-Type": "application/json",
          "Accept": "text/event-stream"
        },
        body: JSON.stringify(requestPayload),
      })

      debug(`[Test Mock] SSE response status: ${response.status}`)
      
      if (!response.ok) {
        // a failed command rejects with its CommandError ({ message }), as Tauri does
        const message = (await response.text()) || response.statusText
        throw { name: "CommandError", message }
      }

      // Read the SSE stream
      const reader = response.body?.getReader()
      if (!reader) {
        throw new Error("No response body")
      }

      await parseSSEStream(reader, (event) => {
        if (progress.onmessage) {
          progress.onmessage(event)
        }
      })

      return null
    }
    catch (error) {
      console.error(`[Test Mock] Error in ${cmd}:`, error)
      throw error
    }
  }

  /**
   * Handle store plugin commands
   */
  function handleStoreCommand(cmd: string, payload: any): any {
    const [, action] = cmd.split("|")
    
    switch (action) {
      case "load":
        // Return the constant store resource ID
        return storeResourceId

      case "get": {
        const key = payload?.key
        if (!key) {
          console.error("[Test Mock] Store get called without key")
          return [null, false]
        }
        
        return (async () => {
          try {
            const response = await fetch(`${serverUrl}/store/${repoId}/${key}`)
            if (response.ok) {
              const value = await response.json()
              return [value, value !== null]  // null means key not found
            }
            
            // Any error means something is wrong
            const errorMsg = `Store get failed: ${response.status} ${response.statusText} for repo=${repoId}, key=${key}`
            console.error(`[Test Mock] ${errorMsg}`)
            throw new Error(errorMsg)
          } catch (error) {
            console.error(`[Test Mock] Store get error for key ${key}:`, error)
            throw error
          }
        })()
      }

      case "set": {
        const key = payload?.key
        const value = payload?.value
        if (!key) {
          console.error("[Test Mock] Store set called without key")
          return null
        }
        
        
        return (async () => {
          try {
            const response = await fetch(`${serverUrl}/store/${repoId}/${key}`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify(value)
            })
            if (!response.ok) {
              const errorMsg = `Store set failed: ${response.status} ${response.statusText} for repo=${repoId}, key=${key}`
              console.error(`[Test Mock] ${errorMsg}`)
              throw new Error(errorMsg)
            }
            return null
          } catch (error) {
            console.error(`[Test Mock] Store set error for key ${key}:`, error)
            throw error
          }
        })()
      }

      case "delete": {
        const key = payload?.key
        if (!key) {
          console.error("[Test Mock] Store delete called without key")
          return null
        }
        
        return (async () => {
          try {
            const response = await fetch(`${serverUrl}/store/${repoId}/${key}`, {
              method: "DELETE"
            })
            if (!response.ok) {
              const errorMsg = `Store delete failed: ${response.status} ${response.statusText} for repo=${repoId}, key=${key}`
              console.error(`[Test Mock] ${errorMsg}`)
              throw new Error(errorMsg)
            }
            return null
          } catch (error) {
            console.error(`[Test Mock] Store delete error for key ${key}:`, error)
            throw error
          }
        })()
      }

      default:
        throw new Error(`Unimplemented store action: ${action}`)
    }
  }

  /**
   * Handle event plugin commands
   */
  // listeners registered with `listen`; a test fires them with `window.__e2eEmitTauriEvent` (`emitTauriEvent` in sync-helpers)
  const eventListeners = new Map<number, { event: string, handler: (event: any) => void }>()
  let nextEventId = 1
  ;(window as any).__e2eEmitTauriEvent = (event: string, payload: unknown) => {
    for (const [id, listener] of eventListeners) {
      if (listener.event === event) {
        listener.handler({ event, id, payload })
      }
    }
  }

  // the window of the page, for a toolbar that draws the title bar (`__BD_TITLE_BAR__`): a test sets
  // `__e2eWindowState` and fires `tauri://resize`; the actions are recorded in `__e2eWindowCalls`, and the minimum size
  // a zoom sets (`useZoom`) in `__e2eWindowState.minSize`
  const windowState = ((window as any).__e2eWindowState ??= { fullscreen: false, maximized: false })
  function handleWindowCommand(cmd: string, payload: any): any {
    const [, action] = cmd.split("|")
    switch (action) {
      case "set_min_size":
        windowState.minSize = payload?.value?.size ?? payload?.value
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
          eventListeners.set(id, { event: payload.event, handler: payload.handler })
        }
        return id
      }

      case "unlisten":
        eventListeners.delete(payload?.eventId)
        return null
        
      case "emit":
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
    const response = await fetch(`${serverUrl}/invoke/${command}/${repoId}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({})  // Empty body since repo_id is in URL
    })
    
    if (!response.ok) {
      throw new Error(`${command} failed: ${response.status} ${response.statusText}`)
    }
    
    return await response.json()
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
          const repoInfoRes = await fetch(`${serverUrl}/repositories/${repoId}`)
          if (repoInfoRes.ok) {
            const repoInfo = await repoInfoRes.json()
            const currentPath: string | undefined = repoInfo?.path
            if (currentPath && currentPath.startsWith("NO_REPO_")) {
              const createRes = await fetch(`${serverUrl}/repositories`, {
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

  const NO_UPDATE = { current_version: "0.0.0-test", available_version: "", is_update_available: false, status: "Idle" }
  
  const COMMAND_HANDLERS: Record<string, (cmd: string, payload: any) => any> = {
    // Plugin commands
    "plugin:store": handleStoreCommand,
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
    // the notification plugin goes through the browser's `Notification`; its permission check never notifies
    "plugin:notification": () => false,
    // links open in the browser: record them for the test (`window.__OPENED_URLS__`)
    "plugin:opener": (_cmd: string, payload: any) => {
      ;((window as any).__OPENED_URLS__ ??= []).push(payload?.url)
      return null
    },
    
    // Repository commands
    "get_branch_prefix_from_git_config": (cmd: string, payload: any) => {
      const params = payload?.params || payload
      const repositoryPath = params?.repositoryPath || ""
      
      // For NO_REPO test cases, fail as the backend does (`branch_prefix.rs`): Tauri rejects with the CommandError
      // ({ message }), which the bindings return as `{ status: "error", error }`
      if (repositoryPath.includes("NO_REPO")) {
        throw { name: "CommandError", message: `Repository not accessible: ${repositoryPath}` }
      }
      
      // For real repositories, proxy to test server
      return proxyToTestServer(cmd, payload)
    },
    
    // Special async commands
    "sync_branches": async (_cmd: string, payload: any) => {
      if (payload && payload.progress) {
        return handleStreamingCommand("sync_branches", payload)
      }
      // Fallback to proxy if no progress channel
      return proxyToTestServer("sync_branches", payload)
    },
    
    // Updater commands: the test server has no updater, the app is always up to date
    // (the main window runs a silent check a few seconds after startup)
    "check_for_updates": () => NO_UPDATE,
    "get_update_status": () => NO_UPDATE,
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
      // the command ran and failed: the app gets its CommandError
      if (error?.name === "CommandError") {
        throw error
      }
      console.error(`[Test Mock] Failed to handle command ${cmd}:`, error)
      throw new Error(`Unimplemented Tauri command: ${cmd}`)
    }
  }

  /**
   * Setup mockIPC handler
   */
  function setupMockIPC(tauriInternals: any) {
    debug("[Test Mock] Setting up mockIPC handler")
    
    if (tauriInternals.clearMocks) {
      tauriInternals.clearMocks()
    }
    
    tauriInternals.mockIPC(async (cmd: string, payload: any) => {
      // Special handling for commands with Channel objects
      if (cmd === "sync_branches" && payload && payload.progress) {
        // Store the onmessage handler before it gets lost
        const onmessageHandler = payload.progress.onmessage
        debug("[Test Mock] Captured onmessage handler:", typeof onmessageHandler)
        
        // Replace progress with our handler reference
        const modifiedPayload = {
          ...payload,
          progress: {
            onmessage: onmessageHandler
          }
        }
        return handleMockCommand(cmd, modifiedPayload)
      }
      return handleMockCommand(cmd, payload)
    })
  }

  // === Initialize Tauri Mocks ===

  // Create a basic structure if Tauri internals don't exist yet
  if (!(window as any).__TAURI_INTERNALS__) {
    debug("[Test Mock] Creating Tauri internals structure")
    // Create a minimal structure that will be replaced when the real API loads
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
        currentWindow: {
          label: "main"
        },
        currentWebview: {
          windowLabel: "main",
          label: "main"
        }
      },
      mockIPC: null as any,
      clearMocks: null as any,
    }

    // Also override the window.invoke if it exists
    if ((window as any).invoke) {
      ;(window as any).invoke = async (cmd: string, args: any) => {
        debug("[Test Mock] Window invoke intercepted", { cmd, args })
        return handleMockCommand(cmd, args)
      }
    }
  }

  // Check if mockIPC is available and set it up
  if ((window as any).__TAURI_INTERNALS__ && (window as any).__TAURI_INTERNALS__.mockIPC) {
    debug("[Test Mock] mockIPC available immediately")
    setupMockIPC((window as any).__TAURI_INTERNALS__)
  }
  else {
    // If mockIPC isn't available yet, wait for it
    debug("[Test Mock] Waiting for mockIPC to become available...")
    const checkInterval = setInterval(() => {
      if ((window as any).__TAURI_INTERNALS__ && (window as any).__TAURI_INTERNALS__.mockIPC) {
        debug("[Test Mock] mockIPC now available")
        clearInterval(checkInterval)
        setupMockIPC((window as any).__TAURI_INTERNALS__)
      }
    }, 10)

    // Stop checking after 5 seconds
    setTimeout(() => clearInterval(checkInterval), 5000)
  }

  // Mock window API for pages that use getCurrentWindow
  ;(window as any).__TAURI__ = (window as any).__TAURI__ || {}
  ;(window as any).__TAURI__.window = {
    getCurrentWindow: () => {
      debug("[Test Mock] getCurrentWindow called")
      return {
        label: "main",
        close: async () => {
          debug("[Test Mock] window.close called (no-op in tests)")
        },
        onFocusChanged: (callback: (focused: boolean) => void) => {
          debug("[Test Mock] window.onFocusChanged registered (no-op in tests)")
          // Return an unlisten function for cleanup
          return Promise.resolve(() => {
            debug("[Test Mock] window.onFocusChanged unlisten called")
          })
        }
      }
    }
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
  const ICON_SVG_ATTRIBUTES = new Set([
    "fill", "height", "stroke", "stroke-linecap", "stroke-linejoin", "stroke-width", "viewBox", "width", "xmlns", "xmlns:xlink",
  ])
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
      }
      else if (attr.name !== "style") {
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
      // Base UI fallback counter ids and @git-diff-view roots
      { pattern: /\bbase-ui-\d+\b/g, replacement: "base-ui-[ID]" },
      { pattern: /diff-root(-\w+)?--\d+/g, replacement: "diff-root$1--[DYNAMIC]" },
    ],

    // Per-run values: applied to text nodes and to attribute values
    // (e.g. the hidden input of the repository Select holds the repository path in `value`)
    paths: [
      // KSUIDs are 27 chars; not a part of a longer token (a 40 char commit SHA in `data-row-id`), may follow `NO_REPO_`
      { pattern: /(?<![A-Za-z0-9])[0-9][A-Za-z0-9]{26}(?![A-Za-z0-9])/g, replacement: "[REPO_ID]" },
      { pattern: /\/var\/folders\/\S+\/\.tmp\S+\/\S+/g, replacement: "[TEMP_PATH]" },
    ],

    // Inline styles - filters for consistency
    style: {
      excludeRules: ["pointer-events", "animation-duration", "animation-name"]
    }
  }

  // Helper function to apply normalization rules
  function applyNormalizationRules(value: string, rules: Array<{pattern: RegExp, replacement: string}>): string {
    return rules.reduce((normalizedValue, rule) => {
      return normalizedValue.replace(rule.pattern, rule.replacement)
    }, value)
  }

  // Helper function to normalize style attributes
  function normalizeStyleAttribute(value: string): string | null {
    const filteredStyle = value
      .split(';')
      .filter(rule => {
        const trimmed = rule.trim()
        return trimmed && !NORMALIZATION_RULES.style.excludeRules.some(excludeRule => 
          trimmed.includes(excludeRule)
        )
      })
      .join('; ')
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
      // the number of finished syncs depends on timing, not on what the UI shows (`waitForSyncComplete` reads it)
      if (name === "data-sync-count") continue
      
      // Apply normalization rules based on attribute type
      if (name === "style") {
        const normalizedStyle = normalizeStyleAttribute(value)
        if (!normalizedStyle) continue // Skip empty styles
        value = normalizedStyle
      }
      else if (name !== "class") {
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
      { pattern: /\b[a-f0-9]{8}\b/g, replacement: '[SHA]' } // 8 character short SHAs
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
      }
      else if (child.nodeType === Node.ELEMENT_NODE) {
        result += formatElementWithNormalization(child as Element, indent + 1) + "\n"
      }
    }
    flushText()

    result += `${spaces}</${tagName}>`
    return result
  }

  debug("[Test Mock] Tauri mock setup complete")
}
