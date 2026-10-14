import { useCallback } from "react"
import { AppWindowIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { CommitList } from "@/components/commit/CommitList"
import { FileDiffList } from "@/components/conflict/FileDiffList"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import type { MergeConflictInfo, MissingCommit } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { openSubWindow } from "@/lib/window-management"
import { useConflictViewerSettings } from "@/stores/settings/conflictViewer"
import type { MissingCommitsWindowData } from "@/types/sub-window"

export interface MissingCommitsDetailsProps {
  missingCommits: readonly MissingCommit[]
  /** Required for the "Open in Window" action */
  conflict?: MergeConflictInfo
  branchName?: string
  /** Rendered in the `missing-commits` sub-window: no "Open in Window" action */
  isInWindow?: boolean
}

/** Commits missing from the target branch that touched the conflicting files, with their file diffs. */
export function MissingCommitsDetails({ missingCommits, conflict, branchName, isInWindow = false }: MissingCommitsDetailsProps) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  const hasAnyFileDiffs = missingCommits.some(commit => commit.fileDiffs.length > 0)

  const renderAfterCommit = useCallback((commit: MissingCommit) => {
    if (commit.fileDiffs.length === 0) {
      return null
    }
    return (
      <div className="mt-3 rounded-lg bg-background">
        <FileDiffList fileDiffs={commit.fileDiffs} keyPrefix={commit.hash} diffViewMode={diffViewMode} />
      </div>
    )
  }, [diffViewMode])

  const openMissingCommitsWindow = () => {
    if (!conflict) {
      return
    }

    const analysis = conflict.conflictAnalysis
    const data: MissingCommitsWindowData = {
      conflictCommitHash: conflict.commit.hash,
      conflictCommitMessage: conflict.commit.message,
      conflictCommitAuthorTime: conflict.commit.authorTime,
      conflictCommitCommitterTime: conflict.commit.committerTime,
      branchName: branchName || "Unknown",
      missingCommits: analysis.missingCommits,
      mergeBase: {
        hash: analysis.mergeBaseHash,
        subject: analysis.mergeBaseSubject,
        message: analysis.mergeBaseMessage,
        author: analysis.mergeBaseAuthor,
        time: analysis.mergeBaseTime,
      },
      divergenceSummary: analysis.divergenceSummary,
    }

    openSubWindow({
      windowId: "missing-commits",
      url: "/missing-commits",
      title: `Missing Commits Analysis - ${branchName || "Unknown Branch"}`,
      data,
    }).catch((error: unknown) => notifyError("Failed to open window", error))
  }

  return (
    <Card className="gap-0 overflow-hidden py-0 shadow-none">
      <div className="border-b border-border p-4 sm:px-6">
        <SectionCardHeader
          title="Missing Commits Details"
          count={missingCommits.length}
          itemSingular="commit"
          itemPlural="commits"
          badgeColor="warning"
          actions={((hasAnyFileDiffs && isInWindow) || !isInWindow) && (
            <>
              {/* in the main window the conflicting files above carry the one diff view toggle */}
              {hasAnyFileDiffs && isInWindow && <DiffViewToggle />}
              {!isInWindow && (
                <Button size="xs" variant="ghost" onClick={openMissingCommitsWindow}>
                  <AppWindowIcon data-icon="inline-start" />
                  Open in Window
                </Button>
              )}
            </>
          )}
        />
      </div>

      <CommitList commits={missingCommits} variant="detailed" showFileCount renderAfterCommit={renderAfterCommit} />

    </Card>
  )
}
