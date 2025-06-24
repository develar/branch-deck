import { CreateVirtualBranches } from "../../wailsjs/go/main/App"
import { ref } from "vue"
import { backend } from "../../wailsjs/go/models"
import {UserError, VcsRequestFactory} from "./vcsRequest"

export const syncStatusToString = new Map([
  [backend.BranchSyncStatus.CREATED, "created"],
  [backend.BranchSyncStatus.UPDATED, "updated"],
  [backend.BranchSyncStatus.UNCHANGED, "unchanged"],
])

interface Result {
  readonly branches?: Array<backend.BranchResult>
  readonly success: boolean
  readonly message?: string
}

export function useSyncBranches(vcsRequestFactory: VcsRequestFactory) {
  const result = ref<Result>(null)
  const isSyncing = ref(false)

  const createBranches = async () => {
    isSyncing.value = true
    try {
      result.value = await CreateVirtualBranches(vcsRequestFactory.createRequest())
    } catch (error) {
      result.value = {
        success: false,
        message: error instanceof UserError ? error.message : `Failed to process repository: ${error.message || error}`,
      }
    } finally {
      isSyncing.value = false
    }
  }
  return { createBranches, result, isSyncing }
}
