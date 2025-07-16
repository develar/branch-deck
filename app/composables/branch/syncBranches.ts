import { shallowRef } from "vue"
import { useTimeoutFn } from "@vueuse/core"
import { UserError } from "../git/vcsRequest"
import type { VcsRequestFactory } from "../git/vcsRequest"
import { commands } from "~/utils/bindings"
import type { SyncEvent, CommitDetail, GroupedBranchInfo, BranchSyncStatus, BranchError } from "~/utils/bindings"
import { Channel } from "@tauri-apps/api/core"

// Sync options interface
export interface SyncOptions {
  autoScroll?: boolean
  autoExpand?: boolean
  targetBranchName?: string
}

// Reactive branch data that updates incrementally
export interface ReactiveBranch {
  name: string
  commits: Map<string, CommitDetail>
  commitCount: number
  status: "Syncing" | BranchSyncStatus
  statusText: string
  processedCount: number
  hasError: boolean
  errorDetails?: BranchError
}

export function useSyncBranches(vcsRequestFactory: VcsRequestFactory, expandBranch: (branchName: string, scroll: boolean) => void) {
  const syncError = shallowRef<string | null>(null)
  const isSyncing = shallowRef(false)
  const showProgress = shallowRef(false)
  const syncProgress = shallowRef("")
  const hasCompletedSync = shallowRef(false)

  // Reactive incremental data
  const branchMap = new Map<string, ReactiveBranch>()

  // Reactive array that will be mutated directly
  const branchArray = ref<ReactiveBranch[]>([])

  // Unassigned commits (no prefix)
  const unassignedCommits = ref<CommitDetail[]>([])

  const { start: startProgressTimer, stop: stopProgressTimer } = useTimeoutFn(
    () => {
      showProgress.value = true
    },
    300,
    { immediate: false },
  )

  const syncBranches = async (options?: SyncOptions) => {
    isSyncing.value = true
    showProgress.value = false
    syncProgress.value = ""
    syncError.value = null
    unassignedCommits.value = []

    // track messages by index
    const messagesByIndex = new Map<number, string>()

    // Start the timer to show progress after 300ms
    startProgressTimer()

    try {
      const onProgress = new Channel<SyncEvent>()
      onProgress.onmessage = (event) => {
        switch (event.type) {
          case "progress":
            handleProgressEvent(event.data, messagesByIndex, syncProgress)
            break

          case "branchesGrouped":
            handleBranchesGroupedEvent(event.data, branchMap, branchArray)
            break

          case "commitSynced":
            handleCommitSyncedEvent(event.data, branchMap)
            break

          case "commitError":
            handleCommitErrorEvent(event.data, branchMap, expandBranch, options)
            break

          case "commitsBlocked":
            handleCommitsBlockedEvent(event.data, branchMap)
            break

          case "branchStatusUpdate":
            handleBranchStatusUpdateEvent(event.data, branchMap, expandBranch, options)
            break

          case "unassignedCommits":
            handleUnassignedCommitsEvent(event.data, unassignedCommits)
            break

          case "completed":
            handleCompletedEvent(syncProgress, hasCompletedSync)
            break
        }
      }

      const request = vcsRequestFactory.createRequest()
      const result = await commands.syncBranches(request.repositoryPath, request.branchPrefix, onProgress)
      if (result.status === "error") {
        syncError.value = result.error
      }
    }
    catch (error) {
      syncError.value = error instanceof UserError ? error.message : `Failed to sync branches: ${error instanceof Error ? error.message : String(error)}`
    }
    finally {
      stopProgressTimer()
      isSyncing.value = false
      showProgress.value = false
    }
  }

  return {
    syncBranches,
    syncError,
    isSyncing,
    showProgress,
    syncProgress,
    branches: branchArray,
    unassignedCommits,
    hasCompletedSync,
  }
}

function resetBranch(branchItem: ReactiveBranch, commitMap: Map<string, CommitDetail>, branch: GroupedBranchInfo) {
  branchItem.commits = commitMap
  branchItem.commitCount = branch.commits.length
  branchItem.status = "Syncing"
  branchItem.statusText = "syncing"
  branchItem.processedCount = 0
  branchItem.hasError = false
  branchItem.errorDetails = undefined
}

// Event handler for Progress events
function handleProgressEvent(
  data: Extract<SyncEvent, { type: "progress" }>["data"],
  messagesByIndex: Map<number, string>,
  syncProgress: Ref<string>,
) {
  const { message, index } = data
  if (message.length === 0) {
    // clear message for this specific task index
    messagesByIndex.delete(index)
  }
  else {
    // update message for this task index
    messagesByIndex.set(index, message)
  }

  // combine all messages with | separator
  let combined = ""
  for (const [idx, msg] of messagesByIndex) {
    if ((idx !== -1 || messagesByIndex.size === 1)) {
      combined += (combined ? " | " : "") + msg
    }
  }
  syncProgress.value = combined
}

