import { useEffect, useEffectEvent, useState } from "react"
import { Card } from "@/components/ui/card"
import { BranchReviewActions, BranchReviewBar, BranchReviewBody } from "@/components/review/BranchReview"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { hasCommitKeys, keyedAction, useBranchReview } from "@/hooks/useBranchReview"
import { askMainWindow, useLinkedFeedback, useLinkedState } from "@/hooks/useMainWindowLink"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { BRANCH_REVIEW_WINDOW, branchGoneReason } from "@/lib/branch-review"
import { notifyError } from "@/lib/errors"
import { openPullRequestUrl } from "@/lib/pull-request"
import { otherRepositoryReason } from "@/stores/review-requests"
import type { BranchReviewFeedback, BranchReviewItem, BranchReviewRequest, BranchReviewWindowData } from "@/types/sub-window"

/**
 * The review window of a branch (`BranchReview` on the canvas of a window of its own): the stack bar and the owner chips
 * in one island, the commits beside the diffs in the other, the footer with the keys and the actions on the branch of
 * the part (`ReviewFooter`). The window doesn't own the branches: the main window sends the heads a sync built and the
 * actions of each branch over the link of the window, and runs what it asks for.
 */
export function BranchReviewPage() {
  const data = useSubWindowData<BranchReviewWindowData>()
  if (data === null) {
    return null
  }
  // a new open (another branch, or the same one from another place) starts over
  return <BranchReviewWindow key={data.openedAt} data={data} />
}

function BranchReviewWindow({ data }: { data: BranchReviewWindowData }) {
  // what the main window says of the chain: the heads a sync built, the actions of each branch
  const linked = useLinkedState(BRANCH_REVIEW_WINDOW, data.state)
  const model = useBranchReview(data, data.focus, { known: linked.selected, tips: linked.tips })
  // the actions of each branch while the main window showed the repository and had it: they stay (those that need the
  // main window disabled) while it shows another one, and for a branch that left
  const [seenItems, setSeenItems] = useState(data.state.items)
  const [knownItems, setKnownItems] = useState(data.state.items)
  if (linked.selected && linked.items !== seenItems) {
    setSeenItems(linked.items)
    setKnownItems({ ...knownItems, ...linked.items })
  }
  useLinkedFeedback<BranchReviewFeedback>(BRANCH_REVIEW_WINDOW, feedback => notifyError(feedback.title, feedback.message))

  const { loaded } = model
  return (
    <div data-testid="branch-review" className="flex h-dvh flex-col gap-2 bg-canvas p-2 text-sm">
      {loaded !== null && (
        <Card className="shrink-0 gap-2 py-2">
          <div className="px-3">
            <BranchReviewBar model={model} />
          </div>
        </Card>
      )}
      <Card className="min-h-0 flex-1 flex-row gap-0 overflow-hidden py-0">
        <div className="min-w-0 flex-1">
          <BranchReviewBody model={model} />
        </div>
      </Card>
      {loaded !== null && (
        <ReviewFooter
          position={loaded.position}
          commits={hasCommitKeys(model)}
          repositoryPath={data.repositoryPath}
          branchName={loaded.target}
          items={shownItems(knownItems[loaded.target] ?? [], { repositoryPath: data.repositoryPath, branchName: loaded.target, selected: linked.selected, gone: model.gone.includes(loaded.target) })}
        />
      )}
    </div>
  )
}

/**
 * The actions of the footer as the main window last sent them: those it runs are disabled with the reason while it shows
 * another repository or the branch left; a pull request opens from here anyway
 */
function shownItems(items: readonly BranchReviewItem[], { repositoryPath, branchName, selected, gone }: { repositoryPath: string, branchName: string, selected: boolean, gone: boolean }): readonly BranchReviewItem[] {
  if (selected && !gone) {
    return items
  }
  return items.map((item) => {
    if (item.url !== undefined) {
      return item
    }
    const reason = gone ? branchGoneReason(branchName) : otherRepositoryReason(repositoryPath, item.id === "push" ? `push ${branchName}` : `start the Safe Push of ${branchName}`)
    return { ...item, disabled: true, description: reason }
  })
}

interface ReviewFooterProps {
  position: string
  /** ⌥↑/⌥↓ step through the commits of the part */
  commits: boolean
  repositoryPath: string
  /** The branch the actions act on */
  branchName: string
  items: readonly BranchReviewItem[]
}

/**
 * The footer below the islands, on the canvas: the position ("File 2 / 5"), the keys of the review, Escape that closes
 * the window, then the actions on the branch, the items of its menu: O opens its pull request (or Create Pull Request
 * the form of one), S asks the main window to open its Safe Push form, "Push" to push it, without a key (P is the
 * previous change). The window doesn't own the branches: the main window runs what it asks for and says why it can't.
 */
function ReviewFooter({ position, commits, repositoryPath, branchName, items }: ReviewFooterProps) {
  const ask = (itemId: BranchReviewRequest["itemId"]) => void askMainWindow<BranchReviewRequest>(BRANCH_REVIEW_WINDOW, { repositoryPath, branchName, itemId })
  const run = (item: BranchReviewItem) => {
    if (item.url !== undefined) {
      openPullRequestUrl(item.url)
    }
    else if (item.id === "safe-push" || item.id === "push") {
      ask(item.id)
    }
  }

  const handleKey = useEffectEvent((event: KeyboardEvent) => {
    // the physical keys, as in the branch preview: the key an item names
    const item = keyedAction(items, event)
    if (item !== undefined) {
      event.preventDefault()
      if (item.disabled !== true) {
        run(item)
      }
    }
  })
  // the whole window: the focus may be on the page itself
  useEffect(() => {
    const listener = (event: KeyboardEvent) => handleKey(event)
    window.addEventListener("keydown", listener)
    return () => window.removeEventListener("keydown", listener)
  }, [])

  return (
    <div className="shrink-0" data-testid="branch-review-footer">
      <PreviewDialogFooter className="border-t-0 px-1 py-0" start={<span className="tabular-nums" data-testid="branch-review-position">{position}</span>}>
        <ReviewKeyHints commits={commits} escape="Close" />
        <BranchReviewActions actions={items} branchName={branchName} onRun={run} />
      </PreviewDialogFooter>
    </div>
  )
}
