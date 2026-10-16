import type { Page } from "@playwright/test"
import { expect } from "@playwright/test"
import { parseSetupOptions, type SetupRepoOptions } from "./setup-options"
import { TestRepositoryBuilder } from "./test-repository"
import { TEST_SERVER_URL } from "./test-server-url"

/**
 * Common test setup for repository-based tests
 */
export async function setupTestRepository(page: Page, templateName: string, options?: SetupRepoOptions): Promise<TestRepositoryBuilder> {
  // Parse options and apply defaults
  const parsedOptions = parseSetupOptions(options)

  // Create a test repository using the specified template
  const repoBuilder = new TestRepositoryBuilder().useTemplate(templateName).withPrepopulateStore(parsedOptions.prepopulateStore)

  await repoBuilder.init()

  // the first sync asks for the pull requests and the Patronus robots
  if (parsedOptions.pullRequests !== undefined) {
    await repoBuilder.setPullRequests(parsedOptions.pullRequests)
  }
  if (parsedOptions.reviewRequests !== undefined) {
    await repoBuilder.setReviewRequests(parsedOptions.reviewRequests)
  }
  if (parsedOptions.patronus !== undefined) {
    await repoBuilder.setPatronus(parsedOptions.patronus)
  }
  if (parsedOptions.branchNameSuggestions !== undefined) {
    await repoBuilder.setBranchNameSuggestions(parsedOptions.branchNameSuggestions)
  }

  // Set initial store values if provided BEFORE navigating
  if (Object.keys(parsedOptions.initialStoreValues).length > 0 && repoBuilder.id) {
    const baseUrl = TEST_SERVER_URL
    for (const [key, value] of Object.entries(parsedOptions.initialStoreValues)) {
      debugLog(`[Test Setup] Setting initial store value: ${key} =`, value)
      const response = await fetch(`${baseUrl}/store/${repoBuilder.id}/${key}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(value),
      })
      if (!response.ok) {
        throw new Error(`Failed to set initial store value for ${key}: ${response.statusText}`)
      }

      // Verify the value was set
      const verifyResponse = await fetch(`${baseUrl}/store/${repoBuilder.id}/${key}`)
      const storedValue = await verifyResponse.json()
      debugLog(`[Test Setup] Verified store value for ${key}:`, storedValue)
    }
  }

  // Inject store values into window.__TAURI_STORE__ before navigation
  // This mimics how production Tauri preloads store data
  // Always inject at least an empty object, since the app expects __TAURI_STORE__ to exist
  const storeData: Record<string, unknown> = { ...parsedOptions.initialStoreValues }

  // If prepopulateStore is true, ensure recentProjects exists
  // This mimics production where a selected repository always exists in the store
  const shouldPopulateStore = parsedOptions.prepopulateStore

  if (shouldPopulateStore && repoBuilder.id) {
    const baseUrl = TEST_SERVER_URL

    // Fetch recentProjects from test server if not already in storeData
    if (!storeData.recentProjects) {
      const recentProjectsResponse = await fetch(`${baseUrl}/store/${repoBuilder.id}/recentProjects`)
      if (recentProjectsResponse.ok) {
        const recentProjects = await recentProjectsResponse.json()
        if (recentProjects) {
          storeData.recentProjects = recentProjects
          debugLog(`[Test Setup] Fetched recentProjects from test server:`, recentProjects)
        }
      }
    }

    // Ensure we always have recentProjects when we expect a repository to be selected
    // This is critical for tests to work like production
    if (!storeData.recentProjects) {
      if (!repoBuilder.path) {
        throw new Error(`Test setup error: Expected repository path but got none for template ${templateName}`)
      }

      // Create the expected store state
      // For NO_REPO, only create recentProjects if explicitly requested (for saved-path-validation tests)
      if (templateName === "NO_REPO") {
        if (parsedOptions.createRecentProject) {
          storeData.recentProjects = [
            {
              path: repoBuilder.path,
            },
          ]
        }
        // Otherwise, leave recentProjects empty so welcome card shows
      } else {
        storeData.recentProjects = [
          {
            path: repoBuilder.path,
            cachedBranchPrefix: "user-name",
          },
        ]
      }
      debugLog(`[Test Setup] Created required recentProjects for ${templateName}:`, storeData.recentProjects)
    }
  }

  await page.addInitScript(storeValues => {
    // Inject the store values just like Tauri does in production: a window starts from `settings.json` as it is now, so
    // a reload starts from what the app wrote to its store since (`rememberStoreWrite` of `tauri-mocks.ts`)
    const written = JSON.parse(sessionStorage.getItem("bd-e2e-store-writes") ?? "{}") as Record<string, unknown>
    const current = Object.fromEntries(Object.entries({ ...storeValues, ...written }).filter(([, value]) => value !== null))
    Object.assign(window, { __TAURI_STORE__: current })
    console.log("[Test Setup] Injected __TAURI_STORE__:", current)
  }, storeData)

  // Navigate to the app with the repository ID in URL for test server tracking
  await page.goto(`/?repoId=${repoBuilder.id}`)

  // Wait for the page to be ready - use domcontentloaded instead of networkidle
  // because networkidle might timeout with SSE connections
  await page.waitForLoadState("domcontentloaded")

  // Wait for the React app to mount and render content
  // The ConfigurationHeader contains the sync button we need
  // Skip this wait if prepopulateStore is false since no repository is loaded
  if (shouldPopulateStore) {
    await page.waitForSelector("[data-testid='configuration-header']", { timeout: 45000 })
  } else {
    // For empty store scenarios (like welcome card), wait for the app to be mounted
    // Wait for the branch creator root element which is always present
    await page.waitForSelector("[data-testid='branch-creator-root']", { timeout: 45000 })
  }

  // Wait for the repository to be loaded and validated by checking if the sync button is enabled
  // This ensures that:
  // 1. The repository path is loaded from the test server
  // 2. The path validation has completed successfully
  // 3. The app is ready for interaction
  // Skip sync button wait for:
  // - NO_REPO and empty-non-git templates since they don't have functional git operations
  // - When prepopulateStore is false since no repository is selected yet
  if (templateName !== "NO_REPO" && templateName !== "empty-non-git" && templateName !== "bare" && parsedOptions.prepopulateStore) {
    // a repository without a branch prefix keeps Sync disabled: the welcome card then says the prefix has loaded
    const syncButton = page.locator('[data-testid="sync-button"]:not([data-disabled])')
    await expect(syncButton.or(page.getByTestId("welcome-card")).first()).toBeVisible({ timeout: 30000 })
  }

  // For NO_REPO, wait for the path validation to complete and error to be processed
  // We expect the sync button to be disabled and error alert to appear
  if (templateName === "NO_REPO" && parsedOptions.prepopulateStore) {
    // Wait for validation error to appear (indicates validation completed)
    await expect(page.locator("[role='alert'], .alert"))
      .toBeVisible({ timeout: 5000 })
      .catch(() => {
        // If no alert appears, just wait for sync button to be present (may be disabled)
        return page.waitForSelector("[data-testid='sync-button']", { timeout: 5000 })
      })
  }

  // The repository path should be loaded from the test server by now
  debugLog(`[Test Setup] Repository ${repoBuilder.id} at path ${repoBuilder.path} should be loaded`)

  return repoBuilder
}

/**
 * Setup browser console logging
 */
/** Logs a setup step with DEBUG_E2E=1 */
function debugLog(...args: unknown[]): void {
  if (process.env.DEBUG_E2E) {
    console.log(...args)
  }
}

/**
 * Cleanup test repository
 */
export async function cleanupTestRepository(repoBuilder: TestRepositoryBuilder): Promise<void> {
  if (repoBuilder) {
    await repoBuilder.cleanup()
  }
}
