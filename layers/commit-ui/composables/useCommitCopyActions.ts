import type { Commit, MissingCommit } from "~/utils/bindings"
import type { SyncedCommit } from "~/composables/branchSyncProvider"

// Union type for all supported commit types
type CommitUnion = Commit | SyncedCommit | MissingCommit

export interface CommitCopyActionsReturn {
  getCopyMenuItems: (commits: CommitUnion[]) => Array<{ label: string, icon: string, onSelect: () => void }>
}

export function useCommitCopyActions(): CommitCopyActionsReturn {
  const { copyToClipboard } = useCopyToClipboard()

  const getCopyMenuItems = (commits: CommitUnion[]) => {
    const isMultiple = commits.length > 1

    return [
      {
        label: isMultiple ? `Copy ${commits.length} Commit Hashes` : "Copy Commit Hash",
        icon: "i-lucide-copy",
        onSelect: () => {
          const hashes = commits.map((commit) => {
            // For Commit type, use originalHash first, then hash if available
            if ("originalHash" in commit && commit.originalHash) {
              return commit.originalHash
            }
            // For SyncedCommit and MissingCommit types, use hash
            if ("hash" in commit && commit.hash) {
              return commit.hash
            }
            return ""
          }).filter(Boolean)
          // noinspection JSIgnoredPromiseFromCall
          copyToClipboard(hashes.join("\n"))
        },
      },
      {
        label: isMultiple ? `Copy ${commits.length} Commit Messages` : "Copy Commit Message",
        icon: "i-lucide-copy",
        onSelect: () => {
          const messages = commits.map(commit => commit.message).filter(Boolean)
          // noinspection JSIgnoredPromiseFromCall
          copyToClipboard(messages.join("\n"))
        },
      },
    ]
  }

  return {
    getCopyMenuItems,
  }
}