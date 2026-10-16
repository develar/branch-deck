import { useEffect } from "react"
import type { RefObject } from "react"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { BranchPreviewActions } from "@/components/branch/BranchPreviewActions"
import { StackBaseLabel } from "@/components/branch/StackBaseLabel"
import { LinkedText } from "@/components/commit/LinkedText"
import { BranchReviewBar, BranchReviewBody } from "@/components/review/BranchReview"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { PreviewDialogBody, PreviewDialogFooter, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { currentFocus, hasCommitKeys, useBranchReview } from "@/hooks/useBranchReview"
import type { BranchReviewSource } from "@/hooks/useBranchReview"
import type { BranchReviewFocus } from "@/lib/branch-review"
import type { Branch } from "@/stores/branch-sync"
import { useBranch, useBranchSync } from "@/stores/branch-sync"

/** What the dialog asks of its review: the branch the keys act on, and where the review is (W moves it there) */
export interface BranchPreviewReviewHandle {
  readonly target: string | null
  readonly focus: () => BranchReviewFocus
}

export interface BranchPreviewReviewProps {
  branch: Branch
  source: BranchReviewSource
  focus: BranchReviewFocus
  /** Escape goes back to the preview (opened from it), else it closes */
  fromPreview: boolean
  bodyRef: RefObject<HTMLDivElement | null>
  handleRef: RefObject<BranchPreviewReviewHandle | null>
  onOpenInWindow: () => void
}

/**
 * The review of a branch in its preview dialog (`BranchReview`, as the review window shows it): the name, then the stack
 * bar and the owner chips; the commits of the part beside the diffs; the position, the keys, W Open in Window and the
 * actions on the branch of the part. The main window owns the branches: the heads of the last sync come from the store.
 */
export function BranchPreviewReview({ branch, source, focus, fromPreview, bodyRef, handleRef, onOpenInWindow }: BranchPreviewReviewProps) {
  // a narrow selector: the heads of the chain as one string; a branch that left has none
  const tipsJson = useBranchSync(state =>
    JSON.stringify(
      Object.fromEntries(
        source.chain.flatMap(name => {
          const tip = state.branches[name]?.commits[0]?.hash
          return tip === undefined ? [] : [[name, tip]]
        }),
      ),
    ),
  )
  const tips = JSON.parse(tipsJson) as Record<string, string>
  // the heads of a finished sync, not of one that runs
  const isSyncing = useBranchSync(state => state.isSyncing)
  const model = useBranchReview(source, focus, { known: !isSyncing, tips })
  const target = useBranch(model.loaded?.target ?? "")
  useEffect(() => {
    handleRef.current = { target: model.loaded?.target ?? null, focus: () => currentFocus(model) }
  })

  return (
    <>
      <PreviewDialogHeader>
        <div className="flex min-w-0 items-center gap-2">
          <DialogTitle className="shrink-0 leading-6">
            <LinkedText text={branch.name} className="text-base font-medium" linkClassName="text-foreground hover:underline" />
          </DialogTitle>
          <StackBaseLabel name={branch.name} />
        </div>
        <BranchReviewBar model={model} />
      </PreviewDialogHeader>
      <PreviewDialogBody ref={bodyRef} itemKey={branch.name} fill data-testid="branch-preview-body">
        <BranchReviewBody model={model} />
      </PreviewDialogBody>
      <PreviewDialogFooter
        data-testid="branch-preview-footer"
        start={
          <span className="tabular-nums" data-testid="branch-review-position">
            {model.loaded?.position ?? ""}
          </span>
        }
      >
        <ReviewKeyHints commits={hasCommitKeys(model)} escape={fromPreview ? "Back" : "Close"}>
          <Button variant="ghost" size="xs" data-testid="branch-review-open-in-window" onClick={onOpenInWindow}>
            <Kbd>W</Kbd>
            Open in Window
          </Button>
        </ReviewKeyHints>
        {target !== undefined && <BranchPreviewActions branch={target} review />}
      </PreviewDialogFooter>
    </>
  )
}
