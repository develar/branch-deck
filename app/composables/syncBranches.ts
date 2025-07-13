import { shallowRef } from "vue"
import { useTimeoutFn } from "@vueuse/core"
import { UserError } from "./vcsRequest"
import type { VcsRequestFactory } from "./vcsRequest"
import { commands } from "~/utils/bindings"
import type { SyncEvent, CommitDetail } from "~/utils/bindings"
import { Channel } from "@tauri-apps/api/core"

// Reactive branch data that updates incrementally
export interface ReactiveBranch {
  name: string
  commits: Map<string, CommitDetail>
  commit_count: number
  status: "Syncing" | BranchSyncStatus
  statusText: string
  processedCount: number
  hasError: boolean
}

export function useSyncBranches(vcsRequestFactory: VcsRequestFactory, expandBranch: (branchName: string) => void) {
  const syncError = shallowRef<string | null>(null)
  const isSyncing = shallowRef(false)
  const showProgress = shallowRef(false)
  const syncProgress = shallowRef("")

  const expanded = ref<Record<string, boolean>>({})

  // Reactive incremental data
  const branchMap = new Map<string, ReactiveBranch>()

  // Reactive array that will be mutated directly
  const branchArray = ref<ReactiveBranch[]>([])

  const {start: startProgressTimer, stop: stopProgressTimer} = useTimeoutFn(
    () => {
      showProgress.value = true
    },
    300,
    {immediate: false},
  )

  const createBranches = async () => {
    isSyncing.value = true
    showProgress.value = false
    syncProgress.value = ""
    syncError.value = null

    // Track messages by index
    const messagesByIndex = new Map<number, string>()

    // Start the timer to show progress after 300ms
    startProgressTimer()

    try {
      const onProgress = new Channel<SyncEvent>()
      onProgress.onmessage = (event) => {
        switch (event.type) {
          case "Progress": {
            const {message, index} = event.data
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
            break
          }

          case "BranchesGrouped": {
            // Initialize branch structure with all commits
            const newBranchNames = new Set<string>()
            for (const branch of event.data.branches) {
              newBranchNames.add(branch.name)
              const commitMap = new Map<string, CommitDetail>()
              for (const commit of branch.commits) {
                commitMap.set(commit.original_hash, commit)
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
                // eslint-disable-next-line @typescript-eslint/no-dynamic-delete
                delete expanded.value[branchName]
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
            break
          }

          case "CommitSynced": {
            // update commit with the new hash and status
            const {branch_name, commit_hash, new_hash, status} = event.data
            const branch = branchMap.get(branch_name)
            const commit = branch?.commits.get(commit_hash)
            if (branch && commit) {
              commit.hash = new_hash
              commit.status = status
              // increment processed count for this branch
              branch.processedCount++
            }
            break
          }

          case "CommitError": {
            // update commit status to error
            const {branch_name, commit_hash, error} = event.data
            const branch = branchMap.get(branch_name)
            const commit = branch?.commits.get(commit_hash)
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
              // auto-expand branch on error
              expanded.value[branch_name] = true
              expandBranch(branch_name)
            }
            break
          }

          case "CommitsBlocked": {
            // Mark remaining commits as blocked
            const {branch_name, blocked_commit_hashes} = event.data
            const branch = branchMap.get(branch_name)
            if (branch) {
              for (const hash of blocked_commit_hashes) {
                const commit = branch.commits.get(hash)
                if (commit) {
                  commit.status = "Blocked"
                }
              }
            }
            break
          }

          case "BranchStatusUpdate": {
            // Branch status update (during processing or completion)
            const {branch_name, status} = event.data
            const branch = branchMap.get(branch_name)
            if (branch) {
              // Store the status in the branch data
              branch.status = status
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
                  branch.statusText = "error"
                  branch.hasError = true
                  break
                default:
                  branch.statusText = status
              }

              // Auto-expand branches with errors, conflicts, or meaningful changes
              if (status === "Error" || status === "MergeConflict" || status === "Created" || status === "Updated") {
                expanded.value[branch_name] = true
                expandBranch(branch_name)
              }
            }
            break
          }

          case "Completed": {
            // Sync completed
            syncProgress.value = "Sync completed"
            break
          }
        }
      }

      const request = vcsRequestFactory.createRequest()
      const result = await commands.syncBranches(request.repositoryPath, request.branchPrefix, onProgress)
      if (result.status === "error") {
        syncError.value = result.error
      }
    }
    catch (error) {
      syncError.value = error instanceof UserError ? error.message : `Failed to sync branches: ${error}`
    }
    finally {
      stopProgressTimer()
      isSyncing.value = false
      showProgress.value = false
    }
  }

  return {
    createBranches,
    syncError,
    isSyncing,
    showProgress,
    syncProgress,
    branches: branchArray,
    expanded,
  }
}

function resetBranch(branchItem: ReactiveBranch, commitMap: Map<string, CommitDetail>, branch: GroupedBranchInfo) {
  branchItem.commits = commitMap
  branchItem.commit_count = branch.commits.length
  branchItem.status = "Syncing"
  branchItem.statusText = "syncing"
  branchItem.processedCount = 0
  branchItem.hasError = false
}