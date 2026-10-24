import { execFileSync, execSync } from "child_process"
import { createHash } from "crypto"
import { existsSync, readFileSync, realpathSync, writeFileSync } from "fs"
import { chromium } from "@playwright/test"
import type { FullConfig } from "@playwright/test"
import { TEST_SERVER_PORT, TEST_SERVER_URL } from "../helpers/test-server-url"

/**
 * Installs Chromium when the version of Playwright in node_modules has none yet (a new checkout, an update of
 * `@playwright/test`); nothing to do otherwise, which a look at the file answers (`playwright install --dry-run` took a
 * second and a half on a loaded Mac and answered "installed" for any state)
 */
function ensureChromium() {
  if (existsSync(chromium.executablePath())) {
    return
  }
  console.log("[Global Setup] Installing Chromium for this version of Playwright (pnpm setup:e2e installs all browsers)")
  execSync("pnpm exec playwright install chromium", { stdio: "inherit" })
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

/** What `pnpm build` reads: the page, its config and its dependencies; the unit tests beside the page are no part of it */
const BUILD_INPUTS = [
  "src",
  "public",
  "index.html",
  "vite.config.ts",
  "package.json",
  "pnpm-lock.yaml",
  "pnpm-workspace.yaml",
  "tsconfig*.json",
  ":(exclude)src/**/*.test.ts",
  ":(exclude)src/test-support",
]

/** The hash of the inputs `dist/` was built from, written after the build; a build empties `dist/`, and with it this file */
const BUILT_INPUTS = "dist/.e2e-build-inputs"

/**
 * The hash of the build inputs as they are now, by git: the blob of every tracked file as the index has it, and the
 * content of each file that differs from the index or is untracked but not ignored. An edit, a new file, a deletion and
 * a checkout all change it, and only the changed files are read: reading all of `src/` takes seconds on a loaded Mac
 */
function buildInputsHash(): string {
  const git = (...args: string[]) => execFileSync("git", args, { encoding: "utf8", env: { ...process.env, GIT_OPTIONAL_LOCKS: "0" } })
  const changed = git("status", "--porcelain=v1", "-z", "--untracked-files=all", "--", ...BUILD_INPUTS)
  const hash = createHash("sha256")
    .update(git("ls-files", "--stage", "-z", "--", ...BUILD_INPUTS))
    .update(changed)
  const entries = changed.split("\0").filter(Boolean)
  for (let index = 0; index < entries.length; index++) {
    const entry = entries[index]!
    // a rename or a copy in the index names its source in the next entry, whose blob the index no longer has
    if (entry[0] === "R" || entry[0] === "C") {
      index++
    }
    const path = entry.slice(3)
    hash.update(path).update("\0")
    try {
      hash.update(readFileSync(path))
    } catch {
      hash.update("deleted")
    }
  }
  return hash.digest("hex")
}

function readBuiltInputs(): string | null {
  try {
    return readFileSync(BUILT_INPUTS, "utf8")
  } catch {
    return null
  }
}

/**
 * Builds the frontend into `dist/` unless it was built from the inputs as they are now: a run after a run without a
 * change of the page starts at once. Its output only when it fails
 */
function buildFrontend() {
  const started = Date.now()
  const inputs = buildInputsHash()
  if (readBuiltInputs() === inputs) {
    console.log(`[Global Setup] Frontend is up to date (${Date.now() - started} ms)`)
    return
  }
  try {
    execSync("pnpm build", { stdio: "pipe", cwd: process.cwd() })
  } catch (error) {
    const { stdout, stderr } = error as { stdout?: Buffer; stderr?: Buffer }
    console.error(`[Global Setup] Frontend build failed:\n${stdout?.toString() ?? ""}${stderr?.toString() ?? ""}`)
    throw error
  }
  writeFileSync(BUILT_INPUTS, inputs)
  console.log(`[Global Setup] Built the frontend in ${((Date.now() - started) / 1000).toFixed(1)} s`)
}

/**
 * Global setup for Playwright tests
 * This runs once before all tests and builds the frontend and sets up the test environment
 */
async function globalSetup(_config: FullConfig) {
  console.log("[Global Setup] Initializing test environment...")

  ensureChromium()
  buildFrontend()

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
