import { AppWindowIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { ConflictingFilesSection } from "@/components/conflict/ConflictingFilesSection"
import { SectionPanel } from "@/components/shared/NestedPanel"
import type { ConflictDetail, ConflictCommit, MergeConflictInfo } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { openSubWindow } from "@/lib/window-management"
import type { ConflictingFilesWindowData } from "@/types/sub-window"

export interface ConflictingFileListCardProps {
  conflicts: readonly ConflictDetail[]
  conflictInfo: MergeConflictInfo
  conflictMarkerCommits: Record<string, ConflictCommit>
  branchName?: string
  /** The branch the commit is copied onto, for the title of the sub-window */
  onto?: string
  /** Rendered in the `conflicting-files` sub-window: an island, no "Open in Window" action */
  isInWindow?: boolean
}

/** The conflicting files of a commit: a nested panel in the expanded row, an island in the sub-window. */
export function ConflictingFileListCard({ conflicts, conflictInfo, conflictMarkerCommits, branchName, onto, isInWindow = false }: ConflictingFileListCardProps) {
  const openConflictingFilesWindow = () => {
    const data: ConflictingFilesWindowData = { conflict: conflictInfo, branchName, onto }
    openSubWindow({
      windowId: "conflicting-files",
      url: "/conflicting-files",
      title: branchName ? `Conflicting files · ${branchName}` : "Conflicting files",
      data,
    }).catch((error: unknown) => notifyError("Failed to open window", error))
  }

  return (
    <SectionPanel
      island={isInWindow}
      data-testid="conflicting-files"
      header={{
        title: "Conflicting Files",
        count: conflicts.length,
        itemSingular: "file",
        itemPlural: "files",
        badgeColor: "warning",
        actions: !isInWindow && (
          <Button size="xs" variant="ghost" onClick={openConflictingFilesWindow}>
            <AppWindowIcon data-icon="inline-start" />
            Open in Window
          </Button>
        ),
      }}
    >
      <div className="bd-padding-card">
        <ConflictingFilesSection conflicts={conflicts} conflictInfo={conflictInfo} conflictMarkerCommits={conflictMarkerCommits} />
      </div>
    </SectionPanel>
  )
}
