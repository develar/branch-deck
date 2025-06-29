import { ref, shallowRef } from "vue"
import { useTimeoutFn } from "@vueuse/core"
import { UserError, VcsRequestFactory } from "./vcsRequest"
import { commands, SyncEvent, Result, SyncBranchResult } from "../bindings"
import { Channel } from "@tauri-apps/api/core"

export function useSyncBranches(vcsRequestFactory: VcsRequestFactory) {
  const result = shallowRef<Result<SyncBranchResult, string> | null>(null)
  const isSyncing = shallowRef(false)
  const showProgress = shallowRef(false)
  const syncProgress = ref("")

  const { start: startProgressTimer, stop: stopProgressTimer } = useTimeoutFn(
    () => {
      showProgress.value = true
    },
    300,
    { immediate: false },
  )

  const createBranches = async () => {
    isSyncing.value = true
    showProgress.value = false
    syncProgress.value = ""

    // Start the timer to show progress after 300ms
    startProgressTimer()

    try {
      const onProgress = new Channel<SyncEvent>()
      onProgress.onmessage = (message) => {
        syncProgress.value = message.message
      }

      const request = vcsRequestFactory.createRequest()
      result.value = await commands.syncBranches(request.repositoryPath, request.branchPrefix, onProgress)
    }
    catch (error) {
      result.value = {
        status: "error",
        error: error instanceof UserError ? error.message : `Failed to sync branches: ${error}`,
      }
    }
    finally {
      stopProgressTimer()
      isSyncing.value = false
      showProgress.value = false
    }
  }
  return { createBranches, syncResult: result, isSyncing, showProgress, syncProgress }
}
