import type { ReactNode } from "react"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import type { CommitMetadataProps } from "@/components/commit/CommitMetadata"
import { CommitSubject } from "@/components/commit/CommitSubject"

export interface CommitHeadingProps extends Omit<CommitMetadataProps, "coloredLineCounts"> {
  subject: string
  /** After the subject, e.g. "Merge conflict" */
  badge?: ReactNode
  /** Below the meta line: the body of the message */
  children?: ReactNode
}

/**
 * A commit as the heading of what follows it (a commit of the branch preview, the commit above its diffs in a review):
 * the subject in `text-md font-medium`, the meta line with the time popover and the line counts coloured, then the
 * body. The commit preview, where the subject is the title of the dialog, is one step above (`text-base`).
 */
export function CommitHeading({ subject, badge, children, ...meta }: CommitHeadingProps) {
  return (
    <div className="flex min-w-0 flex-col gap-2">
      <div className="flex flex-col gap-1">
        <div className="flex items-start gap-2">
          <CommitSubject subject={subject} clamp={false} className="text-md font-medium" />
          {badge}
        </div>
        <CommitMetadata {...meta} coloredLineCounts />
      </div>
      {children}
    </div>
  )
}
