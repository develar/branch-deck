import type { FullConfig } from "@playwright/test"

/**
 * Wait for a URL to be available with retries
 */
async function waitForServer(url: string, name: string, maxRetries = 30, delay = 1000) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      const response = await fetch(url)
      if (response.ok) {
        console.log(`[Global Setup] ${name} server is ready`)
        return
      }
    }
    catch {
      // Server not ready yet
    }

    if (i < maxRetries - 1) {
      await new Promise(resolve => setTimeout(resolve, delay))
    }
  }

  throw new Error(`${name} server failed to start after ${maxRetries * delay / 1000} seconds`)
}

/**
 * Global setup for Playwright tests
 * This runs once before all tests and sets up Tauri mocks
 */
async function globalSetup(_config: FullConfig) {
  console.log("[Global Setup] Initializing test environment...")

  // We'll use browser context's addInitScript in individual tests
  // since global setup doesn't have access to test contexts
  // Instead, we'll verify the test servers are running

  try {
    // Check backend server
    await waitForServer("http://localhost:3030/health", "Backend")

    // Check frontend server - wait until it's fully ready
    await waitForServer("http://localhost:1421", "Frontend")
  }
  catch (error) {
    console.error("[Global Setup] Server check failed:", error)
    throw error
  }

  console.log("[Global Setup] Complete")
}

export default globalSetup
