import type { ReactNode } from "react"
import { cn } from "cn"
import { CommitBodyView } from "@/components/commit/CommitBodyView"
import { CommitHeader } from "@/components/commit/CommitHeader"
import { useCommitFileStats } from "@/hooks/useCommitFiles"
import type { CardCommit } from "@/lib/commit"

export interface CommitCardProps {
  commit: CardCommit
  /** The card of a row's mark (`card`, `text-sm`), or a commit above what follows it (`heading`, the reading text) */
  variant: "card" | "heading"
  /** After the subject: a badge ("Merge conflict"), the review button of a preview, the actions of the commit shown alone */
  addon?: ReactNode
  /** In place of the body of its message: the story a review tells of the commit */
  body?: ReactNode
}

/**
 * The one card of a commit (spec/commit/commit-row.spec.md, "Commit card"): its header (`CommitHeader`), the subject
 * whole over the meta line with its line counts coloured, then its body (`CommitBodyView`), its trailers last. The card
 * of a row's mark (`CommitRowMark`, in the lists of the main window and the tree of a review), the commits of the
 * preview of a branch and the commit a review shows alone; the commit preview shows the same header as its title and
 * the same body. The card of a mark scrolls under its header, which stays (its popover has no top padding of its own)
 */
export function CommitCard({ commit, variant, addon, body }: CommitCardProps) {
  const fileStats = useCommitFileStats(commit.id)
  const header = (
    <CommitHeader
      variant={variant}
      subject={commit.subject}
      hash={commit.id}
      newHash={commit.newHash}
      author={commit.author}
      authorTime={commit.authorTime}
      committerTime={commit.committerTime}
      fileStats={fileStats}
      subjectAddon={addon}
    />
  )
  return (
    <div className={cn("flex min-w-0 flex-col", variant === "heading" && "gap-2")}>
      {variant === "card" ? <div className="sticky top-0 z-10 bg-popover pt-4 pb-2">{header}</div> : header}
      {body ?? <CommitBodyView body={commit.body} size={variant === "card" ? "card" : "reading"} />}
    </div>
  )
}
