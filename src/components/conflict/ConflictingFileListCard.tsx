import { ExternalLinkIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { ConflictingFilesSection } from "@/components/conflict/ConflictingFilesSection"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import type { ConflictDetail, ConflictCommit, MergeConflictInfo } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { openSubWindow } from "@/lib/window-management"
import type { ConflictingFilesWindowData } from "@/types/sub-window"

export interface ConflictingFileListCardProps {
  conflicts: readonly ConflictDetail[]
  conflictInfo: MergeConflictInfo
  conflictMarkerCommits: Record<string, ConflictCommit>
  branchName?: string
  /** Rendered in the `conflicting-files` sub-window: no "Open in Window" action */
  isInWindow?: boolean
}

export function ConflictingFileListCard({ conflicts, conflictInfo, conflictMarkerCommits, branchName, isInWindow = false }: ConflictingFileListCardProps) {
  const openConflictingFilesWindow = () => {
    const data: ConflictingFilesWindowData = {
      conflict: conflictInfo,
      branchName: branchName || "Unknown",
    }
    openSubWindow({
      windowId: "conflicting-files",
      url: "/conflicting-files",
      title: `Conflicting Files - ${branchName || "Unknown Branch"}`,
      data,
    }).catch((error: unknown) => notifyError("Failed to open window", error))
  }

  return (
    <Card className="gap-0 overflow-hidden py-0 shadow-none">
      <div className="border-b border-border p-4 sm:px-6">
        <SectionCardHeader
          title="Conflicting Files"
          count={conflicts.length}
          itemSingular="file"
          itemPlural="files"
          badgeColor="warning"
          actions={!isInWindow && (
            <Button size="xs" variant="ghost" onClick={openConflictingFilesWindow}>
              <ExternalLinkIcon data-icon="inline-start" />
              Open in Window
            </Button>
          )}
        />
      </div>

      <div className="bd-padding-card">
        <ConflictingFilesSection conflicts={conflicts} conflictInfo={conflictInfo} conflictMarkerCommits={conflictMarkerCommits} />
      </div>

    </Card>
  )
}
