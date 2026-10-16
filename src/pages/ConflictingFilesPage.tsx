import { ConflictingFileListCard } from "@/components/conflict/ConflictingFileListCard"
import { ConflictWindowPage } from "@/components/conflict/ConflictWindowPage"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import type { ConflictingFilesWindowData } from "@/types/sub-window"

export function ConflictingFilesPage() {
  const conflictData = useSubWindowData<ConflictingFilesWindowData>()

  return (
    <ConflictWindowPage
      commit={conflictData?.conflict.commit ?? null}
      branchName={conflictData?.branchName}
      onto={conflictData?.onto}
      loadingMessage="Loading conflicting files…"
      fixSteps={conflictData?.fixSteps}
    >
      {conflictData && (
        <ConflictingFileListCard conflicts={conflictData.conflict.conflictingFiles} conflictInfo={conflictData.conflict} branchName={conflictData.branchName} isInWindow />
      )}
    </ConflictWindowPage>
  )
}