// Event handler for BranchesGrouped events
function handleBranchesGroupedEvent(
  data: Extract<SyncEvent, { type: "branchesGrouped" }>["data"],
  branchMap: Map<string, ReactiveBranch>,
  branchArray: Ref<ReactiveBranch[]>,
) {
  // If no branches, clear everything
  if (data.branches.length === 0) {
    branchMap.clear()
    branchArray.value.splice(0)
    return
  }

  // Initialize branch structure with all commits
  const newBranchNames = new Set<string>()
  for (const branch of data.branches) {
    newBranchNames.add(branch.name)
    const commitMap = new Map<string, CommitDetail>()
    for (const commit of branch.commits) {
      commitMap.set(commit.originalHash, commit)
    }

    let branchItem = branchMap.get(branch.name)
    if (branchItem == null) {
      // Create new branch object only if it doesn't exist
      branchItem = reactive({
        name: branch.name,
      } as ReactiveBranch)
      branchMap.set(branch.name, branchItem)
      resetBranch(branchItem, commitMap, branch)
      branchArray.value.push(branchItem)
    }
    else {
      resetBranch(branchItem, commitMap, branch)
    }
  }

  // remove branches that are no longer present
  let hasRemovedBranches = false
  for (const branchName of branchMap.keys()) {
    if (!newBranchNames.has(branchName)) {
      hasRemovedBranches = true
      branchMap.delete(branchName)
    }
  }

  // Update array if branches were removed
  if (hasRemovedBranches) {
    for (let i = branchArray.value.length - 1; i >= 0; i--) {
      if (!newBranchNames.has(branchArray.value[i]!.name)) {
        branchArray.value.splice(i, 1)
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
  const commit = branch?.commits.get(commitHash)
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
  expandBranch: (branchName: string, scroll: boolean) => void,
  options?: SyncOptions,
) {
  const { branchName, commitHash, error } = data
  const branch = branchMap.get(branchName)
  const commit = branch?.commits.get(commitHash)
  if (branch && commit) {
    // increment processed count even for errors
    branch.processedCount++

    commit.status = "Error"
    commit.error = markRaw(error)
    if ("MergeConflict" in error) {
      // update branch status and error info
      branch.statusText = "merge conflict"
    }
    else {
      branch.statusText = error.Generic
    }

    branch.status = "Error"
    branch.hasError = true
    // auto-expand branch on error if enabled
    const autoExpand = options?.autoExpand ?? true
    if (autoExpand) {
      expandBranch(branchName, options?.autoScroll ?? true)
    }
  }
}

// Event handler for CommitsBlocked events
function handleCommitsBlockedEvent(
  data: Extract<SyncEvent, { type: "commitsBlocked" }>["data"],
  branchMap: Map<string, ReactiveBranch>,
) {
  const { branchName, blockedCommitHashes } = data
  const branch = branchMap.get(branchName)
  if (branch) {
    for (const hash of blockedCommitHashes) {
      const commit = branch.commits.get(hash)
      if (commit) {
        commit.status = "Blocked"
      }
    }
  }
}

// Event handler for BranchStatusUpdate events
function handleBranchStatusUpdateEvent(
  data: Extract<SyncEvent, { type: "branchStatusUpdate" }>["data"],
  branchMap: Map<string, ReactiveBranch>,
  expandBranch: (branchName: string, scroll: boolean) => void,
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

    // auto-expand branches with errors, conflicts, or meaningful changes
    const autoExpand = options?.autoExpand ?? true
    if (autoExpand) {
      // If targetBranchName is specified, only expand that specific branch
      if (options?.targetBranchName) {
        if (branchName === options.targetBranchName && (status === "Created" || status === "Updated")) {
          expandBranch(branchName, options?.autoScroll ?? true)
        }
      }
      else {
        // Default behavior: expand on errors, conflicts, and meaningful changes
        if (status === "Error" || status === "MergeConflict" || status === "Created" || status === "Updated") {
          expandBranch(branchName, options?.autoScroll ?? true)
        }
      }
    }
  }
}

// Event handler for UnassignedCommits events
function handleUnassignedCommitsEvent(
  data: Extract<SyncEvent, { type: "unassignedCommits" }>["data"],
  unassignedCommits: Ref<CommitDetail[]>,
) {
  unassignedCommits.value = data.commits
}

// Event handler for Completed events
function handleCompletedEvent(
  syncProgress: Ref<string>,
  hasCompletedSync: Ref<boolean>,
) {
  syncProgress.value = "Sync completed"
  hasCompletedSync.value = true
}
