import type { Locator, Page } from "@playwright/test"
import type { CachedModel } from "../../../src/lib/bindings"
import { expect } from "../fixtures/base-test"
import { clearClipboard } from "./clipboard-helpers"
import { parseSetupOptions, type SetupRepoOptions } from "./setup-options"
import { registerWindowBuild } from "./sub-window-helpers"
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

  // the frame an earlier run left: before every fixture, which the first sync of the page meets
  if (parsedOptions.warmStart) {
    await repoBuilder.buildWarmStart()
    parsedOptions.syncedAgoMs ??= WARM_START_SYNCED_AGO_MS
  }

  // the first sync asks for the pull requests and the Patronus robots
  if (parsedOptions.pullRequests !== undefined) {
    await repoBuilder.setPullRequests(parsedOptions.pullRequests)
  }
  if (parsedOptions.reviewRequests !== undefined) {
    await repoBuilder.setReviewRequests(parsedOptions.reviewRequests)
  }
  if (parsedOptions.gitHubSignIn !== undefined) {
    await repoBuilder.setGitHubSignIn(parsedOptions.gitHubSignIn)
  }
  if (parsedOptions.remoteAnswers !== undefined) {
    await repoBuilder.holdRemoteAnswers(parsedOptions.remoteAnswers)
  }
  if (parsedOptions.patronus !== undefined) {
    await repoBuilder.setPatronus(parsedOptions.patronus)
  }
  if (parsedOptions.patronusProfiles !== undefined) {
    await repoBuilder.setPatronusProfiles(parsedOptions.patronusProfiles)
  }
  if (parsedOptions.reviewAnswers !== undefined) {
    await repoBuilder.setReviewAnswers({ answers: parsedOptions.reviewAnswers })
  }
  if (parsedOptions.youtrack !== undefined) {
    await repoBuilder.setYouTrack(parsedOptions.youtrack)
  }
  if (parsedOptions.patronusDetails !== undefined) {
    await repoBuilder.setPatronusDetails(parsedOptions.patronusDetails)
  }
  if (parsedOptions.rebaseInProgress) {
    await repoBuilder.setRebaseInProgress(true)
  }
  if (parsedOptions.landing !== undefined) {
    await repoBuilder.setLanding(parsedOptions.landing.setting, parsedOptions.landing.gitHub === undefined ? {} : { gitHub: parsedOptions.landing.gitHub })
  }
  if (parsedOptions.syncOverrides !== undefined) {
    await repoBuilder.setSyncOverrides(parsedOptions.syncOverrides)
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

  // what the last sync recorded of the repository: the writer keeps it in repository-state.json, not in the recent projects
  if (parsedOptions.syncedAgoMs !== undefined && repoBuilder.id && repoBuilder.path) {
    const repositoryState = { [repoBuilder.path]: { lastSyncTime: Date.now() - parsedOptions.syncedAgoMs, lastBranchCount: parsedOptions.syncedBranchCount } }
    const response = await fetch(`${TEST_SERVER_URL}/store/${repoBuilder.id}/repositoryState`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(repositoryState),
    })
    if (!response.ok) {
      throw new Error(`Failed to set the repository state: ${response.statusText}`)
    }
    parsedOptions.initialStoreValues.repositoryState = repositoryState
  }

  // the start of the app: the session opened seeded from the frame, its first sync running, and its seed for the page
  if (parsedOptions.warmStart) {
    const model = await repoBuilder.warmStartModel()
    if (model === null) {
      throw new Error(`Test setup error: the frame of ${templateName} seeds no session`)
    }
    await injectWarmModel(page, model)
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
        storeData.recentProjects = [{ path: repoBuilder.path }]
      }
      debugLog(`[Test Setup] Created required recentProjects for ${templateName}:`, storeData.recentProjects)
    }
  }

  // the version of the test server's writer the values are taken at: the page asks for the changes after it
  let storeVersion = 0
  if (repoBuilder.id) {
    const snapshot = await fetch(`${TEST_SERVER_URL}/store/${repoBuilder.id}`)
    if (snapshot.ok) {
      storeVersion = ((await snapshot.json()) as { version: number }).version
    }
  }

  // the main window is built before its page loads, as the app builds it: its open is 1 (`__SUB_WINDOW_OPEN__` of the
  // page mock), and its stages set its deadlines on the test server as in the app
  if (repoBuilder.id) {
    await registerWindowBuild(repoBuilder.id, { label: "main", page: "main", openId: 1 })
  }

  await page.addInitScript(
    ({ storeValues, version }) => {
      // Inject the store values just like Tauri does in production: a window starts from the writer's copy as it is
      // now, so a reload starts from the changes the page heard since, at the version of the last one
      // (`rememberStoreChange` of `tauri-mocks.ts`); a test's own init script may change the copy after this one
      const written = JSON.parse(sessionStorage.getItem("bd-e2e-store-writes") ?? "{}") as Record<string, unknown>
      const current = Object.fromEntries(Object.entries({ ...storeValues, ...written }).filter(([, value]) => value !== null))
      const heard = Number(sessionStorage.getItem("bd-e2e-store-version") ?? 0)
      Object.assign(window, { __TAURI_STORE__: current, __TAURI_STORE_VERSION__: Math.max(version, heard) })
      console.log("[Test Setup] Injected __TAURI_STORE__:", current)
    },
    { storeValues: storeData, version: storeVersion },
  )

  // Navigate to the app with the repository ID in URL for test server tracking
  await page.goto(`/?repoId=${repoBuilder.id}`)

  // Wait for the page to be ready - use domcontentloaded instead of networkidle
  // because networkidle might timeout with SSE connections
  await page.waitForLoadState("domcontentloaded")
  // the clipboard belongs to the browser, which the worker keeps for its next tests, each in a new context: a prompt the
  // test before copied would satisfy an `expect.poll(readClipboard)` before this test's own copy lands
  if (page.context().browser()?.browserType().name() !== "webkit") {
    await clearClipboard(page)
  }

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
  if (templateName !== "NO_REPO" && templateName !== "empty-non-git" && templateName !== "bare" && parsedOptions.prepopulateStore && parsedOptions.waitForSync) {
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

/** How long ago a repository that starts warm was synced (`warmStart`): its session syncs as it opens */
const WARM_START_SYNCED_AGO_MS = 60 * 60 * 1000

/**
 * The seed the start of the app hands the main window's page (`window.__BD_MODEL__`), in an init script of its own: a
 * later one (`reloadWarm`) replaces it for the next load, as the init script of the next start would
 */
export async function injectWarmModel(page: Page, model: CachedModel): Promise<void> {
  await page.addInitScript(injected => Object.assign(window, { __BD_MODEL__: injected }), model)
}

/**
 * The app quits and starts again warm: the sessions of the repository close (each saves its frame), the start opens the
 * session seeded from the frame, and the page loads again with that seed (`window.__BD_MODEL__`); answers the seed
 */
export async function reloadWarm(page: Page, repo: TestRepositoryBuilder): Promise<CachedModel> {
  await repo.closeSessions()
  const model = await repo.warmStartModel()
  if (model === null) {
    throw new Error("The frame of the repository seeds no session")
  }
  await injectWarmModel(page, model)
  await page.reload()
  return model
}

/**
 * `item` of an open menu on the frame of a warm start: disabled, its tooltip "Waits for the first sync"; the menu closes
 * after it
 */
export async function expectWaitingForTheFirstSync(page: Page, item: Locator): Promise<void> {
  await expect(item).toHaveAttribute("aria-disabled", "true")
  await item.hover()
  await expect(page.locator("[data-slot=tooltip-content][data-open]", { hasText: "Waits for the first sync" })).toBeVisible()
  await page.keyboard.press("Escape")
}

/** The first sync of the warm start, held at the backend, runs and commits: the rows are of this run, no frame any more */
export async function releaseTheFirstSync(page: Page, repo: TestRepositoryBuilder): Promise<void> {
  await repo.holdRemoteAnswers({ release: ["sync"] })
  await expect(page.locator("[data-cached]")).toHaveCount(0)
}

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
