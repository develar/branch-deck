import { useEffect, useRef, useState } from "react"
import type { ReactNode } from "react"
import { CommitBodyView } from "@/components/commit/CommitBodyView"
import { CommitHeader } from "@/components/commit/CommitHeader"
import type { CommitHeaderProps } from "@/components/commit/CommitHeader"
import { useCommitFileStats } from "@/hooks/useCommitFiles"
import type { CardCommit } from "@/lib/commit"
import { observeSize } from "@/lib/observe-size"

export interface CommitCardProps {
  commit: CardCommit
  /**
   * The card of a row's mark (`card`, `text-sm`), a commit above what follows it (`heading`, the reading text), the
   * commit a review shows alone, the headline of its document (`document`)
   */
  variant: "card" | "heading" | "document"
  /** After the subject: a badge ("Merge conflict"), the review button of a preview, the actions of the commit shown alone */
  addon?: ReactNode
  /** In place of the body of its message: the story a review tells of the commit */
  body?: ReactNode
  /** The "⋯" of the commit at the end of its meta line (`CommitHeader.menu`) */
  menu?: ReactNode
  /** The props of its subject (`CommitHeader.subjectProps`): the hover of the subject of a heading opens its card */
  subjectProps?: CommitHeaderProps["subjectProps"]
}

/**
 * The one card of a commit (openspec/specs/commit/commit-row/spec.md, "Commit card"): its header (`CommitHeader`), the subject
 * whole over the meta line with its line counts coloured, then its body (`CommitBodyView`), its trailers last. The card
 * of a row's mark (`CommitRowMark`, in the lists of the main window and the tree of a review), the commits of the
 * preview of a branch and the commit a review shows alone; the commit preview shows the same header as its title and
 * the same body. The card of a mark scrolls under its header, which stays (its popover has no top padding of its own)
 */
export function CommitCard({ commit, variant, addon, body, menu, subjectProps }: CommitCardProps) {
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
      subjectProps={subjectProps}
      menu={menu}
    />
  )
  if (variant !== "card") {
    return (
      <div className="flex min-w-0 flex-col gap-2">
        {/* the row of the mark of a heading (`CommitRowMark`): its card opens under the subject and the meta line */}
        {variant === "document" ? <div data-commit-row="">{header}</div> : header}
        {body ?? <CommitBodyView body={commit.body} />}
      </div>
    )
  }
  return <ScrollingCard header={header} body={body ?? <CommitBodyView body={commit.body} size="card" fold />} />
}

/**
 * The card of a mark, 24rem high at most, which scrolls: its header stays at the top, a rule under it while the body
 * is scrolled under it, so no line is cut in half unseen; while more of the body is below, its bottom fades into the
 * popover over 64px above a line "More below", so a body cut by the height never reads as the whole message
 */
function ScrollingCard({ header, body }: { header: ReactNode; body: ReactNode }) {
  const [edges, setEdges] = useState({ scrolled: false, more: false })
  const end = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const sentinel = end.current
    let root = sentinel?.parentElement ?? null
    while (root !== null && !/(auto|scroll)/.test(getComputedStyle(root).overflowY)) {
      root = root.parentElement
    }
    const content = sentinel?.parentElement ?? null
    if (root === null || content === null) {
      return
    }
    // the card scrolls, or grows as a fold opens and a popup places itself: the edges follow each
    const update = () => {
      const next = { scrolled: root.scrollTop > 0, more: root.scrollTop + root.clientHeight < root.scrollHeight - 1 }
      setEdges(current => (current.scrolled === next.scrolled && current.more === next.more ? current : next))
    }
    const observer = observeSize(update)
    observer.observe(root)
    observer.observe(content)
    root.addEventListener("scroll", update, { passive: true })
    update()
    return () => {
      observer.disconnect()
      root.removeEventListener("scroll", update)
    }
  }, [])
  return (
    <div className="flex min-w-0 flex-col">
      <div
        className="sticky top-0 z-10 border-b border-transparent bg-popover pt-4 pb-2 transition-colors data-scrolled:border-border"
        data-scrolled={edges.scrolled ? "" : undefined}
        data-testid="commit-card-header"
      >
        {header}
      </div>
      {body}
      <div
        aria-hidden
        data-testid="commit-card-fade"
        data-more={edges.more ? "" : undefined}
        className="pointer-events-none sticky bottom-0 -mt-16 flex h-16 shrink-0 items-end justify-center bg-linear-to-t from-popover from-30% to-transparent pb-1 text-xs text-muted-foreground opacity-0 transition-opacity data-more:opacity-100"
      >
        More below
      </div>
      {/* the bottom padding of the card: in view, the whole body is */}
      <div ref={end} className="h-4 shrink-0" />
    </div>
  )
}
