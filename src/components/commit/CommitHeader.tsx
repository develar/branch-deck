import type { ReactNode } from "react"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import type { CommitMetadataProps } from "@/components/commit/CommitMetadata"
import { CommitSubject } from "@/components/commit/CommitSubject"

export interface CommitHeaderProps extends CommitMetadataProps {
  subject: string
  /** Rendered after the subject on its line (a body toggle, an exception badge) */
  subjectAddon?: ReactNode
}

/** Subject line over the meta line: the header of a commit wherever one is shown (rows, popovers, conflict views). */
export function CommitHeader({ subject, subjectAddon, ...metadata }: CommitHeaderProps) {
  return (
    <div className="flex min-w-0 flex-col gap-1">
      <div className="flex items-center gap-2">
        <CommitSubject subject={subject} />
        {subjectAddon}
      </div>
      <CommitMetadata {...metadata} />
    </div>
  )
}
