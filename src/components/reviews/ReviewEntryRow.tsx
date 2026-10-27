import type { ReactNode } from "react"
import { cn } from "cn"
import { CheckIcon, MessageSquareIcon, MessageSquareXIcon } from "lucide-react"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchNodeMark, ReviewRequestRow } from "@/components/repository/ReviewRequestRow"
import { useLandingNode } from "@/hooks/useLandingNode"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import type { ReviewEntry } from "@/lib/bindings"
import type { ReviewMark } from "@/presenters/reviews"
import { formatReviewAge, formatReviewHint, formatReviewMeta, reviewMark } from "@/presenters/reviews"
import { requestReveal } from "@/stores/commit-move"
import { unlinkPullRequest } from "@/stores/reviews"

export interface ReviewEntryRowProps {
  entry: ReviewEntry
  /** Rust says its next move is the user's (`ReviewSection.moves`): it alone stands out */
  move: boolean
  repositoryPath: string
  tabStop: boolean
}

/**
 * A pull request of Reviews in the row of a request, by how the user takes part: its mark (the dot of the user's move,
 * else the user's review muted), its short meta, the rest in the tooltip of the meta, its age; an own one of a branch
 * of the table with the node of the branch and where the branch stands on its way into master, as its row says
 */
export function ReviewEntryRow(props: ReviewEntryRowProps) {
  const { involvement } = props.entry
  if (involvement.kind === "authored" && involvement.branch !== null) {
    return <OwnBranchEntryRow {...props} branch={involvement.branch} />
  }
  return <EntryRow {...props} lead={<ReviewMarkGlyph mark={reviewMark(props.entry, props.move)} />} meta={formatReviewMeta(props.entry)} />
}

/** An own pull request of a branch of the table: the node of the branch, its first line after the number */
function OwnBranchEntryRow({ branch, ...props }: ReviewEntryRowProps & { branch: string }) {
  const node = useLandingNode(branch)
  if (node === null) {
    return <EntryRow {...props} lead={<ReviewMarkGlyph mark={reviewMark(props.entry, props.move)} />} meta={formatReviewMeta(props.entry)} />
  }
  return <EntryRow {...props} lead={<BranchNodeMark branch={branch} />} meta={`#${props.entry.request.number} · ${node.label.split("\n", 1)[0]!}`} />
}

function EntryRow({ entry, move, repositoryPath, tabStop, lead, meta }: ReviewEntryRowProps & { lead: ReactNode; meta: string }) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  const { request, involvement } = entry
  const branch = involvement.kind === "authored" ? involvement.branch : null
  return (
    <ReviewRequestRow
      request={request}
      repositoryPath={repositoryPath}
      unread={false}
      tabStop={tabStop}
      source="reviews"
      lead={lead}
      meta={meta}
      metaHint={formatReviewHint(entry)}
      quiet={!move}
      age={
        <span
          className={cn("text-xs whitespace-nowrap tabular-nums", move ? "text-foreground" : "text-muted-foreground")}
          data-testid="review-request-age"
          data-move={move || undefined}
        >
          {formatReviewAge(entry, now)}
        </span>
      }
      {...(branch !== null && { onShowBranch: () => requestReveal({ branchName: branch, commitIds: [] }) })}
      {...(involvement.kind === "linked" && { onRemoveFromReviews: () => void unlinkPullRequest(repositoryPath, request.number) })}
    />
  )
}

/** The mark of a row: the filled dot of the user's move, else the user's review muted; nothing for a team request */
function ReviewMarkGlyph({ mark }: { mark: ReviewMark }) {
  if (mark.kind === "none") {
    return null
  }
  return (
    <Tooltip>
      <TooltipTrigger
        render={<span role="img" aria-label={mark.label} className="flex size-4 items-center justify-center" data-testid="review-entry-mark" data-mark={mark.kind} />}
      >
        <MarkGlyph kind={mark.kind} />
      </TooltipTrigger>
      <TooltipContent>{mark.label}</TooltipContent>
    </Tooltip>
  )
}

function MarkGlyph({ kind }: { kind: Exclude<ReviewMark["kind"], "none"> }) {
  switch (kind) {
    case "move":
      return <span className="size-2 rounded-full bg-primary" />
    case "approved":
      return <CheckIcon className="size-3.5 text-muted-foreground" />
    case "changesRequested":
      return <MessageSquareXIcon className="size-3.5 text-muted-foreground" />
    case "commented":
      return <MessageSquareIcon className="size-3.5 text-muted-foreground" />
  }
}
