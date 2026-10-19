import { AppWindowIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { ConflictingFilesSection } from "@/components/conflict/ConflictingFilesSection"
import { SectionPanel } from "@/components/shared/NestedPanel"
import type { MergeConflictInfo } from "@/lib/bindings"
import { getSubjectLine, parseSubject } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { openSubWindow } from "@/lib/window-management"
import { fixStepsFor } from "@/stores/conflict-fix"
import { getDiffRepositoryPath } from "@/stores/repository"
import type { ConflictingFilesWindowData } from "@/types/sub-window"

export interface ConflictingFileListCardProps {
  conflictInfo: MergeConflictInfo
  branchName?: string
  /** The branch the commit is copied onto, for the title of the sub-window */
  onto?: string
  /** Rendered in the `conflicting-files` sub-window: an island, no "Open in Window" action */
  isInWindow?: boolean
}

/** The conflicting files of a commit: a nested panel in the expanded row, an island in the sub-window. */
export function ConflictingFileListCard({ conflictInfo, branchName, onto, isInWindow = false }: ConflictingFileListCardProps) {
  const conflicts = conflictInfo.conflictingFiles
  const openConflictingFilesWindow = () => {
    const repositoryPath = getDiffRepositoryPath()
    if (repositoryPath === undefined) {
      return
    }
    // the window reads the contents itself
    const data: ConflictingFilesWindowData = { repositoryPath, conflict: conflictInfo, branchName, onto, fixSteps: fixStepsFor(branchName) }
    openSubWindow({
      windowId: "conflicting-files",
      url: "/conflicting-files",
      title: branchName ? `Conflicting files · ${branchName}` : "Conflicting files",
      data,
    }).catch((error: unknown) => notifyError("Cannot open the conflicting files", error))
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
        <ConflictingFilesSection
          files={conflicts}
          source={{ kind: "cherryPick", parent: conflictInfo.originalParent.hash, target: conflictInfo.targetBranch.hash, commit: conflictInfo.commit.hash }}
          labels={{ left: onto ?? "the baseline", right: `“${parseSubject(getSubjectLine(conflictInfo.commit.message)).rest}”` }}
        />
      </div>
    </SectionPanel>
  )
}
