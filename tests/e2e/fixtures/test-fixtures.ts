import { test as base } from "./base-test"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { setupTestRepository, cleanupTestRepository } from "../helpers/common-setup"
import { setupClipboardPermissions } from "../helpers/clipboard-helpers"
import { syncBranches, waitForBranchesLoaded } from "../helpers/sync-helpers"

/**
 * Extended test fixture with common repository setup
 */
export const test = base.extend<{
  repoBuilder: TestRepositoryBuilder
  setupRepo: (templateName: string) => Promise<TestRepositoryBuilder>
  syncAndWaitForBranches: () => Promise<void>
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

        await use(new Proxy({} as TestRepositoryBuilder, {
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
        }))

        // Auto cleanup
        if (builder) {
          await cleanupTestRepository(builder)
        }
      },

      // Helper to setup repository with template
      setupRepo: async ({ page }, use) => {
        let currentBuilder: TestRepositoryBuilder | null = null

        const setupFn = async (templateName: string, options?: { prepopulateStore?: boolean }) => {
          currentBuilder = await setupTestRepository(page, templateName, options)
          return currentBuilder
        }

        await use(setupFn)

        // Cleanup if not already done
        if (currentBuilder) {
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
    })

export { expect } from "@playwright/test"

/**
 * Test fixture for tests that need unassigned commits
 */
export const testWithUnassigned = test.extend<{
  setupUnassignedRepo: () => Promise<void>
}>({
      setupUnassignedRepo: async ({ page, setupRepo }, use) => {
        const setupFn = async () => {
          await setupRepo("unassigned")
          await syncBranches(page)

          // Wait for unassigned commits section
          await page.waitForSelector("[data-testid=\"unassigned-commits-section\"]", { timeout: 10000 })
          await page.waitForSelector("[data-row-id]", { timeout: 10000 })
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
            await syncBranches(page, [
              "[data-testid=\"branch-row\"][data-branch-name=\"feature-auth\"]",
              "[data-testid=\"branch-row\"][data-branch-name=\"bug-fix\"]",
            ])
          }
          else {
            await syncBranches(page)
            await waitForBranchesLoaded(page)
          }
        }

        await use(setupFn)
      },
    })