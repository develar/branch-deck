import { shallowRef } from "vue"
import { UserError, VcsRequestFactory } from "./vcsRequest"
import { commands, Result, SyncBranchResult } from "../bindings"

export function useSyncBranches(vcsRequestFactory: VcsRequestFactory) {
  const result = shallowRef<Result<SyncBranchResult, string> | null>(null)
  const isSyncing = shallowRef(false)

  const createBranches = async () => {
    isSyncing.value = true
    try {
      const request = vcsRequestFactory.createRequest()
      result.value = await commands.syncBranches(request.repositoryPath, request.branchPrefix)
    }
    catch (error) {
      result.value = {
        status: "error",
        error: error instanceof UserError ? error.message : `Failed to sync branches: ${error}`,
      }
    }
    finally {
      isSyncing.value = false
    }
  }
  return { createBranches, syncResult: result, isSyncing }
}
