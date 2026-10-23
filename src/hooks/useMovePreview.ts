import { useEffect, useState } from "react"
import { Channel } from "@tauri-apps/api/core"
import type { Commit, LockWaitEvent, MoveCommitsResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { useBranchSync } from "@/stores/branch-sync"
import { useSelectedProjectPath } from "@/stores/repository"

/** Waits for the typing to pause before the dry run */
const PREVIEW_DELAY_MS = 150

export interface MovePreview {
  /** What the move would do; null before the first dry run and after an error */
  result: MoveCommitsResult | null
  /** Why the move is refused (an invalid plan: a fixup separated from its target, a stack problem) */
  error: string | null
}

const NO_PREVIEW: MovePreview = { result: null, error: null }

/**
 * A dry run of moving `commits` to `targetBranch` while `enabled` (a valid name), debounced and repeated after every
 * sync: the "Move to…" form says what the move does from it, the issues that go by their key and the commits that get
 * a prefix, before anything is written.
 */
export function useMovePreview(commits: readonly Commit[], targetBranch: string, enabled: boolean): MovePreview {
  const repositoryPath = useSelectedProjectPath() ?? ""
  const syncCount = useBranchSync(state => state.syncCount)
  const commitKey = commits.map(commit => commit.originalHash).join(",")
  // a sync repeats the dry run, the last result stays until the new one is there
  const requestKey = enabled && commitKey !== "" ? `${repositoryPath}\n${targetBranch}\n${commitKey}` : null
  const [preview, setPreview] = useState<{ key: string; preview: MovePreview } | null>(null)

  useEffect(() => {
    if (requestKey === null) {
      return
    }
    let isCurrent = true
    const timer = setTimeout(() => {
      void invokeCommand(() =>
        commands.moveCommitsToBranch(
          {
            repositoryPath,
            commitIds: commitKey.split(","),
            targetBranch,
            dryRun: true,
          },
          // a dry run takes no lock: nothing waits, nobody listens
          new Channel<LockWaitEvent>(),
        ),
      ).then(result => {
        if (isCurrent) {
          setPreview({ key: requestKey, preview: result.status === "ok" ? { result: result.data, error: null } : { result: null, error: result.error.message } })
        }
      })
    }, PREVIEW_DELAY_MS)
    return () => {
      isCurrent = false
      clearTimeout(timer)
    }
  }, [requestKey, repositoryPath, targetBranch, commitKey, syncCount])

  return requestKey !== null && preview?.key === requestKey ? preview.preview : NO_PREVIEW
}
