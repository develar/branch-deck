import type { BranchError, BranchSyncStatus, Commit, CommitSyncStatus, GroupedBranchInfo, SyncEvent } from "~/utils/bindings"
import { commands } from "~/utils/bindings"
import { Channel } from "@tauri-apps/api/core"
import { UserError } from "~/composables/git/vcsRequest"
import type { createRepositoryState } from "~/composables/repositoryProvider"
import { createReactiveIndexedCollection } from "~/utils/reactiveIndexedCollection"

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

// Remote status information for a branch
export interface RemoteStatus {
  exists: boolean
  head?: string
  unpushedCommits: string[]
  commitsAhead: number
  commitsBehind: number
  myCommitsAhead: number
}

// Reactive branch data that updates incrementally
export interface ReactiveBranch {
  name: string // immutable
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
  summary: string
  // Remote tracking information (null = not yet loaded)
  remoteStatus: RemoteStatus | null
  // Push state
  isPushing: boolean
}

// Create branch sync state
export function createBranchSyncState(repository: ReturnType<typeof createRepositoryState>) {
  const { loadingPromise, vcsRequestFactory, selectedProject } = repository
  // State - using explicit refs
  const syncError = shallowRef<string | null>(null)
  const isSyncing = shallowRef(false)
  const hasCompletedSync = shallowRef(false)
  const branchCollection = createReactiveIndexedCollection<string, ReactiveBranch>()
  const branches = branchCollection.array // Expose array for UI
  const unassignedCommits = ref<Commit[]>([])

  // Archived branches state
  const archivedBranches = createArchivedBranchesState(repository)

  // Main sync action
  async function syncBranches(options?: SyncOptions) {
    isSyncing.value = true
    syncError.value = null

    // Note: Don't clear archived branches - use reconcile to preserve component state

    try {
      // Wait for any pending branch prefix loading
      if (loadingPromise.value) {
        await Promise.race([
          loadingPromise.value,
          new Promise((_, reject) => setTimeout(() => reject(new UserError("Timeout waiting for branch prefix configuration")), 5000)),
        ])
      }

      const channel = new Channel<SyncEvent>()

      // Handle events
      channel.onmessage = (event) => {
        handleSyncEvent(event, options)
      }

      // Start sync
      const vcsRequest = vcsRequestFactory.createRequest()
      const result = await commands.syncBranches(vcsRequest, channel)

      // Check if the command returned an error via Result type
      if (result.status === "error") {
        syncError.value = result.error || "Failed to sync branches"
        console.error("Sync error:", result.error)
        return
      }

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
      case "issueNavigationConfig":
        handleIssueNavigationConfigEvent(event.data)
        break
      case "branchesGrouped":
        handleBranchesGroupedEvent(event.data)
        break
      case "commitSynced":
        handleCommitSyncedEvent(event.data)
        break
      case "commitError":
        handleCommitErrorEvent(event.data, options)
        break
      case "branchStatusUpdate":
        handleBranchStatusUpdateEvent(event.data, options)
        break
      case "unassignedCommits":
        handleUnassignedCommitsEvent(event.data)
        break
      case "completed":
        handleCompletedEvent()
        break
      case "archivedBranchesFound":
        archivedBranches.updateFromArchivedNames(event.data.branchNames)
        break
      case "branchIntegrationDetected":
        archivedBranches.updateFromIntegrationInfo(event.data.info)
        break
      case "remoteStatusUpdate":
        handleRemoteStatusUpdateEvent(event.data)
        break
    }
  }

  // Event handler for BranchesGrouped events
  function handleBranchesGroupedEvent(
    data: Extract<SyncEvent, { type: "branchesGrouped" }>["data"],
  ) {
    const branchDataMap = new Map(data.branches.map(branch => [branch.name, branch]))

    branchCollection.reconcile(
      new Set(data.branches.map(branch => branch.name)),
      (name) => {
        const branch = branchDataMap.get(name)!
        const branchItem = reactive({
          name: branch.name,
          commits: [] as SyncedCommit[],
          commitMap: new Map<string, SyncedCommit>(),
          commitCount: branch.commits?.length ?? 0,
          status: "Syncing" as const,
          statusText: "syncing…",
          processedCount: 0,
          hasError: false,
          errorDetails: undefined as BranchError | undefined,
          autoExpandRequested: false,
          autoScrollRequested: false,
          latestCommitTime: branch.latestCommitTime,
          summary: branch.summary,
          // Remote tracking (null = not yet loaded)
          remoteStatus: null as RemoteStatus | null,
          // Push state
          isPushing: false,
        }) as ReactiveBranch

        // Add commits to new branch
        for (const commit of branch.commits) {
          const syncedCommit = markRaw(commit as SyncedCommit)
          branchItem.commitMap.set(commit.originalHash, syncedCommit)
          branchItem.commits.push(syncedCommit)
        }

        return branchItem
      },
      (_, existing, branchData) => {
        resetBranch(existing, branchData!)
      },
      branchDataMap,
    )
  }

  // Event handler for CommitSynced events
  function handleCommitSyncedEvent(
    data: Extract<SyncEvent, { type: "commitSynced" }>["data"],
  ) {
    const { branchName, commitHash, newHash, status } = data
    const branch = branchCollection.get(branchName)
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
    options?: SyncOptions,
  ) {
    const { branchName, commitHash, error } = data
    const branch = branchCollection.get(branchName)
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
    options?: SyncOptions,
  ) {
    const { branchName, status, error } = data
    const branch = branchCollection.get(branchName)
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

  // Event handler for IssueNavigationConfig events
  function handleIssueNavigationConfigEvent(
    data: Extract<SyncEvent, { type: "issueNavigationConfig" }>["data"],
  ) {
    if (selectedProject.value && data.config) {
      selectedProject.value.issueNavigationConfig = data.config
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

  // Event handler for RemoteStatusUpdate events
  function handleRemoteStatusUpdateEvent(
    data: Extract<SyncEvent, { type: "remoteStatusUpdate" }>["data"],
  ) {
    const { branchName, remoteExists, remoteHead, unpushedCommits, commitsBehind, myUnpushedCount } = data
    const branch = branchCollection.get(branchName)
    if (branch) {
      branch.remoteStatus = {
        exists: remoteExists,
        head: remoteHead ?? undefined, // Convert null to undefined
        unpushedCommits,
        commitsAhead: unpushedCommits.length,
        commitsBehind,
        myCommitsAhead: myUnpushedCount ?? 0,
      }
    }
  }

  // Event handler for ArchivedBranchesFound events
  // Event handler for single OrphanedBranchDetected events
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
    branchItem.summary = branch.summary
    // Reset remote tracking info (null = not yet loaded)
    branchItem.remoteStatus = null
    // Reset push state
    branchItem.isPushing = false

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

    // Archived branches (new composable)
    archivedBranches,
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
