import { useCallback, useMemo } from "react"
import { AppWindowIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { CommitList } from "@/components/commit/CommitList"
import { SubjectFileDiffList } from "@/components/conflict/SubjectFileDiffList"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { SectionPanel } from "@/components/shared/NestedPanel"
import type { MergeConflictInfo, MissingCommit } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import type { MissingCommitOwners } from "@/lib/merge-conflict"
import { openSubWindow } from "@/lib/window-management"
import { requestReveal } from "@/stores/commit-move"
import { fixStepsFor } from "@/stores/conflict-fix"
import { getDiffRepositoryPath } from "@/stores/repository"
import { useDiffViewMode } from "@/stores/settings/diff-view-settings"
import type { MissingCommitsWindowData } from "@/types/sub-window"

export interface MissingCommitsDetailsProps {
  /** The conflict whose missing commits are listed; the diffs of each commit are read for its files */
  conflict: MergeConflictInfo
  /** The branches of the missing commits (`getMissingCommitOwners`): the list is grouped by them */
  owners: MissingCommitOwners
  branchName?: string
  /** The branch the commit is copied onto, for the title of the sub-window */
  onto?: string
  /** What the branch is built on, for the explanation of the sub-window; undefined for an amend conflict */
  builtOn?: string | null
  /** Rendered in the `missing-commits` sub-window: an island, no "Open in Window" action */
  isInWindow?: boolean
}

/**
 * Commits missing from the target branch that touched the conflicting files, with their file diffs, grouped by the
 * branch they are in (the branch with the most first, the unassigned ones last): a nested panel in the expanded row, an
 * island in the sub-window. The header of a group shows its branch in the main window.
 */
export function MissingCommitsDetails({ conflict, owners, branchName, onto, builtOn, isInWindow = false }: MissingCommitsDetailsProps) {
  const missingCommits = conflict.conflictAnalysis.missingCommits
  const diffViewMode = useDiffViewMode()
  const hasAnyFileDiffs = missingCommits.some(commit => commit.filesTouched.length > 0)
  const groups = useMemo(() => {
    const byHash = new Map(missingCommits.map(commit => [commit.hash, commit]))
    const commitsOf = (ids: readonly string[]) => ids.flatMap(hash => byHash.get(hash) ?? [])
    return [...owners.branches.map(owner => ({ branch: owner.branch, commits: commitsOf(owner.commitIds) })), { branch: null, commits: commitsOf(owners.unassigned) }].filter(
      group => group.commits.length > 0,
    )
  }, [missingCommits, owners])
  const grouped = useMemo(() => groups.flatMap(group => group.commits), [groups])

  const renderSectionHeader = useCallback(
    (commit: MissingCommit) => {
      const group = groups.find(it => it.commits[0] === commit)
      return group === undefined ? null : <MissingCommitsGroupHeader branch={group.branch} commitIds={group.commits.map(it => it.hash)} revealable={!isInWindow} />
    },
    [groups, isInWindow],
  )

  const renderAfterCommit = useCallback(
    (commit: MissingCommit) => {
      if (commit.filesTouched.length === 0) {
        return null
      }
      return (
        <SubjectFileDiffList
          subject={{ kind: "commit", id: commit.hash }}
          files={commit.filesTouched.map(path => ({ path }))}
          keyPrefix={commit.hash}
          diffViewMode={diffViewMode}
        />
      )
    },
    [diffViewMode],
  )

  const openMissingCommitsWindow = () => {
    const repositoryPath = getDiffRepositoryPath()
    if (repositoryPath === undefined) {
      return
    }

    // the window reads the diffs itself
    const data: MissingCommitsWindowData = {
      repositoryPath,
      conflict,
      branchName,
      onto,
      owners,
      builtOn,
      fixSteps: fixStepsFor(branchName),
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
        actions: (hasAnyFileDiffs || !isInWindow) && (
          <>
            {hasAnyFileDiffs && <DiffViewToggle />}
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
      <CommitList commits={grouped} variant="detailed" showFileCount renderAfterCommit={renderAfterCommit} renderSectionHeader={renderSectionHeader} />
    </SectionPanel>
  )
}

function MissingCommitsGroupHeader({ branch, commitIds, revealable }: { branch: string | null; commitIds: string[]; revealable: boolean }) {
  const label = (
    <>
      {branch ?? "Unassigned"}
      <span className="font-normal text-muted-foreground tabular-nums">{formatCommitCount(commitIds.length)}</span>
    </>
  )
  return branch !== null && revealable ? (
    <Button variant="ghost" size="xs" className="-ml-2 gap-2 font-medium" data-missing-group={branch} onClick={() => requestReveal({ branchName: branch, commitIds })}>
      {label}
    </Button>
  ) : (
    <p className="flex items-center gap-2 text-xs font-medium" data-missing-group={branch ?? ""}>
      {label}
    </p>
  )
}
