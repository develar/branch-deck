import { commands } from "@/lib/bindings"
import type { TrimSummary } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { showToast } from "@/lib/toast-message"
import { trimToast } from "@/presenters/disk"

/**
 * Trim to Limit of the Disk Usage dialog (`trim_pools`): the free slots past the limit of every pool of this Mac go,
 * with their Bazel output bases; a toast says what it freed. Null when it failed, its toast said why
 */
export async function trimPools(repositoryPath: string): Promise<TrimSummary | null> {
  const result = await commands.trimPools({ repositoryPath })
  if (result.status === "error") {
    notifyError("Cannot trim the lease pools", result.error)
    return null
  }
  showToast(trimToast(result.data))
  return result.data
}
