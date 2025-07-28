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
    
    const response = await fetch(`http://localhost:3030/invoke/${cmd}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(actualPayload),
    })

    if (!response.ok) {
      const error = await response.text()
      throw new Error(`Test server error: ${response.statusText} - ${error}`)
    }

    const contentType = response.headers.get("content-type")
    if (contentType && contentType.includes("application/json")) {
      return response.json()
    }

    return response.text()
  }

  // === Command Handlers ===

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
  function handleAppCommand(cmd: string): any {
    switch (cmd) {
      case "validate_repository_path":
        // Always return empty string for valid path in tests
        return ""
        
      case "check_model_status":
        // Return model not available for tests
        return {
          available: false,
          progress: null,
          error: null
        }
        
      case "browse_repository":
        // Proxy to test server
        return (async () => {
          const response = await fetch(`http://localhost:3030/invoke/browse_repository`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ repoId })
          })
          
          if (!response.ok) {
            throw new Error(`Browse repository failed: ${response.status} ${response.statusText}`)
          }
          
          return await response.json()
        })()
        
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
  }

  // Commands that should be proxied to test server
  const PROXY_WHITELIST = [
    "get_branch_prefix_from_git_config",
    "create_branch_from_commits",
    "add_issue_reference_to_commits",
    "push_branches",
  ]

  /**
   * Main command handler
   */
  const handleMockCommand = async (cmd: string, payload: any): Promise<any> => {
    debug(`[Test Mock] Handling command: ${cmd}`, payload)

    // Check app-specific commands first
    const appResult = handleAppCommand(cmd)
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

  debug("[Test Mock] Tauri mock setup complete")
}