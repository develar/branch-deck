import { ref, shallowRef } from "vue"
import { UserError, VcsRequestFactory } from "./vcsRequest"
import { commands, SyncEvent, Result, SyncBranchResult } from "../bindings"
import { Channel } from "@tauri-apps/api/core"

export function useSyncBranches(vcsRequestFactory: VcsRequestFactory) {
  const result = shallowRef<Result<SyncBranchResult, string> | null>(null)
  const isSyncing = shallowRef(false)
  const syncProgress = ref("")

  const createBranches = async () => {
    isSyncing.value = true
    try {
      const onProgress = new Channel<SyncEvent>()
      onProgress.onmessage = (message) => {
        if (message.event === "finished") {
          syncProgress.value = ""
        }
        else {
          syncProgress.value = message.data.message
        }
      }

      const request = vcsRequestFactory.createRequest()
      result.value = await commands.syncBranches(request.repositoryPath, request.branchPrefix, onProgress)
    } catch (error) {
      result.value = {
        status: "error",
        error: error instanceof UserError ? error.message : `Failed to sync branches: ${error}`,
      }
    } finally {
      isSyncing.value = false
    }
  }
  return {createBranches, syncResult: result, isSyncing, syncProgress}
}
