import { execSync } from "child_process"
import { realpathSync } from "fs"
import type { FullConfig } from "@playwright/test"
import { TEST_SERVER_PORT, TEST_SERVER_URL } from "../helpers/test-server-url"

/**
 * Check if Playwright browsers are installed
 */
function checkPlaywrightBrowsers() {
  try {
    // Use --dry-run to quickly check if browsers are installed
    execSync("playwright install --dry-run", { stdio: "pipe" })
  } catch {
    console.error("\n❌ Playwright browsers are not installed!\n")
    console.error("Please run one of the following commands:")
    console.error("  • pnpm setup:e2e           (recommended - installs with system dependencies)")
    console.error("  • pnpm exec playwright install\n")
    throw new Error("Playwright browsers missing. Run 'pnpm setup:e2e' to install them.")
  }
}

/**
 * Wait for a URL to be available with retries
 */
async function waitForServer(url: string, name: string, maxRetries = 30, delay = 1000) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      const response = await fetch(url)
      if (response.ok) {
        console.log(`[Global Setup] ${name} is ready`)
        return
      }
    } catch {
      // Server not ready yet
    }

    if (i < maxRetries - 1) {
      await new Promise(resolve => setTimeout(resolve, delay))
    }
  }

  throw new Error(`${name} failed to start after ${(maxRetries * delay) / 1000} seconds`)
}

/**
 * Refuses a server of another checkout: Playwright reuses whatever listens on the port (`reuseExistingServer`), and the
 * test-server of another lease worktree serves that worktree's `dist/` and code
 */
async function checkServerCheckout() {
  const health = await (await fetch(`${TEST_SERVER_URL}/health`)).text()
  // a server older than this check answers "OK"
  const workspaceRoot = health.startsWith("{") ? (JSON.parse(health) as { workspaceRoot: string | null }).workspaceRoot : null
  const own = realpathSync(process.cwd())
  if (workspaceRoot === null || realpathSync(workspaceRoot) !== own) {
    throw new Error(
      `The test server on port ${TEST_SERVER_PORT} serves ${workspaceRoot ?? "an unknown checkout"}, not ${own}. ` +
        `Run on another port (E2E_PORT=${TEST_SERVER_PORT + 1} pnpm e2e) or stop that server.`,
    )
  }
}

/**
 * Global setup for Playwright tests
 * This runs once before all tests and builds the frontend and sets up the test environment
 */
async function globalSetup(_config: FullConfig) {
  console.log("[Global Setup] Initializing test environment...")

  // Check if Playwright browsers are installed
  checkPlaywrightBrowsers()

  // Build the frontend first; its output only when it fails
  try {
    execSync("pnpm build", { stdio: "pipe", cwd: process.cwd() })
  } catch (error) {
    const { stdout, stderr } = error as { stdout?: Buffer; stderr?: Buffer }
    console.error(`[Global Setup] Frontend build failed:\n${stdout?.toString() ?? ""}${stderr?.toString() ?? ""}`)
    throw error
  }

  // Verify the test server is running
  try {
    // Check test server (serves both API and static files)
    await waitForServer(`${TEST_SERVER_URL}/health`, "Test server")
    await checkServerCheckout()
  } catch (error) {
    console.error("[Global Setup] Server check failed:", error)
    throw error
  }

  console.log("[Global Setup] Complete")
}

export default globalSetup
