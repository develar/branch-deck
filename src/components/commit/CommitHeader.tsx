import type { HTMLAttributes, ReactNode } from "react"
import { cn } from "cn"
import { DialogTitle } from "@/components/ui/dialog"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import type { CommitMetadataProps } from "@/components/commit/CommitMetadata"
import { CommitSubject } from "@/components/commit/CommitSubject"

/**
 * Where a header shows its commit:
 * - `compact`: a row of a sidebar (`CommitRow`: the commit column of a review, the commits of a pull request, a commit
 *   row of the tree of a review), the subject clamped to two lines, no meta line;
 * - `row`: a row of a list, the subject in `text-sm` clamped to two lines, the line counts muted;
 * - `card`: the card of a row's mark, the subject whole in `text-sm font-medium`;
 * - `heading`: a commit above what follows it (a commit of the preview of a branch, the commit a review shows alone),
 *   the subject whole in `text-md font-medium`;
 * - `title`: the title of the commit preview, the subject whole in `text-base font-medium`.
 *
 * Every one but the row colours the line counts (`success`, `destructive`)
 */
export type CommitHeaderVariant = "compact" | "row" | "card" | "heading" | "title"

export interface CommitHeaderProps extends Omit<CommitMetadataProps, "coloredLineCounts"> {
  subject: string
  variant?: CommitHeaderVariant
  /** Rendered before the subject on its line (the marker of a fixup) */
  subjectPrefix?: ReactNode
  /** Rendered after the subject on its line (the mark of its card, an exception badge, a review button) */
  subjectAddon?: ReactNode
  /** The props of the subject: the handlers of its hover, which opens the card of the row (`useRowCardHover`), a test id */
  subjectProps?: HTMLAttributes<HTMLElement> & { "data-testid"?: string }
  /** The "⋯" of the commit at the end of its meta line, one place in every header that has a meta line */
  menu?: ReactNode
}

const SUBJECT_CLASS: Record<Exclude<CommitHeaderVariant, "row" | "compact">, string> = {
  card: "font-medium",
  heading: "text-md font-medium",
  title: "text-base font-medium",
}

/**
 * The subject line over the meta line: the one header of a commit wherever one is shown (rows, the card of a row, the
 * previews, the review), in the size of its place (`CommitHeaderVariant`); a compact one its subject line alone
 */
export function CommitHeader({ subject, variant = "row", subjectPrefix, subjectAddon, subjectProps, menu, ...metadata }: CommitHeaderProps) {
  const isRow = variant === "row" || variant === "compact"
  const subjectElement = (
    <CommitSubject
      subject={subject}
      clamp={isRow}
      className={variant === "compact" ? "min-w-0 flex-1 [overflow-wrap:anywhere]" : isRow ? undefined : SUBJECT_CLASS[variant]}
      {...subjectProps}
    />
  )
  const subjectLine = (
    <div className={variant === "compact" ? "flex min-w-0 items-start gap-1.5" : cn("flex", isRow ? "items-center" : "items-start", "gap-2")}>
      {subjectPrefix}
      {variant === "title" ? <DialogTitle className="min-w-0 leading-6">{subjectElement}</DialogTitle> : subjectElement}
      {subjectAddon}
    </div>
  )
  if (variant === "compact") {
    return subjectLine
  }
  return (
    <div className={cn("flex min-w-0 flex-col", isRow ? "gap-0.5" : "gap-1")}>
      {subjectLine}
      {menu === undefined ? (
        <CommitMetadata {...metadata} coloredLineCounts={!isRow} />
      ) : (
        // the "⋯" right after the meta line, its 24px button kept to the height of the line
        <div className="flex min-w-0 items-center gap-1" data-testid="commit-header-meta">
          <CommitMetadata {...metadata} coloredLineCounts={!isRow} />
          <span className="-my-1 shrink-0">{menu}</span>
        </div>
      )}
    </div>
  )
}
