import { useEffect, useRef, useState } from "react"
import type { KeyboardEvent } from "react"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { CommitsPreviewBody } from "@/components/preview/CommitsPreviewBody"
import { Callout } from "@/components/shared/Callout"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader, PreviewNavigationHints } from "@/components/shared/PreviewDialogFrame"
import { useCommitFiles } from "@/hooks/useCommitFiles"
import type { ReviewCommit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { CardCommit } from "@/lib/commit"
import { formatCommitCount, toCardCommit } from "@/lib/commit"
import { handlePreviewNavigationKey, hasKeyModifier } from "@/lib/preview-keys"
import { keptBranchName } from "@/lib/review-actions"
import type { KeptKind } from "@/lib/review-target"
import { openKeptReviewWindow } from "@/lib/review-window"
import { useBranchSync } from "@/stores/branch-sync"
import { useSelectedProjectPath } from "@/stores/repository"

/** What the preview read of a kept branch: its own commits, newest first, by the resolver of its review */
type KeptCommits =
  | { readonly status: "loading" }
  | { readonly status: "loaded"; readonly commits: readonly ReviewCommit[] }
  | { readonly status: "error"; readonly message: string }

/**
 * The commits of the kept branch `name` as its review reads them (`get_review` of the target `kept`: its own commits
 * from where they start, memoised by its tip in Rust)
 */
function useKeptCommits(name: string | null): KeptCommits {
  const repositoryPath = useSelectedProjectPath()
  const baselineBranch = useBranchSync(state => state.baselineBranch)
  const [read, setRead] = useState<{ key: string; commits: KeptCommits } | null>(null)
  const key = name === null || repositoryPath === undefined || baselineBranch === null ? null : `${repositoryPath}\n${baselineBranch}\n${name}`
  useEffect(() => {
    if (key === null || name === null || repositoryPath === undefined || baselineBranch === null) {
      return
    }
    let current = true
    void commands.getReview({ repositoryPath, baselineBranch, target: { kind: "kept", name } }).then(result => {
      if (!current) {
        return
      }
      const commits: KeptCommits =
        result.status === "error"
          ? { status: "error", message: result.error.message }
          : { status: "loaded", commits: result.data.kind === "range" ? result.data.segments.flatMap(segment => segment.commits) : result.data.commits }
      setRead({ key, commits })
    })
    return () => {
      current = false
    }
  }, [key, name, repositoryPath, baselineBranch])
  return read !== null && read.key === key ? read.commits : LOADING
}

const LOADING: KeptCommits = { status: "loading" }

/** A commit of the branch as its preview reads it: the card without its author, as the preview of a branch */
function previewCommitOf(commit: ReviewCommit): CardCommit {
  return { ...toCardCommit(commit), author: undefined }
}

const NO_IDS: readonly string[] = []

/**
 * The preview of an unapplied or archived branch, as the preview of a branch: its name and where it is kept, then its
 * own commits newest first with their whole messages (`CommitsPreviewBody`). Space on its row opens it; ↑/↓ show the
 * previous or next row of its card, D and the footer button review the branch in the review window (a commit's button
 * at that commit), the preview closes once the window opened; Space and Escape close it.
 *
 * Stays mounted while closed (see `PreviewDialogFrame`).
 */
export function KeptBranchPreviewDialog({
  kind,
  names,
  name,
  onNameChange,
  onClose,
}: {
  kind: KeptKind
  /** The full names of the rows of the card, in its order */
  names: readonly string[]
  /** The shown kept branch by its full name; null while closed */
  name: string | null
  onNameChange: (name: string) => void
  onClose: () => void
}) {
  const open = name !== null
  // the last shown branch stays while the closing animation runs
  const [shownName, setShownName] = useState(name)
  if (open && name !== shownName) {
    setShownName(name)
  }
  const currentName = open ? name : shownName
  const index = currentName === null ? -1 : names.indexOf(currentName)
  const read = useKeptCommits(currentName)
  const commits = read.status === "loaded" ? read.commits : null
  const isLoadingFiles = useCommitFiles(open && commits !== null ? commits.map(commit => commit.id) : NO_IDS)
  const bodyRef = useRef<HTMLDivElement>(null)
  const [isOpeningReview, setIsOpeningReview] = useState(false)

  const review = (commitId?: string) => {
    if (currentName === null || isOpeningReview) {
      return
    }
    setIsOpeningReview(true)
    void openKeptReviewWindow({ kind, name: currentName }, commitId === undefined ? {} : { commitId })
      .then(opened => {
        if (opened) {
          onClose()
        }
      })
      .finally(() => setIsOpeningReview(false))
  }
  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.defaultPrevented || hasKeyModifier(event) || handlePreviewNavigationKey(event, index, names.length, next => onNameChange(names[next]!), onClose)) {
      return
    }
    if (event.code === "KeyD") {
      event.preventDefault()
      review()
    }
  }

  const kept = currentName === null ? null : { kind, name: currentName }
  return (
    <PreviewDialogFrame
      data-testid="kept-preview"
      open={open}
      onOpenChange={nextOpen => {
        if (!nextOpen) {
          onClose()
        }
      }}
      initialFocus={bodyRef}
      onKeyDown={handleKeyDown}
    >
      {kept !== null && (
        <>
          <PreviewDialogHeader>
            <DialogTitle className="truncate text-base leading-6 font-medium">{keptBranchName(kept)}</DialogTitle>
            <div className="flex min-w-0 flex-wrap items-center gap-x-2 text-xs text-muted-foreground" data-testid="kept-preview-meta">
              <span>{kind === "unapplied" ? "Unapplied" : "Archived"}</span>
              {commits !== null && (
                <>
                  <span>·</span>
                  <span>{formatCommitCount(commits.length)}</span>
                </>
              )}
            </div>
          </PreviewDialogHeader>
          <PreviewDialogBody ref={bodyRef} itemKey={kept.name} data-testid="kept-preview-body">
            {read.status === "error" ? (
              <Callout tone="destructive" title={`Cannot read the commits of ${keptBranchName(kept)}`}>
                {read.message}
              </Callout>
            ) : commits === null ? (
              <LoadingLine>Loading commits…</LoadingLine>
            ) : (
              <CommitsPreviewBody commits={commits.map(previewCommitOf)} busy={isLoadingFiles} testId="kept-preview" onReview={commit => review(commit.id)} />
            )}
          </PreviewDialogBody>
          <PreviewDialogFooter data-testid="kept-preview-footer" start={<span className="tabular-nums">{`${index + 1} / ${names.length}`}</span>}>
            <PreviewNavigationHints count={names.length} />
            <Button variant="ghost" size="xs" disabled={isOpeningReview} data-testid="kept-preview-review" onClick={() => review()}>
              <Kbd>D</Kbd>
              Review Changes
            </Button>
          </PreviewDialogFooter>
        </>
      )}
    </PreviewDialogFrame>
  )
}
