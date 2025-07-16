/* eslint-disable */

/**
 * Tauri mock script to be injected into the browser context
 * This proxies all Tauri API calls to our test server
 */
export const tauriMockScript = () => {
  console.log("[Test Mock] Initializing Tauri mocks...")

  // Get test repository path and ID from URL if provided
  const urlParams = new URLSearchParams(window.location.search)
  const testRepo = urlParams.get("testRepo") || ""
  const testRepoId = urlParams.get("repoId") || ""

  // Create test app store instance and make it globally available
  // This will be picked up by the app before it initializes
  ;(window as any).__TEST_APP_STORE__ = {
    recentPaths: testRepo ? [testRepo] : [],
    selectedProject: testRepo,
    setTestRepository: function(path: string) {
      this.selectedProject = path
      this.recentPaths = [path]
    },
  }

  // Create a basic structure if Tauri internals don't exist yet
  if (!(window as any).__TAURI_INTERNALS__) {
    console.log("[Test Mock] Creating Tauri internals structure")
    // Create a minimal structure that will be replaced when the real API loads
    ;(window as any).__TAURI_INTERNALS__ = {
      // transformCallback is what's missing in the error
      transformCallback: (callback: any, once: boolean) => {
        console.log("[Test Mock] transformCallback called", { callback, once })
        return callback
      },
      invoke: async (cmd: string, args: any) => {
        console.log("[Test Mock] Direct invoke called", { cmd, args })
        return handleMockCommand(cmd, args)
      },
      mockIPC: null as any,
      clearMocks: null as any,
    }

    // Also override the window.invoke if it exists
    if ((window as any).invoke) {
      ;(window as any).invoke = async (cmd: string, args: any) => {
        console.log("[Test Mock] Window invoke intercepted", { cmd, args })
        return handleMockCommand(cmd, args)
      }
    }
  }

  // Handler function for mock commands
  const handleMockCommand = async (cmd: string, payload: any): Promise<any> => {
    console.log(`[Test Mock] Handling command: ${cmd}`, payload)

    // Special handling for sync_branches with progress channel
    if (cmd === "sync_branches" && payload && payload.progress) {
      const progress = payload.progress
      
      try {
        // Call the test server's sync_branches endpoint with SSE
        const response = await fetch("http://localhost:3030/invoke/sync_branches", {
          method: "POST",
          headers: { 
            "Content-Type": "application/json",
            "Accept": "text/event-stream"
          },
          body: JSON.stringify({
            repositoryPath: payload.repositoryPath,
            branchPrefix: payload.branchPrefix || "",
          }),
        })

        if (!response.ok) {
          throw new Error(`Sync branches failed: ${response.statusText}`)
        }

        // Read the SSE stream
        const reader = response.body?.getReader()
        const decoder = new TextDecoder()
        
        if (!reader) {
          throw new Error("No response body")
        }

        let buffer = ""
        
        while (true) {
          const { done, value } = await reader.read()
          
          if (done) {
            break
          }
          
          buffer += decoder.decode(value, { stream: true })
          
          // Parse SSE events from buffer
          const lines = buffer.split("\n")
          buffer = lines.pop() || "" // Keep incomplete line in buffer
          
          for (const line of lines) {
            console.log('[SSE] Raw line:', JSON.stringify(line))
            if (line.startsWith("data: ")) {
              const data = line.slice(6).trim()
              if (data) {
                try {
                  const event = JSON.parse(data)
                  console.log('[SSE] Received event:', event)
                  if (progress.onmessage) {
                    progress.onmessage(event)
                  }
                } catch (e) {
                  console.error("[Test Mock] Failed to parse event:", e, data)
                }
              }
            }
          }
        }

        return null
      }
      catch (error) {
        console.error("[Test Mock] Error in sync_branches:", error)
        throw error
      }
    }

    // Special handling for validate_repository_path
    if (cmd === "validate_repository_path") {
      // Always return empty string for valid path in tests
      return ""
    }

    // Special handling for store plugin
    if (cmd === "plugin:store|load") {
      return {
        rid: Math.random().toString(36).substring(7),
      }
    }

    if (cmd === "plugin:store|get") {
      // Return null for missing keys (like initial settings)
      return null
    }

    if (cmd === "plugin:store|set") {
      return null
    }

    // Special handling for event plugin
    if (cmd === "plugin:event|listen") {
      // Return a mock unlisten function
      return () => {}
    }

    if (cmd === "plugin:event|emit") {
      return null
    }

    // For all other commands, proxy to test server
    try {
      // Add repository_id if not present
      const requestPayload = { ...payload }
      if (!requestPayload.repository_id) {
        // Use the test repository ID from URL
        requestPayload.repository_id = testRepoId
      }
      
      const response = await fetch(`http://localhost:3030/invoke/${cmd}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(requestPayload),
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
    catch (error) {
      console.error(`[Test Mock] Error handling command ${cmd}:`, error)
      throw error
    }
  }

  // Check if mockIPC is available and set it up
  if ((window as any).__TAURI_INTERNALS__ && (window as any).__TAURI_INTERNALS__.mockIPC) {
    console.log("[Test Mock] mockIPC available, setting up handler")
    if ((window as any).__TAURI_INTERNALS__.clearMocks) {
      ;(window as any).__TAURI_INTERNALS__.clearMocks()
    }
    ;(window as any).__TAURI_INTERNALS__.mockIPC(handleMockCommand)
  }
  else {
    // If mockIPC isn't available yet, wait for it
    console.log("[Test Mock] Waiting for mockIPC to become available...")
    const checkInterval = setInterval(() => {
      if ((window as any).__TAURI_INTERNALS__ && (window as any).__TAURI_INTERNALS__.mockIPC) {
        console.log("[Test Mock] mockIPC now available, setting up handler")
        clearInterval(checkInterval)
        if ((window as any).__TAURI_INTERNALS__.clearMocks) {
          ;(window as any).__TAURI_INTERNALS__.clearMocks()
        }
        ;(window as any).__TAURI_INTERNALS__.mockIPC(handleMockCommand)
      }
    }, 10)

    // Stop checking after 5 seconds
    setTimeout(() => clearInterval(checkInterval), 5000)
  }

  console.log("[Test Mock] Tauri mock setup complete")
}