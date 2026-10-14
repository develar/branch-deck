import { useEffect, useMemo } from "react"
import type { CommitFileStats } from "@/lib/commit-files"
import { getCommitFileStats } from "@/lib/commit-files"
import { loadCommitFiles, useFilesOfCommit } from "@/stores/commit-files"

/**
 * Loads the files of the commits of a shown list (an expanded branch, the unassigned commits, a branch preview): one
 * call when the list mounts or gets other commits, only for the commits not known yet. The rows read them with
 * `useCommitFileStats`.
 */
export function useCommitFiles(commitIds: readonly string[]): void {
  // one string, so a new array of the same ids (a sync) loads nothing
  const key = commitIds.join("\n")
  useEffect(() => {
    if (key !== "") {
      void loadCommitFiles(key.split("\n"))
    }
  }, [key])
}

/** "3 files · +42 −7" of the meta line, once the files of the commit are loaded */
export function useCommitFileStats(commitId: string): CommitFileStats | undefined {
  const files = useFilesOfCommit(commitId)
  return useMemo(() => files === undefined ? undefined : getCommitFileStats(files), [files])
}
