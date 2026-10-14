import { useCallback } from "react"
import { AppWindowIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { CommitList } from "@/components/commit/CommitList"
import { FileDiffList } from "@/components/conflict/FileDiffList"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { SectionPanel } from "@/components/shared/NestedPanel"
import type { MergeConflictInfo, MissingCommit } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { getMergeBase } from "@/lib/merge-conflict"
import { openSubWindow } from "@/lib/window-management"
import { useConflictViewerSettings } from "@/stores/settings/conflictViewer"
import type { MissingCommitsWindowData } from "@/types/sub-window"

export interface MissingCommitsDetailsProps {
  missingCommits: readonly MissingCommit[]
  /** Required for the "Open in Window" action */
  conflict?: MergeConflictInfo
  branchName?: string
  /** The branch the commit is copied onto, for the title of the sub-window */
  onto?: string
  /** Rendered in the `missing-commits` sub-window: an island, no "Open in Window" action */
  isInWindow?: boolean
}

/**
 * Commits missing from the target branch that touched the conflicting files, with their file diffs: a nested panel in
 * the expanded row, an island in the sub-window.
 */
export function MissingCommitsDetails({ missingCommits, conflict, branchName, onto, isInWindow = false }: MissingCommitsDetailsProps) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  const hasAnyFileDiffs = missingCommits.some(commit => commit.fileDiffs.length > 0)

  const renderAfterCommit = useCallback((commit: MissingCommit) => {
    if (commit.fileDiffs.length === 0) {
      return null
    }
    return <FileDiffList fileDiffs={commit.fileDiffs} keyPrefix={commit.hash} diffViewMode={diffViewMode} />
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
      branchName,
      onto,
      missingCommits: analysis.missingCommits,
      mergeBase: getMergeBase(analysis),
      divergenceSummary: analysis.divergenceSummary,
    }

    openSubWindow({
      windowId: "missing-commits",
      url: "/missing-commits",
      title: branchName ? `Missing commits · ${branchName}` : "Missing commits",
      data,
    }).catch((error: unknown) => notifyError("Cannot open the missing commits", error))
  }

  return (
    <SectionPanel
      island={isInWindow}
      data-testid="missing-commits"
      header={{
        title: "Missing Commits Details",
        count: missingCommits.length,
        itemSingular: "commit",
        itemPlural: "commits",
        badgeColor: "warning",
        actions: ((hasAnyFileDiffs && isInWindow) || !isInWindow) && (
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
        ),
      }}
    >
      <CommitList commits={missingCommits} variant="detailed" showFileCount renderAfterCommit={renderAfterCommit} />
    </SectionPanel>
  )
}
