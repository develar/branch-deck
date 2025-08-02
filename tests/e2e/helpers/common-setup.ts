import type { Page } from "@playwright/test"
import { expect } from "@playwright/test"
import { TestRepositoryBuilder } from "./test-repository"

/**
 * Common test setup for repository-based tests
 */
export async function setupTestRepository(
  page: Page,
  templateName: string,
  options?: { prepopulateStore?: boolean, initialStoreValues?: Record<string, unknown>, modelState?: "not_downloaded" | "downloaded" | "downloading" },
): Promise<TestRepositoryBuilder> {
  // Determine model state based on aiMode in initialStoreValues if not explicitly set
  let modelState = options?.modelState
  if (!modelState && options?.initialStoreValues?.modelSettings) {
    const modelSettings = options.initialStoreValues.modelSettings as { aiMode?: string }
    const aiMode = modelSettings.aiMode
    modelState = aiMode === "enabled" ? "downloaded" : "not_downloaded"
  }

  // Create a test repository using the specified template
  const repoBuilder = new TestRepositoryBuilder()
    .useTemplate(templateName)
    .withPrepopulateStore(options?.prepopulateStore ?? true)

  if (modelState) {
    repoBuilder.withModelState(modelState)
  }

  await repoBuilder.init()

  // Set initial store values if provided BEFORE navigating
  if (options?.initialStoreValues && repoBuilder.id) {
    const baseUrl = "http://localhost:3030"
    for (const [key, value] of Object.entries(options.initialStoreValues)) {
      console.log(`[Test Setup] Setting initial store value: ${key} =`, value)
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
      console.log(`[Test Setup] Verified store value for ${key}:`, storedValue)
    }
  }

  // Open browser console to see debug logs
  setupBrowserLogging(page)

  // Navigate to the app with the repository ID in URL for test server tracking
  await page.goto(`/?repoId=${repoBuilder.id}`)

  // Wait for the page to be ready - use domcontentloaded instead of networkidle
  // because networkidle might timeout with SSE connections
  await page.waitForLoadState("domcontentloaded")

  // Wait for Vue app to mount and render content
  // The ConfigurationHeader contains the sync button we need
  await page.waitForSelector(".bg-elevated", { timeout: 10000 })

  // Wait for the repository to be loaded and validated by checking if the sync button is enabled
  // This ensures that:
  // 1. The repository path is loaded from the test server
  // 2. The path validation has completed successfully
  // 3. The app is ready for interaction
  // Skip sync button wait for:
  // - NO_REPO and empty-non-git templates since they don't have functional git operations
  // - When prepopulateStore is false since no repository is selected yet
  if (templateName !== "NO_REPO" && templateName !== "empty-non-git" && (options?.prepopulateStore ?? true)) {
    const syncButton = page.locator("[data-testid=\"sync-button\"]")
    await expect(syncButton).toBeEnabled({ timeout: 15000 })
  }

  // The repository path should be loaded from the test server by now
  console.log(`[Test Setup] Repository ${repoBuilder.id} at path ${repoBuilder.path} should be loaded`)

  return repoBuilder
}

/**
 * Setup browser console logging
 */
export function setupBrowserLogging(page: Page): void {
  page.on("console", (msg) => {
    console.log(`[Browser]`, msg.type(), msg.text())
  })
}

/**
 * Cleanup test repository
 */
export async function cleanupTestRepository(repoBuilder: TestRepositoryBuilder): Promise<void> {
  if (repoBuilder) {
    await repoBuilder.cleanup()
  }
}
