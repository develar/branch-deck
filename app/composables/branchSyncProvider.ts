import type { Commit, CommitSyncStatus, GroupedBranchInfo, BranchError, BranchSyncStatus, SyncEvent } from "~/utils/bindings"
import { Channel } from "@tauri-apps/api/core"
import { UserError } from "~/composables/git/vcsRequest"
import { commands } from "~/utils/bindings"

// Injection key
export const BranchSyncKey = Symbol("branch-sync")

// Sync options interface
export interface SyncOptions {
  autoScroll?: boolean
  autoExpand?: boolean
  targetBranchName?: string
}

// Augmented commit with sync status
export interface SyncedCommit extends Commit {
  hash?: string // New hash after sync
  status?: CommitSyncStatus
  error?: BranchError | null
}

// Reactive branch data that updates incrementally
export interface ReactiveBranch {
  name: string
  commits: SyncedCommit[]
  commitMap: Map<string, SyncedCommit>
  commitCount: number
  status: "Syncing" | BranchSyncStatus
  statusText: string
  processedCount: number
  hasError: boolean
  errorDetails?: BranchError
  autoExpandRequested: boolean
  autoScrollRequested: boolean
  latestCommitTime: number
}

// Create branch sync state
export function createBranchSyncState(repository: RepositoryState) {
  const { loadingPromise, vcsRequestFactory, selectedProject } = repository
  // State - using explicit refs
  const syncError = shallowRef<string | null>(null)
  const isSyncing = shallowRef(false)
  const hasCompletedSync = shallowRef(false)
  const branches = ref<ReactiveBranch[]>([])
  const unassignedCommits = ref<Commit[]>([])

  // Internal state
  const branchMap = new Map<string, ReactiveBranch>()

  // Main sync action
  async function syncBranches(options?: SyncOptions) {
    isSyncing.value = true
    syncError.value = null

    try {
      // Wait for any pending branch prefix loading
      if (loadingPromise.value) {
        try {
          await Promise.race([
            loadingPromise.value,
            new Promise((_, reject) => setTimeout(() => reject(new UserError("Timeout waiting for branch prefix configuration")), 5000)),
          ])
        }
        catch (error) {
          console.error("Error loading branch prefix:", error)
        }
      }

      // Create SSE channel
      const channel = new Channel<SyncEvent>()

      // Handle events
      channel.onmessage = (event) => {
        handleSyncEvent(event, options)
      }

      // Start sync
      const vcsRequest = vcsRequestFactory.createRequest()
      await commands.syncBranches(vcsRequest, channel)

      // Update last sync time
      const now = Date.now()
      if (selectedProject.value) {
        selectedProject.value.lastSyncTime = now
        selectedProject.value.lastBranchCount = branches.value.length
      }
    }
    catch (error) {
      syncError.value = (error instanceof Error ? error.message : String(error)) || "Failed to sync branches"
      console.error("Sync error:", error)
    }
    finally {
      isSyncing.value = false
    }
  }

  // Handle sync events
  function handleSyncEvent(event: SyncEvent, options?: SyncOptions) {
    switch (event.type) {
      case "branchesGrouped":
        handleBranchesGroupedEvent(event.data, branchMap, branches.value)
        // Trigger reactivity by reassigning the array
        branches.value = [...branches.value]
        break
      case "commitSynced":
        handleCommitSyncedEvent(event.data, branchMap)
        break
      case "commitError":
        handleCommitErrorEvent(event.data, branchMap, options)
        break
      case "branchStatusUpdate":
        handleBranchStatusUpdateEvent(event.data, branchMap, options)
        break
      case "unassignedCommits":
        handleUnassignedCommitsEvent(event.data)
        break
      case "completed":
        handleCompletedEvent()
        break
    }
  }

  // Event handler for BranchesGrouped events
  function handleBranchesGroupedEvent(
    data: Extract<SyncEvent, { type: "branchesGrouped" }>["data"],
    branchMap: Map<string, ReactiveBranch>,
    branchArray: ReactiveBranch[],
  ) {
    const newBranchNames = new Set<string>()

    for (const branch of data.branches) {
      newBranchNames.add(branch.name)

      let branchItem = branchMap.get(branch.name)
      if (!branchItem) {
        branchItem = reactive({
          name: branch.name,
          commits: [],
          commitMap: new Map(),
          commitCount: branch.commits.length,
          status: "Syncing" as const,
          statusText: "syncing…",
          processedCount: 0,
          hasError: false,
          autoExpandRequested: false,
          autoScrollRequested: false,
          latestCommitTime: branch.latestCommitTime,
        })
        branchMap.set(branch.name, branchItem)
        branchArray.push(branchItem)

        // Add commits to new branch
        for (const commit of branch.commits) {
          const syncedCommit = markRaw(commit as SyncedCommit)
          branchItem.commitMap.set(commit.originalHash, syncedCommit)
          branchItem.commits.push(syncedCommit)
        }
      }
      else {
        resetBranch(branchItem, branch)
      }
    }

    // Remove branches that no longer exist
    const hasRemovedBranches = branchArray.length > newBranchNames.size
    if (hasRemovedBranches) {
      for (let i = branchArray.length - 1; i >= 0; i--) {
        if (!newBranchNames.has(branchArray[i]!.name)) {
          branchArray.splice(i, 1)
        }
      }
    }
  }

  // Event handler for CommitSynced events
  function handleCommitSyncedEvent(
    data: Extract<SyncEvent, { type: "commitSynced" }>["data"],
    branchMap: Map<string, ReactiveBranch>,
  ) {
    const { branchName, commitHash, newHash, status } = data
    const branch = branchMap.get(branchName)
    const commit = branch?.commitMap.get(commitHash)
    if (branch && commit) {
      commit.hash = newHash
      commit.status = status
      // increment processed count for this branch
      branch.processedCount++
    }
  }

  // Event handler for CommitError events
  function handleCommitErrorEvent(
    data: Extract<SyncEvent, { type: "commitError" }>["data"],
    branchMap: Map<string, ReactiveBranch>,
    options?: SyncOptions,
  ) {
    const { branchName, commitHash, error } = data
    const branch = branchMap.get(branchName)
    const commit = branch?.commitMap.get(commitHash)

    if (branch) {
      // increment processed count even for errors
      branch.processedCount++

      // Set error on commit if found
      if (commit) {
        commit.status = "Error"
        commit.error = markRaw(error)
      }

      // Always set error details on branch
      if ("MergeConflict" in error) {
        // update branch status and error info
        branch.statusText = "merge conflict"
      }
      else {
        branch.statusText = error.Generic
      }

      branch.status = "Error"
      branch.hasError = true
      branch.errorDetails = error // Set error details on branch for display

      // Set auto-expand request on error if enabled
      const autoExpand = options?.autoExpand ?? true
      if (autoExpand) {
        branch.autoExpandRequested = true
        branch.autoScrollRequested = options?.autoScroll ?? true
      }
    }
  }

  // Event handler for BranchStatusUpdate events
  function handleBranchStatusUpdateEvent(
    data: Extract<SyncEvent, { type: "branchStatusUpdate" }>["data"],
    branchMap: Map<string, ReactiveBranch>,
    options?: SyncOptions,
  ) {
    const { branchName, status, error } = data
    const branch = branchMap.get(branchName)
    if (branch) {
      // Store the status in the branch data
      branch.status = status

      // Store error details if provided
      if (error) {
        branch.errorDetails = error
        branch.hasError = true
      }

      // Format status text for display
      switch (status) {
        case "MergeConflict":
          branch.statusText = "merge conflict"
          branch.hasError = true
          break
        case "AnalyzingConflict":
          branch.statusText = "analyzing conflict…"
          break
        case "Error":
          branch.statusText = "internal error"
          branch.hasError = true
          break
        default:
          branch.statusText = status.toLowerCase()
      }

      // Set auto-expand request for branches with errors, conflicts, or meaningful changes
      const autoExpand = options?.autoExpand ?? true
      if (autoExpand) {
        // If targetBranchName is specified, only expand that specific branch
        if (options?.targetBranchName) {
          if (branchName === options.targetBranchName && (status === "Created" || status === "Updated")) {
            branch.autoExpandRequested = true
            branch.autoScrollRequested = options?.autoScroll ?? true
          }
        }
        else {
          // Default behavior: expand on errors, conflicts, and meaningful changes
          if (status === "Error" || status === "MergeConflict" || status === "Created" || status === "Updated") {
            branch.autoExpandRequested = true
            branch.autoScrollRequested = options?.autoScroll ?? true
          }
        }
      }
    }
  }

  // Event handler for UnassignedCommits events
  function handleUnassignedCommitsEvent(
    data: Extract<SyncEvent, { type: "unassignedCommits" }>["data"],
  ) {
    unassignedCommits.value = data.commits
  }

  // Event handler for Completed events
  function handleCompletedEvent() {
    hasCompletedSync.value = true
  }

  // Reset branch data
  function resetBranch(branchItem: ReactiveBranch, branch: GroupedBranchInfo) {
    branchItem.commitCount = branch.commits.length
    branchItem.processedCount = 0
    branchItem.status = "Syncing"
    branchItem.statusText = "syncing…"
    branchItem.hasError = false
    branchItem.errorDetails = undefined
    branchItem.autoExpandRequested = false
    branchItem.autoScrollRequested = false
    branchItem.latestCommitTime = branch.latestCommitTime

    // Clear existing commits and add new ones
    branchItem.commitMap.clear()
    branchItem.commits = []
    for (const commit of branch.commits) {
      const syncedCommit = markRaw(commit as SyncedCommit)
      branchItem.commitMap.set(commit.originalHash, syncedCommit)
      branchItem.commits.push(syncedCommit)
    }
  }

  return {
    // State
    syncError: readonly(syncError),
    isSyncing: readonly(isSyncing),
    hasCompletedSync: readonly(hasCompletedSync),
    branches,
    unassignedCommits,

    // Actions
    syncBranches,
  }
}

// Composable to inject branch sync
export function useBranchSync() {
  const branchSync = inject(BranchSyncKey)
  if (!branchSync) {
    throw new Error("Branch sync state not provided. Make sure BranchCreator component is in the component tree.")
  }
  return branchSync as ReturnType<typeof createBranchSyncState>
}