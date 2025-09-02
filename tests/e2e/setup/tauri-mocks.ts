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

  /**
   * Proxy command to test server
   */
  async function proxyToTestServer(cmd: string, payload: any): Promise<any> {
    debug(`[Test Mock] Proxying command to test server: ${cmd}`)
    
    // Unwrap params if present - the frontend wraps parameters in { params: actualParams }
    // but the test server expects the parameters directly
    const actualPayload = payload?.params || payload
    
    try {
      const response = await fetch(`http://localhost:3030/invoke/${cmd}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(actualPayload),
      })

      if (!response.ok) {
        const error = await response.text()
        // Return a Result type with error status to match Tauri bindings
        const result = {
          status: "error",
          error: `${response.statusText} - ${error}`,
        }
        return result
      }

      const contentType = response.headers.get("content-type")
      let data
      if (contentType && contentType.includes("application/json")) {
        data = await response.json()
      } else {
        data = await response.text()
      }

      // Check if the test server returned a Result-like object
      if (data && typeof data === "object" && "status" in data) {
        if (data.status === "error") {
          // Throw a proper Error object so the bindings layer catches it and wraps it properly
          const error = new Error(data.error)
          error.name = "TauriError" // Mark it as a Tauri error, not a network error
          throw error
        }
        // For success responses, return just the data (bindings will wrap it)
        return data.data
      }

      // For backwards compatibility, return raw data (bindings will wrap it)
      return data
    } catch (networkError) {
      // Re-throw TauriErrors so they reach the bindings layer
      if (networkError.name === "TauriError") {
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
      
      const url = `http://localhost:3030/invoke/${cmd}`
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
        throw new Error(`${cmd} failed: ${response.statusText}`)
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
   * Handle sync_branches command with SSE progress
   */
  async function handleSyncBranches(payload: any): Promise<any> {
    const progress = payload.progress
    
    debug("[Test Mock] sync_branches called with params:", payload.params)
    debug("[Test Mock] progress.onmessage type:", typeof progress.onmessage)
    
    try {
      // Extract params, excluding the progress channel which can't be serialized
      const requestPayload = payload.params
      
      // Call the test server's sync_branches endpoint with SSE
      const response = await fetch("http://localhost:3030/invoke/sync_branches", {
        method: "POST",
        headers: { 
          "Content-Type": "application/json",
          "Accept": "text/event-stream"
        },
        body: JSON.stringify(requestPayload),
      })

      if (!response.ok) {
        throw new Error(`Sync branches failed: ${response.statusText}`)
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
      console.error("[Test Mock] Error in sync_branches:", error)
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
            const response = await fetch(`http://localhost:3030/store/${repoId}/${key}`)
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
            const response = await fetch(`http://localhost:3030/store/${repoId}/${key}`, {
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
            const response = await fetch(`http://localhost:3030/store/${repoId}/${key}`, {
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
  function handleEventCommand(cmd: string): any {
    const [, action] = cmd.split("|")
    
    switch (action) {
      case "listen":
        // Return a mock unlisten function
        return () => {}
        
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
   * Handle app-specific commands
   */
  /**
   * Helper to proxy simple commands to test server with repoId in URL
   */
  async function proxyCommandWithRepoId(command: string, expectJson: boolean = true): Promise<any> {
    const response = await fetch(`http://localhost:3030/invoke/${command}/${repoId}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({})  // Empty body since repo_id is in URL
    })
    
    if (!response.ok) {
      throw new Error(`${command} failed: ${response.status} ${response.statusText}`)
    }
    
    return expectJson ? await response.json() : null
  }

  function handleAppCommand(cmd: string, payload: any): any {
    switch (cmd) {
      case "validate_repository_path":
        // Always return empty string for valid path in tests
        return ""
        
      case "browse_repository":
        return proxyCommandWithRepoId("browse_repository")
        
      case "download_model":
        // Handle streaming command with repoId in URL
        if (payload && payload.progress) {
          return handleStreamingCommand(`download_model/${repoId}`, payload)
        }
        // Fallback shouldn't happen for download_model
        throw new Error("download_model requires progress channel")
        
      case "cancel_model_download":
        return proxyCommandWithRepoId("cancel_model_download", false)  // Returns null, not JSON
        
      default:
        return null // Not handled here
    }
  }

  // === Command Registry ===
  
  const COMMAND_HANDLERS: Record<string, (cmd: string, payload: any) => any> = {
    // Plugin commands
    "plugin:store": handleStoreCommand,
    "plugin:event": handleEventCommand,
    "plugin:path": handlePathCommand,
    "plugin:log": () => null, // Ignore log commands
    
    // Special async commands
    "sync_branches": async (_cmd: string, payload: any) => {
      if (payload && payload.progress) {
        return handleSyncBranches(payload)
      }
      // Fallback to proxy if no progress channel
      return proxyToTestServer("sync_branches", payload)
    },
    
    // AI streaming commands
    "suggest_branch_name_stream": async (_cmd: string, payload: any) => {
      if (payload && payload.progress) {
        return handleStreamingCommand("suggest_branch_name_stream", payload)
      }
      return proxyToTestServer("suggest_branch_name_stream", payload)
    },
  }

  // Commands that should be proxied to test server
  const PROXY_WHITELIST = [
    "get_branch_prefix_from_git_config",
    "create_branch_from_commits",
    "add_issue_reference_to_commits",
    "delete_archived_branch",
    "push_branches",
    "suggest_branch_name_stream",
  ]

  /**
   * Main command handler
   */
  const handleMockCommand = async (cmd: string, payload: any): Promise<any> => {
    debug(`[Test Mock] Handling command: ${cmd}`, payload)

    // Check app-specific commands first
    const appResult = handleAppCommand(cmd, payload)
    if (appResult !== null) {
      return appResult
    }

    // Check command registry
    const prefix = cmd.split("|")[0]
    const handler = COMMAND_HANDLERS[prefix] || COMMAND_HANDLERS[cmd]
    
    if (handler) {
      return handler(cmd, payload)
    }

    // Check if command should be proxied
    if (PROXY_WHITELIST.includes(cmd)) {
      return proxyToTestServer(cmd, payload)
    }

    // Warn about unimplemented commands
    debug(`[Test Mock] Command not implemented: ${cmd}. Attempting to proxy to test server.`)
    
    try {
      return await proxyToTestServer(cmd, payload)
    } catch (error) {
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
      if ((cmd === "sync_branches" || cmd === "suggest_branch_name_stream" || cmd === "download_model") && payload && payload.progress) {
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

  // Normalization configuration for consistent snapshot formatting
  const NORMALIZATION_RULES = {
    // All HTML attributes that contain Vue component IDs - applied in order, most specific first
    attributes: [
      { pattern: /reka-(\w+(?:-\w+)*)-v-\d+(-\d+)?/g, replacement: 'reka-$1-v-[N]' },
      { pattern: /diff-root--\d+/g, replacement: 'diff-root--[DYNAMIC]' },
      { pattern: /v-\d+(-\d+)?/g, replacement: 'v-[N]' }
    ],
    
    // Inline styles - filters for consistency
    style: {
      excludeRules: ['pointer-events', 'animation-duration', 'animation-name']
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

  // Format and normalize HTML element in a single pass
  function formatElementWithNormalization(element: Element, indent = 0): string {
    const spaces = "  ".repeat(indent)
    const tagName = element.tagName.toLowerCase()
    let result = `${spaces}<${tagName}`

    // Collect and normalize attributes in one pass
    const normalizedAttrs: Array<[string, string]> = []
    
    for (const attr of element.attributes) {
      let value = attr.value
      const name = attr.name
      
      // Apply normalization rules based on attribute type
      if (name === 'style') {
        const normalizedStyle = normalizeStyleAttribute(value)
        if (!normalizedStyle) continue // Skip empty styles
        value = normalizedStyle
      }
      else if (name === 'id' || name === 'aria-describedby' || name === 'aria-labelledby' || name === 'aria-controls' || name === 'for') {
        // All attributes that can contain Vue component IDs use the same normalization rules
        value = applyNormalizationRules(value, NORMALIZATION_RULES.attributes)
      }
      
      normalizedAttrs.push([name, value])
    }

    // Sort attributes for consistent output
    normalizedAttrs.sort((a, b) => a[0].localeCompare(b[0]))

    // Add sorted attributes to tag
    for (const [name, value] of normalizedAttrs) {
      result += ` ${name}="${value}"`
    }

    // Handle empty elements
    const hasChildren = element.childNodes.length > 0
    const hasTextContent = element.textContent?.trim()
    
    if (!hasChildren || !hasTextContent) {
      result += " />"
      return result
    }

    result += ">\n"

    // Text content normalization patterns
    const TEXT_NORMALIZATION_RULES = [
      { pattern: /[0-9][A-Za-z0-9]{26}/g, replacement: '[REPO_ID]' }, // KSUIDs are 27 chars
      { pattern: /\/var\/folders\/\S+\/\.tmp\S+\/\S+/g, replacement: '[TEMP_PATH]' },
      { pattern: /\b[a-f0-9]{8}\b/g, replacement: '[SHA]' }, // 8 character short SHAs
      { pattern: /(\[{"prerenderedAt":\d+,"serverRendered":\d+},)\d+,/g, replacement: '$1[TIMESTAMP],' },
      { pattern: /buildId:"[a-f0-9-]{36}"/g, replacement: 'buildId:"[BUILD_ID]"' },
      { pattern: /buildId:"\[SHA]-[a-f0-9-]+"/g, replacement: 'buildId:"[BUILD_ID]"' }
    ]

    // Helper function to normalize text content
    function normalizeTextContent(text: string): string {
      return TEXT_NORMALIZATION_RULES.reduce((normalizedText, rule) => {
        return normalizedText.replace(rule.pattern, rule.replacement)
      }, text)
    }

    // Process children with optimized text normalization
    for (const child of element.childNodes) {
      if (child.nodeType === Node.TEXT_NODE) {
        let text = child.textContent?.trim()
        if (text) {
          text = normalizeTextContent(text)
          result += `${"  ".repeat(indent + 1)}${text}\n`
        }
      }
      else if (child.nodeType === Node.COMMENT_NODE) {
        result += `${"  ".repeat(indent + 1)}<!--${child.textContent}-->\n`
      }
      else if (child.nodeType === Node.ELEMENT_NODE) {
        result += formatElementWithNormalization(child as Element, indent + 1) + "\n"
      }
    }

    result += `${spaces}</${tagName}>`
    return result
  }

  debug("[Test Mock] Tauri mock setup complete")
}