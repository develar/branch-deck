import { useMemo } from "react"
import { Card, CardHeader } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import type { ConflictMarkerCommitInfo } from "@/lib/bindings"
import type { ConflictingFilesWindowData } from "@/types/sub-window"

export function ConflictingFilesPage() {
  const conflictData = useSubWindowData<ConflictingFilesWindowData>()

  const conflictMarkerCommits = useMemo(() => {
    const result: Record<string, ConflictMarkerCommitInfo> = {}
    const commits = conflictData?.conflict.conflictMarkerCommits
    if (commits) {
      for (const [key, value] of Object.entries(commits)) {
        if (value) {
          result[key] = value
        }
      }
    }
    return result
  }, [conflictData])

  return (
    <div className="w-full px-4 py-4 sm:px-6 lg:px-8">
      {conflictData
        ? (
            <div className="flex flex-col gap-4">
              {/* Commit Context */}
              <Card className="bg-muted">
                <CardHeader>
                  {/* TODO(Phase 7): <ConflictCommitContext title="Cannot apply commit" commitMessage commitHash commitAuthorTime commitCommitterTime branchName /> */}
                  <div className="text-sm text-muted-foreground">
                    ConflictCommitContext (TODO Phase 7):
                    {" "}
                    {conflictData.conflict.commitMessage}
                    {" "}
                    on
                    {" "}
                    {conflictData.branchName}
                  </div>
                </CardHeader>
              </Card>

              {/* Conflicting Files Section (main content) */}
              {/* TODO(Phase 7): <ConflictingFileListCard conflicts conflictInfo conflictMarkerCommits branchName isInWindow /> */}
              <div className="text-sm text-muted-foreground">
                ConflictingFileListCard (TODO Phase 7):
                {" "}
                {conflictData.conflict.conflictingFiles.length}
                {" "}
                files,
                {" "}
                {Object.keys(conflictMarkerCommits).length}
                {" "}
                conflict marker commits
              </div>
            </div>
          )
        : <LoadingState message="Loading conflicting files data..." />}
    </div>
  )
}

function LoadingState({ message }: { message: string }) {
  return (
    <div className="flex min-h-[400px] items-center justify-center">
      <div className="flex flex-col items-center gap-3 text-center">
        <Spinner className="size-8 text-muted-foreground" />
        <p className="text-sm text-muted-foreground">{message}</p>
      </div>
    </div>
  )
}
