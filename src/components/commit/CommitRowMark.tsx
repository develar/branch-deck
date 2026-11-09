import type { ReactNode } from "react"
import { cn } from "cn"
import { RectangleEllipsisIcon } from "lucide-react"
import { CommitCard } from "@/components/commit/CommitCard"
import { RowCard } from "@/components/shared/RowCard"
import type { RowCardState } from "@/components/shared/RowCard"
import type { RowCardHover } from "@/hooks/useRowCardHover"
import type { CardCommit } from "@/lib/commit"

export interface CommitRowMarkProps extends RowCardState {
  /** The commit of the row, with its body: a row of a commit without a body shows no mark */
  commit: CardCommit
  /** A click on the mark: the preview of the commit in the main window; none where the row has no preview */
  onOpen?: () => void
  /** A row of a list (the 20px line of a subject) or of a tree (a 24px row, the size of the mark of a file) */
  size?: "list" | "tree"
  /** The "⋯" of the commit at the end of the meta line of its card: the menu its row opens */
  menu?: ReactNode
  /** The handlers of the card for the hover of the subject of its row (`useRowCardHover`) */
  popupHover?: RowCardHover["popup"]
}

/** The row of a mark: a row of a list of commits, the line of a row of the tree of a review (not the rows of its group) */
function rowOfMark(mark: HTMLElement): Element | null {
  // a tree item holds the rows of its group after the line of its own row, its first child
  return mark.closest("tr") ?? mark.closest("[role=treeitem]")?.firstElementChild ?? mark.closest("[role=option], [data-commit-row]") ?? null
}

/**
 * The mark of a commit row whose message has a body (openspec/specs/commit/commit-row/spec.md, "Commit card"): its hover opens
 * the card of the commit (`CommitCard`), the header and the body folded as Rust says, under the whole row at its
 * start, and so does `I` on its row, the focus in the card; Escape gives the focus back to the row (`RowCard`). A click
 * opens the preview of the commit where its list has one. Not a tab stop: Space previews the row, `I` opens its card;
 * the hover of the subject of the row opens it too (`useRowCardHover`). One mark for the rows of the main window and of
 * the sidebar of a review (`CommitRow`)
 */
export function CommitRowMark({ commit, onOpen, size = "list", menu, popupHover, ...card }: CommitRowMarkProps) {
  // the card keeps the click from the row, which would select it
  const handleClick = () => {
    if (onOpen !== undefined) {
      // the preview shows the commit in full: the card goes
      card.onOpenChange(false)
      onOpen()
    }
  }
  return (
    <RowCard
      {...card}
      label="Show full commit message"
      mark={
        <span
          className={cn(
            "flex shrink-0 cursor-default items-center justify-center rounded-md text-muted-foreground hover:text-foreground",
            // `-my-0.5`: the 24px mark keeps the 20px line height of the subject
            // `my-0.5`: on the first line of a row of the tree, whose title may wrap
            size === "list" ? "-my-0.5 size-6 hover:bg-muted" : "my-0.5 size-4",
          )}
          data-testid="commit-row-mark"
          onClick={handleClick}
        />
      }
      icon={<RectangleEllipsisIcon className={size === "list" ? "size-3" : "size-3.5"} />}
      // 24rem high at most, the body scrolls under the header, which stays and pads the top, and over the fade of the
      // card, which pads the bottom (`CommitCard`)
      className="max-h-96 w-[min(36rem,var(--available-width))] overflow-y-auto px-4 py-0"
      data-testid="commit-card"
      focus="card"
      // under the whole row of a list, never over the meta line of the commit the reader was reading; beside the row of
      // a sidebar (the tree, the column, the rail), so the rows around it stay in view
      anchor={rowOfMark}
      side={size === "tree" ? "right" : "bottom"}
      popupHover={popupHover}
    >
      {() => <CommitCard commit={commit} variant="card" menu={menu} />}
    </RowCard>
  )
}
