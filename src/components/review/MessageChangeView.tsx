import { Fragment } from "react"
import { AddedBlocksView, RemovedBlocksView } from "@/components/commit/CommitBodyView"
import { SubjectBefore } from "@/components/review/StoryBody"
import type { MessageChange } from "@/lib/bindings"

/**
 * How the message of a commit changed since the review, as Rust compared it (`MessageChange`,
 * spec/branch/review-plan.spec.md, "In the Review Window"): the subject it had, then each change of its body in the order
 * of the body, the removed blocks struck through before the blocks added in their place. Where no opening tells the
 * commit (Since Your Review as one diff), the overview shows it under the line of the commit
 */
export function MessageChangeView({ change }: { change: MessageChange }) {
  return (
    <div className="flex min-w-0 flex-col gap-2 text-sm text-foreground" data-testid="review-plan-since-message">
      {change.subjectBefore !== null && <SubjectBefore subject={change.subjectBefore} />}
      {change.hunks.map(hunk => (
        <Fragment key={hunk.start}>
          {hunk.removed.blocks.length > 0 && <RemovedBlocksView removed={hunk.removed} />}
          {hunk.added.blocks.length > 0 && <AddedBlocksView added={hunk.added} />}
        </Fragment>
      ))}
    </div>
  )
}
