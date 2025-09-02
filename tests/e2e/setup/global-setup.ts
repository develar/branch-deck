import type { FullConfig } from "@playwright/test"
import { execSync } from "child_process"

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
 * This runs once before all tests and builds the frontend and sets up the test environment
 */
async function globalSetup(_config: FullConfig) {
  console.log("[Global Setup] Initializing test environment...")

  // Build the frontend first
  try {
    console.log("[Global Setup] Building frontend...")
    execSync("pnpm build", { stdio: "inherit", cwd: process.cwd() })
    console.log("[Global Setup] Frontend build completed")
  }
  catch (error) {
    console.error("[Global Setup] Frontend build failed:", error)
    throw error
  }

  // Verify the test server is running
  try {
    // Check test server (serves both API and static files)
    await waitForServer("http://localhost:3030/health", "Test server")
  }
  catch (error) {
    console.error("[Global Setup] Server check failed:", error)
    throw error
  }

  console.log("[Global Setup] Complete")
}

export default globalSetup
