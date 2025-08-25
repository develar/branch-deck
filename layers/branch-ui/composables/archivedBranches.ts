import type { Commit, IntegrationConfidence, BranchIntegrationInfo } from "~/utils/bindings"
import { commands } from "~/utils/bindings"
import type { createRepositoryState } from "~/composables/repositoryProvider"
import { createReactiveIndexedCollection } from "~/utils/reactiveIndexedCollection"

/**
 * Reactive archived branch data that updates progressively
 */
export interface ReactiveArchivedBranch {
  name: string // immutable
  type: "integrated" | "not-integrated" | "placeholder"
  commitCount: number
  confidence: IntegrationConfidence | undefined
  integratedAt: number
  // Additional fields for not-integrated branches with partial integration
  integratedCount: number
  orphanedCount: number
  // Summary extracted from first commit for issue-based branches
  summary: string
  // Dynamic state for commit loading
  commits: Commit[] | undefined
  isLoadingCommits: boolean
  hasLoadedCommits: boolean
  loadError: string | undefined
}

/**
 * Create archived branches state with reactive items
 */
export function createArchivedBranchesState(repository: ReturnType<typeof createRepositoryState>) {
  const { vcsRequestFactory } = repository

  // Use ReactiveIndexedCollection for map+array pattern
  const archivedBranchCollection = createReactiveIndexedCollection<string, ReactiveArchivedBranch>()

  /**
   * Update from archived branch names (creates placeholders)
   */
  function updateFromArchivedNames(names: string[]) {
    archivedBranchCollection.reconcile(
      new Set(names),
      name => (reactive({
        name,
        type: "placeholder" as const,
        commitCount: 0, // Unknown until detection completes
        confidence: undefined,
        integratedAt: 0,
        integratedCount: 0,
        orphanedCount: 0,
        summary: "",
        commits: undefined,
        isLoadingCommits: false,
        hasLoadedCommits: false,
        loadError: undefined,
      }) as ReactiveArchivedBranch),
    )
  }

  /**
   * Ensure a reactive archived branch exists in the collection
   */
  function ensureReactiveBranch(name: string): ReactiveArchivedBranch {
    let rb = archivedBranchCollection.get(name)
    if (!rb) {
      rb = reactive({
        name,
        type: "placeholder" as const,
        commitCount: 0,
        confidence: undefined,
        integratedAt: 0,
        integratedCount: 0,
        orphanedCount: 0,
        summary: "",
        commits: undefined,
        isLoadingCommits: false,
        hasLoadedCommits: false,
        loadError: undefined,
      }) as ReactiveArchivedBranch
      archivedBranchCollection.set(name, rb)
    }
    return rb
  }

  /**
   * Helper to update reactive branch properties in a single operation
   */
  function updateReactiveBranch(
    branch: ReactiveArchivedBranch,
    updates: Partial<Omit<ReactiveArchivedBranch, "name">>,
  ) {
    Object.assign(branch, updates)
  }

  /**
   * Update from unified branch integration info
   */
  function updateFromIntegrationInfo(info: BranchIntegrationInfo) {
    const reactiveBranch = ensureReactiveBranch(info.name)

    // Base properties common to all cases
    const baseUpdate = {
      summary: info.summary,
      confidence: undefined as IntegrationConfidence | undefined,
      integratedCount: 0,
      orphanedCount: 0,
    }

    switch (info.status.kind) {
      case "integrated": {
        const { commitCount, confidence, integratedAt } = info.status
        updateReactiveBranch(reactiveBranch, {
          ...baseUpdate,
          type: "integrated",
          commitCount,
          confidence,
          integratedAt: integratedAt ?? 0,
        })
        break
      }
      case "notIntegrated": {
        const { totalCommitCount, integratedCount, orphanedCount, integratedAt } = info.status
        updateReactiveBranch(reactiveBranch, {
          ...baseUpdate,
          type: "not-integrated",
          commitCount: totalCommitCount,
          integratedAt: integratedAt ?? 0,
          integratedCount,
          orphanedCount,
        })
        break
      }
      case "partial":
        // Represent partial as not-integrated without detailed counts
        updateReactiveBranch(reactiveBranch, {
          ...baseUpdate,
          type: "not-integrated",
          commitCount: 0,
          integratedAt: 0,
        })
        break
    }
  }

  /**
   * Load commits for a specific branch
   */
  async function loadCommitsForBranch(branchName: string): Promise<void> {
    const reactiveBranch = archivedBranchCollection.get(branchName)
    if (!reactiveBranch || reactiveBranch.hasLoadedCommits || reactiveBranch.isLoadingCommits) {
      return
    }

    reactiveBranch.isLoadingCommits = true
    reactiveBranch.loadError = undefined

    try {
      const vcsRequest = vcsRequestFactory.createRequest()
      const result = await commands.getArchivedBranchCommits(
        vcsRequest.repositoryPath,
        branchName,
      )

      if (result.status === "ok") {
        reactiveBranch.commits = result.data
        reactiveBranch.hasLoadedCommits = true
      }
      else {
        reactiveBranch.loadError = result.error
      }
    }
    catch (error) {
      reactiveBranch.loadError = error instanceof Error ? error.message : String(error)
    }
    finally {
      reactiveBranch.isLoadingCommits = false
    }
  }

  /**
   * Extract archive date from branch path for sorting
   * Extracts YYYY-MM-DD from paths like "user/archived/2025-01-19/feature-name"
   */
  function extractArchiveDate(branchName: string): string {
    const match = branchName.match(/\/archived\/(\d{4}-\d{2}-\d{2})\//)
    return match?.[1] || ""
  }

  /**
   * Sorted archived branches - by integration date first, then by archive date
   */
  const sortedArchivedBranches = computed(() => {
    return [...archivedBranchCollection.array.value].sort((a, b) => {
      // Primary sort: by integration date (if available), newest first
      const aIntegratedAt = a.integratedAt
      const bIntegratedAt = b.integratedAt

      if (aIntegratedAt > 0 && bIntegratedAt > 0) {
        return bIntegratedAt - aIntegratedAt
      }
      if (aIntegratedAt > 0 && bIntegratedAt === 0) {
        return -1 // a comes first (has integration date)
      }
      if (aIntegratedAt === 0 && bIntegratedAt > 0) {
        return 1 // b comes first (has integration date)
      }

      // Secondary sort: by archive date from path, newest first
      const dateA = extractArchiveDate(a.name)
      const dateB = extractArchiveDate(b.name)
      return dateB.localeCompare(dateA)
    })
  })

  return {
    // State
    archivedBranches: sortedArchivedBranches,

    // Actions
    updateFromArchivedNames,
    updateFromIntegrationInfo,
    loadCommitsForBranch,
  }
}
