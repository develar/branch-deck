import type { ReactNode } from "react"
import { Badge } from "@/components/ui/badge"
import { CommitHeader } from "@/components/commit/CommitHeader"
import { CommitRowMark } from "@/components/commit/CommitRowMark"
import type { RowCardState } from "@/components/shared/RowCard"
import { useRowCardHover } from "@/hooks/useRowCardHover"
import type { CardCommit } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"

/** What a commit of a review is to the reader beside its subject: since the review, the review of the user, its branch */
export interface CommitRowFacts {
  /** "New", "Rewritten": what it is since the user's review */
  readonly since?: string
  /** "Your review": the commit the user reviewed last */
  readonly review?: string
  /** Its branch, of a commit of the whole stack or of a part of several branches */
  readonly branch?: string
}

/** The facts of a commit of a review on its row: a badge of what it is since the review and of the user's review, its branch muted */
export function CommitRowBadges({ facts }: { facts: CommitRowFacts }) {
  return (
    <>
      {facts.since !== undefined && (
        <Badge variant="info" data-testid="review-commit-since">
          {facts.since}
        </Badge>
      )}
      {facts.review !== undefined && (
        <Badge variant="outline" data-testid="review-commit-review">
          {facts.review}
        </Badge>
      )}
      {facts.branch !== undefined && (
        <span className="max-w-24 shrink-0 truncate text-xs leading-5 text-muted-foreground" data-testid="review-commit-branch">
          {facts.branch}
        </span>
      )}
    </>
  )
}

export interface CommitRowProps {
  /** Its whole subject, in the hover of its title with its hash */
  subject: string
  /** The text of its row: the title the review gives it (without the issue key of the brief); its subject by default */
  title?: string
  hash: string
  facts?: CommitRowFacts
  /** Of a commit with a body, its card (`CommitRowMark`) and the state of it its list holds; none without a body */
  card?: { commit: CardCommit; state: RowCardState; menu?: ReactNode; onOpen?: () => void } | null
  /** The test id of its subject */
  subjectTestId?: string
}

/**
 * A commit on a row of a sidebar (spec/commit/commit-row.spec.md, "Commit card"): the commit column of a review, the
 * commits of a pull request, a commit row of the tree of a review; the compact header (`CommitHeader`), its subject
 * clamped to two lines with its hash in its native hover where it has no card, the facts of a review, the mark of its card, which its hover, the
 * hover of the subject and `I` on the row open. The row around it holds its focus, its selection and its click
 */
export function CommitRow({ subject, title = subject, hash, facts = {}, card = null, subjectTestId }: CommitRowProps) {
  const hover = useRowCardHover(card?.state ?? NO_CARD)
  return (
    <CommitHeader
      variant="compact"
      subject={title}
      hash={hash}
      // the subject and the hash in the native hover of a commit without a card; the card of one says both
      subjectProps={{ ...(card === null ? { title: `${subject}\n${formatShortHash(hash)}` } : hover.text), "data-testid": subjectTestId }}
      subjectAddon={
        <>
          <CommitRowBadges facts={facts} />
          {card !== null && <CommitRowMark commit={card.commit} size="tree" menu={card.menu} onOpen={card.onOpen} popupHover={hover.popup} {...card.state} />}
        </>
      }
    />
  )
}

const NO_CARD: Pick<RowCardState, "open" | "onOpenChange"> = { open: false, onOpenChange: () => {} }
