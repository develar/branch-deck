import { useMemo } from "react"
import { getConflictMarkerCommits } from "@/components/conflict/conflict-marker-commits"
import { ConflictingFileListCard } from "@/components/conflict/ConflictingFileListCard"
import { ConflictWindowPage } from "@/components/conflict/ConflictWindowPage"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import type { ConflictingFilesWindowData } from "@/types/sub-window"

export function ConflictingFilesPage() {
  const conflictData = useSubWindowData<ConflictingFilesWindowData>()
  const conflictMarkerCommits = useMemo(() => getConflictMarkerCommits(conflictData?.conflict), [conflictData])

  return (
    <ConflictWindowPage
      commit={conflictData?.conflict.commit ?? null}
      branchName={conflictData?.branchName}
      onto={conflictData?.onto}
      loadingMessage="Loading conflicting files…"
    >
      {conflictData && (
        <ConflictingFileListCard
          conflicts={conflictData.conflict.conflictingFiles}
          conflictInfo={conflictData.conflict}
          conflictMarkerCommits={conflictMarkerCommits}
          branchName={conflictData.branchName}
          isInWindow
        />
      )}
    </ConflictWindowPage>
  )
}
