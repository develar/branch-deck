import type { FullConfig } from "@playwright/test"

/**
 * Global setup for Playwright tests
 * This runs once before all tests and sets up Tauri mocks
 */
async function globalSetup(_config: FullConfig) {
  console.log("[Global Setup] Initializing test environment...")

  // We'll use browser context's addInitScript in individual tests
  // since global setup doesn't have access to test contexts
  // Instead, we'll just verify the test server is running

  try {
    const response = await fetch("http://localhost:3030/health")
    if (!response.ok) {
      throw new Error("Test server is not running. Please run: pnpm e2e:server")
    }
    console.log("[Global Setup] Test server is running")
  }
  catch (error) {
    console.error("[Global Setup] Test server check failed:", error)
    throw new Error("Test server is not running. Please run: pnpm e2e:server")
  }

  console.log("[Global Setup] Complete")
}

export default globalSetup