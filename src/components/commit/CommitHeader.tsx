import type { ReactNode } from "react"
import { cn } from "cn"
import { DialogTitle } from "@/components/ui/dialog"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import type { CommitMetadataProps } from "@/components/commit/CommitMetadata"
import { CommitSubject } from "@/components/commit/CommitSubject"

/**
 * Where a header shows its commit:
 * - `row`: a row of a list, the subject in `text-sm` clamped to two lines, the line counts muted;
 * - `card`: the card of a row's mark, the subject whole in `text-sm font-medium`;
 * - `heading`: a commit above what follows it (a commit of the preview of a branch, the commit a review shows alone),
 *   the subject whole in `text-md font-medium`;
 * - `title`: the title of the commit preview, the subject whole in `text-base font-medium`.
 *
 * Every one but the row colours the line counts (`success`, `destructive`)
 */
export type CommitHeaderVariant = "row" | "card" | "heading" | "title"

export interface CommitHeaderProps extends Omit<CommitMetadataProps, "coloredLineCounts"> {
  subject: string
  variant?: CommitHeaderVariant
  /** Rendered before the subject on its line (the marker of a fixup) */
  subjectPrefix?: ReactNode
  /** Rendered after the subject on its line (the mark of its card, an exception badge, a review button) */
  subjectAddon?: ReactNode
}

const SUBJECT_CLASS: Record<Exclude<CommitHeaderVariant, "row">, string> = {
  card: "font-medium",
  heading: "text-md font-medium",
  title: "text-base font-medium",
}

/**
 * The subject line over the meta line: the one header of a commit wherever one is shown (rows, the card of a row, the
 * previews, the review), in the size of its place (`CommitHeaderVariant`)
 */
export function CommitHeader({ subject, variant = "row", subjectPrefix, subjectAddon, ...metadata }: CommitHeaderProps) {
  const isRow = variant === "row"
  const subjectElement = <CommitSubject subject={subject} clamp={isRow} className={isRow ? undefined : SUBJECT_CLASS[variant]} />
  return (
    <div className={cn("flex min-w-0 flex-col", isRow ? "gap-0.5" : "gap-1")}>
      <div className={cn("flex", isRow ? "items-center" : "items-start", "gap-2")}>
        {subjectPrefix}
        {variant === "title" ? <DialogTitle className="min-w-0 leading-6">{subjectElement}</DialogTitle> : subjectElement}
        {subjectAddon}
      </div>
      <CommitMetadata {...metadata} coloredLineCounts={!isRow} />
    </div>
  )
}
