import { useMemo } from "react"
import { Card } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { getConflictMarkerCommits } from "@/components/conflict/conflict-marker-commits"
import { ConflictCommitContext } from "@/components/conflict/ConflictCommitContext"
import { ConflictResolutionHint } from "@/components/conflict/ConflictResolutionHint"
import { ConflictingFileListCard } from "@/components/conflict/ConflictingFileListCard"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import type { ConflictingFilesWindowData } from "@/types/sub-window"

export function ConflictingFilesPage() {
  const conflictData = useSubWindowData<ConflictingFilesWindowData>()
  const conflictMarkerCommits = useMemo(() => getConflictMarkerCommits(conflictData?.conflict), [conflictData])

  return (
    <div className="w-full px-4 py-4 sm:px-6 lg:px-8">
      {conflictData
        ? (
            <div className="flex flex-col gap-4">
              {/* Commit context */}
              <Card className="gap-0 py-0">
                <div className="flex flex-col gap-4 p-4 sm:px-6">
                  <ConflictCommitContext
                    title="Cannot apply commit"
                    commitMessage={conflictData.conflict.commit.message}
                    commitHash={conflictData.conflict.commit.hash}
                    commitAuthorTime={conflictData.conflict.commit.authorTime}
                    commitCommitterTime={conflictData.conflict.commit.committerTime}
                    branchName={conflictData.branchName}
                  />
                  <ConflictResolutionHint />
                </div>
              </Card>

              {/* Conflicting files (main content) */}
              <ConflictingFileListCard
                conflicts={conflictData.conflict.conflictingFiles}
                conflictInfo={conflictData.conflict}
                conflictMarkerCommits={conflictMarkerCommits}
                branchName={conflictData.branchName}
                isInWindow
              />
            </div>
          )
        : <LoadingState message="Loading conflicting files…" />}
    </div>
  )
}

function LoadingState({ message }: { message: string }) {
  return (
    <p className="flex items-center gap-2 text-sm text-muted-foreground">
      <Spinner className="size-4" />
      {message}
    </p>
  )
}
