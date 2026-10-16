import { useEffect, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Card } from "@/components/ui/card"
import { RangeReview } from "@/components/review/RangeReview"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { commitReviewTitle } from "@/lib/commit-review-window"
import type { CommitReviewWindowData } from "@/lib/commit-review-window"
import { logError } from "@/lib/errors"
import { commitItems, isSingleCommit, rangePosition } from "@/lib/range-review"
import type { ReviewPosition } from "@/lib/review-position"
import { formatReviewPosition } from "@/lib/review-position"

/**
 * The review window of a commit (`commit-review`): the review of the commit preview on the canvas of a window of its
 * own, with the commits of the list it came from beside it (⌥↑/⌥↓, as in the dialog). It reads only the repository: the
 * commits don't change, so the list it got stays true; nothing is edited here.
 */
export function CommitReviewPage() {
  const data = useSubWindowData<CommitReviewWindowData>()
  if (data === null) {
    return null
  }
  // a new open (another commit, or the same one from another list) starts over
  return <CommitReviewWindow key={data.openedAt} data={data} />
}

function CommitReviewWindow({ data }: { data: CommitReviewWindowData }) {
  const items = commitItems(data.commits)
  const [selected, setSelected] = useState(data.commits[data.index]?.id ?? data.commits[0]?.id ?? "")
  const [position, setPosition] = useState<ReviewPosition | null>(null)
  const [lastPath, setLastPath] = useState(data.path)
  const showPosition = (next: ReviewPosition) => {
    setPosition(next)
    if (next.path !== null) {
      setLastPath(next.path)
    }
  }

  // the native title names the commit shown
  const commit = data.commits.find(it => it.id === selected)
  useEffect(() => {
    if (commit !== undefined) {
      getCurrentWindow().setTitle(commitReviewTitle(commit)).catch((error: unknown) => logError("Cannot retitle the review window", error))
    }
  }, [commit])

  return (
    <div data-testid="commit-review-window" className="flex h-dvh flex-col gap-2 bg-canvas p-2 text-sm">
      <Card className="min-h-0 flex-1 flex-row gap-0 overflow-hidden py-0">
        <div className="min-w-0 flex-1">
          <RangeReview
            items={items}
            selected={selected}
            onSelect={setSelected}
            subject={{ kind: "commit", id: selected }}
            initialPath={lastPath}
            onPositionChange={showPosition}
            rangeNoun="commit"
          />
        </div>
      </Card>
      <PreviewDialogFooter
        className="border-t-0 px-1 py-0"
        data-testid="commit-review-window-footer"
        start={<span className="tabular-nums" data-testid="commit-review-position">{formatReviewPosition({ file: position, ...rangePosition(items, selected) })}</span>}
      >
        <ReviewKeyHints commits={!isSingleCommit(items)} escape="Close" />
      </PreviewDialogFooter>
    </div>
  )
}
