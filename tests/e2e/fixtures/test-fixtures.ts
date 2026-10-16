import type { Page } from "@playwright/test"
import { expect } from "@playwright/test"
import { setupClipboardPermissions } from "../helpers/clipboard-helpers"
import { setupTestRepository, cleanupTestRepository } from "../helpers/common-setup"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { syncBranches, waitForBranchesLoaded } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { TEST_SERVER_URL } from "../helpers/test-server-url"
import { test as base } from "./base-test"

/**
 * Test store API for interacting with the test server's store
 */
class TestStore {
  private baseUrl = TEST_SERVER_URL

  constructor(private page: Page) {}

  private async getRepoId(): Promise<string> {
    const url = new URL(this.page.url())
    const repoId = url.searchParams.get("repoId")
    if (!repoId) {
      throw new Error("No repoId found in URL - make sure to call setupRepo first")
    }
    return repoId
  }

  async get(key: string): Promise<unknown> {
    const repoId = await this.getRepoId()
    const response = await fetch(`${this.baseUrl}/store/${repoId}/${key}`)
    if (!response.ok) {
      throw new Error(`Failed to get store value: ${response.statusText}`)
    }
    const value = await response.json()
    return value
  }

  async set(key: string, value: unknown): Promise<void> {
    const repoId = await this.getRepoId()
    const response = await fetch(`${this.baseUrl}/store/${repoId}/${key}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(value),
    })
    if (!response.ok) {
      throw new Error(`Failed to set store value: ${response.statusText}`)
    }
  }

  async delete(key: string): Promise<void> {
    const repoId = await this.getRepoId()
    const response = await fetch(`${this.baseUrl}/store/${repoId}/${key}`, {
      method: "DELETE",
    })
    if (!response.ok) {
      throw new Error(`Failed to delete store value: ${response.statusText}`)
    }
  }
}

/**
 * Extended test fixture with common repository setup
 */
export const test = base.extend<{
  repoBuilder: TestRepositoryBuilder
  setupRepo: (templateName: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>
  syncAndWaitForBranches: () => Promise<void>
  testStore: TestStore
}>({
  // Auto-setup clipboard permissions
  page: async ({ page, context }, use) => {
    await setupClipboardPermissions(context)
    await use(page)
  },

  // Repository builder that auto-cleans up
  // eslint-disable-next-line no-empty-pattern
  repoBuilder: async ({}, use) => {
    let builder: TestRepositoryBuilder | null = null

    await use(
      new Proxy({} as TestRepositoryBuilder, {
        get(target, prop) {
          if (!builder) {
            throw new Error("Repository not initialized. Call setupRepo() first.")
          }
          return builder[prop as keyof TestRepositoryBuilder]
        },
        set(target, prop, value) {
          if (!builder) {
            builder = {} as TestRepositoryBuilder
          }
          builder[prop as keyof TestRepositoryBuilder] = value
          return true
        },
      }),
    )

    // Auto cleanup
    if (builder) {
      await cleanupTestRepository(builder)
    }
  },

  // Helper to setup repository with template
  setupRepo: async ({ page }, use) => {
    let currentBuilder: TestRepositoryBuilder | null = null

    const setupFn = async (templateName: string, options?: SetupRepoOptions) => {
      currentBuilder = await setupTestRepository(page, templateName, options)
      return currentBuilder
    }

    await use(setupFn)

    if (currentBuilder) {
      // the pages first (a test may open a sub-window), so they start no command for the repository; the server waits
      // for the commands still running before it deletes the files
      await Promise.all(
        page
          .context()
          .pages()
          .map(it => it.close()),
      )
      await cleanupTestRepository(currentBuilder)
    }
  },

  // Helper to sync and wait for branches
  syncAndWaitForBranches: async ({ page }, use) => {
    const syncFn = async () => {
      await syncBranches(page)
      await waitForBranchesLoaded(page)
    }

    await use(syncFn)
  },

  // Test store API
  testStore: async ({ page }, use) => {
    const store = new TestStore(page)
    await use(store)
  },
})

export { expect } from "@playwright/test"

/**
 * Test fixture for the `issue_links` repository, synced before every test (the fixture is automatic); a test that
 * changes the repository behind the app's back asks for `issueLinksRepo`
 */
export const testWithIssueLinks = test.extend<{ issueLinksRepo: TestRepositoryBuilder }>({
  issueLinksRepo: [
    async ({ page, setupRepo, syncAndWaitForBranches }, use) => {
      const repo = await setupRepo("issue_links")
      await syncAndWaitForBranches()
      // the unassigned commits card can render after the branches and push the commit list down
      await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
      await use(repo)
    },
    { auto: true },
  ],
})

/**
 * Test fixture for tests that need unassigned commits
 */
export const testWithUnassigned = test.extend<{
  setupUnassignedRepo: (options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>
}>({
  setupUnassignedRepo: async ({ page, setupRepo }, use) => {
    const setupFn = async (options?: SetupRepoOptions) => {
      const repo = await setupRepo("unassigned", options)
      await syncBranches(page)

      // Wait for unassigned commits section
      await page.waitForSelector('[data-testid="unassigned-commits-section"]', { timeout: 10000 })
      await page.waitForSelector("[data-row-id]", { timeout: 10000 })
      return repo
    }

    await use(setupFn)
  },
})

/**
 * Test fixture for conflict testing
 */
export const testWithConflicts = test.extend<{
  setupConflictRepo: (template: "conflict_branches" | "conflict_unassigned") => Promise<void>
}>({
  setupConflictRepo: async ({ page, setupRepo }, use) => {
    const setupFn = async (template: "conflict_branches" | "conflict_unassigned") => {
      await setupRepo(template)

      if (template === "conflict_branches") {
        // Wait for specific branches that have conflicts
        await syncBranches(page, ['[data-testid="branch-row"][data-branch-name="feature-auth"]', '[data-testid="branch-row"][data-branch-name="bug-fix"]'])
      } else {
        await syncBranches(page)
        await waitForBranchesLoaded(page)
      }
    }

    await use(setupFn)
  },
})
